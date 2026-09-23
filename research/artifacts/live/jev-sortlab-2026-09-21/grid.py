"""The judgment space, run as one grid. xyra-vault run $HOME/x/jev/typed-judgment -- python3 grid.py <cohort> [recipe ...]

Every way of asking Jev to place items, priced per round on the same cohorts, so a winner can be declared per objective.
A round shows every item once. Families (q = questions per call, k = items per state):
  setwise, one question per item      rate-L3/L5/L7/L10 (L standing levels), tophalf (yes/no: in the upper half), k q
  setwise, one question per state     top (which is highest), topbot (+ which is lowest), 1–2 q — a choice PMF is k observations
  three at a time                     tri (which of x, y, z is highest, k cyclic triples inside a k-state), tri3 (3-item states, highest + lowest)
  two at a time                       noul (all ordered pairs), noulcyc (a cycle, both orders), score9 / chain (ratio ladder, all pairs / cycle),
                                      score5cyc (five-rung ladder), widecyc (1/100…100 ladder, cycle), pair2 (2-item states, yes/no both orders),
                                      pair2r (2-item states, ratio both orders), anchor (three pinned anchors, wide ladder)
  perturbations                       k = 4 / 8 / 24 / 48 on rating, k = 24 on tophalf and tri, k = 4 / 24 on choice; fmt (plain numbered lines instead of labelled blocks)
Fit: one least squares. item questions y = u_i + w_call; pairs y = u_i − u_j; a choice PMF gives y_i = log p_i = u_i + w_question.
After each round: ρ and Pearson against the reference, split-half self-agreement, slope, dollars. Writes grid-<cohort>-<recipe>.json.
"""
import json, math, os, random, sys
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__)); sys.path.insert(0, HERE); sys.path.insert(0, f"{HERE}/../jev-bits-2026-09-19")
from jevclient import call_many, pmf
from lab import R9, RW, T10, U9, W9, bits, chunks, cohort, spearman
import wordings

PRICE = 0.042 / 1e6
L3 = ["low", "middle", "high"]; L5 = ["lowest", "below middle", "middle", "above middle", "highest"]
L7 = ["lowest", "low", "somewhat below middle", "middle", "somewhat above middle", "high", "highest"]
R5 = [1 / 4, 1 / 1.5, 1, 1.5, 4]; U5 = ["far weaker: 1/4 or less", "somewhat weaker", "about equal", "somewhat stronger", "far stronger: 4 times or more"]
RECIPES = {  # name: (k, family)
    "rate-L3": (8, "rate"), "rate-L5": (8, "rate"), "rate-L7": (8, "rate"), "rate-L10": (8, "rate"), "rate-L10-k4": (4, "rate"), "rate-L10-k24": (24, "rate"), "rate-L10-fmt": (8, "rate"),
    "tophalf": (8, "tophalf"), "top": (8, "top"), "topbot": (8, "topbot"), "top-k24": (24, "top"), "topbot-k24": (24, "topbot"), "tri": (8, "tri"), "tri3": (3, "tri3"),
    "noul": (8, "noul"), "noulcyc": (8, "noulcyc"), "score9": (8, "score9"), "chain": (8, "chain"), "score5cyc": (8, "score5cyc"), "widecyc": (8, "widecyc"),
    "pair2": (2, "pair2"), "pair2r": (2, "pair2r"), "anchor": (8, "anchor"),
    "rate-L3-k24": (24, "rate"), "rate-L5-k24": (24, "rate"), "rate-L10-k48": (48, "rate"), "tophalf-k24": (24, "tophalf"), "topbot-k4": (4, "topbot"), "tri-k24": (24, "tri"),
}


def load(name):
    if name.startswith("names-"):
        a = name.split("-")[1]; r = json.load(open(f"{HERE}/ref-names.json"))
        items = [{"id": x, "text": x} for x in wordings.NAMES]; ref = np.array([r["ref"][a][x] for x in wordings.NAMES])
        return items, ref, list(range(len(items))), wordings.ATTRS["names"][a][0], "name"
    return cohort(name)


def state_of(items, mem, noun, fmt):
    if fmt == "plain": return "\n".join(f"{s + 1}. {items[i]['text']}" for s, i in enumerate(mem))
    return "\n\n".join(f"{noun.upper()} {s + 1}:\n{items[i]['text']}" for s, i in enumerate(mem))


def questions(recipe, k, attr, noun, na=0):
    head = f"Attribute — {attr}\n"; q = {}; fam = RECIPES[recipe][1]
    stand = lambda x, crit: {"type": "score", "instructions": head + f"Among the {noun}s shown, where does {noun} {x} stand on this attribute?", "criteria": crit}
    rel = lambda x, y, crit: {"type": "score", "instructions": head + f"On this attribute, how strong is {noun} {x} relative to {noun} {y}?", "criteria": crit}
    gt = lambda x, y: {"type": "noul", "instructions": head + f"{noun.capitalize()} {x} is stronger on this attribute than {noun} {y}."}
    opts = lambda ids: {f"o{x}": f"{noun} {x}" for x in ids}
    pairs = [(x, y) for x in range(1, k + 1) for y in range(1, k + 1) if x != y]
    cyc = [(s + 1, (s + 1) % k + 1) for s in range(k)]; cyc = cyc + [(y, x) for x, y in cyc]
    if fam == "rate":
        crit = {"L3": L3, "L5": L5, "L7": L7}.get(recipe.split("-")[1], T10)
        for x in range(1, k + 1): q[f"rate{len(crit)}|{x}"] = stand(x, crit)
    elif fam == "tophalf":
        for x in range(1, k + 1): q[f"half|{x}"] = {"type": "noul", "instructions": head + f"On this attribute, {noun} {x} is in the upper half of the {noun}s shown."}
    elif fam in ("top", "topbot"):
        q["top|all"] = {"type": "choice", "instructions": head + f"Which {noun} shown is the highest on this attribute?", "criteria": opts(range(1, k + 1))}
        if fam == "topbot": q["bot|all"] = {"type": "choice", "instructions": head + f"Which {noun} shown is the lowest on this attribute?", "criteria": opts(range(1, k + 1))}
    elif fam == "tri":
        for s in range(k):
            t = [s + 1, (s + 1) % k + 1, (s + 2) % k + 1]
            q[f"top|{'.'.join(map(str, t))}"] = {"type": "choice", "instructions": head + f"Of {noun}s {t[0]}, {t[1]} and {t[2]}, which is the highest on this attribute?", "criteria": opts(t)}
    elif fam == "tri3":
        q["top|all"] = {"type": "choice", "instructions": head + f"Which {noun} shown is the highest on this attribute?", "criteria": opts(range(1, k + 1))}
        q["bot|all"] = {"type": "choice", "instructions": head + f"Which {noun} shown is the lowest on this attribute?", "criteria": opts(range(1, k + 1))}
    elif fam in ("noul", "pair2"):
        for x, y in pairs: q[f"noul|{x}>{y}"] = gt(x, y)  # pair2: k = 2, the odd tail window has 3
    elif fam == "noulcyc":
        for x, y in cyc: q[f"noul|{x}>{y}"] = gt(x, y)
    elif fam in ("score9", "pair2r"):
        for x, y in pairs: q[f"r9|{x}>{y}"] = rel(x, y, U9)
    elif fam == "chain":
        for x, y in cyc: q[f"r9|{x}>{y}"] = rel(x, y, U9)
    elif fam == "score5cyc":
        for x, y in cyc: q[f"r5|{x}>{y}"] = rel(x, y, U5)
    elif fam == "widecyc":
        for x, y in cyc: q[f"rw|{x}>{y}"] = rel(x, y, W9)
    elif fam == "anchor":
        for x in range(na + 1, k + 1):
            for a in range(1, na + 1): q[f"rw|{x}>{a}"] = rel(x, a, W9)
        for a in range(1, na + 1):
            for b in range(a + 1, na + 1): q[f"rw|{a}>{b}"] = rel(a, b, W9)
    return q


def observe(rec, mem, tag):
    """-> (i, j, fe, y): j = -1 for a one-item observation, fe = the fixed-effect key or None."""
    out = []
    for qid, a in rec["answers"].items():
        kind, spec = qid.split("|")
        if kind.startswith("rate"):
            L = int(kind[4:]); out.append((mem[int(spec) - 1], -1, tag, float(sum(p * (l + 1) for l, p in enumerate(pmf(a, L))))))
        elif kind == "half":
            p = min(max(a["noul"], 0.01), 0.99); out.append((mem[int(spec) - 1], -1, tag, math.log(p / (1 - p))))
        elif kind in ("top", "bot"):
            sign = 1 if kind == "top" else -1; fe = f"{tag}|{qid}"
            for o, p in a["probabilities"].items(): out.append((mem[int(o[1:]) - 1], -1, fe, sign * math.log(max(p, 0.005))))
        else:
            x, y = (int(s) - 1 for s in spec.split(">")); i, j = mem[x], mem[y]
            if kind == "noul":
                p = min(max(a["noul"], 0.01), 0.99); out.append((i, j, None, math.log(p / (1 - p))))
            else:
                lad = {"r9": R9, "r5": R5, "rw": RW}[kind]; out.append((i, j, None, float(sum(p * math.log(r) for p, r in zip(pmf(a, len(lad)), lad)))))
    return out


def fit(obs, n):
    fes = sorted({o[2] for o in obs if o[2] is not None}); col = {f: n + t for t, f in enumerate(fes)}
    A = np.zeros((len(obs) + 1, n + len(fes))); b = np.zeros(len(obs) + 1)
    for r, (i, j, fe, y) in enumerate(obs):
        A[r, i] = 1; b[r] = y
        if j >= 0: A[r, j] = -1
        if fe is not None: A[r, col[fe]] = 1
    A[-1, :n] = 1
    return np.linalg.lstsq(A, b, rcond=None)[0][:n]


def run(name, recipe, rounds=6):
    items, ref, ridx, attr, noun = load(name); n = len(items); k, fam = RECIPES[recipe]; fmt = "plain" if recipe.endswith("-fmt") else "block"
    trace = f"{HERE}/trace-grid-{name}.jsonl"; rng = random.Random(11); anchors = []; obs = []; log = []; tok = 0; calls = 0
    if fam == "anchor":
        pil = [{"state": state_of(items, mem, noun, fmt), "questions": questions("rate-L10", len(mem), attr, noun), "tag": f"{name}|{recipe}|pilot|w{w}", "mem": mem} for w, mem in enumerate(chunks(rng.sample(range(n), n), k))]
        pobs = []
        for rq, rc in zip(pil, call_many(pil, trace)):
            pobs += observe(rc, rq["mem"], rq["tag"]); tok += rc["usage"]["input_tokens"]; calls += 1
        o = np.argsort(fit(pobs, n)); anchors = [int(o[int(q * (n - 1))]) for q in (0.1, 0.5, 0.9)]
    for r in range(rounds):
        order = rng.sample([i for i in range(n) if i not in anchors], n - len(anchors)); reqs = []
        for w, mem in enumerate(chunks(order, k - len(anchors))):
            mem = anchors + mem
            reqs.append({"state": state_of(items, mem, noun, fmt), "questions": questions(recipe, len(mem), attr, noun, len(anchors)), "tag": f"{name}|{recipe}|r{r}|w{w}", "mem": mem})
        for rq, rc in zip(reqs, call_many(reqs, trace)):
            obs += [(r, o) for o in observe(rc, rq["mem"], rq["tag"])]; tok += rc["usage"]["input_tokens"]; calls += 1
        u = fit([o for _, o in obs], n); rho = spearman(u[ridx], ref); pear = float(np.corrcoef(u[ridx], ref)[0, 1])
        if r > 0:
            h = [fit([o for rr, o in obs if rr % 2 == s], n) for s in (0, 1)]; sh = spearman(h[0], h[1]); sb = 2 * sh / (1 + sh)
        else: sb = float("nan")
        slope = float(np.polyfit(ref, u[ridx], 1)[0])
        log.append({"round": r + 1, "calls": calls, "tokens": tok, "dollars": tok * PRICE, "rho": rho, "pearson": pear, "self": sb, "slope": slope})
        print(f"{name:12s} {recipe:13s} round {r + 1} calls {calls:4d} ${tok * PRICE:.4f} rho {rho:.3f} self {sb:.3f} slope {slope:.2f}", flush=True)
    json.dump({"cohort": name, "recipe": recipe, "k": k, "n": n, "q_per_call": len(questions(recipe, k, attr, noun, len(anchors))), "rounds": log, "latent": u.tolist()}, open(f"{HERE}/grid-{name}-{recipe}.json", "w"), indent=0)


if __name__ == "__main__":
    for rec in (sys.argv[2:] or list(RECIPES)): run(sys.argv[1], rec)
