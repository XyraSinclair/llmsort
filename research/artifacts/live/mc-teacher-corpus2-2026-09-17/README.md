# Teacher corpus 2: gemma-4-31b joint setwise latents on the bench's own attribute sets

The second teacher pass, run 2026-09-17 12:41–12:47 PT to give the 0.6B criterion-conditioned
scorer (`../fast-judge-2026-09-17/`) the three attribute triples it had never seen: the
scorer trained on `../mc-teacher-corpus-2026-09-13/` (LessWrong posts, 765 criteria) reached
the teacher's ceiling on the lw triple and collapsed exactly where its training criteria had
no coverage (`concise` in 0 of 765, `civil` in 2). Same judge, same design, same fit as
corpus 1; only the pools and the criteria change.

## What was run

- Pools, bench items excluded, sampled without replacement into 200 lists of 40 per domain:
  - `hn_top` — Hacker News stories with ≥ 50 points, 2026-01-01..2026-09-10, not deleted or
    dead, joined to their earliest top-level comment of ≥ 15 words (12,000 sampled by hash,
    9,758 with a comment). Text: `Title / URL / Points, Comments / Top comment`.
  - `hn_comments` — Hacker News comments of 60–200 words, 2026-06-01..2026-09-10 (8,600).
    Text: the comment.
  - `arxiv` — OpenAlex works indexed in arXiv with abstracts, 2025–2026, computer-science
    field, random sample with a fixed seed (8,551 with an abstract ≥ 300 chars). Text:
    `title\n\nabstract`.
  - Item text truncated to 3,000 chars (means 519 / 591 / 1,393 chars).
- Criteria: the bench's own triples, verbatim from `bench/<domain>.criteria.json` —
  hn_top interesting / credible / actionable; hn_comments informative / civil / concise;
  arxiv novelty / clarity / evidence.
- Judge and design as corpus 1: `google/gemma-4-31b-it` via OpenRouter, temperature 0,
  `--arm joint`, n=40, k=8, overlap 2, 1 round, 2 presentations → 14 calls per list answering
  all three criteria; Huber-IRLS fit with posterior std.

Totals: 600 lists, 8,400 calls, 43 malformed (0.5%; 41 of them on arxiv), 0 errored,
**$8.40** ($2.34 hn_top, $2.30 hn_comments, $3.76 arxiv), 5.6 minutes wall-clock with the
three domains in parallel at 5 lists × concurrency 4 each.

Teacher self-consistency (repeat-presentation flip rate, mean over lists): interesting .15,
credible .17, actionable .11; informative .12, civil .13, concise .16; novelty .15,
clarity .19, evidence .10 — the same band as corpus 1 and as the bench cohorts.

## Files

- `ledger.jsonl.gz` — 72,000 rows, one per (list, item, criterion), same schema as corpus 1:
  `list_id, item_id, criterion_name, criterion_text, latent, std, flip, parsed, judge, seed`.
- `cohorts.json` — the manifest: per list the shard (= domain), seed, criteria with exact
  text, and the 40 item ids.
- `lists.tgz` — the item texts, `lists/<domain>/<list_id>.json` as `[{"id","text"}]`, so the
  corpus retrains anywhere without the pools.
- `scripts/build_corpus2.py` — pools → lists + cohorts (reads the host's ClickHouse mirror of
  Hacker News over loopback and OpenAlex over HTTPS); `scripts/run_corpus2.py` — one teacher
  invocation per list, parallel, resumable; `scripts/flatten.py` — summaries → ledger.
- `runs/teacher-<domain>.log` — the per-list cost/malformed lines; `runs/flatten.log`.
  Per-list `summary-*.json` / `trace-*.jsonl` stay on the judge host (`corpus2/runs/`).

## Reading it

Per domain the three criteria are shared across 8,000 items — a dense block per criterion,
unlike corpus 1's thin rotating triples. Held-out evaluation in `ft_scorer.py` takes 33 lists
per domain (seeded), leaving 501 training lists; the scorer is warm-started from the corpus-1
adapter with 150 corpus-1 lists replayed so the lw triple is not forgotten.
