#!/usr/bin/env python3
"""Analysis for the slot-heterogeneity cell. Reads raw.jsonl; prints the
variance decomposition and the equal-cost backtest."""
import json, math, os, statistics as st

HERE = os.path.dirname(os.path.abspath(__file__))
rows = [json.loads(l) for l in open(os.path.join(HERE, "raw.jsonl"))]
rows = [r for r in rows if "error" not in r and r.get("draws") and len(r["draws"]) == 2
        and all(d is not None for d in r["draws"])]

# Index by (pair, orientation). first==i -> forward.
inv = {}
for r in rows:
    key = (r["i"], r["j"])
    inv.setdefault(key, {})["fwd" if r["first"] == r["i"] else "rev"] = r["draws"]
pairs = {k: v for k, v in inv.items() if len(v) == 2}
n = len(pairs)

# Within-call noise: pooled from the two draws of each invocation (1 df each).
se2_pool = [ (v[o][0]-v[o][1])**2 / 2 for v in pairs.values() for o in ("fwd","rev") ]
sigma_e = math.sqrt(st.mean(se2_pool))

b = []; s = []; s1 = []; m_fwd2 = []
for v in pairs.values():
    mf, mr = st.mean(v["fwd"]), st.mean(v["rev"])
    b.append((mf + mr) / 2)            # slot term (presentation frame)
    s.append((mf - mr) / 2)            # item-frame signal, all 4 draws
    s1.append((v["fwd"][0] - v["rev"][0]) / 2)  # counterbalance @ 1 draw/order
    m_fwd2.append(mf)                   # single order @ 2 draws

gm = st.mean(b); var_obs = st.variance(b)
noise_share = sigma_e**2 / 4
var_beta = max(0.0, var_obs - noise_share)
Eb2 = st.mean(x*x for x in b)
print(f"pairs={n}  sigma_e(within-call)={sigma_e:.3f} nats")
print(f"slot term b: global mean={gm:+.3f}  sd_obs={math.sqrt(var_obs):.3f}")
print(f"decomposition of var(b)={var_obs:.4f}: hetero var(beta)={var_beta:.4f} "
      f"({100*var_beta/var_obs:.0f}%) · within-noise sigma_e^2/4={noise_share:.4f} "
      f"({100*noise_share/var_obs:.0f}%)")
print(f"energy shares of E[b^2]={Eb2:.4f}: global {100*gm*gm/Eb2:.0f}% · "
      f"hetero {100*var_beta/Eb2:.0f}% · noise {100*noise_share/Eb2:.0f}%")
print(f"order residual analog 2*mean|b| = {2*st.mean(map(abs,b)):.3f} nats "
      f"(smoke pack reported 0.277)")

# Least-squares item scores from pair measurements: min sum (x_i - x_j - y_ij)^2.
def fit(yv):
    keys = list(pairs.keys())
    N = 24
    # Gauss-Seidel on the normal equations (deg small, converges fast).
    x = [0.0]*N
    adj = {}
    for (i,j), y in zip(keys, yv):
        adj.setdefault(i, []).append((j, +y))
        adj.setdefault(j, []).append((i, -y))
    for _ in range(500):
        for i in range(N):
            if i in adj:
                x[i] = st.mean(x[j] + y for j, y in adj[i])
        m = st.mean(x)
        x = [v - m for v in x]
    return x

def spearman(a, c):
    def rank(v):
        order = sorted(range(len(v)), key=lambda k: v[k])
        rk = [0]*len(v)
        for pos, idx in enumerate(order): rk[idx] = pos
        return rk
    ra, rc = rank(a), rank(c)
    n = len(a); d2 = sum((ra[k]-rc[k])**2 for k in range(n))
    return 1 - 6*d2/(n*(n*n-1))

truth = fit(s)                                   # all 4 draws, counterbalanced
cb1   = fit(s1)                                  # equal-cost: both orders, 1 draw each
so_raw  = fit(m_fwd2)                            # equal-cost: one order, 2 draws, raw
so_corr = fit([m - gm for m in m_fwd2])          # + global correction
print("\nequal-cost backtest (2 calls/pair), Spearman vs 4-draw counterbalanced truth:")
print(f"  counterbalance, 1 draw/order:        rho={spearman(truth, cb1):.3f}")
print(f"  single order, 2 draws, raw:          rho={spearman(truth, so_raw):.3f}")
print(f"  single order, 2 draws, global-corr:  rho={spearman(truth, so_corr):.3f}")
