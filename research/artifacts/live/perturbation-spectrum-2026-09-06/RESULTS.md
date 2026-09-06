# P1/P2/P4 — the perturbation spectrum, measured (2026-09-06)

Executes the ladder proposed in
`research/notes/logprob-efficiency-2026-09-05/IDEATION.md`. Judge:
**gemma4-31b** (the operator-decreed local judge, fp8, vLLM, top-20
logprobs — route probed live per LOGPROBS.md doctrine), evidence rail
`ratio_letter_v1` via `experiments/examples/perturbation_spectrum.rs`.
Zero marginal dollar cost (owned GPU).

Design per pool: 12 entities, all 66 pairs × both orientations ×
{8 draw-token nonces on the base attribute; 6 whitespace-jitter variants;
6 paraphrase variants} = 2,640 calls.

## Anchor pool (countries by UN-2024 population; truth known) — COMPLETE

2,640/2,640 rows, 0 errors, every row a logprob PMF (`pspec_anchor.jsonl`).

### P1 — the spectrum exists and is ordered as predicted

Median sd of the PMF mean μ across draws/variants, per (pair, orientation):

| perturbation | scatter (nats) | / stated PMF sd |
|---|---|---|
| nonce (null suffix) | 0.0097 | 0.08 |
| whitespace jitter (mid-prompt, null) | 0.0779 | 0.38 |
| paraphrase (framing) | 0.1661 | 0.72 |
| orientation flip | 0.6965 (median gap) | ~4–7 |

The deeper the perturbation reaches into the elicitation, the more the
judgment moves — nonce ≪ jitter < paraphrase < stated sd < orientation.
IDEATION's prediction confirmed except orientation, which on this pool
*exceeds* the stated sd (position bias on bare-name anchors is much larger
than on the lesswrong-comment pool, where gap ≈ stated sd).

### P2 — the PMF is an honest forecast of its own framing sensitivity

Per (pair, orientation), stated PMF sd vs observed paraphrase scatter
(n = 132): **corr 0.865, rank corr 0.888, OLS slope 0.956.** The
decision-token PMF's variance quantitatively predicts how far the answer
moves under re-elicitation with reworded framing — slope ≈ 1, not just
rank agreement. This upgrades IDEATION's aggregate-scale observation to a
per-cell predictive claim.

### P4 — truth arm: the judgment is not just stable, it is right

Pooled μ per pair vs true log population ratios (66 pairs, spanning a
2.2% gap India/China to ~280× India/New Zealand):

| rung | sign accuracy | rank corr | slope vs truth |
|---|---|---|---|
| nonce (base wording) | **1.000** | 0.972 | **0.999** |
| jitter | 0.985 | 0.976 | 0.961 |
| para | 0.985 | 0.976 | 1.111 |

Slope ≈ 1: no magnitude compression (E2 saw judges compress true
log-ratios ~1/3; gemma4-31b on populations is magnitude-calibrated).
Pooling curves saturate immediately — nonce k=1 already 0.997 sign
accuracy; jitter/para asymptote *lower* (0.985): on a factual attribute
the base wording is the best instrument and framing draws add real
framing variance without improving truth accuracy. Framing draws buy
robustness information, not factual accuracy — their value should be on
subjective attributes (lesswrong pool, pending below).

## LessWrong pool (subjective attribute, production 12-entity pool) — PENDING

First attempt lost to engine churn: the judge standby's drain
self-congested the shared vLLM (hundreds queued behind ~24 runs × 16
concurrency), zero cells landed for 2h, and the stall guard restored the
GPU borrow at 12:56Z (engine down, 1h back-off). The driver
(`pspec_driver.sh`, on the judge host) waits and resumes automatically; results
land as an addendum here.

## Notes

- `spec_lesswrong_redacted.json` carries entity ids + lengths only; the
  full comment texts live in the judgement ledger, not this public repo.
- Repeat-elicitation economics: on `ratio_letter_v1` (attribute-first
  prefix) paraphrase draws re-prefill the entities; the "framing draws at
  nonce prices" cost claim from IDEATION.md attaches to the attr-last
  template (`RATIO_LETTER_ATTR_LAST_SLUG`), whose prefix is the entity
  pair. Scatter measurements here are template-layout-independent.
- Replay: `perturbation_spectrum <spec.json> <out.jsonl>` (resumes from a
  partial pack; all successful rows keyed by variant|pair|orientation|draw).
