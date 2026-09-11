# Multi-objective reranking for lists people meet on the web

Date: 2026-09-11. Status: design note + first experiment. Companion to
`signal-feed-design-2026-09-07.md` (feed-side composition) and the scry
extension list sorter (DESIGN.md 2026-09-10, `payload/src/55-sort.js`).

## Where we stand

The extension sorts one criterion per run: hover pill → criterion roster →
setwise instrument in JS (k lettered slots, anchored ring windows, shuffled
repeats → flip-rate gauge, Huber-IRLS fit with posterior σ, brackets within
joint 2σ). Ensembles of judges are combined by mean percentile. Multi-objective
is therefore "run it m times and eyeball" — m× cost, no shared currency, no
gates, no way to see the trade-off.

This library already owns the composition half. `docs/ALGORITHM.md` §
multi-attribute: per-attribute engines, MAD normalization, weights, gates
(`MultiRerankGateSpec {attribute_id, unit, op, threshold}`), a planner over
the combined utility. What it lacks is a cheap *elicitation* half for m
criteria and a contract the extension can call.

Two measured facts bound the design:

- **Set prefix is not free.** NORTH E1: k-wise prefix with a pivot has a halo
  (795/870 ratios < 1, rotation flips 80–93% of pair directions). E10:
  attribute-in-tail saves 29% tokens on long entities and roughly halves
  pairwise truth accuracy. The measured-safe shape is pair-in-prefix,
  attribute-in-tail only where the layout has earned its accuracy back.
- **The extension's instrument is ordinal setwise, not ratio.** It asks for an
  order over k lettered slots and fits from orders; there is no pivot, so E1's
  halo does not apply verbatim. Its flip-rate gauge (✓ < 0.2 / shaky ≥ 0.25)
  is the same reliability primitive as the ledger's order-consistency gate.

## Avenues

### A. Multi-criteria setwise: one call, k entities, m order lines

Prompt = entities block (shared) + m criteria; response = m lines, each a
permutation of the k letters. Cost ≈ one single-criterion call plus m short
output lines; the entities dominate tokens, so marginal criteria are nearly
free. Each line feeds its own per-criterion fit exactly as today.

Research risk — **cross-criterion halo**: when the model emits m orders in one
completion, the second order is anchored on the first (the same mechanism as
E1's pivot halo, now across criteria rather than across slots). Signature:
inter-criterion rank correlation inflates relative to separate-call
elicitation on the same items. This is measurable and is the first
experiment below. Mitigations if it bites: shuffle criterion order per call
(counterbalance, like `swapped`), ask for each order as an independent line
with the criterion restated, or fall back to m calls sharing a cached
entities prefix (E10's caveat applies only when the attribute moves to the
tail of a *pair* prompt; setwise already puts entities first).

### B. One currency, then compose

Per-criterion latent → percentile (or z against a fixed anchor set, per the
signal-feed note) → weighted sum. Extend the roster grammar:

```
sort = novelty:2 rigor:1 price>=p25          # weights + a gate
```

Gates are `criterion op threshold` in percentile or latent units, matching
`MultiRerankGateSpec`. Display: combined order, brackets from the joint σ
(sum of per-criterion posterior variances under the weights), and Pareto
badges for items no other item dominates on every gated criterion. Pareto
alone gives no reading order; the weighted sum gives the order and the badge
says where the trade-off is real.

### C. Cascade for long lists

Above ~60 items, screen with the rerank lane (`/v1/scry/rerank`, embedding
or cross-encoder) per criterion, take the union of per-criterion top bands
plus a random exploration slice, then run A on the union. The random slice
is the recall measurement, not a hedge. This is the validated probe cascade
(random-cohort Spearman 0.74) applied per criterion.

### D. Spend the second round where it changes the answer

After round one, the boundary band (items whose combined bracket straddles
the cut) is small. Round two re-elicits only those items, only on criteria
whose per-item σ is wide. Same logic as the feed note's boundary-band budget
and as `TraitSearchManager`'s challengers.

### E. Library surface

A Rust multi-criteria setwise instrument (`ordinal_set_multi_v1`: entities
first, m criteria, m permutation lines, counterbalanced criterion order)
feeding the existing multi-attribute engines, plus a `MultiRerankRequest`
that already carries attributes and gates. The extension then posts one
request instead of running m sorts, and gets back per-criterion latents,
combined order, brackets, Pareto flags, and flip rate per criterion.

## First experiment (cheap, decisive)

Question: does m-in-one elicitation inflate inter-criterion correlation or
degrade per-criterion accuracy versus m separate calls?

- Items: 3 lists × 40 items — LW random posts (3 axes: novelty, technical
  alpha, rigor), an HN front page (interesting, credible, actionable), a
  product list with price/rating/fit. Same items in both arms.
- Arms: (1) m separate setwise calls per window; (2) one call, m lines,
  criterion order shuffled per call. k = 8, ring windows, 2 shuffled repeats.
- Judge: gemma4-12b ordinal via the judge host (both orders), gemma4-31b as
  the cross-family check on the boundary band.
- Readouts: per-criterion Spearman between arms; inter-criterion Spearman
  within each arm (inflation = arm2 − arm1); flip rate per criterion per arm;
  tokens per pairwise-equivalent observation. Decision rule: adopt A if
  inflation < 0.10 and per-criterion agreement > 0.85 with flip rate
  unchanged; otherwise adopt the cached-prefix m-call fallback.
- Instrument: extend `experiments/examples/setwise_cached.rs` with an ordinal
  answer mode and an m-criteria prompt; run on the judge host under
  `/srv/build/llmsort-bakeoff`, land under
  `research/artifacts/live/multi-criteria-setwise-<date>/RESULTS.md`.

## Handles

- Ledger audit (2026-09-08) fixed coverage collapse and pair over-repeat;
  ordinal switch raised order-correlation 0.29 → 0.70. Refusal (`!`) runs
  31–39% overall and ~100% on some inapplicable cohorts — freelane needs a
  not-applicable admission gate before a criterion consumes budget. The same
  gate belongs in the extension: a criterion that refuses > 40% of windows
  is reported as "not applicable to this list", not sorted.
- Fixed-reference normalization is unvalidated for stationarity (theory
  note 2026-09-06); percentiles within the list are the safe default for the
  extension, anchors for the feed.
