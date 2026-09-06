# Live stick-breaking letter-PMF instrument (order · logprobs · stickbreak)

**Errata:** none yet.

Single run (n=1), `openai/gpt-4.1-mini` via OpenRouter, seed 17, n=8 Manifund
items (1600 chars), 3 rubric attributes, k=3 (18 calls) and k=4 (12), chunk
design, 2 presentations. Instrument: `examples/setwise_cached.rs
--answer order --logprobs --stickbreak` (52f5833); offline gate f360e9e
(stickbreak dominates matched-budget pairwise at every noise level, σ ≤ 1).
Total spend $0.059 (cap $3). Baseline: live pairwise, same items/model/seed.

**The harvest works live: 30/30 calls.** Every call yielded aligned letter
tokens with top_logprobs alternatives; per-slot conditional PMFs persisted in
`trace.jsonl` (`slot_pmfs`). No fallbacks to hard-tier lowering.

**PMF sharpness (66 informative slots):** mean top-choice mass 0.948, median
0.998, min 0.562; mean entropy 0.136 nats. The model is near-deterministic on
most slots and genuinely graded on 12/66 — the evidence rail concentrates
there (e.g. one k=4 first slot splits A 0.218 / D 0.759 / B 0.023). This is
the shape the zero-information-tournament argument wants: hard orders where
the judge is sure, calibrated mass where it is not.

**Cost:** setwise ≈ $0.0001–0.0004/item/attribute vs pairwise ≈ $0.0020 —
~10× cheaper per item at this budget, with k(k−1)/2 edges per call.

**Agreement with pairwise latents (n=8, 2 presentations — noisy):**

| k | attribute | ρ | τ | top-1 | top-3 |
|---|---|---|---|---|---|
| 3 | impact_per_dollar | 0.24 | 0.21 | ✓ | 2/3 |
| 3 | theory_of_change | 0.71 | 0.57 | ✓ | 2/3 |
| 3 | team_evidence | 0.83 | 0.71 | ✓ | 3/3 |
| 4 | impact_per_dollar | 0.02 | 0.07 | ✗ | 1/3 |
| 4 | theory_of_change | 0.64 | 0.50 | ✓ | 2/3 |
| 4 | team_evidence | 0.31 | 0.07 | ✗ | 2/3 |

**Caveats:** mechanism-verification run, not a promotion claim: 8–12 calls
per (k, attribute) arm against a 32-comparison pairwise reference at n=8 —
both sides are noisy, and the pairwise reference is itself an instrument, not
truth. The pairwise arm REFUSED 11/96 calls where E1's canonical_v2 refused
0/96 on the same items/model/seed. Diagnosed from the trace: since 2594b89
this baseline renders `ratio_letter_v1`, whose one-token answer alphabet
includes an explicit refusal token `!` — all 11 are elective single-token
refusals (`output_logprob_token_count = 1`, no error), concentrated on hard
cross-domain pairs (6/11 involve `precautionary-agi-governance`). Both
templates offer refusal; the one-token instrument makes it cheap and salient
where canonical_v2's JSON `{"refused": true}` never fires. An instrument
property to weigh when comparing refusal rates across template families.
Next escalation: more presentations and n, and the halo-channel calibration
(46162bb) folded over the ratio arm for a three-way comparison.
