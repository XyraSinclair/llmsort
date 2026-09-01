#!/usr/bin/env python3
"""Relentless judging: run cardinal over (corpus x attributes), land every
pairwise judgment durably in scry ClickHouse (ratiometer.judgments on colo2).

Production posture: pairwise cache ON (measurement batteries elsewhere use
--no-cache), every judgment traced, traces denormalized (entity texts resolved
by corpus line index) and inserted content-addressed — ReplacingMergeTree on
cache_key_hash makes replays free.

Usage:
  judge_and_land.py --corpus manifund.txt --attributes batteries/manifund_attributes.txt \
      --model gemma4-26b-a4b --budget 240 --seed 1 --run-tag manifund-2026-08-15 \
      --outdir /tmp/judge-runs [--start-at N]

Env: OPENROUTER_BASE_URL (local judges), CARDINAL_PAIRWISE_MAX_OUTPUT_TOKENS
(tight-context serves). Landing goes over ssh to colo2's clickhouse
(/data/clickhouse-twitter-lab/bin/clickhouse client --port 19000) with
JSONEachRow on stdin (no shell interpolation of data).
"""
import argparse, datetime, json, os, shlex, subprocess, sys, time

CARDINAL = os.environ.get(
    "CARDINAL_BIN", os.path.expanduser("~/projects/llmsorting/target/release/cardinal"))
SSH = "/usr/bin/ssh"
CH_LOCAL = "/data/clickhouse-twitter-lab/bin/clickhouse"


def ch_run(query, payload=None):
    """Run a clickhouse query against ratiometer's server: directly when the
    binary is on this box (running ON colo2), else over ssh (running on the Mac)."""
    if os.path.exists(CH_LOCAL):
        cmd = [CH_LOCAL, "client", "--port", "19000", "--query", query]
    else:
        cmd = [SSH, "colo2",
               f"{CH_LOCAL} client --port 19000 --query {shlex.quote(query)}"]
    return subprocess.run(cmd, input=payload, capture_output=True, timeout=120)


def run_cell(corpus, attr, model, budget, seed, outdir, idx, template=None, elaborate=False,
             concurrency=None, no_cache=False):
    slug = f"a{idx:03d}"
    out = os.path.join(outdir, f"{slug}.json")
    trace = os.path.join(outdir, f"{slug}.trace.jsonl")
    errf = os.path.join(outdir, f"{slug}.err")
    env = dict(os.environ)
    env.setdefault("OPENROUTER_API_KEY", "local")
    cmd = [CARDINAL, "sort", corpus, "--by", attr, "--model", model,
           "--budget", str(budget), "--seed", str(seed),
           "--trace", trace, "--format", "json"]
    if template:
        cmd += ["--template", template]
    if elaborate:
        cmd += ["--elaborate"]
    if concurrency:
        cmd += ["--concurrency", str(concurrency)]
    if no_cache:
        cmd += ["--no-cache"]
    with open(out, "w") as fo, open(errf, "w") as fe:
        subprocess.run(cmd, stdout=fo, stderr=fe, env=env, check=True, timeout=3600)
    return out, trace


def _posterior_fields(d):
    """Extract landable logprob-posterior scalars from a trace row."""
    p = d.get("pairwise_logprob_posterior")
    if not p:
        return 0.0, 0.0, 0.0, 0.0, ""
    chosen = d.get("higher_ranked") or p.get("selected_higher_ranked")
    dir_prob = 0.0
    for e in p.get("higher_ranked_distribution", {}).get("support", []):
        if e.get("value") == chosen:
            dir_prob = e.get("probability", 0.0)
    conf = (p.get("confidence") or {}).get("Logprob") or {}
    return (dir_prob, conf.get("entropy", 0.0), conf.get("top_prob", 0.0),
            conf.get("neighborhood_prob", 0.0), json.dumps(p, ensure_ascii=False))


def land(trace_path, corpus_lines, corpus_name, attr, seed, run_tag):
    rows = []
    for line in open(trace_path):
        d = json.loads(line)
        if d.get("error"):
            err = str(d["error"])
        else:
            err = ""
        dp, ent, tp, nb, post = _posterior_fields(d)
        rows.append({
            "ts": (datetime.datetime.fromtimestamp(
                d["timestamp_ms"] / 1000.0, datetime.timezone.utc)
                .strftime("%Y-%m-%d %H:%M:%S.")
                + f'{d["timestamp_ms"] % 1000:03d}'),
            "run_tag": run_tag,
            "corpus": corpus_name,
            "model": d["model"],
            "served_model": d.get("served_model") or d["model"],
            "template": d["prompt_template_slug"],
            "attribute": attr,
            "attribute_prompt_hash": d["attribute_prompt_hash"],
            "seed": seed,
            "entity_a": corpus_lines[d["entity_a_index"]],
            "entity_b": corpus_lines[d["entity_b_index"]],
            "entity_a_hash": d["entity_a_hash"],
            "entity_b_hash": d["entity_b_hash"],
            "cache_key_hash": d["cache_key_hash"],
            "higher_ranked": d.get("higher_ranked") or "",
            "ratio": d.get("ratio") if d.get("ratio") is not None else 0.0,
            "confidence": d.get("confidence") if d.get("confidence") is not None else 0.0,
            "dir_prob": dp, "entropy": ent, "top_prob": tp,
            "neighborhood_prob": nb, "posterior": post,
            "swapped": bool(d.get("swapped")),
            "cached": bool(d.get("cached")),
            "refused": bool(d.get("refused")),
            "input_tokens": d.get("input_tokens") or 0,
            "output_tokens": d.get("output_tokens") or 0,
            "error": err,
        })
    payload = "\n".join(json.dumps(r, ensure_ascii=False) for r in rows).encode()
    proc = ch_run("INSERT INTO ratiometer.judgments FORMAT JSONEachRow", payload)
    if proc.returncode != 0:
        raise RuntimeError(f"land failed: {proc.stderr.decode()[:500]}")
    return len(rows)


def sql_str(s):
    """ClickHouse string literal: single quotes (double quotes are identifiers)."""
    return "'" + s.replace("\\", "\\\\").replace("'", "\\'") + "'"


def ledger_done_attrs(run_tag, model, min_rows):
    """Attributes already landed (>= min_rows rows) for this run_tag+model —
    the resume set a supervisor restart must not re-buy."""
    q = ("SELECT attribute FROM ratiometer.judgments "
         f"WHERE run_tag = {sql_str(run_tag)} AND model = {sql_str(model)} "
         f"GROUP BY attribute HAVING count() >= {int(min_rows)} FORMAT TSVRaw")
    proc = ch_run(q)
    if proc.returncode != 0:
        raise RuntimeError(f"ledger resume query failed: {proc.stderr.decode()[:500]}")
    return {l for l in proc.stdout.decode().split("\n") if l.strip()}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--corpus", required=True)
    ap.add_argument("--attributes", required=True)
    ap.add_argument("--model", required=True)
    ap.add_argument("--budget", type=int, default=240)
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--run-tag", required=True)
    ap.add_argument("--outdir", required=True)
    ap.add_argument("--start-at", type=int, default=0)
    ap.add_argument("--template", default=None,
                    help="canonical_bucket_v1 lands a logprob posterior PMF")
    ap.add_argument("--elaborate", action="store_true",
                    help="expand each attribute into a rubric before judging")
    ap.add_argument("--concurrency", type=int, default=None,
                    help="judgements in flight (cardinal default 8); use 2 on the "
                         "gemini-cli rail, whose 429 backoff punishes bursts")
    ap.add_argument("--no-cache", action="store_true",
                    help="fresh draws (repeat-draw phases need independent samples, "
                         "not cache replays)")
    ap.add_argument("--resume-ledger", action="store_true",
                    help="skip attributes already landed (>= 90%% of budget rows) "
                         "under this run_tag+model — idempotent supervisor restarts")
    ap.add_argument("--parallel-cells", type=int, default=1,
                    help="attribute cells in flight at once. The planner issues "
                         "comparisons in waves and drains the engine to zero at "
                         "every wave tail (measured 2026-08-31: 48->0->48); two "
                         "interleaved cells fill each other's drains. Landing "
                         "stays one INSERT per attribute.")
    a = ap.parse_args()
    os.makedirs(a.outdir, exist_ok=True)
    corpus_lines = [l.rstrip("\n") for l in open(a.corpus) if l.strip()]
    corpus_name = os.path.basename(a.corpus)
    attrs = [l.strip() for l in open(a.attributes) if l.strip()]
    # Landing is one INSERT per attribute (atomic), and the ledger's
    # ReplacingMergeTree key is (model, attribute, ordered pair), so an
    # attribute's row count is its distinct-pair count, not its budget —
    # a 200-budget pass settles at ~100 rows. Presence is the done test;
    # a budget-proportional threshold re-bought every landed attribute on
    # restart (seen 2026-08-21).
    done = (ledger_done_attrs(a.run_tag, a.model, 1)
            if a.resume_ledger else set())
    if done:
        print(f"resume: {len(done)}/{len(attrs)} attributes already landed, skipping",
              flush=True)
    landed_total = 0
    t0 = time.time()
    todo = [(i, attr) for i, attr in enumerate(attrs)
            if i >= a.start_at and attr not in done]

    def cell(i, attr):
        t = time.time()
        out, trace = run_cell(a.corpus, attr, a.model, a.budget, a.seed, a.outdir, i,
                              template=a.template, elaborate=a.elaborate,
                              concurrency=a.concurrency, no_cache=a.no_cache)
        n = land(trace, corpus_lines, corpus_name, attr, a.seed, a.run_tag)
        print(f"[{i+1}/{len(attrs)}] {attr!r}: {n} judgments landed "
              f"({time.time()-t:.1f}s)", flush=True)
        return n

    if a.parallel_cells <= 1:
        for i, attr in todo:
            landed_total += cell(i, attr)
    else:
        import concurrent.futures
        with concurrent.futures.ThreadPoolExecutor(max_workers=a.parallel_cells) as ex:
            for n in ex.map(lambda ia: cell(*ia), todo):
                landed_total += n
    print(f"done: {landed_total} judgments in {time.time()-t0:.0f}s -> ratiometer.judgments")


if __name__ == "__main__":
    main()
