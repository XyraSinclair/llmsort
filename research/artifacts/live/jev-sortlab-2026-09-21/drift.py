"""Time drift: re-ask the jev-highdim k = 8 windows (asked 2026-09-19) two days later and compare answer by answer.

xyra-vault run ~/x/jev/typed-judgment -- python3 drift.py [cohort=arxiv]   (replay from trace-drift-<cohort>.jsonl needs no key)
Reads the original requests back out of the highdim trace by rebuilding them with step.py's builder, re-issues each with
salt "2026-09-21" so the cache does not answer, and prints: per-question Pearson (noul p; score expected level), mean
absolute change, share moved more than 0.05, and Spearman between the latents fitted from each day.
"""
import json, os, sys
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__)); HD = f"{HERE}/../jev-highdim-2026-09-19"; sys.path.insert(0, HD); sys.path.insert(0, f"{HERE}/../jev-bits-2026-09-19")
import step
from jevclient import call_many, load_trace, pmf
from lab import spearman

name = sys.argv[1] if len(sys.argv) > 1 else "arxiv"
items = json.load(open(f"{HD}/{name}.json")); ref = json.load(open(f"{HD}/ref-{name}.json")); n = len(items)
reqs = step.build(items, "elab", 3)
old = load_trace(f"{HD}/trace-{name}.jsonl"); bytag = {r["tag"]: r for r in old.values()}
then = [bytag[r["tag"]] for r in reqs]
for r in reqs: r["salt"] = "2026-09-21"
now = call_many(reqs, f"{HERE}/trace-drift-{name}.jsonl")
tok = sum(r["usage"]["input_tokens"] for r in now)
pn, pt, sn, st = [], [], [], []
for a, b in zip(then, now):
    for q, x in a["answers"].items():
        y = b["answers"][q]
        if x["type"] == "noul": pt.append(x["noul"]); pn.append(y["noul"])
        else:
            lv = len(x["probabilities"]); st.append(sum(p * (l + 1) for l, p in enumerate(pmf(x, lv)))); sn.append(sum(p * (l + 1) for l, p in enumerate(pmf(y, lv))))
pt, pn, st, sn = map(np.array, (pt, pn, st, sn))
print(f"{name}: {len(reqs)} calls re-asked, {tok / 1e6:.2f}M tokens ${tok * 0.042 / 1e6:.3f}")
print(f"noul  n={len(pt)}  pearson {np.corrcoef(pt, pn)[0, 1]:.4f}  mean |dp| {np.mean(np.abs(pt - pn)):.4f}  moved>0.05 {np.mean(np.abs(pt - pn) > 0.05):.3f}  identical {np.mean(pt == pn):.3f}")
print(f"score n={len(st)}  pearson {np.corrcoef(st, sn)[0, 1]:.4f}  mean |dE| {np.mean(np.abs(st - sn)):.4f}  moved>0.05 {np.mean(np.abs(st - sn) > 0.05):.3f}")
# latents per day, per attribute, from the score9 and noul pairs; Spearman day vs day and each vs Fable
out = {}
for ins in ("noul", "score9", "rate"):
    dd, df0, df1 = [], [], []
    for ai, a in enumerate(step.ATTRS):
        lat = []
        for day in (then, now):
            rows = []
            for rq, rc in zip(reqs, day):
                if rq["attr"] != a["name"]: continue
                mem = rq["mem"]
                for q, x in rc["answers"].items():
                    kind, spec = q.split("|")
                    if kind != ins: continue
                    if kind == "rate": rows.append((rq["tag"], mem[int(spec) - 1], sum(p * (l + 1) for l, p in enumerate(pmf(x, 10)))))
                    else:
                        i, j = (mem[int(s) - 1] for s in spec.split(">"))
                        rows.append((i, j, step.clip_logit(x["noul"]) if kind == "noul" else sum(p * np.log(r) for p, r in zip(pmf(x, 9), step.R9))))
            lat.append(step.fit_items(rows, n) if ins == "rate" else step.fit_pairs(rows, n))
        R = np.array([ref["ref"][a["name"]][i] for i in ref["ids"]])
        dd.append(spearman(lat[0], lat[1])); df0.append(spearman(lat[0], R)); df1.append(spearman(lat[1], R))
    out[ins] = (np.mean(dd), np.mean(df0), np.mean(df1))
    print(f"{ins:7s} latents day1 vs day3 {np.mean(dd):.3f}   vs Fable day1 {np.mean(df0):.3f} day3 {np.mean(df1):.3f}")
json.dump({"cohort": name, "calls": len(reqs), "tokens": tok, "noul": {"pearson": float(np.corrcoef(pt, pn)[0, 1]), "mad": float(np.mean(np.abs(pt - pn))), "moved05": float(np.mean(np.abs(pt - pn) > 0.05))},
           "score": {"pearson": float(np.corrcoef(st, sn)[0, 1]), "mad": float(np.mean(np.abs(st - sn))), "moved05": float(np.mean(np.abs(st - sn) > 0.05))},
           "latent": {k: {"day_vs_day": v[0], "fable_day1": v[1], "fable_day3": v[2]} for k, v in out.items()}}, open(f"{HERE}/drift-{name}.json", "w"), indent=0)
