"""Flatten runs/<domain>/summary-L####.json into ledger.jsonl (same row shape as the 2026-09-13 corpus)."""
import json, os
coh = json.load(open("cohorts.json")); n = 0; miss = 0
with open("ledger.jsonl", "w") as f:
    for c in coh:
        p = f"runs/{c['shard']}/summary-{c['list_id']}.json"
        if not os.path.exists(p): miss += 1; continue
        s = json.load(open(p)); arm = s["arms"][0]; ids = s["ids"]
        for k in c["criteria"]:
            pc = arm["per_criterion"][k["name"]]
            for i, item in enumerate(ids):
                f.write(json.dumps({"list_id": c["list_id"], "item_id": item, "criterion_name": k["name"], "criterion_text": k["text"],
                                    "latent": pc["scores"][i], "std": pc["std"][i], "flip": pc["flip"], "parsed": pc["parsed"],
                                    "judge": "google/gemma-4-31b-it", "seed": c["seed"]}) + "\n"); n += 1
print(f"ledger rows {n}, lists missing {miss}")
