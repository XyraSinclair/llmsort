#!/usr/bin/env python3
"""Wording-family fusion: pairwise agreement matrix among #a-#d N=200 runs,
fused (mean-z) target, and ridge-probe CV against the fused target.
Usage: probe_fuse.py <lens> <axis_base>   e.g. lesswrong-posts novel-world-expanding-hit
"""
import json, sys, urllib.request
import numpy as np

CH = "http://127.0.0.1:18123/"
EMBED = "http://127.0.0.1:8040/embed"
lens, base = sys.argv[1], sys.argv[2]
WORDINGS = ["a", "b", "c", "d"]

def ch(q):
    return urllib.request.urlopen(
        urllib.request.Request(CH, data=q.encode()), timeout=120
    ).read().decode().strip()

# Per wording: latest gemma4-31b run with >=150 entities (the N=200 deep runs).
per = {}
for w in WORDINGS:
    ax = base + "#" + w
    q = (
        "SELECT entity_id, latent_mean, run_id FROM scry_judgements.scores_current "
        "WHERE lens = '" + lens + "' AND axis_key = '" + ax + "' AND model = 'gemma4-31b' "
        "AND run_id IN (SELECT run_id FROM scry_judgements.scores_current "
        "WHERE lens = '" + lens + "' AND axis_key = '" + ax + "' AND model = 'gemma4-31b' "
        "GROUP BY run_id HAVING count() >= 150 "
        "ORDER BY max(loaded_at) DESC LIMIT 1) FORMAT JSONEachRow"
    )
    rows = ch(q).splitlines()
    d = {}
    rid = None
    for r in rows:
        j = json.loads(r)
        d[j["entity_id"]] = float(j["latent_mean"])
        rid = j["run_id"]
    per[w] = d
    print(f"#{w}: {len(d)} entities  run={rid}")

shared = sorted(set.intersection(*[set(per[w]) for w in WORDINGS]))
print(f"shared entities across 4 wordings: {len(shared)}")
assert len(shared) >= 100

def spearman(a, b):
    ra = np.argsort(np.argsort(a)).astype(float)
    rb = np.argsort(np.argsort(b)).astype(float)
    ra -= ra.mean(); rb -= rb.mean()
    return float((ra @ rb) / (np.linalg.norm(ra) * np.linalg.norm(rb) + 1e-12))

Z = {}
for w in WORDINGS:
    v = np.asarray([per[w][i] for i in shared])
    Z[w] = (v - v.mean()) / (v.std() + 1e-12)

print("\npairwise wording agreement (Spearman):")
mat = np.zeros((4, 4))
for i, wi in enumerate(WORDINGS):
    row = []
    for j, wj in enumerate(WORDINGS):
        mat[i, j] = spearman(Z[wi], Z[wj])
        row.append(f"{mat[i, j]:+.3f}")
    print(f"  #{wi}: " + "  ".join(row))
off = mat[np.triu_indices(4, 1)]
print(f"mean off-diagonal: {off.mean():.3f}  (min {off.min():.3f}, max {off.max():.3f})")

# Spearman-Brown: reliability of the 4-wording mean given mean inter-wording r.
rbar = off.mean()
sb = 4 * rbar / (1 + 3 * rbar)
print(f"Spearman-Brown fused-score reliability estimate: {sb:.3f}")

fused = np.mean([Z[w] for w in WORDINGS], axis=0)

# split-half check of the fusion itself: (a+b) vs (c+d)
print(f"split-half (a+b vs c+d) Spearman: {spearman(Z['a']+Z['b'], Z['c']+Z['d']):.3f}")

trows = ch(
    "SELECT entity_id, entity_text FROM scry_judgements_private.catalog_entities "
    "WHERE lens = '" + lens + "' FORMAT JSONEachRow"
).splitlines()
texts = {}
for r in trows:
    j = json.loads(r)
    texts[j["entity_id"]] = j["entity_text"]

kept_idx = [k for k, i in enumerate(shared) if texts.get(i, "").strip()]
kept = [shared[k] for k in kept_idx]
y = fused[kept_idx]
print(f"\ntexts: {len(kept)} entities for probe")

X_list = []
for b in range(0, len(kept), 32):
    batch = [texts[i][:6000] for i in kept[b : b + 32]]
    req = urllib.request.Request(
        EMBED,
        data=json.dumps({"texts": batch}).encode(),
        headers={"Content-Type": "application/json"},
    )
    resp = json.loads(urllib.request.urlopen(req, timeout=300).read())
    X_list.extend(resp["embeddings"])
X = np.asarray(X_list, dtype=np.float64)
X = (X - X.mean(0)) / (X.std(0) + 1e-9)
print(f"embeddings: {X.shape[0]} x {X.shape[1]}")

rng = np.random.default_rng(0)
idx = rng.permutation(len(y))
for lam in (10.0, 100.0, 1000.0, 10000.0):
    preds = np.zeros_like(y)
    for f in range(5):
        test = idx[f::5]
        train = np.setdiff1d(idx, test)
        Xt, yt = X[train], y[train]
        K = Xt @ Xt.T
        alpha = np.linalg.solve(K + lam * np.eye(len(yt)), yt - yt.mean())
        preds[test] = (X[test] @ Xt.T) @ alpha + yt.mean()
    n10 = max(2, len(y) // 10)
    tt = set(np.argsort(-y)[:n10]); pt = set(np.argsort(-preds)[:n10])
    print(
        f"lambda={lam:>7.0f}: held-out Spearman {spearman(preds, y):+.3f}  "
        f"Pearson {float(np.corrcoef(preds, y)[0, 1]):+.3f}  top-decile {len(tt & pt)}/{n10}"
    )
