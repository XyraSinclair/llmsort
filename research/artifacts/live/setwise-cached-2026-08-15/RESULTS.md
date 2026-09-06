# Setwise ratio elicitation with a cached entity prefix (k-wise · ratio · point)

**Errata:** none yet.

Single run (n=1), `openai/gpt-4.1-mini` via OpenRouter, seed 17. n=8 Manifund
items (1600 chars each), 3 rubric attributes, k=3 (41 subsets) and k=4 (21),
2 presentations each (pivot rotated), every unordered pair covered ≥2 subsets.
372 setwise calls: 372 parsed, 0 refused, 0 malformed, 0 errors. Baseline:
canonical_v2 `sort_documents`, default budget, 32/32 comparisons per attribute.
Total spend $0.209 (cap $3). Instrument: `examples/setwise_cached.rs`.

**Prompt caching works.** Prefix (system + entities block) ≈1400 tokens; the
attribute swaps in the tail. First attribute per presentation writes; the rest read:

| k | attr position | calls cache_read>0 / calls | mean cached fraction |
|---|---|---|---|
| 3 | 1st | 0/82 | 0.00 |
| 3 | 2nd, 3rd | 78/82, 74/82 | 0.78, 0.75 |
| 4 | 1st | 0/42 | 0.00 |
| 4 | 2nd, 3rd | 37/42, 38/42 | 0.75, 0.78 |

`cache_write_tokens` is reported 0 on all 372 calls (OpenRouter/OpenAI does not
report writes); cache evidence is read-side only.

**Pivot halo dominates.** 795/870 elicited ratios < 1 (mean ln r = −0.95 nats):
the model rates almost everything below reference slot A. Rotating the pivot
flips the implied direction on 25/27, 24/27, 23/27 pairs (k=3) and 23/25,
20/25, 21/25 (k=4) — versus pairwise position flips of 6/16, 3/16, 1/16.

**Agreement with pairwise latents (same items, model, seed):**

| k | attribute | ρ | τ | top-1 | top-3 |
|---|---|---|---|---|---|
| 3 | impact_per_dollar | 0.76 | 0.57 | ✓ | 3/3 |
| 3 | theory_of_change | 0.29 | 0.21 | ✗ | 1/3 |
| 3 | team_evidence | 0.55 | 0.36 | ✓ | 1/3 |
| 4 | impact_per_dollar | 0.79 | 0.64 | ✓ | 3/3 |
| 4 | theory_of_change | 0.36 | 0.21 | ✗ | 1/3 |
| 4 | team_evidence | 0.81 | 0.64 | ✓ | 2/3 |

**Cost:** setwise 870 pairwise-equivalent observations / $0.162 = 5,370 obs/$
(3,191–8,460 per arm; cached attributes ~3–4× the pairwise rate); pairwise 96
comparisons / $0.047 = 2,049 obs/$.

**Re-analysis 2026-09-06 (pivot-halo channel fitted):** replaying this trace
through `bias_calibration::solve_with_additive_offsets` (channel = pivot role,
sign −1; `experiments/examples/e1_halo_recalibration.rs`) fits γ_pivot =
+0.38…+1.48 nats per arm (mean ≈ +0.94, matching the −0.95 headline), cuts
rms residual 2–3× on all six arms, and shows a slot-distance decay under
per-slot channels (γ_B > γ_C > γ_D on every arm, e.g. +1.20/+1.01/+0.74).
Agreement with the pairwise latents improves on all three k=3 arms
(ρ 0.76→0.86, 0.55→0.67, 0.29→0.36) but drops on two k=4 arms
(0.79→0.60, 0.81→0.67); at n=8 those moves are one-to-two adjacent swaps
against a reference that is itself a 32-comparison instrument, so the robust
finding is the residual drop and the fitted halo, not the agreement deltas.
(Both replay arms solve via `from_log_ratio_moments` at unit variance, so the
uncorrected column differs slightly from the confidence-weighted table above.)

**Caveats:** the k−1 observations of a call share its context and pivot but
enter the solver as independent unit-precision points (mirroring canonical_v2);
under this pivot halo that independence is generous. Counterbalancing cancels
the halo only in expectation. One run, one model — instrument demonstration,
not a model property.
