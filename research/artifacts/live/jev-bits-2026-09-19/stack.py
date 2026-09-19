"""Does asking more fields in the same call add information? Per directed pair in one call, regress the
reference latent difference on subsets of instrument readings (5-fold CV R^2 -> Gaussian bits).
Also: both mention orders of one instrument (the free in-call counterbalance). Usage: python3 stack.py <cohort> <arm>"""
import json, math, sys, os, glob
from collections import defaultdict
import numpy as np
HERE = os.path.dirname(os.path.abspath(__file__))
name, arm = sys.argv[1], sys.argv[2]
meta = json.load(open(f"{HERE}/ref-{name}.json")); ids = meta["ids"]
obs = [json.loads(l) for f in sorted(glob.glob(f"{HERE}/obs-{name}*.jsonl")) for l in open(f)]
def cv_bits(X, d):
    X = np.column_stack([X, np.ones(len(d))]); idx = np.arange(len(d)); np.random.default_rng(3).shuffle(idx); pred = np.zeros(len(d))
    for f in range(5):
        te = idx[f::5]; tr = np.setdiff1d(idx, te); pred[te] = X[te] @ np.linalg.lstsq(X[tr], d[tr], rcond=None)[0]
    r = np.corrcoef(pred, d)[0, 1]; return 0.5 * math.log2(1 / (1 - r * r))
for c in meta["ref"]:
    ref = np.array([meta["ref"][c][i] for i in ids]); cell = defaultdict(dict)
    for o in obs:
        if o["arm"] == arm and o["crit"] == c and "j" in o:
            fwd = o["x_pos"] < o["y_pos"]; key = (o["call"], min(o["x_pos"], o["y_pos"]), max(o["x_pos"], o["y_pos"]))
            cell[key][o["instr"] + ("" if fwd else "_rev")] = o["y"] if fwd else -o["y"]
            cell[key]["d"] = (ref[o["i"]] - ref[o["j"]]) * (1 if fwd else -1)
    rows = list(cell.values()); d = np.array([r["d"] for r in rows]); names = sorted(k for k in rows[0] if k != "d")
    sets = [[n] for n in names] + [["noul", "noul_rev"], ["score9", "score9_rev"], ["noul", "noul_rev", "score9", "score9_rev"], names]
    print(f"{name} {arm} {c}: " + "  ".join(f"{'+'.join(s)}={cv_bits(np.array([[r[k] for k in s] for r in rows]), d):.2f}" for s in sets if all(k in rows[0] for k in s)))
