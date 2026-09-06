# How much sorting efficiency do logprobs buy? (2026-09-05)

Question (operator): quantify the efficiency the logprob PMF rail adds over
verdict-only sorting, in rank-reversal terms.

## Design — within-call ablation, no elicitation confound

E9's head-to-head (23× stat error at equal cost) compared two *templates*
(ratio_letter_v1 vs canonical_v2), so prompt and readout were confounded.
Here the confound is removed: production `ratio_letter_v1` rows persist both
the PMF moments (`log_ratio_mean`, `log_ratio_var`) and the plain verdict
from the *same call*, so three readouts of identical calls can be solved and
compared:

- **M** — moments: (μ, v) precision-weighted (production path);
- **P** — point: μ with uniform weights (magnitude, no variance channel);
- **S** — sign: sign(μ) at fixed magnitude (pure binary comparator).

P and S are *generous* to the no-logprob rails: a real sampled-token rail
gets one draw from the PMF, strictly noisier than the PMF mean μ used here.
Measured gaps are therefore lower bounds.

Data: the production judgement ledger (`comparisons` table), template
`ratio_letter_v1`, gemma4-12b evidence lane, 72,813 moment-bearing rows,
37 runs used (n ≥ 12 entities, ≥ 1,000 comparisons; mostly 60-entity
LessWrong pools with nonce draws). Solver: ridge-gauged WLS on
μ ~ s_a − s_b (scripts in this dir; data replayable from the ledger).

Metrics: split-half Kendall τ (rank agreement of two independent halves —
arm-neutral reproducibility; pairwise reversal rate ≈ (1−τ)/2) over
10 shuffles × 37 runs, at budget fractions of the half-run.

## Measured

Split-half τ vs budget fraction:

| frac | M | P | S |
|---|---|---|---|
| 0.05 | 0.053 | 0.046 | 0.032 |
| 0.10 | 0.062 | 0.059 | 0.037 |
| 0.20 | 0.118 | 0.108 | 0.078 |
| 0.30 | 0.196 | 0.167 | 0.125 |
| 0.50 | 0.322 | 0.249 | 0.156 |
| 0.70 | 0.409 | 0.313 | 0.178 |
| 1.00 | 0.494 | 0.355 | 0.162 |

Pairwise reversal rate at full half-budget (~960 comparisons, from the
full-budget split-half run): M 0.24, P 0.30, S 0.39.

Budget multipliers (τ-matched, interpolated): at low budgets **S needs
1.4–2.8× the comparisons of M**; P needs 1.1–1.5×. Beyond ~30% of the
production budget the multiplier is **infinite**: the sign channel
*saturates* at τ ≈ 0.17 while M is still climbing at 0.49 — no binary
budget in the observed range reaches the moment rail's rank stability.
The point channel saturates too (τ ≈ 0.36), later and higher.

Held-out raw draw sign accuracy is flat across arms (~0.559 everywhere):
these pools are dominated by near-tie pairs (|μ| ~ 0.02–0.06 nats), so
single-draw direction is close to a coin flip for every arm and has no
resolution as a metric. That near-tie regime is exactly *why* the sign
channel saturates: sign-only observations of a 0.02-nat separation carry
almost no information at any budget, while the PMF's magnitude + variance
still resolve it by averaging calibrated small readings.

## Reading

Two regimes, one sentence each:

1. **Small budgets:** logprob moments ≈ 1.5–3× fewer comparisons than a
   binary comparator for the same rank stability.
2. **Production budgets on near-tie pools:** the binary rail cannot match
   the moment rail at any budget — its rank-stability ceiling is ~3× lower
   (τ 0.16 vs 0.49, reversal rate 0.39 vs 0.24).

The variance channel specifically (M over P) is worth ~1.4× at production
budget and raises the ceiling ~0.14 τ; the magnitude channel (P over S) is
the larger part of the gap on these pools.

## Honest flags

- Reproducibility ≠ external truth; a shared systematic bias reproduces.
  But all arms read the *same calls*, so call-level biases are common-mode;
  M's higher τ means more extracted signal about a stable latent per call.
  E2/E9 anchor work supports the latent being real.
- One model (gemma4-12b), one template family, LessWrong-ish corpora with
  tiny separations. Pools with large true separations will shrink the
  multiplier (signs stop flipping); the saturation claim is regime-specific.
- Shuffled subsampling breaks the production schedule's structure (ring /
  nonce-draw adjacency) identically for all arms; active-selection gains
  that *depend* on the variance channel are not measured here (they accrue
  only to M).
- P/S constructed from PMF mean, not a resampled token — gaps understate.
