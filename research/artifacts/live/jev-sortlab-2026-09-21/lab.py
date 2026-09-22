"""Sorting recipes on hosted Jev, measured as information per dollar.

xyra-vault run ~/x/jev/typed-judgment -- python3 lab.py <cohort> <recipe> [rounds=8] [k=8]
Replay from trace-<cohort>.jsonl needs no key. Writes res-<cohort>-<recipe>[-k<k>].json: one checkpoint per round with
calls, billed tokens, dollars, agreement with the reference, split-half precision, and the bits each implies.

cohorts  countries  198 sovereign states by population (Wikidata P1082); the reference is log population, a true cardinal.
         arxiv150   150 arXiv abstracts by novelty; the reference is gemma-4-31b's pooled z-score on the 40-item subset
                    of multi-criteria-bench (jev-bits ref-arxiv.json), so agreement there is judge–judge.
recipes  rate    random windows of k, one 10-level standing question per item            (k questions)
         noul    random windows, "x is stronger than y" for every ordered pair            (k(k-1) questions)
         score9  random windows, 9-rung ratio for every ordered pair, polarity-safe       (k(k-1) questions)
         chain   random windows, ratio on a random cycle only, both orders                (2k questions)
         arate   rate, but from round 2 the windows are consecutive blocks of the current fitted order
         achain  chain, the same adaptive windows
         anchor  three pinned anchor items (10/50/90 % of a pilot rate round) in every call beside k-3 targets;
                 wide 9-rung ratio (1/100 … 100) target vs anchor and anchor vs anchor       (3(k-3)+3 questions)
Fits: pairs by least squares on directed log-ratio / logit observations; ratings by least squares with a window
fixed effect (u_i + w_call), which is what makes sorted windows usable.
"""
import csv, json, math, os, random, sys
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__)); sys.path.insert(0, f"{HERE}/../jev-bits-2026-09-19")
from jevclient import call_many, pmf

PRICE = 0.042 / 1e6
R9 = [1 / 8, 1 / 4, 1 / 2, 1 / 1.3, 1, 1.3, 2, 4, 8]
U9 = ["far weaker: 1/8 or less", "much weaker: about 1/4", "weaker: about 1/2", "slightly weaker: about 1/1.3", "about equal",
      "slightly stronger: about 1.3 times", "stronger: about 2 times", "much stronger: about 4 times", "far stronger: 8 times or more"]
RW = [1 / 100, 1 / 30, 1 / 10, 1 / 3, 1, 3, 10, 30, 100]
W9 = ["1/100 or less", "about 1/30", "about 1/10", "about 1/3", "about equal", "about 3 times", "about 10 times", "about 30 times", "100 times or more"]
T10 = ["lowest", "near the bottom", "low", "somewhat below middle", "just below middle", "just above middle", "somewhat above middle", "high", "near the top", "highest"]


def cohort(name):
    if name == "countries":
        rows = list(csv.DictReader(open(f"{HERE}/countries.csv")))
        items = [{"id": r["cLabel"], "text": r["cLabel"]} for r in rows]
        ref = np.log([float(r["pop"]) for r in rows])
        return items, ref, list(range(len(items))), "Population: how many people live in the country (latest estimate).", "country"
    if name == "arxiv150":
        items = json.load(open(f"{HERE}/../../../data/arxiv_abstracts.json"))
        r = json.load(open(f"{HERE}/../jev-bits-2026-09-19/ref-arxiv.json")); pos = {it["id"]: i for i, it in enumerate(items)}
        idx = [pos[i] for i in r["ids"]]
        return items, np.array([r["ref"]["novelty"][i] for i in r["ids"]]), idx, ("Novelty: how much genuinely new understanding the work contributes — a new problem, method, "
                                                            "or finding a well-read researcher in the area would not already possess — rather than incremental variation on known work."), "item"
    raise SystemExit(name)


def chunks(order, k):
    w = [order[i:i + k] for i in range(0, len(order), k)]
    if len(w) > 1 and len(w[-1]) < max(2, k // 2):
        w[-2] += w.pop()
    return w


def questions(recipe, mem, attr, noun, anchors=()):
    k = len(mem); head = f"Attribute — {attr}\n"; q = {}
    rel = lambda x, y, crit: {"type": "score", "instructions": head + f"On this attribute, how strong is {noun} {x} relative to {noun} {y}?", "criteria": crit}
    if recipe in ("rate", "arate"):
        for x in range(1, k + 1):
            q[f"rate|{x}"] = {"type": "score", "instructions": head + f"Among the {noun}s shown, where does {noun} {x} stand on this attribute?", "criteria": T10}
    elif recipe == "noul":
        for x in range(1, k + 1):
            for y in range(1, k + 1):
                if x != y: q[f"noul|{x}>{y}"] = {"type": "noul", "instructions": head + f"{noun.capitalize()} {x} is stronger on this attribute than {noun} {y}."}
    elif recipe == "score9":
        for x in range(1, k + 1):
            for y in range(1, k + 1):
                if x != y: q[f"score9|{x}>{y}"] = rel(x, y, U9)
    elif recipe in ("chain", "achain"):
        for s in range(k):
            x, y = s + 1, (s + 1) % k + 1
            q[f"score9|{x}>{y}"] = rel(x, y, U9); q[f"score9|{y}>{x}"] = rel(y, x, U9)
    elif recipe == "anchor":
        na = len(anchors)
        for x in range(na + 1, k + 1):
            for a in range(1, na + 1):
                q[f"wide|{x}>{a}"] = rel(x, a, W9)
        for a in range(1, na + 1):
            for b in range(1, na + 1):
                if a < b: q[f"wide|{a}>{b}"] = rel(a, b, W9)
    return q


def observe(recipe, rec, mem):
    """-> list of ('pair', i, j, y) or ('item', i, y) in cohort indices."""
    out = []
    for qid, a in rec["answers"].items():
        kind, spec = qid.split("|")
        if kind == "rate":
            p = pmf(a, 10); out.append(("item", mem[int(spec) - 1], float(sum(pp * (l + 1) for l, pp in enumerate(p)))))
        else:
            x, y = (int(s) - 1 for s in spec.split(">")); i, j = mem[x], mem[y]
            if kind == "noul":
                pr = min(max(a["noul"], 0.01), 0.99); out.append(("pair", i, j, math.log(pr / (1 - pr))))
            else:
                lad = RW if kind == "wide" else R9; p = pmf(a, 9); out.append(("pair", i, j, float(sum(pp * math.log(r) for pp, r in zip(p, lad)))))
    return out


def fit(obs, n):
    """Least squares: pairs y = u_i - u_j; ratings y = u_i + w_call; sum u = 0."""
    pairs = [o for o in obs if o[0] == "pair"]; items = [o for o in obs if o[0] == "item"]
    calls = sorted({o[1] for o in items}); ci = {c: n + t for t, c in enumerate(calls)}
    m = len(pairs) + len(items); A = np.zeros((m + 1, n + len(calls))); b = np.zeros(m + 1); r = 0
    for _, i, j, y in pairs:
        A[r, i], A[r, j], b[r] = 1, -1, y; r += 1
    for _, c, i, y in items:
        A[r, i], A[r, ci[c]], b[r] = 1, 1, y; r += 1
    A[-1, :n] = 1
    return np.linalg.lstsq(A, b, rcond=None)[0][:n]


def rank(v):
    return np.argsort(np.argsort(v)).astype(float)


def spearman(a, b):
    return float(np.corrcoef(rank(a), rank(b))[0, 1])


def bits(r):
    return 0.5 * math.log2(1 / max(1 - r * r, 1e-9))


def main():
    name, recipe = sys.argv[1], sys.argv[2]; rounds = int(sys.argv[3]) if len(sys.argv) > 3 else 8; k = int(sys.argv[4]) if len(sys.argv) > 4 else 8
    items, ref, ridx, attr, noun = cohort(name); n = len(items); trace = f"{HERE}/trace-{name}.jsonl"
    rng = random.Random(11); stem = f"{recipe}" + (f"-k{k}" if k != 8 else ""); anchors = []; obs = []; log = []; tok = 0; calls = 0
    if recipe == "anchor":  # pilot: one random rate round picks the anchors, and its cost is charged
        pil = []
        for w, mem in enumerate(chunks(rng.sample(range(n), n), k)):
            st = "\n\n".join(f"{noun.upper()} {s + 1}:\n{items[i]['text']}" for s, i in enumerate(mem))
            pil.append({"state": st, "questions": questions("rate", mem, attr, noun), "tag": f"{name}|{stem}|pilot|w{w}", "mem": mem})
        recs = call_many(pil, trace); pobs = []
        for rq, rc in zip(pil, recs):
            pobs += [("item", rq["tag"], o[1], o[2]) for o in observe("rate", rc, rq["mem"])]
            tok += rc["usage"]["input_tokens"]; calls += 1
        u = fit(pobs, n); o = np.argsort(u); anchors = [int(o[int(q * (n - 1))]) for q in (0.1, 0.5, 0.9)]
    for r in range(rounds):
        if recipe in ("arate", "achain") and r > 0:
            u = fit([o for _, o in obs], n); order = list(np.argsort(u)); off = rng.randrange(k); order = order[off:] + order[:off]
        else:
            order = rng.sample([i for i in range(n) if i not in anchors], n - len(anchors))
        reqs = []
        for w, mem in enumerate(chunks(order, k - len(anchors))):
            mem = anchors + mem
            st = "\n\n".join(f"{noun.upper()} {s + 1}:\n{items[i]['text']}" for s, i in enumerate(mem))
            reqs.append({"state": st, "questions": questions(recipe, mem, attr, noun, anchors), "tag": f"{name}|{stem}|r{r}|w{w}", "mem": mem})
        recs = call_many(reqs, trace)
        for rq, rc in zip(reqs, recs):
            for o in observe(recipe, rc, rq["mem"]):
                obs.append((r, ("item", rq["tag"], o[1], o[2]) if o[0] == "item" else o))
            tok += rc["usage"]["input_tokens"]; calls += 1
        u = fit([o for _, o in obs], n); rho = spearman(u[ridx], ref); pear = float(np.corrcoef(u[ridx], ref)[0, 1])
        # split half by round parity (odd rounds against even), Spearman-Brown to full length; undefined before round 2
        if r > 0:
            h = [fit([o for rr, o in obs if rr % 2 == s], n) for s in (0, 1)]; sh = spearman(h[0], h[1]); sb = 2 * sh / (1 + sh)
        else:
            sh = sb = float("nan")
        slope = float(np.polyfit(ref, u[ridx], 1)[0]) if name == "countries" else None
        log.append({"round": r + 1, "calls": calls, "tokens": tok, "dollars": tok * PRICE, "rho": rho, "pearson": pear, "bits_ref": bits(pear),
                    "split_half": sh, "self": sb, "bits_self": bits(math.sqrt(max(sb, 0))), "slope": slope})
        print(f"{name} {stem} round {r + 1:2d} calls {calls:4d} tokens {tok / 1e3:7.1f}K ${tok * PRICE:.4f} rho {rho:.3f} pearson {pear:.3f} self {sb:.3f} bits_ref {bits(pear):.2f}" + (f" slope {slope:.2f}" if slope is not None else ""), flush=True)
    json.dump({"cohort": name, "recipe": recipe, "k": k, "n": n, "anchors": anchors, "rounds": log, "latent": u.tolist()}, open(f"{HERE}/res-{name}-{stem}.json", "w"), indent=0)


if __name__ == "__main__":
    main()
