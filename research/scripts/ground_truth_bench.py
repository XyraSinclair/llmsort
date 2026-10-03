#!/usr/bin/env python3
"""Run the llmsort ground-truth benchmark with Python 3.9 stdlib only."""

import argparse
import concurrent.futures
import datetime as dt
import hashlib
import json
import math
import os
from pathlib import Path
import random
import re
import statistics
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request


ROOT = Path(__file__).resolve().parents[2]
POOL_DIR = ROOT / "research" / "data" / "ground-truth"
DEFAULT_OUTPUT = ROOT / "research" / "artifacts" / "live" / "ground-truth-2026-10-03"
DEFAULT_REPORT = ROOT / "REPORT.md"
MODEL = "openai/gpt-5.6-terra"
SEEDS = (17, 29)
SPEND_CAP = 4.0
API = "https://openrouter.ai/api/v1"
USER_AGENT = (
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) "
    "AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0 Safari/537.36"
)


def atomic_text(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile("w", encoding="utf-8", dir=str(path.parent), delete=False) as f:
        f.write(value)
        tmp = Path(f.name)
    os.replace(str(tmp), str(path))


def atomic_json(path, value):
    atomic_text(path, json.dumps(value, indent=2, ensure_ascii=False) + "\n")


def read_json(path):
    with path.open(encoding="utf-8") as f:
        return json.load(f)


def load_key():
    if os.environ.get("OPENROUTER_API_KEY"):
        return os.environ["OPENROUTER_API_KEY"]
    proc = subprocess.run(
        ["zsh", "-lc", 'print -rn -- "$OPENROUTER_API_KEY"'],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        check=False,
    )
    key = proc.stdout
    if proc.returncode or not key:
        raise RuntimeError("OPENROUTER_API_KEY is absent from the login-shell environment")
    return key


def request_json(url, key=None, payload=None, retries=2):
    headers = {
        "User-Agent": USER_AGENT,
        "Accept": "application/json",
        "HTTP-Referer": "https://github.com/XyraSinclair/llmsort",
        "X-Title": "llmsort ground-truth benchmark",
    }
    data = None
    if key:
        headers["Authorization"] = "Bearer " + key
    if payload is not None:
        headers["Content-Type"] = "application/json"
        data = json.dumps(payload, ensure_ascii=False).encode("utf-8")
    req = urllib.request.Request(url, data=data, headers=headers, method="POST" if data else "GET")
    for attempt in range(retries + 1):
        try:
            with urllib.request.urlopen(req, timeout=180) as resp:
                return json.loads(resp.read().decode("utf-8"))
        except urllib.error.HTTPError as exc:
            transient = exc.code == 429 or 500 <= exc.code < 600
            detail = exc.read(2048).decode("utf-8", "replace")
            if not transient or attempt == retries:
                raise RuntimeError("OpenRouter HTTP {}: {}".format(exc.code, detail)) from exc
        except (urllib.error.URLError, TimeoutError) as exc:
            if attempt == retries:
                raise RuntimeError("OpenRouter transport failure: {}".format(exc)) from exc
        time.sleep(2 ** attempt)
    raise AssertionError("unreachable")


def auth_snapshot(key):
    raw = request_json(API + "/auth/key", key=key)
    data = raw.get("data", raw)
    keep = ("limit", "limit_remaining", "usage", "usage_daily", "usage_weekly", "usage_monthly", "is_free_tier")
    return {name: data.get(name) for name in keep if name in data}


def as_float(value):
    if value is None:
        return None
    try:
        return float(value)
    except (TypeError, ValueError):
        return None


def model_pricing():
    raw = request_json(API + "/models")
    for model in raw.get("data", []):
        if model.get("id") == MODEL:
            pricing = model.get("pricing", {})
            prompt = as_float(pricing.get("prompt"))
            completion = as_float(pricing.get("completion"))
            if prompt is None or completion is None:
                break
            return {
                "model": MODEL,
                "prompt_per_token": prompt,
                "completion_per_token": completion,
            }
    raise RuntimeError("OpenRouter model catalog has no usable pricing for " + MODEL)


def usage_cost(response, pricing):
    usage = response.get("usage") or {}
    cost = as_float(usage.get("cost"))
    if cost is not None:
        return cost, False
    prompt = as_float(usage.get("prompt_tokens")) or 0.0
    completion = as_float(usage.get("completion_tokens")) or 0.0
    return (
        prompt * pricing["prompt_per_token"] + completion * pricing["completion_per_token"],
        True,
    )


def projected_api_cost(payload, pricing):
    chars = sum(len(m.get("content", "")) for m in payload.get("messages", []))
    input_tokens = math.ceil(chars / 3.0) + 32
    output_tokens = int(payload.get("max_tokens", 512))
    return 1.25 * (
        input_tokens * pricing["prompt_per_token"]
        + output_tokens * pricing["completion_per_token"]
    )


def response_content(response):
    choices = response.get("choices") or []
    if not choices:
        raise RuntimeError("OpenRouter response has no choices")
    content = (choices[0].get("message") or {}).get("content", "")
    if isinstance(content, str):
        return content
    if isinstance(content, list):
        return "".join(part.get("text", "") for part in content if isinstance(part, dict))
    return str(content)


def point_payload(pool, item, seed):
    return {
        "model": MODEL,
        "temperature": 0,
        "seed": seed,
        "max_tokens": 32,
        "reasoning": {"enabled": False},
        "response_format": {"type": "json_object"},
        "messages": [
            {
                "role": "system",
                "content": "Rate one item on a 1-10 integer scale. Return only JSON: {\"score\": INTEGER}.",
            },
            {
                "role": "user",
                "content": "Criterion: {}\nItem: {}\nRate the item from 1 (least) to 10 (most).".format(
                    pool["attribute"], item["text"]
                ),
            },
        ],
    }


def listwise_payload(pool, items, seed):
    shown = [{"id": item["id"], "item": item["text"]} for item in items]
    return {
        "model": MODEL,
        "temperature": 0,
        "seed": seed,
        "max_tokens": 512,
        "reasoning": {"enabled": False},
        "response_format": {"type": "json_object"},
        "messages": [
            {
                "role": "system",
                "content": (
                    "Sort every supplied item from MOST to LEAST on the criterion. "
                    "Return only JSON: {\"order\":[\"id\",...]}. Include every id exactly once."
                ),
            },
            {
                "role": "user",
                "content": "Criterion: {}\nItems:\n{}".format(
                    pool["attribute"], json.dumps(shown, ensure_ascii=False)
                ),
            },
        ],
    }


def parse_score(response):
    content = response_content(response).strip()
    try:
        value = json.loads(content).get("score")
        score = int(value)
    except (AttributeError, TypeError, ValueError, json.JSONDecodeError):
        match = re.search(r"(?<!\d)(10|[1-9])(?!\d)", content)
        if not match:
            return None
        score = int(match.group(1))
    return score if 1 <= score <= 10 else None


def parse_order(response):
    content = response_content(response).strip()
    try:
        parsed = json.loads(content)
        order = parsed.get("order") if isinstance(parsed, dict) else parsed
    except json.JSONDecodeError:
        match = re.search(r"\[[\s\S]*\]", content)
        order = json.loads(match.group(0)) if match else []
    return [str(value) for value in order] if isinstance(order, list) else []


def rank_values(values):
    indexed = sorted(enumerate(values), key=lambda pair: pair[1])
    ranks = [0.0] * len(values)
    i = 0
    while i < len(indexed):
        j = i + 1
        while j < len(indexed) and indexed[j][1] == indexed[i][1]:
            j += 1
        rank = (i + 1 + j) / 2.0
        for k in range(i, j):
            ranks[indexed[k][0]] = rank
        i = j
    return ranks


def pearson(xs, ys):
    if len(xs) != len(ys) or len(xs) < 2:
        return None
    mx, my = statistics.fmean(xs), statistics.fmean(ys)
    dx = [x - mx for x in xs]
    dy = [y - my for y in ys]
    denom = math.sqrt(sum(x * x for x in dx) * sum(y * y for y in dy))
    return sum(x * y for x, y in zip(dx, dy)) / denom if denom else None


def spearman(xs, ys):
    return pearson(rank_values(xs), rank_values(ys))


def kendall_tau_b(xs, ys):
    concordant = discordant = ties_x = ties_y = 0
    for i in range(len(xs)):
        for j in range(i + 1, len(xs)):
            sx = (xs[i] > xs[j]) - (xs[i] < xs[j])
            sy = (ys[i] > ys[j]) - (ys[i] < ys[j])
            if sx == 0 and sy == 0:
                continue
            if sx == 0:
                ties_x += 1
            elif sy == 0:
                ties_y += 1
            elif sx == sy:
                concordant += 1
            else:
                discordant += 1
    denom = math.sqrt((concordant + discordant + ties_x) * (concordant + discordant + ties_y))
    return (concordant - discordant) / denom if denom else None


def metric_block(predicted, truth):
    return {
        "spearman_rho": spearman(predicted, truth),
        "kendall_tau_b": kendall_tau_b(predicted, truth),
    }


def validate_binary(binary):
    proc = subprocess.run([str(binary), "--version"], capture_output=True, text=True, check=False)
    version = proc.stdout.strip()
    if proc.returncode or version != "llmsort 0.15.0":
        raise RuntimeError("expected shipped llmsort 0.15.0, got {!r}".format(version))
    digest = hashlib.sha256(binary.read_bytes()).hexdigest()
    return {
        "version": version,
        "sha256": digest,
        "release_asset": "https://github.com/XyraSinclair/llmsort/releases/download/v0.15.0/llmsort-aarch64-apple-darwin.tar.gz",
    }


def jsonl_input(pool, seed):
    items = list(pool["items"])
    random.Random(seed).shuffle(items)
    return "".join(json.dumps(item, ensure_ascii=False) + "\n" for item in items)


def cli_estimate(binary, pool):
    with tempfile.TemporaryDirectory(prefix="llmsort-ground-truth-") as temp:
        input_path = Path(temp) / "pool.jsonl"
        input_path.write_text(jsonl_input(pool, SEEDS[0]), encoding="utf-8")
        proc = subprocess.run(
            [
                str(binary), "sort", str(input_path), "--by", pool["attribute"],
                "--field", "text", "--model", MODEL, "--estimate",
            ],
            capture_output=True,
            text=True,
            check=False,
        )
    if proc.returncode:
        raise RuntimeError("llmsort estimate failed: " + proc.stderr.strip())
    match = re.search(r"~\$([0-9.]+) typical", proc.stdout)
    if not match:
        raise RuntimeError("could not parse llmsort estimate: " + proc.stdout.strip())
    return float(match.group(1)), proc.stdout.strip()


def preflight(binary, pools, auth, pricing):
    estimates = {}
    pairwise = 0.0
    direct = 0.0
    setwise = 0.0
    for pool in pools:
        typical, line = cli_estimate(binary, pool)
        estimates[pool["slug"]] = {"pairwise_typical_per_seed": typical, "binary_output": line}
        pairwise += len(SEEDS) * typical
        for seed in SEEDS:
            for item in pool["items"]:
                direct += projected_api_cost(point_payload(pool, item, seed), pricing)
            direct += projected_api_cost(listwise_payload(pool, pool["items"], seed), pricing)
        # n=20, k=8, overlap=2: four ring windows, each presented twice.
        calls = math.ceil(len(pool["items"]) / 6.0) * 2
        pool_setwise = len(SEEDS) * calls * 1.25 * (
            500 * pricing["prompt_per_token"] + 64 * pricing["completion_per_token"]
        )
        setwise += pool_setwise
        estimates[pool["slug"]]["setwise_projected_per_seed"] = pool_setwise / len(SEEDS)
    projected = 1.20 * (pairwise + direct + setwise)
    remaining = as_float(auth.get("limit_remaining"))
    if projected >= SPEND_CAP:
        raise RuntimeError(
            "preflight projects ${:.3f}, which would exceed the ${:.2f} hard cap".format(
                projected, SPEND_CAP
            )
        )
    if remaining is not None and remaining < projected:
        raise RuntimeError(
            "OpenRouter limit_remaining is ${:.3f}, below the ${:.3f} projected run".format(
                remaining, projected
            )
        )
    return {
        "auth_probe": {"limit_remaining_was_sufficient": True},
        "pricing": pricing,
        "cells": estimates,
        "projected_pairwise": pairwise,
        "projected_direct_upper": direct,
        "projected_setwise_upper": setwise,
        "projected_with_20pct_margin": projected,
        "hard_cap": SPEND_CAP,
    }


def run_pointwise(pool, seed, raw_dir, key, pricing):
    path = raw_dir / "pointwise-{}-seed{}.jsonl".format(pool["slug"], seed)
    meta_path = raw_dir / "pointwise-{}-seed{}.meta.json".format(pool["slug"], seed)
    existing = {}
    if path.exists():
        for line in path.read_text(encoding="utf-8").splitlines():
            row = json.loads(line)
            existing[row["item_id"]] = row
    items = list(pool["items"])
    random.Random(seed).shuffle(items)
    missing = [item for item in items if item["id"] not in existing]
    started = time.monotonic()

    def one(item):
        payload = point_payload(pool, item, seed)
        t0 = time.monotonic()
        response = request_json(API + "/chat/completions", key=key, payload=payload)
        return {
            "item_id": item["id"],
            "request": payload,
            "response": response,
            "wall_seconds": time.monotonic() - t0,
        }

    if missing:
        with concurrent.futures.ThreadPoolExecutor(max_workers=8) as executor:
            futures = {executor.submit(one, item): item for item in missing}
            for future in concurrent.futures.as_completed(futures):
                row = future.result()
                existing[row["item_id"]] = row
                ordered = [existing[item["id"]] for item in items if item["id"] in existing]
                atomic_text(path, "".join(json.dumps(x, ensure_ascii=False) + "\n" for x in ordered))
        elapsed = time.monotonic() - started
        atomic_json(meta_path, {"wall_seconds": elapsed})
    elif meta_path.exists():
        elapsed = read_json(meta_path)["wall_seconds"]
    else:
        elapsed = sum(row.get("wall_seconds", 0.0) for row in existing.values())
    rows = [existing[item["id"]] for item in items]
    scores = {row["item_id"]: parse_score(row["response"]) for row in rows}
    valid = [value for value in scores.values() if value is not None]
    if not valid:
        raise RuntimeError("pointwise {} seed {} produced no valid scores".format(pool["slug"], seed))
    imputed = statistics.median(valid)
    predicted = [scores[item["id"]] if scores[item["id"]] is not None else imputed for item in pool["items"]]
    truth = [item["truth"] for item in pool["items"]]
    cost_parts = [usage_cost(row["response"], pricing) for row in rows]
    result = metric_block(predicted, truth)
    counts = {str(score): predicted.count(score) for score in sorted(set(predicted))}
    result.update(
        {
            "pool": pool["slug"], "method": "pointwise", "seed": seed,
            "n": len(items), "model": MODEL,
            "modal_score_fraction": max(counts.values()) / len(items),
            "score_counts": counts,
            "parse_failures": sum(value is None for value in scores.values()),
            "dollars": sum(part[0] for part in cost_parts),
            "cost_is_estimate": any(part[1] for part in cost_parts),
            "wall_seconds": elapsed,
            "raw": str(path.relative_to(raw_dir.parent)),
        }
    )
    return result


def run_listwise(pool, seed, raw_dir, key, pricing):
    path = raw_dir / "listwise-{}-seed{}.json".format(pool["slug"], seed)
    meta_path = raw_dir / "listwise-{}-seed{}.meta.json".format(pool["slug"], seed)
    items = list(pool["items"])
    random.Random(seed).shuffle(items)
    payload = listwise_payload(pool, items, seed)
    started = time.monotonic()
    if path.exists():
        row = read_json(path)
        elapsed = read_json(meta_path)["wall_seconds"] if meta_path.exists() else 0.0
    else:
        response = request_json(API + "/chat/completions", key=key, payload=payload)
        row = {"request": payload, "response": response}
        atomic_json(path, row)
        elapsed = time.monotonic() - started
        atomic_json(meta_path, {"wall_seconds": elapsed})
    order = parse_order(row["response"])
    valid_ids = {item["id"] for item in items}
    seen = set()
    unique = []
    duplicate_count = 0
    unknown = []
    for item_id in order:
        if item_id not in valid_ids:
            unknown.append(item_id)
        elif item_id in seen:
            duplicate_count += 1
        else:
            seen.add(item_id)
            unique.append(item_id)
    missing = sorted(valid_ids - seen)
    score = {item_id: len(items) - rank for rank, item_id in enumerate(unique)}
    predicted = [score.get(item["id"], 0) for item in pool["items"]]
    truth = [item["truth"] for item in pool["items"]]
    dollars, estimated = usage_cost(row["response"], pricing)
    result = metric_block(predicted, truth)
    result.update(
        {
            "pool": pool["slug"], "method": "listwise", "seed": seed,
            "n": len(items), "model": MODEL,
            "items_dropped": len(missing), "dropped_ids": missing,
            "items_duplicated": duplicate_count, "unknown_ids": unknown,
            "dollars": dollars, "cost_is_estimate": estimated,
            "wall_seconds": elapsed,
            "raw": str(path.relative_to(raw_dir.parent)),
        }
    )
    return result


def parse_cli_cost(stderr):
    match = re.search(r" · (~?)\$([0-9.]+)", stderr)
    if not match:
        raise RuntimeError("could not parse llmsort cost from stderr")
    return float(match.group(2)), bool(match.group(1))


def run_llmsort(binary, pool, seed, method, llmsort_dir, key, max_dollars):
    stem = "{}-{}-seed{}".format(method, pool["slug"], seed)
    output_path = llmsort_dir / (stem + ".jsonl")
    stderr_path = llmsort_dir / (stem + ".stderr.txt")
    meta_path = llmsort_dir / (stem + ".meta.json")
    trace_path = llmsort_dir / (stem + ".trace.jsonl")
    input_path = llmsort_dir / (stem + ".input.jsonl")
    if output_path.exists() and stderr_path.exists() and meta_path.exists():
        elapsed = read_json(meta_path)["wall_seconds"]
    else:
        atomic_text(input_path, jsonl_input(pool, seed))
        args = [
            str(binary), "sort", str(input_path), "--by", pool["attribute"],
            "--field", "text", "--format", "jsonl", "--model", MODEL,
            "--seed", str(seed),
        ]
        if method == "setwise":
            args.append("--setwise")
        else:
            args.extend(["--no-cache", "--trace", str(trace_path), "--max-dollars", "{:.6f}".format(max_dollars)])
        env = os.environ.copy()
        env["OPENROUTER_API_KEY"] = key
        started = time.monotonic()
        proc = subprocess.run(args, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, env=env, check=False)
        elapsed = time.monotonic() - started
        if proc.returncode:
            raise RuntimeError("{} failed for {} seed {}: {}".format(method, pool["slug"], seed, proc.stderr[-3000:]))
        atomic_text(output_path, proc.stdout)
        atomic_text(stderr_path, proc.stderr)
        atomic_json(meta_path, {"wall_seconds": elapsed, "command": args})
    stderr = stderr_path.read_text(encoding="utf-8")
    dollars, estimated = parse_cli_cost(stderr)
    rows = [json.loads(line) for line in output_path.read_text(encoding="utf-8").splitlines() if line]
    by_id = {row["id"]: row for row in rows}
    if len(by_id) != len(pool["items"]):
        raise RuntimeError("{} {} seed {} returned {} of {} items".format(method, pool["slug"], seed, len(by_id), len(pool["items"])))
    predicted = [float(by_id[item["id"]]["llmsort"]["latent_mean"]) for item in pool["items"]]
    truth = [item["truth"] for item in pool["items"]]
    result = metric_block(predicted, truth)
    result.update(
        {
            "pool": pool["slug"], "method": method, "seed": seed,
            "n": len(pool["items"]), "model": MODEL,
            "pearson_latent_vs_log_truth": pearson(predicted, [math.log(value) for value in truth]),
            "dollars": dollars, "cost_is_estimate": estimated,
            "wall_seconds": elapsed,
            "output": str(output_path.relative_to(llmsort_dir.parent)),
            "stderr": str(stderr_path.relative_to(llmsort_dir.parent)),
            "trace": str(trace_path.relative_to(llmsort_dir.parent)) if trace_path.exists() else None,
        }
    )
    gauge = re.search(r"gauge: flip ([0-9.]+)", stderr)
    if gauge:
        result["setwise_flip_rate"] = float(gauge.group(1))
    comparisons = re.search(r" · (\d+) comparisons", stderr)
    calls = re.search(r" · (\d+) calls", stderr)
    if comparisons:
        result["comparisons"] = int(comparisons.group(1))
    if calls:
        result["calls"] = int(calls.group(1))
    return result


def known_spend(results):
    return sum(row.get("dollars", 0.0) for row in results)


def ensure_room(spent, projected, label):
    if spent + projected >= SPEND_CAP:
        raise RuntimeError(
            "stopping before {}: ${:.3f} spent + ${:.3f} projected would reach the ${:.2f} cap".format(
                label, spent, projected, SPEND_CAP
            )
        )


def fmt(value, digits=2):
    return "—" if value is None else format(value, ".{}f".format(digits))


def cell_text(row):
    base = "{}/{}".format(fmt(row["spearman_rho"]), fmt(row["kendall_tau_b"]))
    if row["method"] == "pointwise":
        extra = "mode {:.0%}".format(row["modal_score_fraction"])
    elif row["method"] == "listwise":
        extra = "drop/dup {}/{}".format(row["items_dropped"], row["items_duplicated"])
    else:
        extra = "r {}".format(fmt(row["pearson_latent_vs_log_truth"]))
    return "{}; {}; ${:.4f}/{}s".format(base, extra, row["dollars"], round(row["wall_seconds"]))


def aggregate(results):
    grouped = {}
    for row in results:
        grouped.setdefault((row["pool"], row["method"]), []).append(row)
    out = []
    for (pool, method), rows in sorted(grouped.items()):
        entry = {"pool": pool, "method": method, "seeds": [row["seed"] for row in rows]}
        numeric = (
            "spearman_rho", "kendall_tau_b", "modal_score_fraction", "items_dropped",
            "items_duplicated", "pearson_latent_vs_log_truth", "dollars", "wall_seconds",
        )
        for name in numeric:
            values = [row[name] for row in rows if row.get(name) is not None]
            if values:
                entry[name] = {
                    "mean": statistics.fmean(values), "min": min(values), "max": max(values)
                }
        out.append(entry)
    return out


def write_findings(output, report_path, results, preflight_data, binary_info):
    by_key = {(row["pool"], row["method"], row["seed"]): row for row in results}
    lines = [
        "# Ground-truth benchmark: 2026-10-03", "",
        "**Errata:** None yet. Corrections belong here; raw model responses and shipped-binary outputs remain unchanged.", "",
        "## Result", "",
    ]
    losses = []
    for pool in sorted({row["pool"] for row in results}):
        for seed in SEEDS:
            best = max((by_key[(pool, method, seed)] for method in ("pointwise", "listwise", "setwise", "pairwise")), key=lambda row: row["spearman_rho"])
            if best["method"] not in ("pairwise", "setwise"):
                losses.append("{} seed {}: {} had the highest order rho ({:.2f})".format(pool, seed, best["method"], best["spearman_rho"]))
    if losses:
        lines.append("llmsort does not win every order cell: " + "; ".join(losses) + ".")
    else:
        lines.append("A llmsort arm has the highest order agreement in every pool and seed.")
    pair_r = [row["pearson_latent_vs_log_truth"] for row in results if row["method"] == "pairwise"]
    lines.append(
        "The pairwise latent/log-truth Pearson r spans {:.2f}–{:.2f}; this is the direct test of recovered gaps, not only order.".format(min(pair_r), max(pair_r))
    )
    lines.extend(["", "## Protocol", ""])
    lines.append(
        "Three n=20 public-fact pools, four methods, two seeds (17 and 29), one judge (`{}`). Pointwise uses one 1–10 call per item; listwise uses one whole-pool call; pairwise and setwise are the shipped v0.15.0 CLI with their algorithmic defaults, an explicit common model, and seed. Pairwise disables cache so both seeds are live. Missing listwise ids, if any, tie below returned ids; duplicated ids after the first are ignored and counted. Spearman uses average tie ranks and Kendall is tau-b.".format(MODEL)
    )
    lines.extend(["", "## Denominators and artifacts", ""])
    lines.append(
        "24 cells = 3 pools × 4 methods × 2 seeds; {} item-level pointwise calls, 6 listwise calls, plus the CLI calls/comparisons recorded per cell. Raw direct responses are in `raw/`; CLI JSONL, stderr, inputs, and pairwise traces are in `llmsort/`; `results.json` is the machine-readable reduction.".format(3 * 20 * 2)
    )
    lines.extend(["", "## Caveats", ""])
    lines.extend([
        "- These are factual-memory pools of bare names, not subjective sorting tasks; model familiarity is part of what is measured.",
        "- River length depends on source/tributary/mouth convention. The cited table is fixed benchmark truth; Amazon/Nile claims are especially contested.",
        "- Two seeds measure planning/presentation spread, not a model-wide error distribution.",
        "- Pointwise has no native uncertainty. Setwise latent scores are an ordinal fusion scale, so its Pearson r is descriptive; pairwise r is the cardinal claim.",
        "- CLI dollars come from the shipped binary's provider accounting and are printed to four decimal places; direct-call dollars use OpenRouter's response usage.",
    ])
    lines.extend(["", "## Reproduce", "", "```console", "python3 research/scripts/ground_truth_bench.py --binary ./llmsort", "```", ""])
    atomic_text(output / "FINDINGS.md", "\n".join(lines))

    header = "| Pool · seed | Pointwise rho/tau | Listwise rho/tau | `sort --setwise` rho/tau | `sort` rho/tau |"
    table = [header, "|---|---:|---:|---:|---:|"]
    for pool in ("countries", "rivers", "mountains"):
        for seed in SEEDS:
            cells = [by_key[(pool, method, seed)] for method in ("pointwise", "listwise", "setwise", "pairwise")]
            table.append("| {} · {} | {} | {} | {} | {} |".format(pool, seed, *(cell_text(row) for row in cells)))
    total = known_spend(results)
    caption = (
        "Each cell is Spearman rho / Kendall tau-b, then the method-specific diagnostic and provider dollars / wall time. "
        "Pointwise reports its modal-score share, listwise reports dropped/duplicated items, and llmsort reports Pearson r between latent mean and log truth. "
        "All 24 cells use `{}` on n=20 public-fact pools; two seeds expose presentation/planning spread.".format(MODEL)
    )
    report = ["# Ground-truth benchmark", "", *table, "", caption, "", "Rerun: `python3 research/scripts/ground_truth_bench.py --binary ./llmsort`", "", "Total provider spend: `${:.4f}` (hard cap `${:.2f}`).".format(total, SPEND_CAP), "", "Caveats: river lengths are convention-sensitive; the pools test factual recall on bare names; two seeds are not a population estimate; setwise's fused latent scale is ordinal; direct and CLI cost fields have different rounding precision.", ""]
    if len(" ".join(report).split()) > 400:
        raise RuntimeError("REPORT.md would exceed 400 words")
    atomic_text(report_path, "\n".join(report))

    results_doc = {
        "schema": 1,
        "generated_at": dt.datetime.now(dt.timezone.utc).isoformat(),
        "claim": "Direct 1-10 ratings cluster and one-prompt sorts lose items; pairwise ratio fitting recovers order and gaps.",
        "model": MODEL,
        "seeds": list(SEEDS),
        "hard_cap_usd": SPEND_CAP,
        "total_dollars": total,
        "binary": binary_info,
        "preflight": preflight_data,
        "metric_definitions": {
            "spearman_rho": "Pearson correlation of average tie ranks against numeric truth",
            "kendall_tau_b": "Kendall tau-b, including ties",
            "pointwise_modal_score_fraction": "largest 1-10 score-bin count divided by n",
            "listwise_dropped_or_duplicated": "missing valid ids; repeated valid ids after first",
            "pearson_latent_vs_log_truth": "Pearson correlation of llmsort latent_mean with natural log(truth)",
        },
        "cells": results,
        "aggregates": aggregate(results),
    }
    atomic_json(output / "results.json", results_doc)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=Path("llmsort"), help="path to shipped llmsort v0.15.0 binary")
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--report", type=Path, default=DEFAULT_REPORT)
    args = parser.parse_args()

    binary = args.binary.expanduser().resolve()
    output = args.output.expanduser().resolve()
    report_path = args.report.expanduser().resolve()
    pools = [read_json(POOL_DIR / (name + ".json")) for name in ("countries", "rivers", "mountains")]
    for pool in pools:
        if len(pool["items"]) != 20 or len({item["id"] for item in pool["items"]}) != 20:
            raise RuntimeError("pool {} must contain 20 unique ids".format(pool["slug"]))

    binary_info = validate_binary(binary)
    key = load_key()
    auth = auth_snapshot(key)
    pricing = model_pricing()
    print("preflight: OpenRouter limit probed; estimating shipped-binary run", flush=True)
    preflight_data = preflight(binary, pools, auth, pricing)
    print("preflight: projected ${:.3f} under ${:.2f} cap".format(preflight_data["projected_with_20pct_margin"], SPEND_CAP), flush=True)
    output.mkdir(parents=True, exist_ok=True)
    raw_dir = output / "raw"
    llmsort_dir = output / "llmsort"
    raw_dir.mkdir(exist_ok=True)
    llmsort_dir.mkdir(exist_ok=True)
    atomic_json(output / "run.json", {"binary": binary_info, "preflight": preflight_data})

    results = []
    for pool in pools:
        for seed in SEEDS:
            label = "pointwise {} seed {}".format(pool["slug"], seed)
            projected = sum(projected_api_cost(point_payload(pool, item, seed), pricing) for item in pool["items"])
            ensure_room(known_spend(results), projected, label)
            print("running " + label, flush=True)
            results.append(run_pointwise(pool, seed, raw_dir, key, pricing))

            label = "listwise {} seed {}".format(pool["slug"], seed)
            ensure_room(known_spend(results), projected_api_cost(listwise_payload(pool, pool["items"], seed), pricing), label)
            print("running " + label, flush=True)
            results.append(run_listwise(pool, seed, raw_dir, key, pricing))

    for method in ("setwise", "pairwise"):
        for pool in pools:
            for seed in SEEDS:
                label = "{} {} seed {}".format(method, pool["slug"], seed)
                if method == "pairwise":
                    projected = 1.25 * preflight_data["cells"][pool["slug"]]["pairwise_typical_per_seed"]
                else:
                    projected = preflight_data["cells"][pool["slug"]]["setwise_projected_per_seed"]
                ensure_room(known_spend(results), projected, label)
                print("running " + label, flush=True)
                max_dollars = max(0.001, SPEND_CAP - known_spend(results) - 0.02)
                results.append(run_llmsort(binary, pool, seed, method, llmsort_dir, key, max_dollars))

    if len(results) != 24:
        raise RuntimeError("expected 24 completed cells, got {}".format(len(results)))
    write_findings(output, report_path, results, preflight_data, binary_info)
    print("complete: 24/24 cells, ${:.4f}; {}".format(known_spend(results), report_path), flush=True)


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        print("ERROR: {}".format(exc), file=sys.stderr)
        raise
