# Ideation: is nonce-flatness a good sign, or knots in the wrong places? (2026-09-05)

RESAMPLING.md found nonce redraws carry ~1% of their stated information.
Operator framing: either the models are just this stable (good sign), or we
are not perturbing where the judgment actually forms — not re-eliciting.
The existing ledger adjudicates further than expected.

## Discriminating checks (perturbation_checks.py, same 72,813 rows)

1. **The PMF knows its own perturbability.** Across 11,568 draw groups,
   corr(log between-nonce scatter, log stated PMF var) = **0.69**
   (rank 0.53). The variance channel is a calibrated sensitivity meter —
   groups the model reports as uncertain are the ones nonces move.
2. **But it prices framing, not token noise.** Stated sd (median 0.085
   nats) is ~13× the nonce scatter (0.0066) and almost exactly the
   orientation-flip gap (median 0.078; ratio 0.82, corr 0.83 across 5,664
   pairs). The model's stated spread quantitatively matches how far its
   answer moves under a different *presentation*.
3. **The nonce is a null by design.** `draw-token:` appended after every
   stable byte (`rerank/sampling.rs` states the intent: measure
   irrelevant-context noise σ_w with maximal cache share). Zero
   exact-duplicate groups — it perturbs, microscopically.
4. **One reading resolves nothing here by the model's own account.**
   Median stated z of a pair = 0.12 on these near-tie pools.

## Reading

Both operator hypotheses are true, in different subspaces. The judgment is
*quenched* with respect to semantically-null context (genuinely good:
σ_w ≈ 0.007 nats, and the model's variance honestly rank-tracks even that).
The real uncertainty is *annealed* over framings — presentation order,
and presumably wording — at the 0.08-nat scale the PMF itself declares.
Nonce draws sample the quenched direction; the elicitation distribution we
should be integrating over is the framing orbit. The model has been telling
us where the knots belong all along, through the variance channel.

## The design unlock: framing draws at nonce prices

In the seriate/E1 layout the entities are the cached prefix and the
attribute/instruction is the suffix. Therefore any *suffix-level framing
perturbation* — attribute paraphrase, ratio-ladder rewording, whitespace
jitter (E8 apparatus), instruction reordering — costs the same ~0.21
marginal as a nonce draw but perturbs the thing that actually varies.
Orientation flips change the prefix (full prefill); paraphrase draws do
not. If framing scatter is ~10× nonce scatter (as the orientation gap and
stated sd predict), framing draws are ~real replication at cached prices —
the resampling instrument nonce_draws was supposed to be.

## Experiment ladder (P-series proposal)

- **P1 — perturbation spectrum.** Same pairs, gemma4-12b logprob rail,
  K draws per rung: {tail nonce, suffix whitespace jitter, attribute
  paraphrase bank, ladder rewording, orientation flip, setwise companion
  change}. Readout: scatter per rung vs stated PMF sd (one calibration
  plot — the invariance spectrum of the elicitation group). Prediction
  from this note: nonce ≪ jitter < paraphrase ≈ orientation ≈ stated sd.
- **P2 — is the PMF a forecast?** Per pair, does stated sd *predict* the
  observed framing scatter (not just correlate in aggregate)? If yes at
  slope ~1, publishable claim: the decision-token PMF is the model's
  honest forecast of its own framing sensitivity — and pooling framings
  is sampling the distribution the PMF describes.
- **P3 — framing draws vs nonce draws at equal compute.** RESAMPLING.md's
  design harness rerun with paraphrase draws in place of nonce draws
  (suffix-only, 0.21-priced). Decision rung: replace `nonce_draws` with
  `framing_draws` on evidence-rail lanes if split-half tau per unit
  compute beats both nonce-deepening and pure coverage.
- **P4 — truth anchor arm.** P3 on the E2 anchor pools: does framing
  pooling improve *accuracy against known ratios* per dollar, or only
  reproducibility? Separates marginalizing real bias from averaging noise.

Open riddle worth holding: if stated sd ≈ framing sensitivity, then
solver-side the moments are already partially double-counting framing
noise when we also counterbalance — the honest-variance bookkeeping of
FINDINGS.md's M arm may be conservative. P2's slope tells us whether to
deflate v after counterbalancing.
