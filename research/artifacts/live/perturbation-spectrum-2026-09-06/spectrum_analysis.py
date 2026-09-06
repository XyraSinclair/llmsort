#!/usr/bin/env python3
"""Analyze a perturbation_spectrum pack (P1/P2 + anchor truth arm P4).

Usage: spectrum_analysis.py <rows.jsonl> [truth.json]
Rows: rung/variant/entity_a/entity_b/orientation/draw with PMF moments
(presented A-over-B). Canonical coordinates: mu toward lexicographically
smaller id.
"""
import collections, json, math, sys
import numpy as np

def canon(row):
    a, b, mu = row['entity_a'], row['entity_b'], row['log_ratio_mean']
    if a < b: return (a, b), mu
    return (b, a), -mu

def main():
    path = sys.argv[1]
    truth = json.load(open(sys.argv[2])) if len(sys.argv) > 2 else None
    rows = [json.loads(l) for l in open(path)]
    rows = [r for r in rows if r.get('error') is None and r.get('log_ratio_mean') is not None]
    print(f'{len(rows)} usable rows')
    # index: rung -> (pair, orientation) -> list of (variant, draw, mu, var)
    idx = collections.defaultdict(lambda: collections.defaultdict(list))
    for r in rows:
        pair, mu = canon(r)
        idx[r['rung']][(pair, r['orientation'])].append((r['variant'], r['draw'], mu, r['log_ratio_var']))

    print('\n== P1: scatter per rung (sd of mu across draws/variants, per pair-orientation) ==')
    stated_all = []
    scatter = {}
    for rung in ('nonce', 'jitter', 'para'):
        sds, stated = [], []
        for key, draws in idx[rung].items():
            mus = [d[2] for d in draws]; vs = [d[3] for d in draws]
            if len(mus) >= 3:
                sds.append(float(np.std(mus, ddof=1)))
                stated.append(float(np.mean([math.sqrt(v) for v in vs])))
        scatter[rung] = (np.array(sds), np.array(stated))
        if len(sds):
            print(f'  {rung:7s}: median scatter sd {np.median(sds):.4f} nats '
                  f'(n={len(sds)}); median stated PMF sd {np.median(stated):.4f}; '
                  f'ratio {np.median(np.array(sds)/np.maximum(stated,1e-9)):.3f}')
        stated_all.extend(stated)

    # orientation gap on the base (nonce) rung
    om = collections.defaultdict(dict)
    for (pair, o), draws in idx['nonce'].items():
        mus = [d[2] for d in draws]
        om[pair][o] = float(np.mean(mus))
    gaps = [abs(d[0] - d[1]) for d in om.values() if len(d) == 2]
    print(f'  orient : median |gap| {np.median(gaps):.4f} nats (n={len(gaps)}) [pooled-mean gap, base attribute]')

    print('\n== P2: does stated PMF sd predict framing (para) scatter per cell? ==')
    if len(scatter['para'][0]) > 5:
        s, st = scatter['para']
        lc = np.corrcoef(st, s)[0, 1]
        rk = np.corrcoef(np.argsort(np.argsort(st)), np.argsort(np.argsort(s)))[0, 1]
        slope = float(np.polyfit(st, s, 1)[0])
        print(f'  corr {lc:.3f}, rank corr {rk:.3f}, OLS slope {slope:.3f} '
              f'(slope ~1 & tight = PMF forecasts its own framing sensitivity)')

    if truth:
        print('\n== P4: anchor truth arm ==')
        def true_lr(pair):
            return math.log(truth[pair[0]] / truth[pair[1]])
        # per rung: pooled mu per pair (both orientations, all draws/variants)
        for rung in ('nonce', 'jitter', 'para'):
            pool = collections.defaultdict(list)
            for (pair, o), draws in idx[rung].items():
                pool[pair].extend(d[2] for d in draws)
            pairs = sorted(pool)
            mu = np.array([np.mean(pool[p]) for p in pairs])
            tl = np.array([true_lr(p) for p in pairs])
            sign_acc = float(np.mean(np.sign(mu) == np.sign(tl)))
            rk = np.corrcoef(np.argsort(np.argsort(mu)), np.argsort(np.argsort(tl)))[0, 1]
            print(f'  {rung:7s}: pairwise sign accuracy vs truth {sign_acc:.3f}, '
                  f'rank corr {rk:.3f}, slope {np.polyfit(tl, mu, 1)[0]:.3f} (n={len(pairs)})')
        # pooling curves at matched call count: k in 1..6
        print('  pooling curve (calls per pair-orientation k -> sign acc):')
        rng = np.random.default_rng(7)
        for rung in ('nonce', 'jitter', 'para'):
            accs_by_k = {}
            for k in (1, 2, 4, 6):
                accs = []
                for trial in range(20):
                    pool = collections.defaultdict(list)
                    for (pair, o), draws in idx[rung].items():
                        if len(draws) >= k:
                            take = rng.choice(len(draws), size=k, replace=False)
                            pool[pair].extend(draws[i][2] for i in take)
                    ok = tot = 0
                    for p, mus in pool.items():
                        tot += 1
                        if np.sign(np.mean(mus)) == np.sign(true_lr(p)): ok += 1
                    accs.append(ok / tot)
                accs_by_k[k] = np.mean(accs)
            print(f'    {rung:7s}: ' + '  '.join(f'k={k}:{v:.3f}' for k, v in accs_by_k.items()))

if __name__ == '__main__':
    main()
