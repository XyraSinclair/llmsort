"""One-off provenance for lw.json: 24 recent public LessWrong comments, 600-2,500 characters of markdown, sampled with
seed 19 from the 400 most recent (fetched 2026-09-19 15:01 PT via the public GraphQL endpoint). Rerunning fetches a
different window, so lw.json is the frozen cohort; this file records how it was made. Usernames are not kept."""
import json, os, random, urllib.request
HERE = os.path.dirname(os.path.abspath(__file__))
q = '{ comments(input:{terms:{view:"recentComments",limit:400}}){ results{ _id postedAt baseScore contents{markdown} post{title} } } }'
req = urllib.request.Request("https://www.lesswrong.com/graphql", data=json.dumps({"query": q}).encode(),
                             headers={"Content-Type": "application/json", "User-Agent": "Mozilla/5.0 llmsort-research"})
res = json.load(urllib.request.urlopen(req, timeout=60))["data"]["comments"]["results"]
ok = [c for c in res if c.get("contents") and c.get("post") and 600 <= len(c["contents"]["markdown"] or "") <= 2500]
pick = random.Random(19).sample(sorted(ok, key=lambda c: c["_id"]), 24)
json.dump([{"id": c["_id"], "text": f"COMMENT ON THE POST: {c['post']['title']}\n\n{c['contents']['markdown'].strip()}"} for c in pick],
          open(f"{HERE}/lw.json", "w"), indent=1)
