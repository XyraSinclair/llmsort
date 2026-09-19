"""Lever: decompose each abstract attribute into concrete yes/no propositions (decomp.json: 5 each; decomp10.json: 10 each; signs fixed in advance).

python3 decomp.py <cohort> [decomp10.json]   -- same windows as step.py; ONE call per window carries every proposition for
every item (8 items x 60 nouls). Latent = equal-weight mean of signed, within-call-centred logits; nothing is fitted
to the reference. Prints rho beside step.py's elab row, the per-proposition rho, and the z-mean of elab + decomp.
"""
import json, os, sys
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import step as S
from step import call_many, clip_logit, spearman, fit_items, fit_pairs


def main():
    name = sys.argv[1]; rounds = 3; dfile = sys.argv[2] if len(sys.argv) > 2 else "decomp.json"; stem = dfile[:-5]
    items = json.load(open(f"{HERE}/{name}.json")); n = len(items)
    meta = json.load(open(f"{HERE}/ref-{name}.json")); ids = meta["ids"]; D = json.load(open(f"{HERE}/{dfile}"))
    reqs = []
    for wtag, mem in S.windows(n, rounds):
        state = "\n\n".join(f"ITEM {s + 1}:\n{items[i]['text']}" for s, i in enumerate(mem))
        q = {f"{ai}|{pi}|{x}": {"type": "noul", "instructions": p.replace("{x}", f"Item {x}") if p.startswith("{x}") else p.replace("{x}", f"item {x}")}
             for ai, a in enumerate(meta["attrs"]) for pi, (p, _) in enumerate(D[a]) for x in range(1, S.K + 1)}
        reqs.append({"state": state, "questions": q, "tag": f"{stem}|{wtag}", "mem": mem})
    recs = call_many(reqs, f"{HERE}/trace-{name}.jsonl", workers=9)
    tok = sum(r["usage"]["input_tokens"] for r in recs)
    obs = {}  # (attr index, prop index) -> [(call, item, logit)]
    for rq, rec in zip(reqs, recs):
        for qid, ans in rec["answers"].items():
            ai, pi, x = (int(v) for v in qid.split("|"))
            obs.setdefault((ai, pi), []).append((rq["tag"], rq["mem"][x - 1], clip_logit(ans["noul"])))
    elab = [json.loads(l) for l in open(f"{HERE}/obs-{name}-elab.jsonl")]
    print(f"# {name} · {stem}: {len(reqs)} calls, {tok / 1e6:.2f}M tokens (${tok * 0.042 / 1e6:.3f})")
    print(f"{'attribute':38s} {'fable':>5s} | {'elab':>5s} {'decomp':>6s} {'both':>5s} | per-proposition rho (signed)")
    rows = []
    for ai, a in enumerate(meta["attrs"]):
        ref = np.array([meta["ref"][a][i] for i in ids])
        props = [sg * fit_items(obs[(ai, pi)], n) for pi, (_, sg) in enumerate(D[a])]
        dec = sum((p - p.mean()) / p.std() for p in props)
        e = sum((v - v.mean()) / v.std() for v in (
            fit_pairs([(o["i"], o["j"], o["y"]) for o in elab if o["attr"] == a and o["instr"] == ins], n) if ins != "rate" else
            fit_items([(o["call"], o["i"], o["y"]) for o in elab if o["attr"] == a and o["instr"] == ins], n) for ins in ("noul", "score9", "rate")))
        both = (e - e.mean()) / e.std() + (dec - dec.mean()) / dec.std()
        r = (spearman(e, ref), spearman(dec, ref), spearman(both, ref)); rows.append(r)
        print(f"{a:38s} {meta['reliability'][a][1]:5.2f} | {r[0]:5.2f} {r[1]:6.2f} {r[2]:5.2f} | " + " ".join(f"{spearman(p, ref):+.2f}" for p in props))
    m = np.mean(rows, axis=0)
    print(f"{'mean':38s} {'':5s} | {m[0]:5.2f} {m[1]:6.2f} {m[2]:5.2f}")
    json.dump({a: dict(zip(("elab", "decomp", "both"), map(lambda v: round(float(v), 3), r))) for a, r in zip(meta["attrs"], rows)}, open(f"{HERE}/rho-{name}-{stem}.json", "w"), indent=0)


if __name__ == "__main__":
    main()
