#!/usr/bin/env python3
"""How much sorting efficiency do logprob PMF moments buy vs point/binary verdicts?

Within-call ablation on production ratio_letter_v1 rows (production judgement ledger):
same calls, three readouts:
  M = (mu, var) precision-weighted   [production: logprob PMF moments]
  P = mu, uniform weights            [point log-ratio, no variance channel]
  S = sign(mu) only, fixed magnitude [pure binary comparator]

Per run: WLS solve s from observations mu ~ s_a - s_b.
Metrics:
  1. holdout pair-direction accuracy vs budget (consensus sign from unweighted
     pooled mu per pair over ALL draws = truth proxy, conservative toward S/P)
  2. split-half Kendall tau (rank reversals between independent halves)
  3. budget multiplier: comparisons S needs to match M at reference budgets
"""
import collections, math, random, sys
import numpy as np

VAR_FLOOR = 1e-4

def load(path):
    runs = collections.defaultdict(list)
    for line in open(path):
        r, a, b, mu, var, hr, conf = line.rstrip('\n').split('\t')
        mu = float(mu); var = max(float(var), VAR_FLOOR)
        runs[r].append((a, b, mu, var))
    return runs

def solve(obs, ents, mode):
    """WLS: minimize sum w_i (y_i - (s_a - s_b))^2 + ridge. Returns scores."""
    idx = {e: i for i, e in enumerate(ents)}
    n = len(ents)
    A = np.zeros((len(obs), n)); y = np.zeros(len(obs)); w = np.ones(len(obs))
    for k, (a, b, mu, var) in enumerate(obs):
        A[k, idx[a]] = 1.0; A[k, idx[b]] = -1.0
        if mode == 'M': y[k] = mu; w[k] = 1.0 / var
        elif mode == 'P': y[k] = mu; w[k] = 1.0
        else: y[k] = math.copysign(1.0, mu) if mu != 0 else 0.0; w[k] = 1.0
    sw = np.sqrt(w)
    Aw = A * sw[:, None]; yw = y * sw
    # gauge + ridge: append small ridge rows
    lam = 1e-3 * math.sqrt(np.mean(w))
    Aw = np.vstack([Aw, lam * np.eye(n)]); yw = np.concatenate([yw, np.zeros(n)])
    s, *_ = np.linalg.lstsq(Aw, yw, rcond=None)
    return {e: s[idx[e]] for e in ents}

def kendall_tau(r1, r2, ents):
    n = len(ents); num = 0; den = 0
    v1 = [r1[e] for e in ents]; v2 = [r2[e] for e in ents]
    for i in range(n):
        for j in range(i + 1, n):
            d1 = v1[i] - v1[j]; d2 = v2[i] - v2[j]
            if d1 == 0 or d2 == 0: continue
            den += 1; num += 1 if (d1 > 0) == (d2 > 0) else -1
    return num / den if den else float('nan')

def pair_consensus(obs):
    """Unweighted mean mu per unordered pair (canonical orientation)."""
    acc = collections.defaultdict(list)
    for a, b, mu, var in obs:
        if a < b: acc[(a, b)].append(mu)
        else: acc[(b, a)].append(-mu)
    out = {}
    for pr, mus in acc.items():
        m = float(np.mean(mus))
        se = float(np.std(mus, ddof=1) / math.sqrt(len(mus))) if len(mus) > 1 else float('inf')
        out[pr] = (m, se, len(mus))
    return out

def direction_accuracy(scores, consensus, z_min=2.0):
    ok = tot = 0
    for (a, b), (m, se, k) in consensus.items():
        if se == 0: se = 1e-9
        if abs(m) / se < z_min or k < 4: continue
        tot += 1
        if (scores[a] - scores[b] > 0) == (m > 0): ok += 1
    return (ok / tot if tot else float('nan')), tot

def main():
    runs = load(sys.argv[1] if len(sys.argv) > 1 else '/tmp/ratio_letter_moments.tsv')
    rng = random.Random(18)
    budgets_frac = [0.02, 0.04, 0.08, 0.15, 0.25, 0.5, 1.0]
    n_shuffles = 12

    # aggregate accumulators: mode -> frac -> list of accuracies
    acc_curve = collections.defaultdict(lambda: collections.defaultdict(list))
    tau_half = collections.defaultdict(list)
    run_summaries = []

    for rid, obs in sorted(runs.items(), key=lambda kv: -len(kv[1])):
        ents = sorted({e for o in obs for e in (o[0], o[1])})
        n = len(ents)
        if n < 6 or len(obs) < 200: continue
        consensus = pair_consensus(obs)
        n_dec = sum(1 for (m, se, k) in consensus.values() if se > 0 and abs(m)/max(se,1e-9) >= 2.0 and k >= 4)
        if n_dec < 10: continue
        for sh in range(n_shuffles):
            o = obs[:]; rng.shuffle(o)
            for f in budgets_frac:
                B = max(10, int(f * len(o)))
                sub = o[:B]
                for mode in 'MPS':
                    sc = solve(sub, ents, mode)
                    a, tot = direction_accuracy(sc, consensus)
                    if not math.isnan(a): acc_curve[mode][f].append(a)
            # split half tau
            half = len(o) // 2
            for mode in 'MPS':
                s1 = solve(o[:half], ents, mode); s2 = solve(o[half:], ents, mode)
                tau_half[mode].append(kendall_tau(s1, s2, ents))
        run_summaries.append((rid[:14], n, len(obs), n_dec))

    print('runs used:', len(run_summaries))
    for r in run_summaries: print('  ', r)
    print('\nholdout pair-direction accuracy vs budget fraction (mean over runs x shuffles):')
    print('frac      M        P        S     (M=moments, P=point-unweighted, S=sign-only)')
    for f in budgets_frac:
        print(f'{f:5.2f}  ' + '  '.join(f'{np.mean(acc_curve[m][f]):.4f}' for m in 'MPS'))
    print('\nsplit-half Kendall tau (rank agreement between independent halves):')
    for m in 'MPS':
        v = np.array(tau_half[m])
        print(f'  {m}: mean {v.mean():.4f}  median {np.median(v):.4f}  (reversal rate ~ {(1-v.mean())/2:.4f})')

if __name__ == '__main__':
    main()
