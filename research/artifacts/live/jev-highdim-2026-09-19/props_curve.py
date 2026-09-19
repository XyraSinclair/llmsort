"""No new calls: rho of the decomposition latent against the number of propositions used (all subsets, mean over attributes)."""
import json, os, sys, itertools
import numpy as np
HERE = os.path.dirname(os.path.abspath(__file__)); sys.path.insert(0, HERE)
import step as S
from step import clip_logit, spearman, fit_items
sys.path.insert(0, f"{HERE}/../jev-bits-2026-09-19")
from jevclient import load_trace
for name in ("arxiv", "manifund"):
    meta = json.load(open(f"{HERE}/ref-{name}.json")); ids = meta["ids"]; n = len(ids); D = json.load(open(f"{HERE}/decomp.json"))
    wins = dict(S.windows(n, 3)); obs = {}
    for rec in load_trace(f"{HERE}/trace-{name}.jsonl").values():
        tag = rec.get("tag") or ""
        if tag.startswith("decomp|"):
            mem = wins[tag.split("|")[1]]
            for qid, ans in rec["answers"].items():
                ai, pi, x = (int(v) for v in qid.split("|")); obs.setdefault((ai, pi), []).append((tag, mem[x - 1], clip_logit(ans["noul"])))
    out = []
    for k in range(1, 6):
        rs = []
        for ai, a in enumerate(meta["attrs"]):
            ref = np.array([meta["ref"][a][i] for i in ids]); z = []
            for pi, (_, sg) in enumerate(D[a]):
                p = sg * fit_items(obs[(ai, pi)], n); z.append((p - p.mean()) / p.std())
            rs.append(np.mean([spearman(sum(z[i] for i in c), ref) for c in itertools.combinations(range(5), k)]))
        out.append(float(np.mean(rs)))
    print(name, "rho by number of propositions 1..5:", " ".join(f"{v:.2f}" for v in out))
