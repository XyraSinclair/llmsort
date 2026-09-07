# Signal feed: which latents, how to spend the judge, how to stay stationary

Design note, 2026-09-07. Question: a feed that surfaces the most interesting content by
eliciting a few multi-objective cardinal latents — which qualities, what pipeline, what
first measurement. Sources: my recommendation, an astra consult, and a Claude (opus-4-8) consult, all
grounded against the repo state below. Every claim about the repo cites the file.

## What the repo already settles

- **Cascade recipe stands.** `experiments/PROBE_RESULTS.md`: 4 wordings × N=200 random
  anchors (SB reliability 0.97) → ridge probe on embeddings → corpus score → judge the
  over-sampled top slice (~2.5×). Held-out probe Spearman +0.74 corpus-wide, ~0 inside a
  curated top-200 (range restriction), +0.46 inside probe-top-200. Top-decile recall
  6–8/20 — screening recall is a measured quantity, not an assumption.
- **Judge for bulk passes.** `research/artifacts/live/judge-bakeoff-2026-09-06/RESULTS.md`:
  gemma-4-12b ordinal, entity-ordered, one 5090 — fp8 full 6.6 calls/s @ +0.98, fp8 4K
  chars 11 @ +0.96, NVFP4 4K 20.4 @ +0.93. Fidelity is Spearman vs the same model's
  full-text pack, not vs truth.
- **Attribute-last is a measured loss.** `docs/NORTH.md` E10 (2026-08-29): putting the
  attribute after the entities caches 38.5% of input (−29% cost on long entities) but
  roughly halves truth accuracy and drops paraphrase coherence. The working baseline is
  attribute-first grouped by first entity (cache hit 28%→44%), which
  `src/rerank/multi/orchestrator.rs` already does. My earlier "attribute-as-tail makes
  extra attributes nearly free" claim is retracted; see the open problem below.
- **Composition is per-cohort, so it is non-stationary.** `src/trait_search/solve.rs`
  recomputes each attribute's scale from the cohort just solved. A daily feed would
  re-normalize daily; anchoring is required for a feed, not optional.
- **Taste axes agree less across judges than scale axes.** Two-pole pilot
  (`research/artifacts/live/two-pole-pilot-2026-08-31/RESULTS.md`): cross-judge
  agreement ~.94 where a scale exists vs ~.71 for taste. Prefer latents with a scale.
- **The auditor wording ratified the discovery board** (a-vs-d ρ +0.86, 12/20 overlap),
  so the agent-foundations tilt of the pilot board is real signal; diversity is a reader
  choice layered on top, not a correction of the ranking.

## Latents (the "juiciest qualities")

Three, with the third as a floor rather than a summed objective:

1. **Conceptual novelty** — a new mechanism, distinction, or synthesis relative to a
   *declared* knowledgeable reader (the rubric names the reader's reference knowledge so
   "new" is stationary across days).
2. **Technical alpha** — a concrete measurement, technique, or design insight a strong
   practitioner could not cheaply regenerate. Overlaps novelty; it earns its place only
   if it retrieves worthwhile items novelty misses (measured, below).
3. **Epistemic rigor** — claims proportioned to evidence, alternatives examined,
   uncertainty honest. Used as a *permissive gate*, low enough to admit labeled
   speculation; its job is to keep exciting nonsense off the board, not to rank.

Rubrics adapt from `research/batteries/judge_bakeoff_axes.json`. Each axis is elicited
with the wording family (`docs/NORTH.md`: A, A′, ¬A, both orders, nulls) so reliability
and slot bias are measured per axis, not assumed.

## Pipeline

1. **Anchors.** Per axis: 4 wordings × N=200 fresh random posts (not a curated cohort —
   range restriction kills the probe), gemma-4-12b fp8 at 4K chars (+0.96) for
   probe-training quality. ~6.4K comparisons per axis, ~10 min per axis on one card.
2. **Probe.** One ridge probe per axis on the frozen embedding model. Score the corpus.
3. **Candidate slice = union** of per-axis top slices (over-sample 2.5× each) plus a
   random exploration slice. Never intersect — requiring high probe scores on every
   axis discards complementary discoveries.
4. **Judge the slice** with the NVFP4 judge (20 calls/s, +0.93) for the bulk ordinal
   pass; re-judge the admission boundary with fp8/full-text and a different-family
   check (gemma-31b or qwen3.7-flash), refusals reported with denominators.
5. **Rank.** Rigor gate (~25th percentile of the slice), then equal-weight sum of
   novelty + alpha in **rank/z space on a fixed reference frame** — never raw log-ratio
   magnitudes (`research/artifacts/live/SYNTHESIS-2026-09-06.md` #3 and the stickbreak
   errata: near-greedy PMFs manufacture ±20-nat edges at the clamp; ordinal structure
   survives, magnitudes are artifacts). Diminishing returns on near-duplicate topics and
   MMR-style per-topic caps are applied at render time only — the auditor proved the
   agent-foundations concentration is signal, so the latent is not suppressed. A few
   slots for strong Pareto alternatives (Pareto alone gives no reading order).
6. **Stationarity.** Anchored lineups (`research/notes/elicitation-theory-2026-09-06.md`
   #1): a small fixed topic-spanning anchor set, Latin-square positions, frozen rubric +
   judge + instrument + anchor texts + normalization; replacement anchors bridged through
   overlap; monitor anchor-pair drift and re-scores of unchanged items. The theory note
   proposes this and does not validate it — validation is part of the first run.

Judge budget goes where uncertainty can change an outcome: feed admission, rigor
eligibility, or a diversity substitution. The probe is null inside the elite band
(+0.02–0.07 within curated top-200), so over-sample to probe-top-500 to catch the true
top-100 and spend judge calls on the K/K+1 frontier via the effective-resistance planner
(`docs/ALGORITHM.md`). Everything deep in the accepted or rejected region gets the probe
score only, plus random audits to measure screening misses.

## "Juicy" is a test, not a vibe

An axis earns a slot iff (a) wording/order reliability ≥ ~0.9, (b) cross-family
agreement (gemma × qwen3.7-flash, which retires the OpenAI-family confound) ≥ ~0.7 with
refusal denominators reported, and (c) coverage lift — it retrieves worthwhile items the
other axes miss (top-K set change / partial correlation; the one-factor fit in
`research/notes/elicitation-theory-2026-09-06.md` §1 gives loading = validity and
uniqueness = coverage in one model). Fail (c) and the axis is retired.

## First measurement (< 1 hour on one 5090)

1. **Three-axis anchor run** on 200 fresh random LessWrong posts, disjoint from the
   pilot cohort, every item judged independently of probe selection (4 wordings × 3 axes
   ≈ 19K comparisons, ~20 min). Yields per axis: reliability, slot bias, cross-family
   agreement, and the axis correlation matrix. Compare top-20 feeds under equal judge
   budgets — novelty alone, novelty+alpha, both+rigor gate — and report coverage =
   worthwhile discoveries retrieved ÷ worthwhile discoveries among the 200, precision
   /20, and topic concentration.
2. **Two-ranker cascade on the 44K pool** using those anchors as probe training: score
   the pool per axis, judge each probe-top slice (~3K calls each), then Spearman between
   the novelty and technical-alpha leaderboards on the shared probe-top-500. ρ > 0.8 ⇒
   alpha is redundant, collapse to novelty; 0.4–0.7 ⇒ it is the decorrelator — check
   specifically whether it demotes pure-jargon agent-foundations items. Denominator:
   2 × 6.4K anchor + 2 × ~3K slice ≈ 25K ordinal calls ÷ 20/s ≈ 20 min GPU + ~17 min
   embedding.

## Open problem: multi-attribute at cached-prefix prices

E10 closes plain attribute-last. The untested shape: prefix = system + **all k
attribute rubrics** + entity A + entity B (byte-identical across attributes), tail = one
line selecting which rubric to answer. The judge reads every rubric before the entities
(attention is still attribute-first), while the prefix is shared across all k attributes,
so k attributes cost roughly one prefill plus k short decodes. Bench it on the existing
harness (entity order, 4K chars) against the reference pack; bar ≥ +0.95 on each axis.
Until it passes, attribute-first grouped by first entity stays the baseline.

## What changed from my first answer

- Retracted: attribute-tail as the cheap multi-attribute path (E10 measured the loss).
- Added: rigor as a gate rather than a third summed objective; union-not-intersection of
  probe slices; fixed-reference rank/z normalization replacing per-cohort scaling in
  `solve.rs` for feed use; diversity at render time only; the leaderboard-Spearman
  redundancy test with thresholds; the rubric-family-prefix variant as the actual
  caching experiment.
- Both consults independently reached the same three latents, the same gate-not-rank
  role for rigor, the same E10 correction, and the same anchored-panel stationarity
  design — with the shared caveat that anchored stationarity is a prediction, not yet a
  measured run.
- Kept: novelty + technical alpha as the summed pair; cascade; anchored lineups; judge
  spend concentrated at the admission boundary; the 3-axis anchor run as step one.
