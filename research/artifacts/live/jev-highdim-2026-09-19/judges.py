"""Three judges on the same items: hosted Jev (typed, windows of k), gemma-4-31b (generating, setwise windows of k),
Fable 5.1 (the reference, magnitude estimation). python3 judges.py -> judges.json, replay only.

Per cohort and k: agreement with Fable (Spearman, mean over 12 attributes), self-agreement (Jev: latents from one
round of windows against another; gemma: seed 1 against seed 2; Fable: replica against replica), and the order
inconsistency (Jev: sd of L(x,y)+L(y,x) over sd of L; gemma: the setwise gauge's direction flip rate).
"""
import json, os, sys, glob
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__)); sys.path.insert(0, HERE)
from step import fit_pairs, fit_items, spearman
COHORTS = ("arxiv", "manifund", "lw"); attrs = json.load(open(f"{HERE}/attributes.json")); A = [a["name"] for a in attrs]
out = {}
for c in COHORTS:
    ref = json.load(open(f"{HERE}/ref-{c}.json")); ids = ref["ids"]; n = len(ids)
    texts = [it["text"] for it in json.load(open(f"{HERE}/{c}.json"))]
    R = {a: np.array([ref["ref"][a][i] for i in ids]) for a in A}
    o = {"fable": {"self": float(np.mean([ref["reliability"][a][0] for a in A]))}, "jev": {}, "gemma": {}}
    # Jev by window size
    for k in (2, 4, 8, 12, 24):
        f = f"{HERE}/obs-{c}-elab{'' if k == 8 else f'-k{k}'}.jsonl"
        if not os.path.exists(f): continue
        obs = [json.loads(l) for l in open(f)]; row = {"tok": None}
        for ins in ("noul", "score9", "rate"):
            rho, rr, order = [], [], []
            for a in A:
                rows = [x for x in obs if x["instr"] == ins and x["attr"] == a]
                fit = (lambda rs: fit_items([(x["call"], x["i"], x["y"]) for x in rs], n)) if ins == "rate" else (lambda rs: fit_pairs([(x["i"], x["j"], x["y"]) for x in rs], n))
                lat = fit(rows); rho.append(spearman(lat, R[a]))
                per = [fit([x for x in rows if f"|r{r}w" in x["call"]]) for r in range(3)]
                rr.append(np.mean([spearman(per[i], per[j]) for i in range(3) for j in range(i)]))
                if ins != "rate":
                    L = {(x["call"], x["i"], x["j"]): x["y"] for x in rows}
                    S = [L[(cl, i, j)] + L[(cl, j, i)] for (cl, i, j) in L if (cl, j, i) in L and i < j]
                    order.append(np.std(S) / np.std(list(L.values())))
            row[ins] = {"rho": float(np.mean(rho)), "self": float(np.mean(rr)), "order": float(np.mean(order)) if order else None}
        o["jev"][k] = row
    # gemma by window size
    for k in (4, 8, 24):
        lat = {}; flips = []; cost = 0.0; calls = 0
        for s in (1, 2):
            for ai, a in enumerate(A):
                f = f"{HERE}/gemma/{c}-a{ai}-k{k}-s{s}.json"
                if not os.path.exists(f): continue
                d = json.load(open(f)); by = {it["text"]: it["latent_mean"] for it in d["items"]}
                lat[(s, a)] = np.array([by[t] for t in texts]); flips.append(d["gauge"]["flip_rate"]) if d["gauge"].get("flip_rate") is not None else None; cost += d["cost_nanodollars"] / 1e9; calls += d["calls"]
        if not lat: continue
        rho1 = [spearman(lat[(1, a)], R[a]) for a in A if (1, a) in lat]
        both = [spearman((lat[(1, a)] + lat[(2, a)]) / 2, R[a]) for a in A if (1, a) in lat and (2, a) in lat]
        selfr = [spearman(lat[(1, a)], lat[(2, a)]) for a in A if (1, a) in lat and (2, a) in lat]
        o["gemma"][k] = {"rho1": float(np.mean(rho1)), "rho2": float(np.mean(both)) if both else None, "self": float(np.mean(selfr)) if selfr else None,
                         "flip": float(np.mean(flips)) if flips else None, "cost": cost, "calls": calls, "n": len(lat)}
    out[c] = o
    print(f"# {c}  Fable self {o['fable']['self']:.2f}")
    for k, r in o["jev"].items():
        print(f"  jev   k={k:<3} " + "  ".join(f"{i} rho {r[i]['rho']:.2f} self {r[i]['self']:.2f}" + (f" order {r[i]['order']:.2f}" if r[i]['order'] else "") for i in ("noul", "score9", "rate")))
    for k, r in o["gemma"].items():
        print(f"  gemma k={k:<3} rho(1 seed) {r['rho1']:.2f}  rho(2 seeds) {r['rho2'] if r['rho2'] is None else round(r['rho2'], 2)}  self {r['self'] if r['self'] is None else round(r['self'], 2)}  flip {r['flip'] if r['flip'] is None else round(r['flip'], 2)}  ${r['cost']:.3f} {r['calls']} calls  ({r['n']} sorts)")
json.dump(out, open(f"{HERE}/judges.json", "w"), indent=0)
