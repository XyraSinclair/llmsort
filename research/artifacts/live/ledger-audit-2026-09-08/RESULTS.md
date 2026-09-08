# Judgement ledger audit — are we sorting, or spinning? (2026-09-08)

Question from Xyra: are judgements robustly landing in a table, and are we getting
clearer about signal vs noise? Audited `scry_judgements_private.comparisons` (the
private ledger every cardinald run lands into) over 2026-09-05..08 and fixed the
two things it exposed. All numbers below are from the ledger itself, queried live.

## The store is sound

1.16M comparisons since 2026-07-28, 25 models, 962 axes, 17 lenses. Every row carries
model, template slug + hash, axis prompt hash, entity hashes, presentation order,
refusal/error, ratio, PMF log-ratio mean/var, token counts, cost, and a digest into
`rendered_prompts` holding the exact bytes. Both presentation orders are always asked.
Nothing about the storage needed fixing. What the rows *contained* did.

## Finding 1 — the planner was re-asking the same few pairs (fixed, deployed)

Per run (item_count 20, budget 640; 929 runs in 3 days): median **7 of 20 entities
ever compared**, pair coverage 8–26%, single pairs asked **80–136×** with the same
answer every time, essentially every run ending `budget_exhausted`. ~80% of 1.14M rows
in three days were repeats. Cause: the portable run set `max_pair_repeats: None`, and
the top-k frontier planner keeps proposing the tied boundary pair — a deterministic
judge cannot resolve a tie by repetition, and once the frontier was the only thing it
knew, it never touched the rest of the list.

Reproduced without an LLM (`experiments/examples/planner_coverage_repro.rs`, wiremock
deterministic judge, exact cardinald request shape):

| n / k / draws | pairs asked | max asks per cell | Spearman vs truth | stop |
|---|---|---|---|---|
| 20/10/4 before | 43 / 190 | 136 | +0.983 | budget |
| 20/10/4 after  | **80 / 190** | **4** | **+1.000** | budget |
| 60/10/4 before | 178 / 1770 | 60 | +0.980 | budget |
| 60/10/4 after  | **240 / 1770** | **4** | **+0.986** | budget |

Fix (commit e996766): one counterbalanced round per pair (`max_pair_repeats =
2 × nonce_draws`), and when the frontier is refused/capped out the orchestrator spends
the remaining budget on unasked rank-neighbour pairs (stride 1, then wider) — the
sorting-native use of leftover budget. 80/190 and 240/1770 are exactly the documented
"~4n distinct pairs" the 8n budget was designed for.

Live after deploy (10:56 UTC), per hour of runs:

| window | entity coverage | pair coverage | asks per cell |
|---|---|---|---|
| 02:00–10:00 UTC (before) | 0.78–0.93 | 0.21–0.32 | 19–31 |
| 11:00–14:00 UTC (after) | **0.99–1.00** | **0.39–0.42** | **8** (= 2 orders × 4 draws) |

## Finding 2 — the local judges were on the wrong instrument (fixed, deployed)

Order-consistency of the signed judgement across the two presentation orders
(same pair, same axis, same model), last 4 days:

| instrument × model | pairs | order corr | same direction | decisive pairs |
|---|---|---|---|---|
| ratio_letter_v1 × gemma4-12b | 28,870 | **−0.54** | **19%** | 0.7% |
| ratio_letter_v1 × gemma4-31b | 34,143 | 0.29 | 55% | 1.5% |
| canonical_v2 × gemma4-12b | 13,167 | 0.61 | 64% | 30% |
| canonical_v2 × gemma4-31b | 7,308 | **0.77** | **79%** | 76% |

Chance is 50%. The ratio-letter read on the local gemma judges is position-driven noise
(12b anti-correlates with itself across orders), and ~700K of the 3-day rows were on
that cell — the judge-bakeoff already found the ratio ladder unreadable below ~30B and
picked `ordinal_letter_v1` for gemma. Cause: `default_template_slug` sends every
logprob-route model to ratio-letter.

Fix (commit 666815c): the route can carry an operator-measured default instrument —
`CARDINAL_SERIATE_LOGPROB_MODELS=gemma4-12b:20:ordinal,gemma4-31b:20:ordinal` — and the
env on the judge host now says so. Ratio-letter remains the default for API judges
where E9 measured it.

## What "clear about signal and noise" looks like from here

The two order-consistency queries above are the standing instrument. They should be
run per (instrument, model, lens) before any cell is allowed to consume budget, and an
axis earns a place only with the "juicy" test in
`research/notes/signal-feed-design-2026-09-07.md`: wording-family reliability ≥ ~0.9,
cross-family agreement ≥ ~0.7 with refusal denominators, coverage lift.

## Files

- `experiments/examples/planner_coverage_repro.rs` — the no-LLM reproduction.
- Queries used: `/tmp/consist.sql`, `/tmp/lrm.sql`, `/tmp/cov.sql`, `/tmp/after.sql`
  on the judge host (ad hoc; the shapes are reproduced in the tables above).

## Early verdict on the ordinal switch (first 3 runs, 704 rows, 15:08 UTC)

Same judge (gemma4-31b), same lens (lesswrong-posts), ordinal_letter_v1:

| measure | ratio_letter_v1 (4 days) | ordinal_letter_v1 (first 62 both-order pairs) |
|---|---|---|
| same direction across presentation orders | 55% | **93.5%** |
| order correlation of the signed read | 0.29 | **0.905** |
| slot A rate | 36% | 31–41% (B 25–52%) — balanced |

Refusals are the `!` "not applicable / undecidable" token, not ties (`=` was never
chosen), and they are axis-shaped: 34–44% on `thought-experiment-rigor` (most posts
contain no thought experiment) vs 7% on `theory-ladenness-awareness`. A refused pair
is never re-asked, so that budget is spent honestly rather than on a coin flip. Keep
watching the per-axis refusal denominator; the consistency bar (≥ 0.7) is cleared.
