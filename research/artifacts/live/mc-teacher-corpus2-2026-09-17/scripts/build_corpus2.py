"""corpus2 pools and lists: hn_top / hn_comments from the HN table on the build host (loopback, read-only),
arxiv from OpenAlex (arXiv-indexed CS works 2025-26, random sample). 200 lists x 40 per domain, bench items
excluded, criteria = the bench triples verbatim. Writes lists/<domain>/<L>.json and cohorts.json."""
import json, os, random, re, time, urllib.request, urllib.parse
CH = "http://127.0.0.1:18123/?readonly=1&max_execution_time=300"
def q(sql):
    r = urllib.request.urlopen(urllib.request.Request(CH, data=(sql + " FORMAT JSONEachRow").encode()), timeout=320)
    return [json.loads(l) for l in r.read().decode().splitlines() if l.strip()]
def bench(d): return {it["id"] for it in json.load(open(f"../bench/{d}.json"))}, [{"name": k["name"], "text": k["prompt"]} for k in json.load(open(f"../bench/{d}.criteria.json"))]
pools = {}
# hn_top: front-page-class stories with their earliest substantive top-level comment
st = q("SELECT hn_id, title, outbound_url, upvotes, comment_count FROM hackernews.items WHERE hn_type = 'story' AND upvotes >= 50 "
       "AND original_timestamp >= '2026-01-01' AND original_timestamp < '2026-09-10' AND NOT is_deleted AND NOT dead AND title != '' "
       "ORDER BY cityHash64(hn_id) LIMIT 12000")
ids = ",".join(str(s["hn_id"]) for s in st)
cm = {c["s"]: c["c"] for c in q(f"SELECT parent_hn_id AS s, argMin(payload, original_timestamp) AS c FROM hackernews.items WHERE hn_type = 'comment' "
                                 f"AND parent_hn_id IN ({ids}) AND NOT is_deleted AND NOT dead AND word_count >= 15 GROUP BY s")}
bx, crit_top = bench("hn_top")
pools["hn_top"] = ([{"id": str(s["hn_id"]), "text": f"Title: {s['title']}\nURL: {s['outbound_url'] or ''}\nPoints: {s['upvotes']} Comments: {s['comment_count']}\n\nTop comment: {cm[s['hn_id']]}"}
                    for s in st if s["hn_id"] in cm and str(s["hn_id"]) not in bx], crit_top)
print("hn_top stories", len(st), "with comment", len(cm), "pool", len(pools["hn_top"][0]), flush=True)
# hn_comments: 60-200 word comments, summer 2026, bench day excluded
cs = q("SELECT hn_id, payload FROM hackernews.items WHERE hn_type = 'comment' AND word_count BETWEEN 60 AND 200 AND original_timestamp >= '2026-06-01' "
       "AND original_timestamp < '2026-09-10' AND toDate(original_timestamp) != '2026-08-24' AND NOT is_deleted AND NOT dead ORDER BY cityHash64(hn_id) LIMIT 8600")
bx, crit_c = bench("hn_comments")
pools["hn_comments"] = ([{"id": str(c["hn_id"]), "text": c["payload"]} for c in cs if str(c["hn_id"]) not in bx and len(c["payload"]) > 200], crit_c)
print("hn_comments pool", len(pools["hn_comments"][0]), flush=True)
# arxiv: OpenAlex random sample of arXiv-indexed CS works with abstracts
bx, crit_a = bench("arxiv"); ax = []
for page in range(1, 44):
    u = ("https://api.openalex.org/works?filter=indexed_in:arxiv,has_abstract:true,publication_year:2025|2026,primary_topic.field.id:fields/17"
         f"&sample=8600&seed=17&per_page=200&page={page}&select=id,title,abstract_inverted_index&mailto=xyra@datasells.ai")
    for attempt in range(4):
        try: d = json.load(urllib.request.urlopen(urllib.request.Request(u, headers={"User-Agent": "llmsort-corpus2"}), timeout=60)); break
        except Exception as e: print("openalex retry", page, e, flush=True); time.sleep(3 * (attempt + 1))
    for w in d["results"]:
        inv = w.get("abstract_inverted_index") or {}; pos = {}
        for tok, ps in inv.items():
            for p in ps: pos[p] = tok
        ab = " ".join(pos[i] for i in sorted(pos)); wid = w["id"].rsplit("/", 1)[-1]
        if len(ab) >= 300 and w.get("title") and wid not in bx: ax.append({"id": wid, "text": f"{w['title']}\n\n{ab}"})
    if not d["results"]: break
    time.sleep(0.15)
pools["arxiv"] = (ax, crit_a); print("arxiv pool", len(ax), flush=True)
# lists + manifest
coh = []; pref = {"hn_top": "T", "hn_comments": "C", "arxiv": "A"}
for k, (d, (pool, crit)) in enumerate(pools.items()):
    rng = random.Random(2026 + k); seen = set(); pool = [p for p in pool if not (p["id"] in seen or seen.add(p["id"]))]; rng.shuffle(pool)
    assert len(pool) >= 8000, (d, len(pool)); os.makedirs(f"lists/{d}", exist_ok=True)
    for i in range(200):
        items = pool[i * 40:(i + 1) * 40]; L = f"{pref[d]}{i:04d}"
        json.dump(items, open(f"lists/{d}/{L}.json", "w"))
        coh.append({"list_id": L, "shard": d, "seed": i + 1, "criteria_source": d, "criteria": crit, "ids": [it["id"] for it in items]})
json.dump(coh, open("cohorts.json", "w"))
print("lists", len(coh), {d: sum(c["shard"] == d for c in coh) for d in pools}, "mean chars", {d: int(sum(len(p["text"]) for p in pools[d][0][:8000]) / 8000) for d in pools})
