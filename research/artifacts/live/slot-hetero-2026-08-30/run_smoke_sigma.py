#!/usr/bin/env python3
"""Follow-up: sigma_e on the EXACT smoke corpus (NB: canonical_v2 JSON via the nonce_draws silent fallback, NOT 2p — see NOTES.md incident) + full slot decomposition on
its 16 planned-pair analogs. All 28 pairs of the 8 items, both orders,
2 nonce draws each = 112 draws."""
import itertools, json, subprocess, os
from concurrent.futures import ThreadPoolExecutor
HERE = os.path.dirname(os.path.abspath(__file__))
BIN = os.path.join(HERE, "../../../../target/debug/llmsort")
items = [l.strip() for l in open(os.path.join(HERE, "smoke_corpus.txt")) if l.strip()]
assert len(items) == 8
def one(spec):
    pi, pj, first, second = spec
    cmd = [BIN, "judge", items[first], items[second], "--by", "usefulness as advice",
           "--model", "openai/gpt-5.6-terra", "--template", "ratio_letter_2p_v1",
           "--draws", "2", "--json", "--no-cache"]
    r = subprocess.run(cmd, capture_output=True, text=True, timeout=600)
    if r.returncode != 0:
        return {"i": pi, "j": pj, "first": first, "error": r.stderr[-300:]}
    rep = json.loads(r.stdout)
    return {"i": pi, "j": pj, "first": first, "draws": rep["draws"],
            "cost_nanodollars": rep["cost_nanodollars"]}
specs = []
for (i, j) in itertools.combinations(range(8), 2):
    specs.append((i, j, i, j)); specs.append((i, j, j, i))
with ThreadPoolExecutor(max_workers=8) as ex:
    results = list(ex.map(one, specs))
with open(os.path.join(HERE, "raw_smoke.jsonl"), "w") as f:
    for r in results: f.write(json.dumps(r) + "\n")
errs = sum(1 for r in results if "error" in r)
print(f"done: {len(results)} invocations, {errs} errors, "
      f"${sum(r.get('cost_nanodollars',0) for r in results)/1e9:.4f}")
