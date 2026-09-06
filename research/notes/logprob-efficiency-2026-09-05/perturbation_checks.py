#!/usr/bin/env python3
"""Discriminating checks: is the PMF variance honest about perturbability?

A) corr(between-draw scatter, stated PMF var) per group — does the model know
   when nonces will move it?
B) corr(stated PMF sd, orientation gap) per pair — does PMF variance price
   presentation sensitivity?
C) exact-duplicate mu fraction within groups — is the nonce doing anything?
D) do gaps/scatter concentrate on near-tie pairs?
E) magnitude compression: |pooled mu| vs stated sd scale (are all judgements
   reported near-tie with wide PMFs?).
"""
import collections, math
import numpy as np

groups = collections.defaultdict(list)
for line in open('/tmp/ratio_letter_draws.tsv'):
    r, ci, a, b, sw, mu, var, it, ot, ca = line.rstrip('\n').split('\t')
    a_, b_ = (a, b) if a < b else (b, a)
    mu_c = float(mu) if a < b else -float(mu)
    groups[(r, a_, b_, sw)].append((mu_c, float(var)))

sb2, vbar, dup = [], [], 0
ngrp = 0
per_pair = collections.defaultdict(dict)
for (r, a, b, sw), draws in groups.items():
    mus = [d[0] for d in draws]; vs = [d[1] for d in draws]
    if len(draws) >= 3:
        ngrp += 1
        sb2.append(np.var(mus, ddof=1)); vbar.append(np.mean(vs))
        if len(set(round(m, 12) for m in mus)) == 1: dup += 1
    if len(draws) >= 2:
        per_pair[(r, a, b)][sw] = (np.mean(mus), np.mean(vs), len(mus))

sb2 = np.array(sb2); vbar = np.array(vbar)
mask = sb2 > 0
print(f'A) groups k>=3: {ngrp}; corr(log sb2, log vbar) = '
      f'{np.corrcoef(np.log(sb2[mask]), np.log(vbar[mask]))[0,1]:.3f}  (n={mask.sum()})')
print(f'   Spearman-ish check by rank: '
      f'{np.corrcoef(np.argsort(np.argsort(sb2[mask])), np.argsort(np.argsort(vbar[mask])))[0,1]:.3f}')
print(f'C) exact-duplicate groups (all draws identical mu): {dup}/{ngrp} = {dup/ngrp:.3f}')

gaps, sds, seps = [], [], []
for pair, d in per_pair.items():
    if len(d) == 2:
        (m0, v0, k0), (m1, v1, k1) = list(d.values())
        gaps.append(abs(m0 - m1))
        sds.append(math.sqrt((v0 + v1) / 2))
        seps.append(abs((m0 + m1) / 2))
gaps = np.array(gaps); sds = np.array(sds); seps = np.array(seps)
print(f'B) pairs both orientations: {len(gaps)}; corr(stated sd, |orientation gap|) = '
      f'{np.corrcoef(sds, gaps)[0,1]:.3f}; rank corr = '
      f'{np.corrcoef(np.argsort(np.argsort(sds)), np.argsort(np.argsort(gaps)))[0,1]:.3f}')
print(f'   median stated sd {np.median(sds):.4f}, median gap {np.median(gaps):.4f}, '
      f'ratio gap/sd median {np.median(gaps/np.maximum(sds,1e-9)):.2f}')
print(f'D) corr(|pooled mu| (separation), gap) = {np.corrcoef(seps, gaps)[0,1]:.3f}; '
      f'rank {np.corrcoef(np.argsort(np.argsort(seps)), np.argsort(np.argsort(gaps)))[0,1]:.3f}')
print(f'E) median |pooled mu| {np.median(seps):.4f} nats vs median stated sd {np.median(sds):.4f} '
      f'-> stated z of a typical pair: {np.median(seps/np.maximum(sds,1e-9)):.2f}')
