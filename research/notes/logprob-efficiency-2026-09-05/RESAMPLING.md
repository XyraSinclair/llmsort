# Resampling (nonce-draw) efficiency on the logprob rail (2026-09-05)

Companion to FINDINGS.md, same data (72,813 moment-bearing `ratio_letter_v1`
rows, gemma4-12b; dominant design 4 nonce draws × 2 orientations per pair,
single decision token). Question: what does a repeat draw of the same
comparison buy, at its real marginal cost (~0.21 of a fresh call — 79%
prefix hit, prefill-dominated, output ≈ 2 tokens)?

## Q1 — the PMF makes redraws nearly redundant

Per (run, pair, orientation) group with ≥3 draws (11,568 groups):
between-draw sd of the PMF mean μ is **0.0066 nats** median, vs the PMF's
own stated sd **0.080** — ratio ρ = between-draw var / reported var has
median **0.01**. The model's answer distribution barely moves across
nonces; one logprob read already contains ~99% of what repeated draws
reveal.

Equivalent statement of the logprob advantage in resampling currency: a
sampled-token rail draws from that PMF (sd ≈ 0.08), so it would need
(0.080/0.0066)² ≈ **150 sampled draws per pair-orientation** to localize
the answer to the precision one PMF read gives for one call. This is also
why nonce draws — built as a sampling-rail affordance — buy little once
the rail reads logprobs.

## Q2 — the orientation flip is the resample that carries information

For pairs measured in both orientations: median |orientation gap| in
pooled μ is **0.078 nats** (z-score median 9.1; 87% of pairs z > 2) —
~12× the between-nonce scatter, and the same order as the pair
separations themselves on these pools. Position bias, not draw noise, is
the dominant per-call error; counterbalancing is load-bearing.

## Q3/Q4 — design comparison at equal compute, independent reference

Reference = held-out draws (2 per orientation, all pairs); arms sample
disjoint draws. 27 runs × 8 trials, tau vs reference (repeat draw = 0.21
fresh-call units):

| budget | design | tau |
|---|---|---|
| 2P | all P pairs × both orients × 1 draw (wideCB) | **0.686 ± 0.006** |
| 2P | 0.83P pairs × both orients × 2 draws (deep2) | 0.653 ± 0.007 |
| 1.21P | 0.6P pairs × both orients × 1 draw | **0.515 ± 0.008** |
| 1.21P | P pairs × ONE orient × 2 draws (mono2) | 0.385 ± 0.009 |

Coverage beats deepening at equal compute even with redraws priced at
21%; and spending the redraw budget on the orientation flip instead is
strongly better (mono2 is the worst arm despite covering the most pairs).

Marginal value per unit compute: **distinct-pair coverage > orientation
flip > nonce redraw**.

## Implication (finding, not a change)

On the logprob rail, `nonce_draws` > 1 is close to pure cost: at ρ ≈ 0.01
a redraw at 0.21 cost loses to reallocating that budget to counterbalanced
coverage. The nonce-draw mechanism stays right for sampled-token rails
(E8's jitter result — samples from the PMF do vary) and for engines where
draws 2..n are nearly free anyway; for evidence-rail production runs the
budget belongs in pairs and orientations. Operator decision: default
nonce_draws for seriate-logprob-routed lanes.

Caveats: one model/template family; between-draw scatter could grow with
temperature or with models whose PMFs are less stable call-to-call; the
0.21 marginal-cost figure is the measured prefix-hit economics of the
local lane, not a constant.
