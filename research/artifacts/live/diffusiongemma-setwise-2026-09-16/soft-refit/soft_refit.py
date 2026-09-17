"""Offline refit of stored DiffusionGemma setwise traces: greedy permutation vs soft PL chain.

Reads trace-*.jsonl (clamp: `matrix` = k stage PMFs over remaining letters; one-forward: k slot marginals),
rebuilds the pairwise observations for the Huber-IRLS log-linear fit under several readouts, and reports the
same bars as the live bench (agreement rho separate~joint, halo inflation, flip rate, rho vs gemma-4-31b).
"""
import argparse, json, math, os, sys
import numpy as np

LN_RATIO = math.log(2.78)
COHORTS = ["hn_top", "hn_comments", "arxiv", "lw"]


def fit(n, obs):
    ridge, delta = 1e-3, 1.0
    hi = np.array([o[0] for o in obs], dtype=int); lo = np.array([o[1] for o in obs], dtype=int); r = np.array([o[2] for o in obs])
    w = np.ones(len(obs)); s = np.zeros(n)
    for _ in range(6):
        A = np.eye(n) * ridge; b = np.zeros(n)
        np.add.at(A, (hi, hi), w); np.add.at(A, (lo, lo), w); np.add.at(A, (hi, lo), -w); np.add.at(A, (lo, hi), -w)
        np.add.at(b, hi, w * r); np.add.at(b, lo, -w * r)
        s = np.linalg.solve(A, b)
        res = np.abs(s[hi] - s[lo] - r)
        w = np.where(res <= delta, 1.0, delta / np.maximum(res, 1e-300))
    return s


def ranks(v):
    v = np.asarray(v, dtype=float); order = np.argsort(-v); r = np.empty(len(v)); r[order] = np.arange(len(v)); return r


def spearman(a, b):
    ra, rb = ranks(a), ranks(b)
    return float(np.corrcoef(ra, rb)[0, 1])


def pair_prefs(t, ci, mode, clip):
    """Return {(item_a, item_b): r} with r = estimated log-odds that a ranks above b within this window."""
    order = t["order"]; k = len(order)
    prefs = {}
    if mode == "greedy":
        slots = t["parsed"][ci]
        if slots is None: return prefs
        ranked = [order[s] for s in slots]
        for a in range(k):
            for b in range(a + 1, k): prefs[(ranked[a], ranked[b])] = LN_RATIO
        return prefs
    M = np.array(t["matrix"][ci], dtype=float)  # k stages/slots x k letters
    if mode in ("chain", "stage0"):
        stages = range(k) if mode == "chain" else [0]
        acc = {}
        for s in stages:
            p = M[s]; rem = [j for j in range(k) if p[j] > 0]
            lp = np.log(np.maximum(p, 1e-12))
            for x in range(len(rem)):
                for y in range(x + 1, len(rem)):
                    j, l = rem[x], rem[y]
                    r = float(np.clip(lp[j] - lp[l], -clip, clip))
                    acc.setdefault((order[j], order[l]), []).append(r)
        return {key: v for key, v in acc.items()}  # list of per-stage measurements
    if mode in ("marginal", "sinkhorn"):  # one-forward slot marginals: P(a before b) under independent slots
        P = M.copy()
        if mode == "sinkhorn":
            for _ in range(50):
                P = P / np.maximum(P.sum(1, keepdims=True), 1e-12); P = P / np.maximum(P.sum(0, keepdims=True), 1e-12)
        P = P / np.maximum(P.sum(0, keepdims=True), 1e-12)  # normalise each letter's column over slots
        for j in range(k):
            for l in range(j + 1, k):
                pj, pl = P[:, j], P[:, l]
                before = sum(pj[s] * pl[s2] for s in range(k) for s2 in range(s + 1, k)); after = sum(pl[s] * pj[s2] for s in range(k) for s2 in range(s + 1, k))
                z = before + after
                if z <= 0: continue
                pr = before / z
                r = float(np.clip(math.log(max(pr, 1e-9)) - math.log(max(1 - pr, 1e-9)), -clip, clip))
                prefs[(order[j], order[l])] = [r]
        return prefs
    raise ValueError(mode)


def summarize(traces, names, n, mode, clip):
    per, fits = {}, []
    for name in names:
        obs, pres_prefs = [], {}
        for t in traces:
            if name not in t["criteria"]: continue
            ci = t["criteria"].index(name)
            prefs = pair_prefs(t, ci, mode, clip)
            if not prefs: continue
            signed = {}
            for (a, b), r in prefs.items():
                rs = r if isinstance(r, list) else [r]
                for x in rs: obs.append((a, b, x))
                signed[(a, b)] = float(np.sum(rs))
            pres_prefs.setdefault((t["plan"], name), []).append(signed)
        compared = flips = 0
        for lst in pres_prefs.values():
            for i in range(len(lst)):
                for j in range(i + 1, len(lst)):
                    for (a, b), r in lst[i].items():
                        r2 = lst[j].get((a, b)); 
                        if r2 is None: r2 = -lst[j].get((b, a), 0.0)
                        if r2 == 0.0: continue
                        compared += 1; flips += (r > 0) != (r2 > 0)
        s = fit(n, obs); fits.append(s)
        per[name] = {"scores": s.tolist(), "flip": flips / compared if compared else None, "obs": len(obs)}
    inter = [spearman(fits[i], fits[j]) for i in range(len(names)) for j in range(i + 1, len(names))]
    return per, float(np.mean(inter))


def run(tracedir, mode, clip, baselines):
    rows = []
    for label in COHORTS:
        path = os.path.join(tracedir, f"trace-{label}.jsonl")
        if not os.path.exists(path): continue
        traces = [json.loads(l) for l in open(path)]
        names = sorted({c for t in traces for c in t["criteria"]}, key=lambda c: [t["criteria"] for t in traces if t["arm"] == "separate"].index([c]))
        n = 40
        sep_t = [t for t in traces if t["arm"] == "separate"]; joi_t = [t for t in traces if t["arm"] == "joint"]
        sep, sep_inter = summarize(sep_t, names, n, mode, clip); joi, joi_inter = summarize(joi_t, names, n, mode, clip)
        base = json.load(open(os.path.join(baselines, f"summary-{label}.json")))
        gsep = [a for a in base["arms"] if a["arm"] == "separate"][0]["per_criterion"]
        for nm in names:
            rows.append({"cohort": label, "criterion": nm, "agreement": spearman(sep[nm]["scores"], joi[nm]["scores"]),
                         "flip_sep": sep[nm]["flip"], "flip_joint": joi[nm]["flip"],
                         "rho_gemma_sep": spearman(sep[nm]["scores"], gsep[nm]["scores"]), "rho_gemma_joint": spearman(joi[nm]["scores"], gsep[nm]["scores"]),
                         "halo": joi_inter - sep_inter})
    return rows


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("--traces", required=True); ap.add_argument("--baselines", required=True)
    ap.add_argument("--modes", default="greedy,chain,stage0"); ap.add_argument("--clip", type=float, default=3.0)
    ap.add_argument("--json", default=None)
    args = ap.parse_args()
    allrows = {}
    for mode in args.modes.split(","):
        rows = run(args.traces, mode, args.clip, args.baselines); allrows[mode] = rows
        ag = np.array([r["agreement"] for r in rows]); fs = np.array([r["flip_sep"] for r in rows]); fj = np.array([r["flip_joint"] for r in rows])
        halo = {r["cohort"]: r["halo"] for r in rows}; hv = np.array(list(halo.values()))
        rg = np.array([r["rho_gemma_sep"] for r in rows]); rgj = np.array([r["rho_gemma_joint"] for r in rows])
        print(f"{mode:9s} clip={args.clip:.0f} | agreement {ag.mean():+.3f} [min {ag.min():+.3f}] pass {int((ag > .85).sum())}/{len(ag)} | halo {hv.mean():+.3f} [max {hv.max():+.3f}] | flips sep {fs.mean():.3f} -> joint {fj.mean():.3f} | rho~gemma sep {rg.mean():+.3f} [{rg.min():+.3f}..{rg.max():+.3f}] joint {rgj.mean():+.3f}")
        for r in rows:
            print(f"   {r['cohort']:12s} {r['criterion']:14s} agr {r['agreement']:+.3f} flip {r['flip_sep']:.3f}->{r['flip_joint']:.3f} rho~g {r['rho_gemma_sep']:+.3f}/{r['rho_gemma_joint']:+.3f} halo {r['halo']:+.3f}")
    if args.json: json.dump(allrows, open(args.json, "w"), indent=1)
