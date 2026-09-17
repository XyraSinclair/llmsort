"""Zero-shot fast judge: the live Qwen3 rerankers (typed-judgment `judge`) as a criterion-conditioned pointwise
scorer, scored against gemma-4-31b's teacher latents on a fixed stratified sample of corpus lists (eval99, seed 2026:
33 lists per criteria source). Usage: zs_judge.py <tier> <out.json> [n_per_source]"""
import json, sys, random, time, collections, numpy as np
sys.path.insert(0, "/home/scry1/typed-judgment")
from judge import judge_many
C = "/srv/build/llmsort-bakeoff/research/artifacts/live/mc-teacher-corpus-2026-09-13"
tier, out = sys.argv[1], sys.argv[2]; per = int(sys.argv[3]) if len(sys.argv) > 3 else 33

def eval_lists(per):
    coh = json.load(open(C + "/cohorts.json")); by = collections.defaultdict(list)
    for c in coh: by[c["criteria_source"]].append(c)
    rng = random.Random(2026); sel = []
    for s in ("lw", "highdim_elaborated", "fable_subtle_1000_elaborated"): sel += rng.sample(by[s], 33)[:per]
    return sel

def spearman(a, b):
    ra = np.argsort(np.argsort(a)); rb = np.argsort(np.argsort(b)); return float(np.corrcoef(ra, rb)[0, 1])

sel = eval_lists(per); want = {c["list_id"] for c in sel}
lat = collections.defaultdict(dict)
for line in open(C + "/ledger.jsonl"):
    r = json.loads(line)
    if r["list_id"] in want: lat[(r["list_id"], r["criterion_name"])][r["item_id"]] = r["latent"]
reqs = []
for c in sel:
    items = json.load(open(f"{C}/lists/{c['shard']}/{c['list_id']}.json"))
    for it in items:
        reqs.append({"id": f"{c['list_id']}\t{it['id']}", "state": it["text"][:3000], "tier": tier,
                     "questions": [{"id": cc["name"], "type": "noul", "text": f"Attribute: {cc['text']}\n\nThe text above rates high on this attribute."} for cc in c["criteria"]]})
t0 = time.time(); resps = judge_many(reqs); dt = time.time() - t0
score = collections.defaultdict(dict)
for rq, rs in zip(reqs, resps):
    lid, iid = rq["id"].split("\t")
    for a in rs["answers"]: score[(lid, a["id"])][iid] = a["logit"]
rows = []; src = {c["list_id"]: c["criteria_source"] for c in sel}
for (lid, cn), sc in score.items():
    ids = [i for i in sc if i in lat[(lid, cn)]]
    rows.append({"list": lid, "criterion": cn, "source": src[lid], "n": len(ids),
                 "rho": spearman(np.array([sc[i] for i in ids]), np.array([lat[(lid, cn)][i] for i in ids]))})
struct = []
for c in sel:
    names = [cc["name"] for cc in c["criteria"]]; lid = c["list_id"]
    for i in range(3):
        for j in range(i + 1, 3):
            ids = sorted(set(score[(lid, names[i])]) & set(lat[(lid, names[j])]))
            sj = spearman(np.array([score[(lid, names[i])][x] for x in ids]), np.array([score[(lid, names[j])][x] for x in ids]))
            tj = spearman(np.array([lat[(lid, names[i])][x] for x in ids]), np.array([lat[(lid, names[j])][x] for x in ids]))
            struct.append({"list": lid, "source": src[lid], "pair": [names[i], names[j]], "judge": sj, "teacher": tj})
summ = {"tier": tier, "lists": len(sel), "slots": 3 * len(reqs), "seconds": dt, "slots_per_s": 3 * len(reqs) / dt,
        "rho_mean": float(np.mean([r["rho"] for r in rows])),
        "rho_by_source": {s: float(np.mean([r["rho"] for r in rows if r["source"] == s])) for s in sorted(set(src.values()))},
        "struct_absdiff_mean": float(np.mean([abs(x["judge"] - x["teacher"]) for x in struct])),
        "struct_corr": float(np.corrcoef([x["judge"] for x in struct], [x["teacher"] for x in struct])[0, 1])}
json.dump({"summary": summ, "rows": rows, "struct": struct}, open(out, "w"), indent=1)
print(json.dumps(summ))
