"""Pool the Fable magnitude estimates into a reference and print its reliability.

Each cohort has 2 replicas (own item shuffle, own labels, own attribute grouping and order); a replica is two reads of
6 attributes. Reliability = Spearman between replicas on log magnitude; SB = Spearman-Brown for the 2-replica mean.
Writes ref-<cohort>.json {ids, attrs, ref: {attr: {id: mean log magnitude, z-scored}}, reliability: {attr: [r, sb]}}.
"""
import json, math, os, sys
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, f"{HERE}/../jev-bits-2026-09-19")
from analyze import spearman

attrs = [a["name"] for a in json.load(open(f"{HERE}/attributes.json"))]
for name in ("manifund", "arxiv"):
    ids = [it["id"] for it in json.load(open(f"{HERE}/{name}.json"))]
    reps = []
    for rep in (0, 1):
        got = {}
        for half in (0, 1):
            lab = json.load(open(f"{HERE}/ref/{name}-r{rep}-h{half}.labels.json"))
            for a, row in json.load(open(f"{HERE}/ref/{name}-r{rep}-h{half}.json")).items():
                assert a in attrs and set(row) == set(lab), (name, rep, half, a)
                got[a] = {lab[t]: math.log(float(v)) for t, v in row.items()}
        assert set(got) == set(attrs), (name, rep, set(attrs) - set(got))
        reps.append(got)
    out = {"ids": ids, "attrs": attrs, "ref": {}, "reliability": {}}
    print(f"# {name}: Fable replica agreement (Spearman), Spearman-Brown for the pooled mean, log-magnitude sd")
    for a in attrs:
        x, y = (np.array([r[a][i] for i in ids]) for r in reps)
        r = spearman(x, y); sb = 2 * r / (1 + r); m = (x + y) / 2
        out["ref"][a] = dict(zip(ids, ((m - m.mean()) / m.std()).round(4).tolist())); out["reliability"][a] = [round(r, 3), round(sb, 3)]
        print(f"  {a:32s} r {r:5.2f}  sb {sb:5.2f}  sd {m.std():.2f}")
    M = np.array([[out["ref"][a][i] for i in ids] for a in attrs]); C = np.corrcoef(M)
    ev = np.linalg.eigvalsh(C)[::-1]
    print(f"  mean r {np.mean([v[0] for v in out['reliability'].values()]):.2f}; attribute PCA variance shares {np.round(ev[:4] / ev.sum(), 2).tolist()}")
    hi = sorted(((abs(C[i, j]), attrs[i], attrs[j], C[i, j]) for i in range(12) for j in range(i)), reverse=True)[:4]
    print("  most correlated attribute pairs:", "; ".join(f"{p} ~ {q} {c:+.2f}" for _, p, q, c in hi))
    json.dump(out, open(f"{HERE}/ref-{name}.json", "w"), indent=0)
