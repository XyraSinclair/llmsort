"""Offline closure of the open cells the enumeration atlas named for the winner (rate-L10-k24): readout (expectation vs
top vs full-PMF likelihood), fit (least squares vs PMF-variance weights vs Huber IRLS), and slot position bias inside
the window. Replays the committed trace; no key, no spend. python3 refit.py -> refit.json"""
import json, math, random, sys
import numpy as np

sys.path.insert(0, __file__.rsplit("/", 1)[0])
import grid as G
from jevclient import pmf
from lab import spearman

REC, L, ROUNDS = "rate-L10-k24", 10, 6


def collect(name):
    items, ref, ridx, attr, noun = G.load(name); n = len(items); k = G.RECIPES[REC][0]; rng = random.Random(11); rows = []
    for r in range(ROUNDS):
        order = rng.sample(range(n), n); reqs = []
        for w, mem in enumerate(G.chunks(order, k)):
            reqs.append({"state": G.state_of(items, mem, noun, "block"), "questions": G.questions(REC, len(mem), attr, noun), "tag": f"{name}|{REC}|r{r}|w{w}", "mem": mem})
        for rq, rc in zip(reqs, G.call_many(reqs, f"{G.HERE}/trace-grid-{name}.jsonl")):
            for qid, a in rc["answers"].items():
                slot = int(qid.split("|")[1]) - 1; p = np.array(pmf(a, L)); lv = np.arange(1, L + 1)
                e = float(p @ lv); rows.append((r, rq["mem"][slot], rq["tag"], slot, e, float(lv[p.argmax()]), float(p @ (lv - e) ** 2), p))
    return rows, n, ref, ridx


def design(rows, n, ycol):
    fes = sorted({t for _, _, t, *_ in rows}); col = {f: n + t for t, f in enumerate(fes)}
    A = np.zeros((len(rows), n + len(fes))); y = np.array([row[ycol] for row in rows])
    for q, (_, i, t, *_) in enumerate(rows): A[q, i] = 1; A[q, col[t]] = 1
    return A, y


def solve(A, y, w=None, n=None):
    Ac = np.vstack([A, np.r_[np.ones(n), np.zeros(A.shape[1] - n)]]); yc = np.r_[y, 0]
    if w is not None: sw = np.sqrt(np.r_[w, 1]); Ac, yc = Ac * sw[:, None], yc * sw
    return np.linalg.lstsq(Ac, yc, rcond=None)[0]


def huber(A, y, n, c=1.0, it=10):
    w = np.ones(len(y)); b = solve(A, y, w, n)
    for _ in range(it):
        res = y - A @ b; s = np.median(np.abs(res)) / 0.6745 + 1e-9; z = np.abs(res) / s
        w = np.where(z <= c, 1.0, c / z); b = solve(A, y, w, n)
    return b


def ordered_probit_score(rows, n):
    """Graded-response readout without an item-level threshold fit: thresholds tau_l = Phi^-1(pooled cumulative level
    frequency); an answer's latent is its PMF's expectation of the bin midpoints in z-space (ends clipped at +-2.5)."""
    from statistics import NormalDist
    nd = NormalDist(); pooled = np.mean([row[7] for row in rows], axis=0); cum = np.clip(np.cumsum(pooled), 1e-4, 1 - 1e-4)
    tau = np.r_[-2.5, [nd.inv_cdf(c) for c in cum[:-1]], 2.5]; mid = (tau[:-1] + tau[1:]) / 2
    return [float(row[7] @ mid) for row in rows]


def design_slotfe(rows, n, ycol):
    """The baseline design plus 24 slot columns (position inside the window), so the fit absorbs a slot effect."""
    A, y = design(rows, n, ycol); S = np.zeros((len(rows), max(r[3] for r in rows) + 1))
    for q, row in enumerate(rows): S[q, row[3]] = 1
    return np.hstack([A, S]), y


def score(rows, n, ref, ridx, b):
    u = b[:n]; return spearman(u[ridx], ref)


def selfagree(rows, n, fitter):
    h = [fitter([row for row in rows if row[0] % 2 == s])[:n] for s in (0, 1)]; sh = spearman(h[0], h[1]); return 2 * sh / (1 + sh)


out = {}
for name in ("countries", "films", "companies", "names-aura"):
    rows, n, ref, ridx = collect(name); res = {}
    fitters = {
        "ls-expectation": lambda rs: solve(*design(rs, n, 4), None, n),
        "ls-top": lambda rs: solve(*design(rs, n, 5), None, n),
        "wls-1/var": lambda rs: solve(*design(rs, n, 4), 1 / (np.array([r[6] for r in rs]) + 0.05), n),
        "huber-irls": lambda rs: huber(*design(rs, n, 4), n),
        "ls-slotfe": lambda rs: solve(*design_slotfe(rs, n, 4), None, n),
    }
    zrows = [row[:4] + (z,) for row, z in zip(rows, ordered_probit_score(rows, n))]
    fitters["probit-readout"] = lambda rs, zr=zrows: solve(*design([z for z in zr if (z[0], z[1], z[2]) in {(r[0], r[1], r[2]) for r in rs}], n, 4), None, n)
    for fname, f in fitters.items():
        b = f(rows); res[fname] = {"rho": round(score(rows, n, ref, ridx, b), 4), "self": round(selfagree(rows, n, f), 4)}
    # slot bias: residual of the baseline fit against slot index
    A, y = design(rows, n, 4); b = solve(A, y, None, n); resid = y - A.dot(b); slot = np.array([r[3] for r in rows]); assert np.isfinite(resid).all()
    sl = float(np.polyfit(slot, resid, 1)[0]); by = [float(resid[slot == s].mean()) for s in range(slot.max() + 1)]; se = float(resid.std() / math.sqrt((slot == 0).sum()))
    res["slot"] = {"window_sizes": sorted({int(x) for x in np.bincount([hash(r[2]) % 10**9 for r in rows]) if x} or {0}), "levels_per_slot": round(sl, 4), "first": round(by[0], 3), "last": round(by[-1], 3), "range": round(max(by) - min(by), 3), "se_slot_mean": round(se, 3), "sd_resid": round(float(resid.std()), 3), "rows_per_slot": int((slot == 0).sum())}
    for R in (1, 2):  # where a slot effect would bite: few rounds, so each item sat in few slots
        rs = [r for r in rows if r[0] < R]
        res["slot"][f"r{R}_rho_ls/slotfe"] = [round(score(rs, n, ref, ridx, fitters[f](rs)), 4) for f in ("ls-expectation", "ls-slotfe")]
    out[name] = res
    print(name, json.dumps(res), flush=True)
json.dump(out, open(f"{G.HERE}/refit.json", "w"), indent=1)
