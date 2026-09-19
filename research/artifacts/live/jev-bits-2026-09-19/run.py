"""jev-bits: how much ordering information does one Jev call carry, by instrument and by state packing?

Arms (state packing):   P  one pair per state (k=2), both state orders
                        W  k=8 windows, R rounds of random partitions
                        F  the whole cohort in one state, L shuffled labelings, questions chunked
                        S  one item per state (k=1), absolute 10-level rubric  [pointwise baseline]
Instruments (question):  noul      "item x is greater than item y"                      -> logit
                        score5    5-rung ratio ladder (10, 3, 1)                       -> E[log ratio]
                        score9    9-rung ratio ladder (8, 4, 2, 1.3, 1)                -> E[log ratio]
                        thermo    8 nouls "x is at least r times y" (a CDF by questions) -> E[log ratio]
                        choice    top-1 and bottom-1 over the state's items (W, F)
                        rate      10-level standing of one item among those shown (W, F), absolute (S)
P carries every pair instrument in the same call (questions are isolated: probe.py), so instruments
are compared on identical states. score9 and noul are asked in both mention orders everywhere.

Writes obs-<cohort>.jsonl (one row per directed observation) and calls-<cohort>.jsonl (token ledger).
Run: xyra-vault run ~/x/jev/typed-judgment -- python3 run.py <cohort> [arms]
"""
import json, math, os, random, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from jevclient import call_many, pmf

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = f"{HERE}/../../.."
SEED = 7
W_K, W_ROUNDS, F_LABELINGS, CHUNK_TOKENS = 8, 20, 4, 45_000

R9 = [1 / 8, 1 / 4, 1 / 2, 1 / 1.3, 1, 1.3, 2, 4, 8]
R5 = [1 / 10, 1 / 3, 1, 3, 10]
TH = [1 / 8, 1 / 4, 1 / 2, 1 / 1.3, 1.3, 2, 4, 8]
WORD = {8: "eight times or more", 10: "ten times or more", 4: "about four times", 3: "about three times",
        2: "about two times", 1.3: "slightly, about 1.3 times"}


def cohort(name):
    if name in ("countries", "rivers"):
        items = json.load(open(f"{ROOT}/data/anchors_{name}.json"))
        truth = json.load(open(f"{ROOT}/data/anchors_{name}_truth.json"))
        crit = {"countries": {"population": "Population: the number of people who live in the country."},
                "rivers": {"length": "Length: the length of the river from source to mouth."}}[name]
        ref = {c: {it["id"]: math.log(truth[it["id"]]) for it in items} for c in crit}
        return items, crit, ref
    B = f"{ROOT}/batteries/multi-criteria-bench"
    items = json.load(open(f"{B}/{name}.json"))
    crit = {c["name"]: c["prompt"] for c in json.load(open(f"{B}/{name}.criteria.json"))}
    s = json.load(open(f"{B}/baselines/summary-{name}.json"))
    ref = {c: {} for c in crit}
    for c in crit:  # reference = mean of gemma-4-31b's separate and joint arm z-scores
        for arm in s["arms"]:
            sc = arm["per_criterion"][c]["scores"]
            mu = sum(sc) / len(sc); sd = (sum((x - mu) ** 2 for x in sc) / len(sc)) ** 0.5
            for i, x in zip(s["ids"], sc):
                ref[c][i] = ref[c].get(i, 0) + (x - mu) / sd / len(s["arms"])
    return items, crit, ref


def ladder(rs, x, y):
    out = []
    for r in rs:
        if r == 1:
            out.append(f"Item {x} and item {y} are about equal")
        elif r < 1:
            out.append(f"Item {y} is greater than item {x}: {WORD[round(1 / r, 1) if 1 / r < 2 else round(1 / r)]}")
        else:
            out.append(f"Item {x} is greater than item {y}: {WORD[round(r, 1) if r < 2 else round(r)]}")
    return out


def pair_questions(c, ctext, x, y, tagx, full):
    """x, y are labels in the state; returns {qid: question}. qid = crit|instr|x|y[|k]."""
    head = f"Criterion — {ctext}\n"
    q = {f"{c}|score9|{tagx}": {"type": "score", "instructions": head + f"On this criterion, how many times greater is item {x} than item {y}?", "criteria": ladder(R9, x, y)},
         f"{c}|noul|{tagx}": {"type": "noul", "instructions": head + f"On this criterion, item {x} is greater than item {y}."}}
    if full:
        q[f"{c}|score5|{tagx}"] = {"type": "score", "instructions": head + f"On this criterion, how many times greater is item {x} than item {y}?", "criteria": ladder(R5, x, y)}
        for k, r in enumerate(TH):
            phrase = f"at least {r:g} times as great as" if r > 1 else f"at least 1/{1 / r:g} as great as"
            q[f"{c}|thermo|{tagx}|{k}"] = {"type": "noul", "instructions": head + f"On this criterion, item {x} is {phrase} item {y}."}
    return q


def state_of(its):
    return "\n\n".join(f"ITEM {n + 1}:\n{it['text']}" for n, it in enumerate(its))


def est_tokens(q):
    return sum(len(json.dumps(v)) for v in q.values()) / 4.0 + 12 * len(q)


def build(items, crit, arms):
    rng = random.Random(SEED)
    n = len(items)
    reqs = []
    if "P" in arms:
        for i in range(n):
            for j in range(n):
                if i == j:
                    continue
                q = {}
                for c, ct in crit.items():
                    q.update(pair_questions(c, ct, 1, 2, "1>2", True))
                    q.update(pair_questions(c, ct, 2, 1, "2>1", False))
                reqs.append({"arm": "P", "members": [i, j], "state": state_of([items[i], items[j]]), "questions": q, "tag": f"P|{i}|{j}"})
    if "W" in arms and n > W_K:
        for r in range(W_ROUNDS):
            perm = list(range(n)); rng.shuffle(perm)
            for w in range(0, n - n % W_K, W_K):
                mem = perm[w:w + W_K]
                reqs.append(window_req("W", mem, items, crit, f"W|{r}|{w // W_K}"))
    if "T" in arms and n > W_K:  # W's exact windows, terse wording: criteria stated once in the state, short rungs
        rt = random.Random(SEED)
        for r in range(W_ROUNDS):
            perm = list(range(n)); rt.shuffle(perm)
            for w in range(0, n - n % W_K, W_K):
                reqs.append(terse_window_req(perm[w:w + W_K], items, crit, f"T|{r}|{w // W_K}"))
    if "U" in arms and n > W_K:  # T's windows, score9 only, polarity-safe wording ("stronger on", never "greater")
        rt = random.Random(SEED)
        for r in range(W_ROUNDS):
            perm = list(range(n)); rt.shuffle(perm)
            for w in range(0, n - n % W_K, W_K):
                rq = terse_window_req(perm[w:w + W_K], items, crit, f"U|{r}|{w // W_K}")
                rq["arm"] = "U"
                rq["questions"] = {qid: {"type": "score", "criteria": U9, "instructions": "On {}, how strong is item {} relative to item {}?".format(qid.split("|")[0], *qid.split("|")[2].split(">"))}
                                   for qid in rq["questions"] if qid.split("|")[1] == "score9"}
                reqs.append(rq)
    if "F" in arms:
        for lab in range(F_LABELINGS):
            perm = list(range(n)); rng.shuffle(perm)
            full = window_req("F", perm, items, crit, f"F|{lab}")
            qs, chunk, tok, part = list(full["questions"].items()), {}, 0, 0
            for qid, qq in qs:  # chunk the question list; every chunk re-pays the state
                t = est_tokens({qid: qq})
                if chunk and tok + t > CHUNK_TOKENS:
                    reqs.append({**full, "questions": chunk, "tag": f"F|{lab}|{part}"}); chunk, tok, part = {}, 0, part + 1
                chunk[qid] = qq; tok += t
            reqs.append({**full, "questions": chunk, "tag": f"F|{lab}|{part}"})
    if "S" in arms:
        levels = ["Lowest: almost none of this", "Very low", "Low", "Somewhat below the middle", "Just below the middle",
                  "Just above the middle", "Somewhat above the middle", "High", "Very high", "Highest: an extreme amount of this"]
        for i in range(n):
            q = {f"{c}|rate|1": {"type": "score", "instructions": f"Criterion — {ct}\nWhere does item 1 stand on this criterion?", "criteria": levels} for c, ct in crit.items()}
            reqs.append({"arm": "S", "members": [i], "state": state_of([items[i]]), "questions": q, "tag": f"S|{i}"})
    return reqs


def window_req(arm, mem, items, crit, tag):
    k = len(mem)
    q = {}
    standing = ["The lowest of the items shown", "Near the bottom", "Low", "Somewhat below the middle", "Just below the middle",
                "Just above the middle", "Somewhat above the middle", "High", "Near the top", "The highest of the items shown"]
    for c, ct in crit.items():
        for a in range(k):
            for b in range(a + 1, k):
                q.update(pair_questions(c, ct, a + 1, b + 1, f"{a + 1}>{b + 1}", False))
                q.update(pair_questions(c, ct, b + 1, a + 1, f"{b + 1}>{a + 1}", False))
        opts = {f"item_{a + 1}": f"Item {a + 1}" for a in range(k)}
        q[f"{c}|top|0"] = {"type": "choice", "instructions": f"Criterion — {ct}\nWhich item is the highest on this criterion?", "criteria": opts}
        q[f"{c}|bottom|0"] = {"type": "choice", "instructions": f"Criterion — {ct}\nWhich item is the lowest on this criterion?", "criteria": opts}
        for a in range(k):
            q[f"{c}|rate|{a + 1}"] = {"type": "score", "instructions": f"Criterion — {ct}\nWhere does item {a + 1} stand on this criterion among the items shown?", "criteria": standing}
    return {"arm": arm, "members": mem, "state": state_of([items[m] for m in mem]), "questions": q, "tag": tag}


T9 = ["1/8 or less", "about 1/4", "about 1/2", "about 1/1.3", "about equal", "about 1.3 times", "about 2 times", "about 4 times", "8 times or more"]
U9 = ["far weaker: 1/8 or less", "much weaker: about 1/4", "weaker: about 1/2", "slightly weaker: about 1/1.3", "about equal",
      "slightly stronger: about 1.3 times", "stronger: about 2 times", "much stronger: about 4 times", "far stronger: 8 times or more"]
T10 = ["lowest", "near the bottom", "low", "somewhat below middle", "just below middle", "just above middle", "somewhat above middle", "high", "near the top", "highest"]


def terse_window_req(mem, items, crit, tag):
    k, q = len(mem), {}
    head = "CRITERIA\n" + "\n".join(f"{c}: {ct}" for c, ct in crit.items()) + "\n\n"
    for c in crit:
        for a in range(1, k + 1):
            for b in range(1, k + 1):
                if a != b:
                    q[f"{c}|score9|{a}>{b}"] = {"type": "score", "instructions": f"On {c}, how many times greater is item {a} than item {b}?", "criteria": T9}
                    q[f"{c}|noul|{a}>{b}"] = {"type": "noul", "instructions": f"On {c}, item {a} is greater than item {b}."}
            q[f"{c}|rate|{a}"] = {"type": "score", "instructions": f"On {c}, where does item {a} stand among the items shown?", "criteria": T10}
        opts = {f"item_{a}": f"Item {a}" for a in range(1, k + 1)}
        q[f"{c}|top|0"] = {"type": "choice", "instructions": f"Which item is the highest on {c}?", "criteria": opts}
        q[f"{c}|bottom|0"] = {"type": "choice", "instructions": f"Which item is the lowest on {c}?", "criteria": opts}
    return {"arm": "T", "members": mem, "state": head + state_of([items[m] for m in mem]), "questions": q, "tag": tag}


def clip_logit(p):
    p = min(max(p, 0.005), 0.995)
    return math.log(p / (1 - p))


def thermo_mean(s):
    t = [math.log(r) for r in TH]
    g = [(t[min(k + 1, len(t) - 1)] - t[max(k - 1, 0)]) / (2 if 0 < k < len(t) - 1 else 1) for k in range(len(t))]
    return sum(sk * gk for sk, gk in zip(s, g)) - sum(g) / 2


def observations(req, rec):
    mem, arm, ans = req["members"], req["arm"], rec["answers"]
    rows, th = [], {}
    for qid, a in ans.items():
        parts = qid.split("|")
        c, instr = parts[0], parts[1]
        if instr in ("score9", "score5", "noul", "thermo"):
            x, y = (int(v) for v in parts[2].split(">"))
            base = {"arm": arm, "call": req["tag"], "crit": c, "i": mem[x - 1], "j": mem[y - 1], "x_pos": x, "y_pos": y}
            if instr == "thermo":
                th.setdefault((c, parts[2]), (base, {}))[1][int(parts[3])] = a["noul"]
            elif instr == "noul":
                rows.append({**base, "instr": "noul", "y": clip_logit(a["noul"]), "p": a["noul"]})
            else:
                rs = R9 if instr == "score9" else R5
                d = pmf(a, len(rs))
                rows.append({**base, "instr": instr, "y": sum(p * math.log(r) for p, r in zip(d, rs)), "dist": d})
        elif instr in ("top", "bottom"):
            for a_pos in range(len(mem)):
                rows.append({"arm": arm, "call": req["tag"], "crit": c, "instr": instr, "i": mem[a_pos], "x_pos": a_pos + 1,
                             "p": a["probabilities"].get(f"item_{a_pos + 1}", 0.0)})
        elif instr == "rate":
            pos = int(parts[2])
            rows.append({"arm": arm, "call": req["tag"], "crit": c, "instr": "rate", "i": mem[pos - 1], "x_pos": pos, "y": a["score"]})
    for (c, _), (base, s) in th.items():
        sv = [s[k] for k in range(len(TH))]
        rows.append({**base, "instr": "thermo", "y": thermo_mean(sv), "dist": sv})
    return rows


def main():
    name = sys.argv[1]
    arms = sys.argv[2] if len(sys.argv) > 2 else "PWFS"
    items, crit, ref = cohort(name)
    reqs = build(items, crit, arms)
    est = sum(len(r["state"]) / 4 + est_tokens(r["questions"]) for r in reqs)
    print(f"{name}: {len(items)} items, {len(crit)} criteria, {len(reqs)} calls, ~{est / 1e6:.2f}M tokens (~${est * 0.042 / 1e6:.3f})", flush=True)
    recs = call_many(reqs, f"{HERE}/trace-{name}.jsonl", workers=int(os.environ.get("WORKERS", "12")))
    sfx = "" if arms == "PWFS" else f"-{arms}"
    with open(f"{HERE}/obs-{name}{sfx}.jsonl", "w") as fo, open(f"{HERE}/calls-{name}{sfx}.jsonl", "w") as fc:
        for req, rec in zip(reqs, recs):
            for row in observations(req, rec):
                fo.write(json.dumps(row) + "\n")
            fc.write(json.dumps({"call": req["tag"], "arm": req["arm"], "n_q": len(req["questions"]), "state_tokens_est": len(req["state"]) / 4,
                                 "input_tokens": rec["usage"].get("input_tokens"), "latency_ms": rec["latency_ms"], "attempts": rec["attempts"]}) + "\n")
    json.dump({"ids": [it["id"] for it in items], "ref": ref}, open(f"{HERE}/ref-{name}.json", "w"))
    tot = sum(r["usage"].get("input_tokens", 0) for r in recs)
    print(f"done: {tot / 1e6:.2f}M input tokens billed (${tot * 0.042 / 1e6:.3f})")


if __name__ == "__main__":
    main()
