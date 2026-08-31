#!/usr/bin/env python3
import json, math, os, statistics as st
HERE = os.path.dirname(os.path.abspath(__file__))
rows = [json.loads(l) for l in open(os.path.join(HERE, "raw_smoke.jsonl"))]
rows = [r for r in rows if "error" not in r and r.get("draws") and len(r["draws"]) == 2]
inv = {}
for r in rows:
    inv.setdefault((r["i"], r["j"]), {})["fwd" if r["first"] == r["i"] else "rev"] = r["draws"]
pairs = {k: v for k, v in inv.items() if len(v) == 2}
se2 = [ (v[o][0]-v[o][1])**2/2 for v in pairs.values() for o in ("fwd","rev") ]
sigma_e = math.sqrt(st.mean(se2))
b = [ (st.mean(v["fwd"])+st.mean(v["rev"]))/2 for v in pairs.values() ]
b1 = [ (v["fwd"][0]+v["rev"][0])/2 for v in pairs.values() ]  # 1-draw analog of the smoke run
gm, var_obs = st.mean(b), st.variance(b)
noise = sigma_e**2/4
print(f"pairs={len(pairs)}  sigma_e={sigma_e:.3f} nats  (24-corpus cell: 0.141)")
print(f"b: global mean={gm:+.3f} sd_obs={math.sqrt(var_obs):.3f}  "
      f"hetero var(beta)={max(0,var_obs-noise):.4f} ({100*max(0,var_obs-noise)/var_obs:.0f}%) "
      f"noise share={100*noise/var_obs:.0f}%")
print(f"2*mean|b| (2-draw) = {2*st.mean(map(abs,b)):.3f} nats")
print(f"2*mean|b| (1-draw analog) = {2*st.mean(map(abs,b1)):.3f} nats  (smoke run reported 0.277 on its 16 planned pairs)")
pred = 2*math.sqrt(sigma_e**2/2 + gm*gm)*math.sqrt(2/math.pi)
print(f"noise-only prediction for 1-draw 2*E|b| = {pred:.3f}")
