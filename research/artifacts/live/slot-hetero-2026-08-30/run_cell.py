#!/usr/bin/env python3
"""Slot-heterogeneity cell (2026-08-30): decompose terra order residual (NB: ran canonical_v2 JSON via the nonce_draws silent fallback, NOT the 2p template — see NOTES.md incident) into
global slot bias, per-pair slot bias, and within-call noise.

Design: K=48 pairs sampled (seed 7) from the fixed 24-item corpus; per pair,
both presentation orders; per order, `llmsort judge --draws 2` (nonce repeats).
Signed draws are presentation-frame nats (+ = first-listed favored).
"""
import itertools, json, random, subprocess, sys, os
from concurrent.futures import ThreadPoolExecutor

HERE = os.path.dirname(os.path.abspath(__file__))
BIN = os.path.join(HERE, "../../../../target/debug/llmsort")
BY = "usefulness as advice"
MODEL = "openai/gpt-5.6-terra"
TEMPLATE = "ratio_letter_2p_v1"
K_PAIRS, DRAWS, SEED, CONCURRENCY = 48, 2, 7, 8

items = [l.strip() for l in open(os.path.join(HERE, "corpus.txt")) if l.strip()]
assert len(items) == 24
rng = random.Random(SEED)
pairs = rng.sample(list(itertools.combinations(range(24), 2)), K_PAIRS)

def one(order_spec):
    pi, pj, first, second = order_spec
    cmd = [BIN, "judge", items[first], items[second], "--by", BY,
           "--model", MODEL, "--template", TEMPLATE,
           "--draws", str(DRAWS), "--json", "--no-cache"]
    r = subprocess.run(cmd, capture_output=True, text=True, timeout=600)
    if r.returncode != 0:
        return {"i": pi, "j": pj, "first": first, "error": r.stderr[-500:]}
    rep = json.loads(r.stdout)
    return {"i": pi, "j": pj, "first": first, "draws": rep["draws"],
            "nonces": rep["nonces"], "sigma_w": rep.get("sigma_w"),
            "cost_nanodollars": rep["cost_nanodollars"],
            "cache_read": rep["cache_read_tokens_total"],
            "input_tokens": rep["input_tokens_total"], "refusals": rep["refusals"]}

specs = []
for (i, j) in pairs:
    specs.append((i, j, i, j))   # forward order: i listed first
    specs.append((i, j, j, i))   # reversed order: j listed first
with ThreadPoolExecutor(max_workers=CONCURRENCY) as ex:
    results = list(ex.map(one, specs))
with open(os.path.join(HERE, "raw.jsonl"), "w") as f:
    for r in results:
        f.write(json.dumps(r) + "\n")
errs = [r for r in results if "error" in r]
cost = sum(r.get("cost_nanodollars", 0) for r in results) / 1e9
print(f"done: {len(results)} invocations, {len(errs)} errors, ${cost:.4f}")
for e in errs[:3]:
    print("ERR", e["i"], e["j"], e["error"][:200], file=sys.stderr)
