#!/usr/bin/env python3
"""Differentiation of candidate reader-priority attributes against the existing battery.

Inputs (same 20 entities per lens):
  cand/ledger.jsonl        candidate latents (gemma-4-31b, joint setwise, 12 criteria)
  axes/<lens>.tsv          existing-axis latents (gemma-4-31b): axis_key entity_id latent_mean latent_std run_id
Outputs: a per-lens table per candidate.
"""
import json, collections, gzip, sys
import numpy as np

lens_of = {"P": "lesswrong-posts", "C": "lesswrong-comments", "M": "manifund-proposals"}
names = [c["name"] for c in json.load(open("cand.criteria.json"))]
TGT = "regret-if-missed"

cand = collections.defaultdict(dict); flip = {}
for l in gzip.open("ledger.jsonl.gz","rt"):
    r = json.loads(l); L = lens_of[r["list_id"][0]]
    cand[(L, r["criterion_name"])][r["item_id"]] = r["latent"]; flip[(L, r["criterion_name"])] = r["flip"]

def z(v):
    v = np.asarray(v, float); s = v.std()
    return (v - v.mean()) / s if s > 0 else v * 0

def r2_on(basis, y):
    beta = np.linalg.lstsq(basis.T, y, rcond=None)[0]
    return 1 - ((y - basis.T @ beta) ** 2).sum() / (y ** 2).sum()

out = {}
for L in lens_of.values():
    ex = collections.defaultdict(dict)
    for l in gzip.open(f"existing-axes/{L}.tsv.gz","rt"):
        a, e, m, s, r = l.rstrip("\n").split("\t"); ex[a][e] = float(m)
    ents = sorted(cand[(L, TGT)]); axes = sorted(a for a in ex if all(e in ex[a] for e in ents))
    X = np.array([z([ex[a][e] for e in ents]) for a in axes])
    X = X[X.std(1) > 0]; axes = [a for a in axes if True]  # keep names aligned below
    keep = [i for i, a in enumerate(sorted(a for a in ex if all(e in ex[a] for e in ents))) if np.asarray([ex[a][e] for e in ents]).std() > 0]
    axes = [sorted(a for a in ex if all(e in ex[a] for e in ents))[i] for i in keep]
    C = np.array([z([cand[(L, n)][e] for e in ents]) for n in names])
    n = len(ents)
    U, S, Vt = np.linalg.svd(X, full_matrices=False)
    var = S ** 2 / (S ** 2).sum()
    k80 = int(np.searchsorted(np.cumsum(var), .8) + 1)
    pcs4 = Vt[:4]; pcsk = Vt[:k80]
    rc = C @ C.T / n; rx = C @ X.T / n
    ti = names.index(TGT); t = C[ti]
    print(f"\n## {L}: {len(axes)} existing axes, n={n} items, SE(r)~.22; existing PC1 {var[0]:.0%}, {k80} PCs for 80%")
    print(f"{'candidate':24s} flip  r(tgt)  max|r|ex  nearest existing axis                    R2@4PC R2@{k80}PC  nearest cand")
    rows = []
    for i, nm in enumerate(names):
        j = int(np.argmax(np.abs(rx[i])))
        others = [k for k in range(len(names)) if k != i]; jc = max(others, key=lambda k: abs(rc[i, k]))
        row = dict(lens=L, name=nm, flip=flip[(L, nm)], r_target=rc[i, ti], max_r_existing=abs(rx[i, j]), nearest_axis=axes[j],
                   sign=np.sign(rx[i, j]), r2_4pc=r2_on(pcs4, C[i]), r2_kpc=r2_on(pcsk, C[i]), nearest_cand=names[jc], r_nearest_cand=rc[i, jc])
        rows.append(row)
        print(f"{nm:24s} {row['flip']:.2f}  {row['r_target']:+.2f}   {row['max_r_existing']:.2f}     {axes[j][:40]:40s} {row['r2_4pc']:.2f}   {row['r2_kpc']:.2f}     {names[jc]}({rc[i,jc]:+.2f})")
    # target structure
    beta = np.linalg.lstsq(pcsk.T, t, rcond=None)[0]; res = t - pcsk.T @ beta
    print(f"target R2 on {k80} existing PCs {1-(res**2).sum()/(t**2).sum():.2f}; residual r with candidates:",
          ", ".join(f"{names[i]} {np.corrcoef(res, C[i])[0,1]:+.2f}" for i in range(len(names)) if i != ti and abs(np.corrcoef(res, C[i])[0,1]) > .3) or "none > .3")
    top = np.argsort(-np.abs(X @ t / n))[:6]
    print("existing axes most correlated with target:", ", ".join(f"{axes[k]}({(X[k]@t/n):+.2f})" for k in top))
    top = np.argsort(-np.abs(rc[ti]))[1:5]
    print("candidates most correlated with target:", ", ".join(f"{names[k]}({rc[ti,k]:+.2f})" for k in top))
    # candidate block structure
    Uc, Sc, _ = np.linalg.svd(C, full_matrices=False); vc = Sc ** 2 / (Sc ** 2).sum()
    print(f"candidate block: PC1 {vc[0]:.0%}, PC2 {vc[1]:.0%}, {int(np.searchsorted(np.cumsum(vc), .8)+1)} PCs for 80%")
    out[L] = rows
json.dump(out, open("analysis.json", "w"), indent=1, default=float)
