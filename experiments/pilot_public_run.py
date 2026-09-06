#!/usr/bin/env python3
"""Pilot curated PUBLIC run: gemma4-31b judges one hero axis over the
catalog cohort, landing in scry_judgements.* (public) via cardinald.
Operator decision 2026-09-06: gemma4-31b is the pinned judge for this lane.
Usage: pilot_public_run.py <lens> <axis_key> [requested_k]
"""
import json, sys, urllib.request

CH = "http://127.0.0.1:18123/"
CARDINALD = "http://127.0.0.1:8093"

lens = sys.argv[1]
axis_key = sys.argv[2]
requested_k = int(sys.argv[3]) if len(sys.argv) > 3 else 20
n_entities = int(sys.argv[4]) if len(sys.argv) > 4 else 20

def ch(q):
    req = urllib.request.Request(CH, data=q.encode())
    return urllib.request.urlopen(req, timeout=30).read().decode()

axis_prompt = ch(
    "SELECT axis_prompt FROM scry_judgements_private.catalog_axes "
    f"WHERE lens = '{lens}' AND axis_key = '{axis_key}' "
    "ORDER BY created_at DESC LIMIT 1"
).strip()
assert axis_prompt, f"no axis_prompt for {lens}/{axis_key}"

rows = ch(
    "SELECT entity_id, entity_text FROM scry_judgements_private.catalog_entities "
    f"WHERE lens = '{lens}' ORDER BY rank ASC LIMIT {n_entities} FORMAT JSONEachRow"
).strip().splitlines()
entities = []
for line in rows:
    r = json.loads(line)
    entities.append({"id": r["entity_id"], "text": r["entity_text"]})
assert len(entities) >= 2, f"cohort too small: {len(entities)}"

payload = {
    "entities": entities,
    "axis_key": axis_key,
    "axis_prompt": axis_prompt,
    "requested_k": requested_k,
    "model": "gemma4-31b",
    "provider_base_url": "http://127.0.0.1:8023/v1",
    "privacy": "public",
    "lens": lens,
}
req = urllib.request.Request(
    CARDINALD + "/v1/runs",
    data=json.dumps(payload).encode(),
    headers={"Content-Type": "application/json"},
)
try:
    resp = urllib.request.urlopen(req, timeout=60)
    print(resp.status, resp.read().decode())
except urllib.error.HTTPError as e:
    print("HTTP", e.code, e.read().decode())
