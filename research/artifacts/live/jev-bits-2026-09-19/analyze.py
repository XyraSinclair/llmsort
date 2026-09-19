"""Read obs-/calls-/ref- files from run.py; print the instrument x packing table and learning curves.

Per (cohort, criterion, arm, instrument):
  rho        Spearman of the least-squares latents (all observations) against the reference
  r_obs      Pearson of ONE directed observation y against the reference latent difference
  bits_obs   Gaussian-channel information of one observation about the difference, 0.5*log2(1/(1-r^2))
  auc        P(observation orders a random pair as the reference does)
  m, s       mention-first bias (mean y), state-position bias (half the first/second gap), in y units / sd(y)
  tok/obs    billed tokens per observation if the call carried only this instrument
Learning curves: random subsets of an arm's calls, refit, rho against tokens and against calls.
Usage: python3 analyze.py <cohort> [--curves]
"""
import json, math, os, random, sys
from collections import defaultdict
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import run as R


def rank(v):
    v = np.asarray(v, float); o = np.argsort(v, kind="mergesort"); r = np.empty(len(v)); r[o] = np.arange(len(v))
    for x in np.unique(v):
        m = v == x
        if m.sum() > 1:
            r[m] = r[m].mean()
    return r


def spearman(a, b):
    return float(np.corrcoef(rank(a), rank(b))[0, 1])


def auc(y, d):
    y, d = np.asarray(y), np.asarray(d); keep = d != 0; y, pos = y[keep], d[keep] > 0
    r = rank(y); n1, n0 = pos.sum(), (~pos).sum()
    return float((r[pos].sum() - n1 * (n1 - 1) / 2) / (n1 * n0))


def fit_pairs(obs, n):
    A = np.zeros((len(obs) + 1, n)); b = np.zeros(len(obs) + 1)
    for k, (i, j, y) in enumerate(obs):
        A[k, i], A[k, j], b[k] = 1, -1, y
    A[-1, :] = 1
    return np.linalg.lstsq(A, b, rcond=None)[0]


def fit_items(obs, n):  # obs: (call, i, y) -> mean of within-call-centred y
    by = defaultdict(list)
    for c, i, y in obs:
        by[c].append((i, y))
    acc, cnt = np.zeros(n), np.zeros(n)
    for rows in by.values():
        mu = np.mean([y for _, y in rows]) if len(rows) > 1 else 0.0
        for i, y in rows:
            acc[i] += y - mu; cnt[i] += 1
    return acc / np.maximum(cnt, 1)


def main():
    name = sys.argv[1]
    curves = "--curves" in sys.argv
    meta = json.load(open(f"{HERE}/ref-{name}.json")); ids = meta["ids"]; n = len(ids)
    ref = {c: np.array([meta["ref"][c][i] for i in ids]) for c in meta["ref"]}
    import glob
    obs = [json.loads(l) for f in sorted(glob.glob(f"{HERE}/obs-{name}*.jsonl")) for l in open(f)]
    calls = {json.loads(l)["call"]: json.loads(l) for f in sorted(glob.glob(f"{HERE}/calls-{name}*.jsonl")) for l in open(f)}
    arms_run = "".join(sorted({c["arm"] for c in calls.values()}))
    # per-call, per-instrument token cost: state + that instrument's questions, scaled to the billed total
    items, crit, _ = R.cohort(name)
    cost = {}
    for rq in R.build(items, crit, arms_run):
        c = calls[rq["tag"]]; st = len(rq["state"]) / 4; est_all = st + R.est_tokens(rq["questions"]); k = c["input_tokens"] / est_all
        per = defaultdict(float)
        for qid, q in rq["questions"].items():
            ins = qid.split("|")[1]; per["choice" if ins in ("top", "bottom") else ins] += R.est_tokens({qid: q})
        tq = sum(per.values())
        for ins, t in per.items():
            # one criterion's share of this instrument's questions plus the state; F chunks exist only because
            # every instrument rides together, so an F chunk's state is charged pro rata to the instrument.
            share = t / tq * len(crit) if rq["arm"] == "F" else 1.0
            cost[(rq["tag"], ins)] = (st * min(share, 1.0) + t / len(crit)) * k
    groups = defaultdict(list)
    for o in obs:
        ins = "choice" if o["instr"] in ("top", "bottom") else o["instr"]
        groups[(o["crit"], o["arm"], ins)].append(o)
    print(f"# {name}: n={n}, criteria {list(ref)}")
    print(f"{'crit':10s} {'arm':3s} {'instr':7s} {'rho':>6s} {'r_obs':>6s} {'bits':>5s} {'auc':>5s} {'m':>6s} {'s':>6s} {'obs':>6s} {'calls':>5s} {'tok/obs':>8s} {'Mtok':>6s}")
    summary = defaultdict(list)
    for (c, arm, ins), rows in sorted(groups.items()):
        tags = sorted({o["call"] for o in rows}); mtok = sum(cost[(t, ins)] for t in tags) / 1e6
        if ins in ("rate", "choice"):
            if ins == "choice":
                o3 = [(o["call"], o["i"], o["p"] if o["instr"] == "top" else -o["p"]) for o in rows]
                acc = np.zeros(n)
                for _, i, y in o3:
                    acc[i] += y
                s = acc
            else:
                s = fit_items([(o["call"], o["i"], o["y"]) for o in rows], n)
            rho = spearman(s, ref[c])
            print(f"{c:10s} {arm:3s} {ins:7s} {rho:6.3f} {'':6s} {'':5s} {'':5s} {'':6s} {'':6s} {len(rows):6d} {len(tags):5d} {mtok * 1e6 / len(rows):8.0f} {mtok:6.3f}")
        else:
            y = np.array([o["y"] for o in rows]); d = np.array([ref[c][o["i"]] - ref[c][o["j"]] for o in rows])
            s = fit_pairs([(o["i"], o["j"], o["y"]) for o in rows], n); rho = spearman(s, ref[c])
            r = float(np.corrcoef(y, d)[0, 1]); bits = 0.5 * math.log2(1 / (1 - r * r))
            first = np.array([o["x_pos"] < o["y_pos"] for o in rows]); sd = y.std()
            m = y.mean() / sd; sb = (y[first].mean() - y[~first].mean()) / 2 / sd if first.any() and (~first).any() else float('nan')
            print(f"{c:10s} {arm:3s} {ins:7s} {rho:6.3f} {r:6.3f} {bits:5.2f} {auc(y, d):5.3f} {m:+6.2f} {sb:+6.2f} {len(rows):6d} {len(tags):5d} {mtok * 1e6 / len(rows):8.0f} {mtok:6.3f}")
        summary[(arm, ins)].append(rho)
    print("\n# mean rho over criteria")
    for (arm, ins), v in sorted(summary.items()):
        print(f"  {arm} {ins:7s} {np.mean(v):.3f}")
    if curves:
        rng = random.Random(11); out = []
        print("\n# learning curves: mean rho over criteria and 12 resamples, by fraction of the arm's calls")
        print(f"{'arm':3s} {'instr':7s} {'calls':>6s} {'Ktok':>8s} {'rho':>6s}")
        for (arm, ins) in sorted({(a, i) for (_, a, i) in groups}):
            if arm == "S":
                continue
            tags_all = sorted({o["call"] for (c, a, i), rows in groups.items() if (a, i) == (arm, ins) for o in rows})
            done = set()
            for f in (0.01, 0.02, 0.05, 0.1, 0.2, 0.5, 1.0):
                kk = max(1, round(f * len(tags_all)))
                if kk in done:
                    continue
                done.add(kk)
                rhos, toks = [], []
                for _ in range(12 if kk < len(tags_all) else 1):
                    sub = set(rng.sample(tags_all, kk))
                    toks.append(sum(cost[(t, ins)] for t in sub))
                    for c in ref:
                        rows = [o for o in groups[(c, arm, ins)] if o["call"] in sub]
                        if ins == "rate":
                            s = fit_items([(o["call"], o["i"], o["y"]) for o in rows], n)
                        elif ins == "choice":
                            s = np.zeros(n)
                            for o in rows:
                                s[o["i"]] += o["p"] if o["instr"] == "top" else -o["p"]
                        else:
                            s = fit_pairs([(o["i"], o["j"], o["y"]) for o in rows], n)
                        rhos.append(spearman(s, ref[c]) if np.std(s) > 0 else 0.0)
                print(f"{arm:3s} {ins:7s} {kk:6d} {np.mean(toks) / 1e3:8.1f} {np.mean(rhos):6.3f}")
                out.append({"arm": arm, "instr": ins, "calls": kk, "ktok": float(np.mean(toks)) / 1e3, "rho": float(np.mean(rhos))})
        json.dump(out, open(f"{HERE}/curves-{name}.json", "w"), indent=0)


if __name__ == "__main__":
    main()
