#!/usr/bin/env python3
"""Distillation cascade for judged axes (runs on the judging host).

fuse      <anchor_lens> <axis_base>            reliability matrix + probe CV
propagate <anchor_lens> <axis_base> <out_lens> train on fused anchors, score
                                               the LW post pool into
                                               scry_judgements_private.probe_scores
                                               (resumable; banked per batch)

Anchor protocol: 4 wording variants (#a-#d) each judged at N=200 on the
anchor lens; fused target = mean of per-wording z-scores.
NOTE: split ClickHouse JSONEachRow on "\n" only — str.splitlines() also
splits on U+2028/U+2029, which are legal unescaped inside JSON strings.
"""
import json, sys, time, urllib.request
from urllib.parse import quote
import numpy as np

CH = "http://127.0.0.1:18123/"
EMBED = "http://127.0.0.1:8040/embed"
MODEL_OUT = "voyage4nano-ridge-v1"
LAM = 1000.0
WORDINGS = ["a", "b", "c", "d"]


def ch(q, data=None):
    body = (q if data is None else data).encode()
    url = CH if data is None else CH + "?query=" + quote(q)
    return urllib.request.urlopen(
        urllib.request.Request(url, data=body), timeout=300
    ).read().decode().strip()


def jrows(resp):
    return [json.loads(r) for r in resp.split("\n") if r.strip()]


def embed(texts):
    req = urllib.request.Request(
        EMBED,
        data=json.dumps({"texts": [t[:6000] for t in texts]}).encode(),
        headers={"Content-Type": "application/json"},
    )
    return json.loads(urllib.request.urlopen(req, timeout=600).read())["embeddings"]


def embed_all(texts, bs=32):
    out = []
    for b in range(0, len(texts), bs):
        out.extend(embed(texts[b : b + bs]))
    return np.asarray(out, dtype=np.float64)


def spearman(a, b):
    ra = np.argsort(np.argsort(a)).astype(float)
    rb = np.argsort(np.argsort(b)).astype(float)
    ra -= ra.mean(); rb -= rb.mean()
    return float((ra @ rb) / (np.linalg.norm(ra) * np.linalg.norm(rb) + 1e-12))


def fused_anchors(lens, base, min_n=150):
    """Latest big judge run per wording; returns (ids, fused_z, per-wording Z)."""
    per = {}
    for w in WORDINGS:
        ax = f"{base}#{w}"
        rows = jrows(ch(
            "SELECT entity_id, latent_mean, run_id FROM scry_judgements.scores_current "
            f"WHERE lens = '{lens}' AND axis_key = '{ax}' AND model = 'gemma4-31b' "
            "AND run_id IN (SELECT run_id FROM scry_judgements.scores_current "
            f"WHERE lens = '{lens}' AND axis_key = '{ax}' AND model = 'gemma4-31b' "
            f"GROUP BY run_id HAVING count() >= {min_n} "
            "ORDER BY max(loaded_at) DESC LIMIT 1) FORMAT JSONEachRow"
        ))
        per[w] = {r["entity_id"]: float(r["latent_mean"]) for r in rows}
        rid = rows[0]["run_id"] if rows else None
        print(f"#{w}: {len(per[w])} entities  run={rid}")
    shared = sorted(set.intersection(*[set(per[w]) for w in WORDINGS]))
    assert len(shared) >= min_n, f"shared anchors: {len(shared)}"
    Z = {}
    for w in WORDINGS:
        v = np.asarray([per[w][i] for i in shared])
        Z[w] = (v - v.mean()) / (v.std() + 1e-12)
    return shared, np.mean([Z[w] for w in WORDINGS], axis=0), Z


def anchor_texts(lens):
    return {r["entity_id"]: r["entity_text"] for r in jrows(ch(
        "SELECT entity_id, entity_text FROM scry_judgements_private.catalog_entities "
        f"WHERE lens = '{lens}' FORMAT JSONEachRow"
    ))}


def train_matrix(lens, base):
    shared, fused, Z = fused_anchors(lens, base)
    texts = anchor_texts(lens)
    kept = [i for i in shared if texts.get(i, "").strip()]
    y = np.asarray([fused[shared.index(i)] for i in kept])
    X = embed_all([texts[i] for i in kept])
    print(f"anchors with text: {len(kept)}, embeddings {X.shape[0]} x {X.shape[1]}")
    return X, y, Z, kept


def cmd_fuse(lens, base):
    X, y, Z, _ = train_matrix(lens, base)
    print("\npairwise wording agreement (Spearman):")
    mat = np.zeros((4, 4))
    for i, wi in enumerate(WORDINGS):
        for j, wj in enumerate(WORDINGS):
            mat[i, j] = spearman(Z[wi], Z[wj])
        print(f"  #{wi}: " + "  ".join(f"{mat[i, j]:+.3f}" for j in range(4)))
    off = mat[np.triu_indices(4, 1)]
    rbar = off.mean()
    print(f"mean off-diagonal: {rbar:.3f}  (min {off.min():.3f}, max {off.max():.3f})")
    print(f"Spearman-Brown fused reliability: {4 * rbar / (1 + 3 * rbar):.3f}")
    print(f"split-half (a+b vs c+d): {spearman(Z['a'] + Z['b'], Z['c'] + Z['d']):.3f}")
    Xs = (X - X.mean(0)) / (X.std(0) + 1e-9)
    rng = np.random.default_rng(0)
    idx = rng.permutation(len(y))
    for lam in (10.0, 100.0, 1000.0, 10000.0):
        preds = np.zeros_like(y)
        for f in range(5):
            test = idx[f::5]
            train = np.setdiff1d(idx, test)
            Xt, yt = Xs[train], y[train]
            alpha = np.linalg.solve(Xt @ Xt.T + lam * np.eye(len(yt)), yt - yt.mean())
            preds[test] = (Xs[test] @ Xt.T) @ alpha + yt.mean()
        n10 = max(2, len(y) // 10)
        tt = set(np.argsort(-y)[:n10]); pt = set(np.argsort(-preds)[:n10])
        print(f"lambda={lam:>7.0f}: held-out Spearman {spearman(preds, y):+.3f}  "
              f"Pearson {float(np.corrcoef(preds, y)[0, 1]):+.3f}  top-decile {len(tt & pt)}/{n10}")


def cmd_propagate(lens, base, out_lens):
    ch("CREATE TABLE IF NOT EXISTS scry_judgements_private.probe_scores ("
       "lens String, axis_key String, entity_id String, score Float64, "
       "model String, created_at DateTime64(3) DEFAULT now64(3)) "
       "ENGINE = ReplacingMergeTree ORDER BY (lens, axis_key, entity_id, model)")
    X, y, _, _ = train_matrix(lens, base)
    mu, sd = X.mean(0), X.std(0) + 1e-9
    Xs = (X - mu) / sd
    alpha = np.linalg.solve(Xs @ Xs.T + LAM * np.eye(len(y)), y - y.mean())
    ymean = y.mean()
    done = set(ch(
        "SELECT entity_id FROM scry_judgements_private.probe_scores "
        f"WHERE lens = '{out_lens}' AND axis_key = '{base}' AND model = '{MODEL_OUT}' "
        "FORMAT TSV").split("\n"))
    pool = ch(
        "SELECT post_key FROM (SELECT post_key FROM forums.posts "
        "WHERE site_key = 'lesswrong' AND kind = 'post' AND word_count >= 100 "
        "ORDER BY post_key LIMIT 1 BY post_key) FORMAT TSV").split("\n")
    todo = [p for p in pool if p and p not in done]
    print(f"pool {len(pool)}, already scored {len(done)}, todo {len(todo)}", flush=True)
    t0 = time.time()
    for b in range(0, len(todo), 256):
        keys = todo[b : b + 256]
        in_list = ",".join("'" + k.replace("'", "\\'") + "'" for k in keys)
        kv = [(r["post_key"], r["t"]) for r in jrows(ch(
            "SELECT post_key, concat(coalesce(title,''), '\\n\\n', payload) AS t "
            f"FROM forums.posts WHERE post_key IN ({in_list}) "
            "ORDER BY post_key LIMIT 1 BY post_key FORMAT JSONEachRow"))
            if r["t"].strip()]
        E = (embed_all([t for _, t in kv]) - mu) / sd
        scores = (E @ Xs.T) @ alpha + ymean
        ch("INSERT INTO scry_judgements_private.probe_scores "
           "(lens, axis_key, entity_id, score, model) FORMAT JSONEachRow",
           data="\n".join(
               json.dumps({"lens": out_lens, "axis_key": base, "entity_id": k,
                           "score": float(s), "model": MODEL_OUT})
               for (k, _), s in zip(kv, scores)))
        n = b + len(keys)
        print(f"{n}/{len(todo)} scored ({n / (time.time() - t0 + 1e-9):.0f}/s)", flush=True)
    print("DONE", flush=True)


if __name__ == "__main__":
    cmd = sys.argv[1]
    if cmd == "fuse":
        cmd_fuse(sys.argv[2], sys.argv[3])
    elif cmd == "propagate":
        cmd_propagate(sys.argv[2], sys.argv[3], sys.argv[4])
    else:
        sys.exit(__doc__)
