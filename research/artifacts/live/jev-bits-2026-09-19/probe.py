"""API-shape probe before the experiment: isolation, determinism, question-count scaling, state size.
Run: xyra-vault run ~/x/jev/typed-judgment -- python3 probe.py
"""
import json, os, statistics as st, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from jevclient import call, load_trace, pmf, JevError

HERE = os.path.dirname(os.path.abspath(__file__))
TRACE = f"{HERE}/trace-probe.jsonl"
B = f"{HERE}/../../../batteries/multi-criteria-bench"
items = json.load(open(f"{B}/arxiv.json"))
crit = {c["name"]: c["prompt"] for c in json.load(open(f"{B}/arxiv.criteria.json"))}
seen = load_trace(TRACE)

L5 = ["B is far greater than A, ten times or more", "B is clearly greater than A, about three times",
      "A and B are about equal", "A is clearly greater than B, about three times", "A is far greater than B, ten times or more"]


def pair_state(a, b):
    return f"ITEM A:\n{a['text']}\n\nITEM B:\n{b['text']}"


def q5(c):
    return {"type": "score", "instructions": f"Criterion — {crit[c]}\nOn this criterion, how does item A compare with item B, as a ratio?", "criteria": L5}


a, b = items[3], items[17]
S = pair_state(a, b)

print("== determinism: same request x4")
ds = [pmf(call(S, {"r": q5("novelty")}, TRACE, seen, salt=f"rep{i}", tag="det")["answers"]["r"], 5) for i in range(4)]
for d in ds:
    print("  ", [round(x, 4) for x in d])

print("== isolation: the same question alone vs beside 1, 10, 60 other questions")
base = ds[0]
for n in (1, 10, 60):
    qs = {"r": q5("novelty")}
    for i in range(n):
        c = ["clarity", "evidence"][i % 2]
        qs[f"x{i}"] = {"type": "noul", "instructions": f"Variant {i}. Item A is stronger than item B on: {crit[c]}"}
    r = call(S, qs, TRACE, seen, tag=f"iso{n}")
    d = pmf(r["answers"]["r"], 5)
    print(f"  +{n:3d} q: maxabs diff {max(abs(x - y) for x, y in zip(d, base)):.4f}  latency {r['latency_ms']:.0f} ms  usage {r['usage']}")

print("== question-count scaling (distinct noul questions on one pair state)")
for n in (1, 30, 100, 300, 1000):
    qs = {f"x{i}": {"type": "noul", "instructions": f"Variant {i}. Item A is stronger than item B on: {crit['novelty']}"} for i in range(n)}
    try:
        r = call(S, qs, TRACE, seen, tag=f"scale{n}")
        ps = [r["answers"][f"x{i}"]["noul"] for i in range(n)]
        print(f"  {n:5d} q: latency {r['latency_ms']:.0f} ms usage {r['usage']} p mean {st.mean(ps):.3f} sd {st.pstdev(ps):.3f}")
    except JevError as e:
        print(f"  {n:5d} q: {e}")

print("== state size: k items in one state, one question")
for k in (8, 16, 40):
    Sk = "\n\n".join(f"ITEM {i + 1}:\n{it['text']}" for i, it in enumerate(items[:k]))
    try:
        r = call(Sk, {"r": {"type": "choice", "instructions": f"Which item is highest on: {crit['novelty']}",
                            "criteria": {f"item_{i + 1}": f"Item {i + 1}" for i in range(k)}}}, TRACE, seen, tag=f"state{k}")
        pr = r["answers"]["r"]["probabilities"]
        top = sorted(pr.items(), key=lambda kv: -kv[1])[:3]
        print(f"  k={k:3d}: latency {r['latency_ms']:.0f} ms usage {r['usage']} top3 {[(a, round(p, 3)) for a, p in top]}")
    except JevError as e:
        print(f"  k={k:3d}: {e}")

print("== score level cap: 10 vs 11 levels")
for n in (10, 11):
    try:
        r = call(S, {"r": {"type": "score", "instructions": "How does A compare with B on novelty?", "criteria": [f"level {i}" for i in range(n)]}}, TRACE, seen, tag=f"lev{n}")
        print(f"  {n} levels ok")
    except JevError as e:
        print(f"  {n} levels: {e}")
