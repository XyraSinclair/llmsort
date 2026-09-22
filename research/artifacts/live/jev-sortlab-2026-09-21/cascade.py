"""Does Jev know when it is wrong? Escalation as a sorting primitive. python3 cascade.py (no key; reads jev-highdim).

Per cohort and attribute (3 cohorts x 12 attributes, 24 items, jev-highdim k = 8, three rounds): Jev's item latent
from all rounds; two per-item uncertainty signals Jev itself provides — (sd) the spread of the item's latent across the
three single-round fits, (conf) one minus the mean |p - 0.5| x 2 of its yes/no answers; the item's rank error against
Fable replica B. Then a cascade: hand the q most uncertain items to Fable replica A (an independent read), keep Jev's
order for the rest, and score the merged order against replica B — versus handing q random items, versus all-Jev and
all-A. If uncertainty predicts error, the curve beats random and the cost of a frontier read is spent where it counts.
"""
import json, math, os, sys
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__)); HD = f"{HERE}/../jev-highdim-2026-09-19"; sys.path.insert(0, HERE)
from lab import spearman, rank

ATTRS = [a["name"] for a in json.load(open(f"{HD}/attributes.json"))]


def replica(name, rep, ids):
    got = {}
    for half in (0, 1):
        lab = json.load(open(f"{HD}/ref/{name}-r{rep}-h{half}.labels.json"))
        for a, row in json.load(open(f"{HD}/ref/{name}-r{rep}-h{half}.json")).items():
            got[a] = {lab[t]: math.log(float(v)) for t, v in row.items()}
    return {a: np.array([got[a][i] for i in ids]) for a in ATTRS}


def fit_pairs(rows, n):
    A = np.zeros((len(rows) + 1, n)); b = np.zeros(len(rows) + 1)
    for t, (i, j, y) in enumerate(rows): A[t, i], A[t, j], b[t] = 1, -1, y
    A[-1] = 1; return np.linalg.lstsq(A, b, rcond=None)[0]


def merge(jev_u, other_u, pick):
    """Order: items not picked keep Jev's order; picked items are placed by the other judge's value mapped through
    Jev's empirical scale (quantile match on the unpicked set), so the two scales are commensurable."""
    keep = np.array([i for i in range(len(jev_u)) if i not in pick])
    if len(pick) == 0: return jev_u.copy()
    if len(keep) < 2: return other_u.copy()
    # linear map of other -> jev fitted on the kept items
    a, b = np.polyfit(other_u[keep], jev_u[keep], 1)
    u = jev_u.copy(); u[list(pick)] = a * other_u[list(pick)] + b
    return u


out = {}
QS = [0, 2, 4, 6, 8, 12, 16, 24]
for name in ("arxiv", "manifund", "lw"):
    ids = [it["id"] for it in json.load(open(f"{HD}/{name}.json"))]; n = len(ids)
    A, B = replica(name, 0, ids), replica(name, 1, ids)
    obs = [json.loads(l) for l in open(f"{HD}/obs-{name}-elab.jsonl")]
    gtexts = json.load(open(f"{HD}/gemma/{name}.items.json")); items = json.load(open(f"{HD}/{name}.json"))
    assert all(gtexts[i].startswith(items[i]["text"][:60]) for i in range(n))
    def gemma(ai):
        z = np.zeros(n)
        for seed in (1, 2):
            for it in json.load(open(f"{HD}/gemma/{name}-a{ai}-k8-s{seed}.json"))["items"]: z[int(it["id"].split("-")[1])] += it["z_score"]
        return z
    res = {"pred_gem": [], "pred_sd": [], "pred_conf": [], "gem_cascade": {q: [] for q in QS}, "attr_jg": [], "attr_jB": [], "cascade": {q: [] for q in QS}, "random": {q: [] for q in QS}, "oracle": {q: [] for q in QS}, "jev": [], "A": []}
    rng = np.random.default_rng(3)
    for ai, a in enumerate(ATTRS):
        g = gemma(ai)
        rows = [(o["i"], o["j"], o["y"]) for o in obs if o["attr"] == a and o["instr"] in ("noul", "score9")]
        u = fit_pairs(rows, n)
        per = [fit_pairs([(o["i"], o["j"], o["y"]) for o in obs if o["attr"] == a and o["instr"] in ("noul", "score9") and f"|r{r}w" in o["call"]], n) for r in range(3)]
        sd = np.std(np.array(per), axis=0)
        conf = np.zeros(n); cnt = np.zeros(n)
        for o in obs:
            if o["attr"] == a and o["instr"] == "noul":
                p = 1 / (1 + math.exp(-o["y"])); c = abs(p - .5) * 2
                conf[o["i"]] += c; conf[o["j"]] += c; cnt[o["i"]] += 1; cnt[o["j"]] += 1
        unc = 1 - conf / cnt
        err = np.abs(rank(u) - rank(B[a]))
        dis = np.abs(rank(u) - rank(g)); order_gem = np.argsort(-dis)
        res["pred_gem"].append(spearman(dis, err)); res["attr_jg"].append(spearman(u, g)); res["attr_jB"].append(spearman(u, B[a]))
        for q in QS: res["gem_cascade"][q].append(spearman(merge(u, A[a], set(order_gem[:q].tolist())), B[a]))
        res["pred_sd"].append(spearman(sd, err)); res["pred_conf"].append(spearman(unc, err))
        res["jev"].append(spearman(u, B[a])); res["A"].append(spearman(A[a], B[a]))
        order_unc = np.argsort(-sd); order_err = np.argsort(-err)
        for q in QS:
            res["cascade"][q].append(spearman(merge(u, A[a], set(order_unc[:q].tolist())), B[a]))
            res["oracle"][q].append(spearman(merge(u, A[a], set(order_err[:q].tolist())), B[a]))
            res["random"][q].append(np.mean([spearman(merge(u, A[a], set(rng.choice(n, q, replace=False).tolist())), B[a]) for _ in range(8)]))
    m = lambda xs: float(np.mean(xs))
    out[name] = {"pred_gem": m(res["pred_gem"]), "gem_cascade": {q: m(v) for q, v in res["gem_cascade"].items()}, "attr_jg": res["attr_jg"], "attr_jB": res["attr_jB"], "pred_sd": m(res["pred_sd"]), "pred_conf": m(res["pred_conf"]), "jev": m(res["jev"]), "A": m(res["A"]),
                 "cascade": {q: m(v) for q, v in res["cascade"].items()}, "random": {q: m(v) for q, v in res["random"].items()}, "oracle": {q: m(v) for q, v in res["oracle"].items()}}
    o = out[name]
    print(f"{name:9s} uncertainty->error: round-sd {o['pred_sd']:.2f}  low-confidence {o['pred_conf']:.2f}  gemma-disagreement {o['pred_gem']:.2f}   all-Jev {o['jev']:.3f}  all-Fable-A {o['A']:.3f}")
    print("  escalate q:  " + "  ".join(f"{q:2d}" for q in QS))
    for k in ("cascade", "gem_cascade", "random", "oracle"): print(f"  {k:9s}    " + "  ".join(f"{o[k][q]:.2f}"[1:] for q in QS))
jg = [v for c in out.values() for v in c["attr_jg"]]; jB = [v for c in out.values() for v in c["attr_jB"]]
print(f"attribute level (36 cells): Jev–gemma agreement predicts Jev–Fable agreement at r {np.corrcoef(jg, jB)[0, 1]:.2f}, Spearman {spearman(np.array(jg), np.array(jB)):.2f}")
json.dump(out, open(f"{HERE}/cascade.json", "w"), indent=0)
