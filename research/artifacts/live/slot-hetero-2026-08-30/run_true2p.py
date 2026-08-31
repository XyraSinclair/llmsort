#!/usr/bin/env python3
"""True-rail cell: terra ratio_letter_2p_v1 through `judge` single calls
(identical code path to sort's comparisons: compare_pair -> seriate 2p,
temperature 0, no nonce). All 28 pairs of the smoke corpus, both orders,
2 repeats each = 224 gateway calls (112 comparisons)."""
import itertools, json, subprocess, os
from concurrent.futures import ThreadPoolExecutor
HERE = os.path.dirname(os.path.abspath(__file__))
BIN = os.path.join(HERE, "../../../../target/debug/llmsort")
items = [l.strip() for l in open(os.path.join(HERE, "smoke_corpus.txt")) if l.strip()]
assert len(items) == 8
def one(spec):
    pi, pj, first, second, rep = spec
    cmd = [BIN, "judge", items[first], items[second], "--by", "usefulness as advice",
           "--model", "openai/gpt-5.6-terra", "--template", "ratio_letter_2p_v1",
           "--json", "--no-cache"]
    r = subprocess.run(cmd, capture_output=True, text=True, timeout=600)
    if r.returncode != 0:
        return {"i": pi, "j": pj, "first": first, "rep": rep, "error": r.stderr[-300:]}
    d = json.loads(r.stdout)
    d.update({"i": pi, "j": pj, "first": first, "rep": rep})
    return d
specs = []
for (i, j) in itertools.combinations(range(8), 2):
    for rep in (0, 1):
        specs.append((i, j, i, j, rep)); specs.append((i, j, j, i, rep))
with ThreadPoolExecutor(max_workers=8) as ex:
    results = list(ex.map(one, specs))
with open(os.path.join(HERE, "raw_true2p.jsonl"), "w") as f:
    for r in results: f.write(json.dumps(r) + "\n")
errs = sum(1 for r in results if "error" in r)
print(f"done: {len(results)} comparisons, {errs} errors, "
      f"${sum(r.get('cost_nanodollars',0) for r in results)/1e9:.4f}")
