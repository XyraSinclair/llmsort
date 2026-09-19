"""Two offline reads of the stored PMFs. (1) Does inverse-variance weighting from the score9 PMF beat the
unweighted fit at small budgets (W arm, 5/10/20 calls, 24 resamples)? (2) Cardinal calibration on countries:
slope of E[log ratio] against the true log ratio. Usage: python3 weight.py"""
import json, math, os, random, glob
import numpy as np
from analyze import spearman
HERE = os.path.dirname(os.path.abspath(__file__))
R9 = np.log([1 / 8, 1 / 4, 1 / 2, 1 / 1.3, 1, 1.3, 2, 4, 8])
def load(name):
    meta = json.load(open(f"{HERE}/ref-{name}.json"))
    return meta, [json.loads(l) for f in sorted(glob.glob(f"{HERE}/obs-{name}*.jsonl")) for l in open(f)]
def fit(rows, n, weighted):
    A = np.zeros((len(rows) + 1, n)); b = np.zeros(len(rows) + 1)
    for k, o in enumerate(rows):
        p = np.array(o["dist"]); var = float(p @ (R9 - o["y"]) ** 2); w = 1 / math.sqrt(var + 0.05) if weighted else 1.0
        A[k, o["i"]], A[k, o["j"]], b[k] = w, -w, w * o["y"]
    A[-1, :] = 1
    return np.linalg.lstsq(A, b, rcond=None)[0]
for name, arm in (("arxiv", "W"), ("hn_comments", "U")):
    meta, obs = load(name); ids = meta["ids"]; n = len(ids); rng = random.Random(5)
    rows_all = [o for o in obs if o["arm"] == arm and o["instr"] == "score9"]; tags = sorted({o["call"] for o in rows_all})
    for kk in (5, 10, 20):
        res = {False: [], True: []}
        for _ in range(24):
            sub = set(rng.sample(tags, kk))
            for c in meta["ref"]:
                ref = np.array([meta["ref"][c][i] for i in ids]); rows = [o for o in rows_all if o["call"] in sub and o["crit"] == c]
                for w in (False, True):
                    res[w].append(spearman(fit(rows, n, w), ref))
        print(f"{name} {arm} score9 {kk:3d} calls: unweighted {np.mean(res[False]):.3f}  inverse-variance {np.mean(res[True]):.3f}")
meta, obs = load("countries"); ids = meta["ids"]; ref = np.array([meta["ref"]["population"][i] for i in ids])
for arm in ("P", "W", "F"):
    for ins in ("score9", "score5", "thermo"):
        rows = [o for o in obs if o["arm"] == arm and o["instr"] == ins]
        if rows:
            y = np.array([o["y"] for o in rows]); d = np.array([ref[o["i"]] - ref[o["j"]] for o in rows])
            print(f"countries {arm} {ins}: slope {np.polyfit(d, y, 1)[0]:.2f} nat per true nat, r {np.corrcoef(y, d)[0, 1]:.3f}, max|y| {abs(y).max():.2f} (ladder end {math.log(10 if ins == 'score5' else 8):.2f})")
