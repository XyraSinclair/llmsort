"""The same question asked many ways. xyra-vault run ~/x/jev/typed-judgment -- python3 wordings.py <cohort> [k=24]

At temperature zero, re-asking is not a reliability test (drift is nil). The test is invariance under wording: the same
attribute in eight phrasings — canonical definition, plain, a persona, a numeric frame, casual, negated polarity (sign
flipped in analysis), formal, and a leading spin — each one rating round on random windows of k. Two designs at once:
same windows (every wording on window seed 0, so only the words move) and fresh windows (wording i on seed i, against
the canonical wording on seeds 0..7), which asks the money question — at equal cost, do eight wordings on eight window
draws beat eight rounds of one wording? Pools are z-scored means; self is Spearman-Brown split-half by round parity.
Cohorts: countries (population; truth) and names (150 first names; VC-respectability, rounds a founder would raise, aura,
sounds gay; no truth — the reference is Fable, ref-names.json, when present).
Writes wordings-<cohort>.json per attribute: same-window matrix, fresh-window agreement, pooled diverse vs pooled repeated
(self and vs reference), every latent, and dollars.
"""
import json, os, random, sys
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__)); sys.path.insert(0, HERE); sys.path.insert(0, f"{HERE}/../jev-bits-2026-09-19")
from jevclient import call_many
from lab import T10, chunks, cohort, fit, observe, spearman

NAMES = ("James Michael Robert David William Richard Joseph Thomas Christopher Daniel Matthew Anthony Mark Steven Andrew Kevin Brian "
         "Jason Ryan Jacob Tyler Brandon Justin Kyle Cody Dylan Hunter Logan Ethan Mason Liam Noah Aiden Jaxon Chad Brad Trevor Tucker Preston "
         "Sebastian Julian Xavier Ezra Silas Atticus Theodore Felix Oscar Hugo Arthur Louis Henry Charles George Edward Alexander Nathaniel "
         "Mohammed Ahmed Omar Ali Hassan Yusuf Ibrahim Tariq Raj Arjun Vikram Rahul Sanjay Priya Deepa Anjali Wei Jun Ming Hao Kenji Hiroshi Yuki "
         "Jose Carlos Luis Miguel Diego Santiago Mateo Juan Sofia Isabella Camila Valentina Lucia Maria Ana Elena "
         "Mary Jennifer Linda Elizabeth Susan Jessica Sarah Karen Lisa Nancy Ashley Emily Amanda Megan Brittany Tiffany Crystal Amber Heather "
         "Olivia Emma Ava Charlotte Amelia Harper Evelyn Abigail Ella Scarlett Grace Chloe Zoe Lily Hannah Natalie Victoria Audrey Claire "
         "DeShawn Jamal Tyrone Darnell Terrell Marquis Andre Malik Latoya Shanice Keisha Aaliyah Imani Nia Ebony Jada "
         "Sasha Blake Jordan Taylor Casey Riley Morgan Avery Quinn Skyler Dakota Remy Sage Rowan Phoenix Ezekiel Moses Ira Chaim").split()

ATTRS = {
    "countries": {"population": ("Population: how many people live in the country (latest estimate).", [
        ("canonical", "Attribute — {A}\nAmong the {N}s shown, where does {N} {x} stand on this attribute?", 1),
        ("plain", "Where does {N} {x} rank by population among the {N}s listed?", 1),
        ("persona", "A demographer is ordering the {N}s listed by number of inhabitants. Where would she place {N} {x}?", 1),
        ("numeric", "Think of each {N}'s population as a number. Relative to the others listed, how large is {N} {x}'s number?", 1),
        ("casual", "Roughly how many people live in {N} {x} compared to the other {N}s here?", 1),
        ("negated", "How few people live in {N} {x}, compared with the {N}s listed? (highest = the fewest people)", -1),
        ("formal", "Rank the listed sovereign states by total resident population. Position of {N} {x}:", 1),
        ("spin", "{N} {x} is often thought of as a small place. Considering the {N}s listed, where does it actually stand by population?", 1)])},
    "names": {
        "vc": ("VC respectability: how much the name alone signals a fundable founder to a venture capitalist reading a pitch deck.", [
            ("canonical", "Attribute — {A}\nAmong the {N}s shown, where does {N} {x} stand on this attribute?", 1),
            ("plain", "How respectable does {N} {x} seem to a VC, compared with the other {N}s listed?", 1),
            ("persona", "A partner at a top venture firm is skimming founder names on a deal list. Where does {N} {x} land for her, among the {N}s shown?", 1),
            ("numeric", "Give each {N} listed a fundability score from its name alone. Relative to the others, how high is {N} {x}'s score?", 1),
            ("casual", "Would VCs take {N} {x} seriously as a founder name? Compared to the other {N}s here, where does it sit?", 1),
            ("negated", "How unfundable does {N} {x} sound to a VC, compared with the {N}s listed? (highest = the least fundable)", -1),
            ("formal", "Order the listed given names by the credibility they confer on a founder in institutional venture capital. Position of {N} {x}:", 1),
            ("spin", "{N} {x} is the kind of name people say VCs overlook. Considering the {N}s listed, where does it actually stand in VC respectability?", 1)]),
        "rounds": ("Rounds raised: how many venture rounds a founder with this name would be expected to raise over a career, judged from the name alone.", [
            ("canonical", "Attribute — {A}\nAmong the {N}s shown, where does {N} {x} stand on this attribute?", 1),
            ("plain", "How many funding rounds would a founder named {N} {x} raise, compared with the other {N}s listed?", 1),
            ("persona", "An investor is guessing, from first names only, which founders on a list will raise the most rounds. Where does {N} {x} land?", 1),
            ("numeric", "Estimate the number of venture rounds a founder with each listed {N} would raise. Relative to the others, how large is {N} {x}'s number?", 1),
            ("casual", "Does {N} {x} sound like someone who keeps raising money? Compared to the other {N}s here, where does it sit?", 1),
            ("negated", "How few rounds would a founder named {N} {x} raise, compared with the {N}s listed? (highest = the fewest rounds)", -1),
            ("formal", "Order the listed given names by the expected count of priced venture rounds raised by a founder bearing each. Position of {N} {x}:", 1),
            ("spin", "Founders named {N} {x} supposedly struggle to raise. Considering the {N}s listed, where does the name actually stand on rounds raised?", 1)]),
        "aura": ("Aura: how much presence, charisma and memorability the name carries on its own.", [
            ("canonical", "Attribute — {A}\nAmong the {N}s shown, where does {N} {x} stand on this attribute?", 1),
            ("plain", "How much aura does {N} {x} have, compared with the other {N}s listed?", 1),
            ("persona", "A casting director is ranking the listed {N}s by sheer presence. Where does {N} {x} land?", 1),
            ("numeric", "Give each {N} listed an aura score. Relative to the others, how high is {N} {x}'s score?", 1),
            ("casual", "Does {N} {x} have aura? Compared to the other {N}s here, where does it sit?", 1),
            ("negated", "How little aura does {N} {x} have, compared with the {N}s listed? (highest = the least aura)", -1),
            ("formal", "Order the listed given names by the charisma, presence and memorability each confers unaided. Position of {N} {x}:", 1),
            ("spin", "{N} {x} is usually called a forgettable name. Considering the {N}s listed, where does it actually stand in aura?", 1)]),
        "gay": ("Sounds gay: how strongly the name alone reads as belonging to a gay man or woman, in contemporary American perception.", [
            ("canonical", "Attribute — {A}\nAmong the {N}s shown, where does {N} {x} stand on this attribute?", 1),
            ("plain", "How gay does {N} {x} sound, compared with the other {N}s listed?", 1),
            ("persona", "Someone guessing sexual orientation from first names alone is ranking the listed {N}s. Where does {N} {x} land?", 1),
            ("numeric", "Give each {N} listed a score for how gay it sounds. Relative to the others, how high is {N} {x}'s score?", 1),
            ("casual", "Does {N} {x} sound gay? Compared to the other {N}s here, where does it sit?", 1),
            ("negated", "How straight does {N} {x} sound, compared with the {N}s listed? (highest = the straightest)", -1),
            ("formal", "Order the listed given names by the strength with which each, unaided, is perceived as a gay person's name in the present-day United States. Position of {N} {x}:", 1),
            ("spin", "{N} {x} is the kind of name nobody would read as gay. Considering the {N}s listed, where does it actually stand?", 1)])},
}
PRICE = 0.042 / 1e6


def z(v): return (v - v.mean()) / v.std()


def load(name):
    if name == "names":
        items = [{"id": x, "text": x} for x in NAMES]; ref = None
        reps = [f"{HERE}/ref/names-r{r}.json" for r in (0, 1)]
        if all(map(os.path.exists, reps)) and not os.path.exists(f"{HERE}/ref-names.json"):
            A, B = (json.load(open(f)) for f in reps); out = {"ref": {}, "replica_agreement": {}}
            for a in A:
                x, y = (np.array([float(R[a][nm]) for nm in NAMES]) for R in (A, B))
                out["replica_agreement"][a] = spearman(x, y); out["ref"][a] = dict(zip(NAMES, ((z(x) + z(y)) / 2).tolist()))
            json.dump(out, open(f"{HERE}/ref-names.json", "w"), indent=0)
        if os.path.exists(f"{HERE}/ref-names.json"):
            r = json.load(open(f"{HERE}/ref-names.json")); ref = {a: np.array([r["ref"][a][x] for x in NAMES]) for a in r["ref"]}
        return items, ref, "name"
    items, ref, ridx, attr, noun = cohort(name); return items, {"population": ref}, noun


def sb(lats):
    """Split-half by parity of the round list, Spearman-Brown corrected."""
    a = z(np.mean([z(l) for l in lats[0::2]], axis=0)); b = z(np.mean([z(l) for l in lats[1::2]], axis=0)); r = spearman(a, b)
    return 2 * r / (1 + r)


def run(name, k):
    items, ref, noun = load(name); n = len(items); trace = f"{HERE}/trace-wordings-{name}.jsonl"; out = {}; tokens = 0
    for attr, (A, W) in ATTRS[name].items():
        ws = [w for w, _, _ in W]
        runs = [(w, tpl, sign, seed) for wi, (w, tpl, sign) in enumerate(W) for seed in (range(8) if w == "canonical" else (0, wi))]
        reqs = []
        for w, tpl, sign, seed in runs:
            rng = random.Random(100 + seed)
            for wi, mem in enumerate(chunks(rng.sample(range(n), n), k)):
                st = "\n\n".join(f"{noun.upper()} {s + 1}:\n{items[i]['text']}" for s, i in enumerate(mem))
                q = {f"rate|{x}": {"type": "score", "instructions": tpl.format(A=A, N=noun, x=x), "criteria": T10} for x in range(1, k + 1)}
                reqs.append({"state": st, "questions": q, "tag": f"{name}|{attr}|{w}|s{seed}|w{wi}", "mem": mem, "run": (w, seed), "sign": sign})
        recs = call_many(reqs, trace); lat = {}
        for (w, tpl, sign, seed) in runs:
            obs = []
            for rq, rc in zip(reqs, recs):
                if rq["run"] == (w, seed):
                    obs += [("item", rq["tag"], o[1], o[2]) for o in observe("rate", rc, rq["mem"])]
            lat[(w, seed)] = sign * fit(obs, n)
        tokens += sum(rc["usage"]["input_tokens"] for rc in recs)
        canon = [lat[("canonical", s)] for s in range(8)]
        same_win = {w: spearman(lat[(w, 0)], canon[0]) for w in ws[1:]}                      # words move, windows fixed
        fresh_win = {w: spearman(lat[(w, wi)], canon[0]) for wi, w in enumerate(ws) if wi}   # words and windows move
        same_word = [spearman(canon[0], canon[s]) for s in range(1, 8)]                       # windows move, words fixed
        diverse = [lat[(w, wi)] for wi, w in enumerate(ws)]; repeated = canon
        pos = [l for wi, l in enumerate(diverse) if W[wi][2] > 0]                            # diverse without the negated wording
        pool = lambda ls: np.mean([z(l) for l in ls], axis=0)
        R = ref[attr] if ref and attr in ref else None
        keys = list(lat)
        out[attr] = {"keys": [f"{w}|s{s}" for w, s in keys], "latent": {f"{w}|s{s}": lat[(w, s)].tolist() for w, s in keys},
                     "same_window": same_win, "fresh_window": fresh_win, "same_wording": same_word,
                     "self_diverse": sb(diverse), "self_diverse_pos": sb(pos), "self_repeated": sb(repeated),
                     "vs_ref": {w: spearman(lat[(w, 0)], R) for w in ws} if R is not None else None,
                     "pooled_diverse_vs_ref": spearman(pool(diverse), R) if R is not None else None,
                     "pooled_diverse_pos_vs_ref": spearman(pool(pos), R) if R is not None else None,
                     "pooled_repeated_vs_ref": spearman(pool(repeated), R) if R is not None else None,
                     "matrix": [[spearman(lat[a], lat[b]) for b in keys] for a in keys]}
        o = out[attr]
        print(f"{name} {attr}: same-wording/fresh-windows {np.mean(same_word):.3f} | same-windows " + " ".join(f"{w} {v:.2f}" for w, v in same_win.items()), flush=True)
        print(f"   fresh-windows " + " ".join(f"{w} {v:.2f}" for w, v in fresh_win.items()) + f" | self: 8 wordings {o['self_diverse']:.3f} (7 positive {o['self_diverse_pos']:.3f}) vs 8 rounds one wording {o['self_repeated']:.3f}", flush=True)
        if R is not None: print(f"   vs ref: " + " ".join(f"{w} {v:.2f}" for w, v in o["vs_ref"].items()) + f" | pooled 8 wordings {o['pooled_diverse_vs_ref']:.3f} (7 positive {o['pooled_diverse_pos_vs_ref']:.3f}) vs 8 rounds {o['pooled_repeated_vs_ref']:.3f}", flush=True)
    out["_"] = {"tokens": tokens, "dollars": tokens * PRICE, "k": k, "n": n}; print(f"{name}: ${tokens * PRICE:.4f}")
    json.dump(out, open(f"{HERE}/wordings-{name}.json", "w"), indent=0)


if __name__ == "__main__":
    run(sys.argv[1], int(sys.argv[2]) if len(sys.argv) > 2 else 24)
