"""No new calls, no reference labels used for selection: drop propositions whose item-rest correlation (against the
z-sum of their siblings) is under 0.2, then re-sum. Prints rho before/after and how many propositions survive."""
import json, os, sys
import numpy as np
HERE = os.path.dirname(os.path.abspath(__file__)); sys.path.insert(0, HERE)
import step as S
from step import clip_logit, spearman, fit_items
sys.path.insert(0, f"{HERE}/../jev-bits-2026-09-19")
from jevclient import load_trace
for name in ("arxiv", "manifund", "lw"):
    meta = json.load(open(f"{HERE}/ref-{name}.json")); ids = meta["ids"]; n = len(ids); D = json.load(open(f"{HERE}/decomp10.json"))
    wins = dict(S.windows(n, 3)); obs = {}
    for rec in load_trace(f"{HERE}/trace-{name}.jsonl").values():
        tag = rec.get("tag") or ""
        if tag.startswith("decomp10|"):
            mem = wins[tag.split("|")[1]]
            for qid, ans in rec["answers"].items():
                ai, pi, x = (int(v) for v in qid.split("|")); obs.setdefault((ai, pi), []).append((tag, mem[x - 1], clip_logit(ans["noul"])))
    elab = json.load(open(f"{HERE}/rho-{name}-elab.json")); before, after, kept = [], [], []
    print(f"# {name}: {'attribute':36s} all10 pruned kept")
    for ai, a in enumerate(meta["attrs"]):
        ref = np.array([meta["ref"][a][i] for i in ids]); z = []
        for pi, (_, sg) in enumerate(D[a]):
            p = sg * fit_items(obs[(ai, pi)], n); z.append((p - p.mean()) / p.std())
        z = np.array(z); tot = z.sum(0)
        keep = [i for i in range(len(z)) if np.corrcoef(z[i], tot - z[i])[0, 1] >= 0.2]
        b, c = spearman(tot, ref), spearman(z[keep].sum(0), ref); before.append(b); after.append(c); kept.append(len(keep))
        print(f"  {a:38s} {b:5.2f} {c:6.2f} {len(keep):4d}")
    print(f"  {'mean':38s} {np.mean(before):5.2f} {np.mean(after):6.2f} {np.mean(kept):4.1f}   (elab abstract arm: {np.mean([v['all'] for v in elab.values()]):.2f})")
