# Teacher corpus: gemma-4-31b joint setwise latents over 1,000 LessWrong cohorts

The training set for everything downstream of
`research/artifacts/live/multi-criteria-setwise-2026-09-11/RESULTS.md` (criterion-as-input
reranker, joint-safety distillation, permutation-invariance RL). One judge, one design, many
criteria: per-item, per-criterion latent positions with posterior std, produced by the joint
m=3 setwise call that RESULTS.md certified for this judge.

## What was run

- Pool: ~44K LessWrong posts (word_count ≥ 100) from the ledger database on the build host,
  sampled without replacement into 1,000 lists of 40 (40,000 distinct posts). Item text is
  `Title: …\n\n<body>` truncated to 3,000 chars.
- Judge: `google/gemma-4-31b-it` via OpenRouter, temperature 0, `--arm joint` only
  (RESULTS.md: 0.36–0.40× the cost of separate calls, halo +0.001 on orthogonal criteria).
- Design: n=40, k=8, overlap 2, 1 round, 2 presentations per window (window order, then a
  shuffle) → 7 windows × 2 = 14 calls per list, each answering all three criteria.
- Criteria, m=3 per list, order shuffled per call:
  - 500 lists: the LW battery triple — novelty / alpha / rigor (wording "a" from
    `research/batteries/judge_bakeoff_axes.json`).
  - 250 lists: rotating triples from `research/batteries/highdim_attributes_elaborated.txt`.
  - 250 lists: rotating triples from `research/batteries/fable_subtle_1000_elaborated.txt`.
  - 765 distinct criteria in total; exact text per list in `cohorts.json`.
- Fit: Huber-IRLS on all ordered pairs of each answer (ln 2.78 per pair), ridge 1e-3, posterior
  std from the Hessian; the same fit as the harness and the extension.

Totals: 1,000 lists, 14,000 calls, 13,562 parsed, 438 malformed (3.1%, dropped from the fit —
a list keeps its other 13 windows), 0 errored, **$8.17**, run in 8 shards × 2 sub-shards at
concurrency 4 on the build host (CPU only; no GPU was used).

## Files

- `ledger.jsonl.gz` — 120,000 rows, one per (list, item, criterion):
  `list_id, item_id, criterion_name, criterion_text, latent, std, flip, parsed, judge, seed`.
  `latent` is the fitted position within its list (zero-mean per list per criterion; comparable
  within a list, not across lists); `std` its posterior std; `flip` the list-level
  repeat-presentation disagreement for that criterion; `parsed` the number of windows (of 14)
  that parsed.
- `cohorts.json` — the manifest: per list the shard, seed, criteria source, and the three
  criteria with exact text and the 40 item ids.
- Kept on the build host only (reproducible from `cohorts.json` + the pool; 180 MB):
  `lists/` (the item texts), `runs/` (per-list `summary-L####.json` and `trace-L####.jsonl`
  with every raw answer), `shards/` (pre-merge ledgers).

## Regenerate

`experiments/examples/multi_criteria_setwise.rs --arm joint --k 8 --overlap 2 --rounds 1
--repeats 2 --model google/gemma-4-31b-it --base-url https://openrouter.ai/api/v1
--api-key-env OPENROUTER_API_KEY --items lists/<shard>/<list_id>.json --criteria "<from
cohorts.json>" --seed <from cohorts.json> --out runs/<shard>`, one invocation per list; flatten
`summary-<list_id>.json` (`arms[0].per_criterion[name].scores/std`) into ledger rows.

## Reading it

Within-list latents are what a reranker trains on: for a criterion, pairs of items in the same
list with |Δlatent| well above their stds are confident teacher preferences; pairs inside the
noise are not labels. The 500 LW-battery lists share three criteria across 20,000 items, which
is the dense block for a per-criterion probe; the 500 rotating-triple lists spread 762 further
criteria thinly (40 items each), which is the block for a criterion-as-input model to be tested
on held-out criteria.
