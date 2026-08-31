#!/usr/bin/env python3
import json, math, os, statistics as st
HERE = os.path.expanduser("~/projects/llmsort/research/artifacts/live/slot-hetero-2026-08-30")
rows = [json.loads(l) for l in open(os.path.join(HERE, "raw_true2p.jsonl"))]
rows = [r for r in rows if "error" not in r and not r.get("refused")]
def m_pres(r):
    m = math.log(r["ratio"]); return m if r["higher_ranked"] == "A" else -m
inv = {}
for r in rows:
    key=(r["i"],r["j"]); ori = "fwd" if r["first"]==r["i"] else "rev"
    inv.setdefault(key,{}).setdefault(ori,{})[r["rep"]] = m_pres(r)
pairs = {k:v for k,v in inv.items() if len(v)==2 and all(len(o)==2 for o in v.values())}
se2=[(v[o][0]-v[o][1])**2/2 for v in pairs.values() for o in ("fwd","rev")]
sigma_e=math.sqrt(st.mean(se2))
b=[(st.mean(v["fwd"].values())+st.mean(v["rev"].values()))/2 for v in pairs.values()]
b1=[(v["fwd"][0]+v["rev"][0])/2 for v in pairs.values()]
s=[(st.mean(v["fwd"].values())-st.mean(v["rev"].values()))/2 for v in pairs.values()]
gm,var_obs=st.mean(b),st.variance(b); noise=sigma_e**2/4
vb=max(0,var_obs-noise); Eb2=st.mean(x*x for x in b)
print(f"TRUE 2p RAIL (temp 0, no nonce, seriate path): pairs={len(pairs)} usable_rows={len(rows)}")
print(f"sigma_e = {sigma_e:.3f} nats/call")
print(f"b: global mean={gm:+.3f}  sd_obs={math.sqrt(var_obs):.3f}")
print(f"var(b)={var_obs:.4f}: hetero var(beta)={vb:.4f} ({100*vb/var_obs:.0f}%) noise={100*noise/var_obs:.0f}%")
print(f"energy E[b^2]={Eb2:.4f}: global {100*gm*gm/Eb2:.0f}% hetero {100*vb/Eb2:.0f}% noise {100*(sigma_e**2/4)/Eb2:.0f}%")
print(f"2*mean|b| 2-draw={2*st.mean(map(abs,b)):.3f}  1-draw analog={2*st.mean(map(abs,b1)):.3f}  (sort smoke: 0.277)")
pred=2*math.sqrt(sigma_e**2/2+gm*gm+vb)*math.sqrt(2/math.pi)
print(f"noise+bias prediction for 1-draw 2*E|b| = {pred:.3f}")
print(f"signal sd(s) = {st.stdev(s):.3f}  -> per-call noise/signal = {sigma_e/st.stdev(s):.2f}")
