#!/usr/bin/env python3
"""Budget curves: split-half tau and raw-draw holdout accuracy vs budget, per arm.
Efficiency multiplier = budget ratio for S (or P) to match M."""
import collections, math, random, sys
import numpy as np

VAR_FLOOR = 1e-4

def load(path):
    runs = collections.defaultdict(list)
    for line in open(path):
        r, a, b, mu, var, hr, conf = line.rstrip('\n').split('\t')
        runs[r].append((a, b, float(mu), max(float(var), VAR_FLOOR)))
    return runs

def solve(obs, idx, n, mode):
    A = np.zeros((len(obs), n)); y = np.zeros(len(obs)); w = np.ones(len(obs))
    for k, (a, b, mu, var) in enumerate(obs):
        A[k, idx[a]] = 1.0; A[k, idx[b]] = -1.0
        if mode == 'M': y[k] = mu; w[k] = 1.0 / var
        elif mode == 'P': y[k] = mu
        else: y[k] = math.copysign(1.0, mu) if mu else 0.0
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
    runs = load('/tmp/ratio_letter_moments.tsv')
    rng = random.Random(42)
    fracs = [0.05, 0.1, 0.2, 0.3, 0.5, 0.7, 1.0]  # of the HALF budget for tau; of fit pool for holdout
    n_shuffles = 10
    tau_curve = collections.defaultdict(lambda: collections.defaultdict(list))
    acc_curve = collections.defaultdict(lambda: collections.defaultdict(list))

    used = 0
    for rid, obs in sorted(runs.items(), key=lambda kv: -len(kv[1])):
        ents = sorted({e for o in obs for e in (o[0], o[1])})
        n = len(ents)
        if n < 12 or len(obs) < 1000: continue
        used += 1
        idx = {e: i for i, e in enumerate(ents)}
        for sh in range(n_shuffles):
            o = obs[:]; rng.shuffle(o)
            half = len(o) // 2
            h1, h2 = o[:half], o[half:2*half]
            # holdout: fit pool = h1, eval raw signs on h2
            ho = [(idx[a], idx[b], math.copysign(1, mu)) for a, b, mu, var in h2 if mu != 0]
            for f in fracs:
                B = max(15, int(f * half))
                for mode in 'MPS':
                    s1 = solve(h1[:B], idx, n, mode)
                    s2 = solve(h2[:B], idx, n, mode)
                    tau_curve[mode][f].append(kendall(s1, s2))
                    ok = sum(1 for a, b, sg in ho if (s1[a] - s1[b] > 0) == (sg > 0))
                    acc_curve[mode][f].append(ok / len(ho))
    print(f'runs used: {used}, shuffles {n_shuffles}')
    print('\nsplit-half Kendall tau vs budget (B = frac x half-run, per arm):')
    print('frac      M        P        S')
    for f in fracs:
        print(f'{f:5.2f}  ' + '  '.join(f'{np.mean(tau_curve[m][f]):.4f}' for m in 'MPS'))
    print('\nheld-out raw draw sign accuracy vs budget:')
    print('frac      M        P        S')
    for f in fracs:
        print(f'{f:5.2f}  ' + '  '.join(f'{np.mean(acc_curve[m][f]):.4f}' for m in 'MPS'))

    # budget multipliers by interpolation on tau curves
    def budget_to_match(curve_target, curve_other):
        # for each target frac, find frac where other reaches same tau (linear interp, may exceed 1.0 -> report >x)
        out = {}
        fo = np.array(fracs); to_ = np.array([np.mean(curve_other[f]) for f in fracs])
        for f in fracs:
            t = np.mean(curve_target[f])
            if t > to_.max():
                out[f] = float('inf')
            else:
                out[f] = float(np.interp(t, to_, fo))
        return out
    for other, name in [('P', 'point'), ('S', 'sign')]:
        m = budget_to_match(tau_curve['M'], tau_curve[other])
        print(f'\nbudget {name} needs to match M (tau), as multiple of M budget:')
        for f in fracs:
            v = m[f]
            print(f'  M at {f:.2f}: {name} needs {"beyond full budget (>" + format(1.0/f, ".1f") + "x)" if math.isinf(v) else format(v/f, ".2f") + "x"}')

if __name__ == '__main__':
    main()
