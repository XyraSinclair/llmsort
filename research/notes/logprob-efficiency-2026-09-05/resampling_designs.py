#!/usr/bin/env python3
"""Design comparison with independent reference.

Per run (4x2 structure): shuffle draws per (pair,orientation); draws [2:4] -> reference
(both orientations, all pairs). Arms sample from draws [0:2] only.

Budget 2P (fresh-call units, repeat draw = 0.21):
  wideCB(P pairs, both orient, 1 draw)        cost 2.00/pair
  deep2(int(2P/2.42) pairs, both orient, 2)   cost 2.42/pair
Budget 1.21P:
  wideCB(int(1.21P/2) pairs)                  cost 2.00/pair
  mono2(P pairs, ONE random orient, 2 draws)  cost 1.21/pair
"""
import collections, math, random
import numpy as np

VAR_FLOOR = 1e-4

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
    rows = collections.defaultdict(list)
    for line in open('/tmp/ratio_letter_draws.tsv'):
        r, ci, a, b, sw, mu, var, it, ot, ca = line.rstrip('\n').split('\t')
        rows[r].append((a, b, int(sw), float(mu), max(float(var), VAR_FLOOR)))
    rng = random.Random(11)
    taus = collections.defaultdict(list)
    n_used = 0
    for rid, obs in rows.items():
        ents = sorted({e for o in obs for e in (o[0], o[1])})
        n = len(ents)
        if n < 30: continue
        idx = {e: i for i, e in enumerate(ents)}
        g = collections.defaultdict(list)
        for a, b, sw, mu, var in obs:
            g[(min(a, b), max(a, b), sw)].append((a, b, mu, var))
        pairs = collections.defaultdict(dict)
        for (a, b, sw), draws in g.items():
            if len(draws) >= 4: pairs[(a, b)][sw] = draws
        full = {p: d for p, d in pairs.items() if len(d) == 2}
        if len(full) < 100: continue
        n_used += 1
        plist = sorted(full); P = len(plist)
        for trial in range(8):
            fit = {}; refobs = []
            for p in plist:
                for sw, draws in full[p].items():
                    d = rng.sample(draws, 4)
                    fit[(p, sw)] = d[:2]; refobs.extend(d[2:4])
            ref = solve_M(refobs, idx, n)
            def tau_of(sub): return kendall(solve_M(sub, idx, n), ref)
            # budget 2P
            sub = [fit[(p, sw)][0] for p in plist for sw in full[p]]
            taus['2P wideCB (P pairs x2orient x1draw)'].append(tau_of(sub))
            k2 = rng.sample(plist, int(2 * P / 2.42))
            sub = [d for p in k2 for sw in full[p] for d in fit[(p, sw)]]
            taus['2P deep2 (0.83P pairs x2orient x2draws)'].append(tau_of(sub))
            # budget 1.21P
            wc = rng.sample(plist, int(1.21 * P / 2))
            sub = [fit[(p, sw)][0] for p in wc for sw in full[p]]
            taus['1.21P wideCB (0.6P pairs x2orient x1draw)'].append(tau_of(sub))
            sub = []
            for p in plist:
                sw = rng.choice(list(full[p]))
                sub.extend(fit[(p, sw)])
            taus['1.21P mono2 (P pairs x1orient x2draws)'].append(tau_of(sub))
    print(f'{n_used} runs x 8 trials; tau vs independent held-out-draw reference:')
    for k in ['2P wideCB (P pairs x2orient x1draw)', '2P deep2 (0.83P pairs x2orient x2draws)',
              '1.21P wideCB (0.6P pairs x2orient x1draw)', '1.21P mono2 (P pairs x1orient x2draws)']:
        v = np.array(taus[k]); print(f'  {k}: tau {v.mean():.4f} +- {v.std()/math.sqrt(len(v)):.4f}')

if __name__ == '__main__':
    main()
