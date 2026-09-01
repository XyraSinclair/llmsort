# Consistency-line calibration — 2026-08-31

Question: does the `consistency:` line's predicted rerun agreement match
measured agreement between genuinely independent runs?

Cell: smoke_corpus (8 proverbs) · "usefulness as advice" ·
openai/gpt-5.6-luna · --budget 64 · --no-cache · seeds 7–17
(11 fresh runs, 55 run-pairs × 28 item-pairs). Total spend ≈ $0.06.

## Measured

Pooled cross-run pairwise-order agreement: **1140/1540 = 74%**
(per run-pair range 68–79%; per-run mean vs fleet 69–78%).

## The original (posterior plug-in) line under-predicted: 54–69%

Predictions across seeds: 56, 54, 63, 63, 55, 54, 69, 61, 55 percent —
all below every measured run-pair. Two defects, both fixed:

1. **Wrong form.** p²+(1−p)² with p = Φ(|gap|/σ_diff) answers
   "two fresh runs vs truth", not "a fresh run vs THIS run". The correct
   conditional is: fresh gap | this run ~ N(gap, 2σ_n²), so match
   probability = Φ(|gap|/(√2·σ_n)).
2. **Wrong σ.** Posterior σ mixes epistemic spread (the judge's expressed
   PMF variance — reproduced identically on rerun, luna σ_w ≈ 0.05 nats
   vs posterior σ_diff ≈ 0.48) with aleatoric noise (σ_w — the only part
   that resamples). Empirical per-pair rerun σ across the 5 JSON runs was
   ~0.30× the posterior σ_diff. Only σ_w-noise should enter a
   reproducibility prediction.

## The fix (landed)

- `meta.evidence_obs_sigma_rms` = sqrt(mean PMF var + σ_w²) over evidence
  observations (orchestrator, beside the σ_w refit).
- κ = σ_w / obs_sigma_rms rescales posterior stds to rerun stds;
  `rerun_agreement` uses Φ(|gap|/(√2·κ·σ_diff)) — labeled
  "noise-scaled estimate". Without a σ_w estimator κ=1: a conservative
  "posterior floor" label.
- Wording: "reproduce", not "agree with" — this line is reproducibility,
  NOT correctness. A deterministic judge reproduces its errors too;
  correctness stays with stat±/resolution.

Validation on a fresh seed under the new binary (seed 17, healthy):
predicted 66% noise-scaled vs 74% measured — residual conservatism ~8
points, driven by σ_w's own small-sample noise (per-run σ_w estimates
ranged 0.051–0.191 nats on identical configs — few counterbalanced pairs
per run). Directionally honest: the line now under-promises slightly
instead of by 20 points.

## Incidental findings

- Posterior scale is seed-unstable on this cell: mean latent_std ranged
  0.075–0.767 across identical-config runs (10×). The gauge's absolute σ
  is itself a noisy estimate at 8×~30 comparisons.
- Two of 11 runs stopped early on transient provider failures (seed 16:
  7 comparisons, stop consecutive_failures → no counterbalance → no σ_w
  → the surfaces correctly degraded to "posterior floor" +
  "budget-limited"). Even that starved run agreed 71% with the fleet —
  luna reproduces large gaps at any budget.
