#!/usr/bin/env python3
"""Resampling (nonce-draw) efficiency on production ratio_letter_v1 rows.

Q1: variance honesty — does the PMF's stated v cover the draw-to-draw scatter?
    rho = empirical between-draw var / mean reported v, per (run,pair,orientation).
Q2: position bias — orientation mean gap vs pooled SE (does counterbalancing carry real weight?).
Q3: diminishing returns — tau(fit vs full-run reference) as draws/orientation k = 1..4, pairs fixed.
Q4: deep vs wide at EQUAL COMPUTE — repeat draw costs MARGINAL_FRAC of a fresh call
    (prefix cache; output ~2 tokens). Compare k-draw designs at matched compute.
"""
import collections, math, random
import numpy as np

VAR_FLOOR = 1e-4
MARGINAL_FRAC = 0.21   # repeat nonce draw cost as fraction of fresh call (79% prefix hit)

def load():
    rows = collections.defaultdict(list)  # run -> list
    for line in open('/tmp/ratio_letter_draws.tsv'):
        r, ci, a, b, sw, mu, var, it, ot, ca = line.rstrip('\n').split('\t')
        rows[r].append((a, b, int(sw), float(mu), max(float(var), VAR_FLOOR)))
    return rows

def solve_M(obs, idx, n):
    A = np.zeros((len(obs), n)); y = np.zeros(len(obs)); w = np.zeros(len(obs))
    for k, (a, b, mu, var) in enumerate(obs):
        A[k, idx[a]] = 1; A[k, idx[b]] = -1; y[k] = mu; w[k] = 1.0 / var
    sw = np.sqrt(w); Aw = A * sw[:, None]; yw = y * sw
    lam = 1e-3 * math.sqrt(np.mean(w))
    Aw = np.vstack([Aw, lam * np.eye(n)]); yw = np.concatenate([yw, np.zeros(n)])
    s, *_ = np.linalg.lstsq(Aw, yw, rcond=None)
    return s

def kendall(s1, s2):
    n = len(s1); num = den = 0
    for i in range(n):
        for j in range(i + 1, n):
            d1 = s1[i] - s1[j]; d2 = s2[i] - s2[j]
            if d1 == 0 or d2 == 0: continue
            den += 1; num += 1 if (d1 > 0) == (d2 > 0) else -1
    return num / den if den else float('nan')

def main():
    runs = load()
    rng = random.Random(7)

    # ---- Q1 + Q2: variance decomposition
    rhos = []; sig_b2s = []; vbars = []; bias_z = []; bias_abs = []
    for rid, obs in runs.items():
        g = collections.defaultdict(list)
        for a, b, sw, mu, var in obs:
            # canonical orientation: mu toward min(a,b)
            key = (min(a, b), max(a, b), sw)
            mu_c = mu if a < b else -mu
            g[key].append((mu_c, var))
        om = collections.defaultdict(dict)  # (pair) -> sw -> (mean, se, k)
        for (a, b, sw), draws in g.items():
            mus = [d[0] for d in draws]; vs = [d[1] for d in draws]
            if len(draws) >= 3:
                sb2 = float(np.var(mus, ddof=1)); vbar = float(np.mean(vs))
                rhos.append(sb2 / vbar); sig_b2s.append(sb2); vbars.append(vbar)
            if len(draws) >= 2:
                om[(a, b)][sw] = (float(np.mean(mus)),
                                  float(np.std(mus, ddof=1) / math.sqrt(len(mus))), len(mus))
        for pair, d in om.items():
            if len(d) == 2:
                (m0, se0, _), (m1, se1, _) = d[0], d[1]
                gap = m0 - m1; se = math.sqrt(se0**2 + se1**2) or 1e-9
                bias_z.append(abs(gap) / se); bias_abs.append(abs(gap))
    rhos = np.array(rhos)
    print(f'Q1 variance honesty: rho = between-draw var / reported PMF var, {len(rhos)} groups (k>=3)')
    print(f'   median rho {np.median(rhos):.2f}   mean {rhos.mean():.2f}   p25 {np.percentile(rhos,25):.2f}  p75 {np.percentile(rhos,75):.2f}')
    print(f'   median between-draw sd {math.sqrt(np.median(sig_b2s)):.4f} nats; median reported sd {math.sqrt(np.median(vbars)):.4f}')
    bz = np.array(bias_z)
    print(f'Q2 position bias: |orientation gap| z-score median {np.median(bz):.2f}, frac z>2: {(bz>2).mean():.2f}; median |gap| {np.median(bias_abs):.4f} nats')

    # ---- Q3 + Q4: design curves on runs with the canonical 4x2 structure
    tau_by_k = collections.defaultdict(list)       # k draws/orientation, all pairs
    tau_designs = collections.defaultdict(list)    # equal-compute designs
    n_used = 0
    for rid, obs in runs.items():
        ents = sorted({e for o in obs for e in (o[0], o[1])})
        n = len(ents)
        if n < 30: continue
        idx = {e: i for i, e in enumerate(ents)}
        g = collections.defaultdict(list)
        for a, b, sw, mu, var in obs:
            g[(a, b, sw)].append((a, b, mu, var))
        # keep pairs with exactly 2 orientations x >=4 draws
        pairs = collections.defaultdict(dict)
        for (a, b, sw), draws in g.items():
            if len(draws) >= 4: pairs[(min(a,b), max(a,b))][sw] = draws
        full_pairs = {p: d for p, d in pairs.items() if len(d) == 2}
        if len(full_pairs) < 100: continue
        n_used += 1
        ref = solve_M([(a, b, mu, var) for a, b, sw, mu, var in obs], idx, n)
        plist = sorted(full_pairs)
        P = len(plist)
        for trial in range(6):
            # Q3: all pairs, k draws per orientation
            for k in (1, 2, 3, 4):
                sub = []
                for p in plist:
                    for sw, draws in full_pairs[p].items():
                        sub.extend(rng.sample(draws, k))
                tau_by_k[k].append(kendall(solve_M(sub, idx, n), ref))
            # Q4: equal compute C = P * 2 * (1 + 3*MF)  (the deep design on ALL pairs)
            C = P * 2 * (1 + 3 * MARGINAL_FRAC)
            per_pair = {1: 2 * 1.0, 2: 2 * (1 + MARGINAL_FRAC), 4: 2 * (1 + 3 * MARGINAL_FRAC)}
            for k, cpp in per_pair.items():
                npairs = min(P, int(C / cpp))
                chosen = rng.sample(plist, npairs)
                sub = []
                for p in chosen:
                    for sw, draws in full_pairs[p].items():
                        sub.extend(rng.sample(draws, k))
                tau_designs[k].append(kendall(solve_M(sub, idx, n), ref))
    print(f'\nQ3 diminishing returns ({n_used} runs x 6 trials, all pairs, k draws/orientation, tau vs full-run reference):')
    for k in (1, 2, 3, 4):
        v = np.array(tau_by_k[k]); print(f'   k={k}: tau {v.mean():.4f}  (compute/pair {2*(1+(k-1)*MARGINAL_FRAC):.2f} fresh-call units)')
    print(f'\nQ4 equal-compute designs (repeat draw = {MARGINAL_FRAC} of fresh call):')
    for k in sorted(tau_designs):
        v = np.array(tau_designs[k]); print(f'   {k} draw(s)/orientation, pairs to budget: tau {v.mean():.4f}')

if __name__ == '__main__':
    main()
