"""How consistent are Jev's within-window pairwise reads with a single latent per item?  (replay only; no key)

python3 holonomy.py [variant=elab]   -- reads obs-<cohort>-<variant>.jsonl for arxiv, manifund, lw.
Per (call = window x attribute) the 56 ordered pairs give L(x,y): score9 E[log ratio] or noul logit.
  order   : sd of L(x,y) + L(y,x)  over sd of L            (0 = perfectly antisymmetric)
  bias    : mean of L(x,y) + L(y,x) in sd units             (mention-first bias)
  curl    : 1 - R^2 of the antisymmetric part on u_x - u_y   (share of the pairwise signal no potential explains)
  tri     : sd of L~(a,b)+L~(b,c)+L~(c,a) over sd of L~ ; sqrt(3) if edges were independent noise, 0 if a gradient
"""
import json, os, sys, itertools
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
var = sys.argv[1] if len(sys.argv) > 1 else "elab"; K = 8


def potential_r2(Lt):  # Lt: K x K antisymmetric; least squares u with L~(x,y) ~ u_x - u_y
    u = Lt.mean(axis=1)  # exact LS solution for complete antisymmetric data
    pred = u[:, None] - u[None, :]
    m = ~np.eye(K, dtype=bool)
    return 1 - ((Lt - pred)[m] ** 2).sum() / (Lt[m] ** 2).sum()


print(f"{'cohort':9s} {'instr':7s} {'order':>6s} {'bias':>6s} {'curl':>6s} {'tri':>6s} {'edge sd':>8s}  (mean over calls; curl/tri range over attributes)")
for name in ("arxiv", "manifund", "lw"):
    obs = [json.loads(l) for l in open(f"{HERE}/obs-{name}-{var}.jsonl")]
    for ins in ("score9", "noul"):
        calls = {}
        for o in obs:
            if o["instr"] == ins:
                calls.setdefault((o["call"], o["attr"]), {})[(o["i"], o["j"])] = o["y"]
        rows = []; per_attr = {}
        for (c, a), d in calls.items():
            ids = sorted({i for i, _ in d}); idx = {v: k for k, v in enumerate(ids)}
            L = np.zeros((K, K))
            for (i, j), y in d.items(): L[idx[i], idx[j]] = y
            m = ~np.eye(K, dtype=bool)
            S = (L + L.T)[m]; Lt = (L - L.T) / 2
            tri = [Lt[a_, b] + Lt[b, c_] + Lt[c_, a_] for a_, b, c_ in itertools.combinations(range(K), 3)]
            r = (S.std() / L[m].std(), S.mean() / L[m].std(), 1 - potential_r2(Lt), np.std(tri) / Lt[m].std(), L[m].std())
            rows.append(r); per_attr.setdefault(a, []).append(r)
        M = np.mean(rows, axis=0)
        pa = {a: np.mean(v, axis=0) for a, v in per_attr.items()}
        cr = [p[2] for p in pa.values()]; tr = [p[3] for p in pa.values()]
        print(f"{name:9s} {ins:7s} {M[0]:6.2f} {M[1]:+6.2f} {M[2]:6.2f} {M[3]:6.2f} {M[4]:8.2f}  curl {min(cr):.2f}–{max(cr):.2f}  tri {min(tr):.2f}–{max(tr):.2f}")
        if ins == "score9":
            worst = sorted(pa.items(), key=lambda kv: -kv[1][2])[:2]
            print(f"{'':9s} most curl: " + "; ".join(f"{a} {p[2]:.2f}" for a, p in worst))

# Across windows: latents fitted from one round alone, against another round and against the Fable reference.
sys.path.insert(0, HERE)
from step import fit_pairs, fit_items, spearman
print("\nround vs round / one round vs Fable (mean over attributes)")
for name in ("arxiv", "manifund", "lw"):
    obs = [json.loads(l) for l in open(f"{HERE}/obs-{name}-{var}.jsonl")]; ref = json.load(open(f"{HERE}/ref-{name}.json")); n = len(ref["ids"]); out = []
    for ins in ("score9", "noul", "rate"):
        rr, rf = [], []
        for a in ref["attrs"]:
            rows = [[o for o in obs if o["instr"] == ins and o["attr"] == a and f"|r{r}w" in o["call"]] for r in range(3)]
            lat = [fit_items([(o["call"], o["i"], o["y"]) for o in R], n) if ins == "rate" else fit_pairs([(o["i"], o["j"], o["y"]) for o in R], n) for R in rows]
            R0 = np.array([ref["ref"][a][i] for i in ref["ids"]])
            rr.append(np.mean([spearman(lat[i], lat[j]) for i in range(3) for j in range(i)])); rf.append(np.mean([spearman(l, R0) for l in lat]))
        out.append(f"{ins} {np.mean(rr):.2f} / {np.mean(rf):.2f}")
    print(f"{name:9s} " + "   ".join(out))
