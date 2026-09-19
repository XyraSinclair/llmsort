"""One small Jev step on the hard attributes: python3 step.py <cohort> <variant> [rounds]

Fixed for every variant: the same k=8 windows (seed 7; n=24 -> 3 windows a round), one call per (window, attribute),
three instruments per call -- noul both mention orders, polarity-safe score9 ratio ladder both orders, 10-level rate.
A variant changes only the WORDING of the attribute, so variants are compared on identical states.
Writes obs-<cohort>-<variant>.jsonl and prints rho per attribute against the Fable reference (ref.py).
Key: xyra-vault run ~/x/jev/typed-judgment -- python3 step.py ...   (replay from the trace needs no key)
"""
import json, math, os, random, sys
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, f"{HERE}/../jev-bits-2026-09-19")
from jevclient import call_many, pmf
from run import U9, T10, R9, clip_logit
from analyze import spearman, fit_pairs, fit_items

K = 8
ATTRS = json.load(open(f"{HERE}/attributes.json"))
EXTRA = json.load(open(f"{HERE}/wordings.json")) if os.path.exists(f"{HERE}/wordings.json") else {}


def wording(variant, a):
    if variant == "bare":
        return a["name"]
    if variant == "elab":
        return a["text"]
    return EXTRA[variant][a["name"]]


def windows(n, rounds):
    rng = random.Random(7); out = []
    for r in range(rounds):
        p = list(range(n)); rng.shuffle(p)
        out += [(f"r{r}w{w}", p[w * K:(w + 1) * K]) for w in range(n // K)]
    return out


def build(items, variant, rounds):
    reqs = []
    for wtag, mem in windows(len(items), rounds):
        state = "\n\n".join(f"ITEM {s + 1}:\n{items[i]['text']}" for s, i in enumerate(mem))
        for ai, a in enumerate(ATTRS):
            head = f"Attribute — {wording(variant, a)}\n"; q = {}
            for x in range(1, K + 1):
                q[f"rate|{x}"] = {"type": "score", "instructions": head + f"Among the items shown, where does item {x} stand on this attribute?", "criteria": T10}
                for y in range(1, K + 1):
                    if x != y:
                        q[f"noul|{x}>{y}"] = {"type": "noul", "instructions": head + f"Item {x} is stronger on this attribute than item {y}."}
                        q[f"score9|{x}>{y}"] = {"type": "score", "instructions": head + f"On this attribute, how strong is item {x} relative to item {y}?", "criteria": U9}
            reqs.append({"state": state, "questions": q, "tag": f"{variant}|{wtag}|a{ai}", "mem": mem, "attr": a["name"]})
    return reqs


def main():
    name, variant = sys.argv[1], sys.argv[2]; rounds = int(sys.argv[3]) if len(sys.argv) > 3 else 3
    items = json.load(open(f"{HERE}/{name}.json")); n = len(items)
    meta = json.load(open(f"{HERE}/ref-{name}.json")); ids = meta["ids"]
    assert ids == [it["id"] for it in items]
    reqs = build(items, variant, rounds)
    recs = call_many(reqs, f"{HERE}/trace-{name}.jsonl", workers=12)
    obs, tok, lat = [], 0, []
    for rq, rec in zip(reqs, recs):
        tok += rec["usage"]["input_tokens"]; lat.append(rec["latency_ms"]); mem = rq["mem"]
        for qid, ans in rec["answers"].items():
            ins, rest = qid.split("|")
            if ins == "rate":
                obs.append({"attr": rq["attr"], "instr": ins, "call": rq["tag"], "i": mem[int(rest) - 1], "y": sum(l * p for l, p in enumerate(pmf(ans, 10)))})
            else:
                x, y = (int(v) for v in rest.split(">"))
                val = clip_logit(ans["noul"]) if ins == "noul" else sum(p * math.log(r) for p, r in zip(pmf(ans, 9), R9))
                obs.append({"attr": rq["attr"], "instr": ins, "call": rq["tag"], "i": mem[x - 1], "j": mem[y - 1], "y": val})
    with open(f"{HERE}/obs-{name}-{variant}.jsonl", "w") as f:
        f.writelines(json.dumps(o) + "\n" for o in obs)
    print(f"# {name} · {variant}: {len(reqs)} calls, {tok / 1e6:.2f}M tokens (${tok * 0.042 / 1e6:.3f}), p50 {np.median(lat):.0f} ms")
    print(f"{'attribute':38s} {'fable':>5s} | {'noul':>5s} {'score9':>6s} {'rate':>5s} {'all':>5s}")
    cols = {k: [] for k in ("noul", "score9", "rate", "all")}
    for a in meta["attrs"]:
        ref = np.array([meta["ref"][a][i] for i in ids]); lat_ = {}
        for ins in ("noul", "score9", "rate"):
            rows = [o for o in obs if o["attr"] == a and o["instr"] == ins]
            lat_[ins] = fit_items([(o["call"], o["i"], o["y"]) for o in rows], n) if ins == "rate" else fit_pairs([(o["i"], o["j"], o["y"]) for o in rows], n)
        lat_["all"] = sum((v - v.mean()) / v.std() for v in list(lat_.values()))
        r = {k: spearman(v, ref) for k, v in lat_.items()}
        for k in cols:
            cols[k].append(r[k])
        print(f"{a:38s} {meta['reliability'][a][1]:5.2f} | {r['noul']:5.2f} {r['score9']:6.2f} {r['rate']:5.2f} {r['all']:5.2f}")
    print(f"{'mean':38s} {np.mean([v[1] for v in meta['reliability'].values()]):5.2f} | " + " ".join(f"{np.mean(cols[k]):{w}.2f}" for k, w in (("noul", 5), ("score9", 6), ("rate", 5), ("all", 5))))
    json.dump({a: {k: round(cols[k][i], 3) for k in cols} for i, a in enumerate(meta["attrs"])}, open(f"{HERE}/rho-{name}-{variant}.json", "w"), indent=0)


if __name__ == "__main__":
    main()
