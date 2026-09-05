# Probe wave 3 — technologist_alpha (2026-09-05)

Operator brief: "alpha for a smart technologist" — a novel
config-space file format > a JavaScript Sets explainer > Zuckerberg's
workouts. Family definitions in `lens-e-alpha.md`; frozen roles and
prediction in `wave3/technologist_alpha.roles.md`; method and
thresholds identical to WAVE2_SPEC.md. Runner is `wave3.py` (llmsort
binary — the cardinal harness of waves 1–2 is retired).

## Verdict: PASS (T1 ∧ T2; T3 near-miss)

| metric | value | threshold |
|---|---|---|
| fr↔fr (opus↔sol) | **+0.867** | ≥ 0.60 (T1) |
| opus↔mini | +0.455 | — |
| sol↔mini | +0.566 | — |
| tier gap | **+0.357** | ≥ 0.20 (T2) |
| primary decoy (item 2) fr best / mini | 7 / 6 | T3 wants ≥7 ∧ mini ≥3 higher — missed on margin, T2 carried |

Orders (1-based items):
- opus46: 4 1 7 10 11 3 6 5 12 2 8 9
- gpt56sol: 7 4 1 10 3 11 2 8 6 5 12 9
- mini54: 7 1 11 8 4 2 6 12 10 5 9 3

## Reading

- **Prediction hit.** Both frontiers' top-3 is exactly the genuine-high
  set {1, 4, 7}. The operator ladder holds on frontiers: config-lattice
  format top-3; JS Sets explainer mid (6th/5th); Zuckerberg workouts
  bottom (8th/10th).
- **The confounder is real and tier-splitting.** The mini promoted the
  hacker-news-register decoy (item 2: 6th, vs 10th for opus), credited
  jargon depth (Rust Pin tutorial 3rd), and credited territory-contact
  texture without alpha (priced-in war story 4th vs opus's 11th). It
  also dumped the JS Sets explainer to 12th — below the workouts — i.e.
  it cannot hold "competent but priced-in" as a *middle* value; it
  collapses to exciting/boring.
- **Frontiers punish priced-in-ness specifically.** Opus's bottom-3
  {2, 8, 9} contains the primary decoy and the folklore-lesson war
  story — evidence the wording's "could not cheaply regenerate from
  standard documentation" clause is doing conditioning work, not just
  topic-gating.

## Addendum (same day): phrasing coherence + costume screen

Wordings in `wave3/prompts-screen.json`; same 12-item set.

**Phrasing coherence** (rank ρ vs canonical wording, same model):
`#b` ("expert-conditioned informational value") opus +0.769 / sol
+0.678; `#c` ("technical alpha … edge a senior engineer did not
have") opus +0.748 / sol +0.818. All three wordings, both frontiers,
put the same items top-3 ({1,4,7} in some order). The latent survives
rewording — the PASS was not a wording artifact.

**Costume screen** (opus46): ρ(alpha, scar_tissue_density) = +0.552;
ρ(alpha, live_wire_prose) = +0.531. Not costumes. The discriminant is
exactly the engineered splitter: item 8 (real war story, folklore
lesson) ranks 2nd under both contact axes and 11th under alpha — a
9-rank split — while item 4 (fsync measurement) tops all three axes,
which is correct: measurement essays are high-contact AND high-alpha.
Territory contact is evidence for alpha; priced-in-ness is what alpha
subtracts and contact axes don't.

## Cheap-judge bake-off (2026-09-05, same probe set)

Question: the cheapest/fastest model that judges technologist_alpha
*consistently* — frontier-consensus agreement plus decoy discipline —
for volume sorting (HN-stream class). Ground truth = opus46+sol mean
rank. 24-comparison pairwise runs unless noted.

| model | ρ vs consensus | decoy₂ | war₈ | lat | $/24cmp |
|---|---|---|---|---|---|
| **google/gemma-4-31b-it** | **+0.881** | 8 ✓ | 6 | 6.1s | $0.0018 |
| google/gemma-4-26b-a4b-it | +0.860 | 7 ✓ | 5 | 5.5s | $0.0013 |
| google/gemini-3.8-flash | +0.860 | 7 ✓ | 6 | 17.4s | $0.0534 |
| deepseek/deepseek-v4-flash | +0.790 | 8 ✓ | 6 | 15.4s | $0.0020 |
| anthropic/claude-haiku-4.5 | +0.545 | 8 | 3 ✗ | 14.3s | $0.0495 |
| openai/gpt-5.4-mini | +0.531 | 6 ✗ | 4 ✗ | 5.0s | $0.0124 |
| qwen/qwen3-32b | +0.497 | 4 ✗ | 9 | 63.3s | $0.0058 |

qwen3.7-flash / qwen3.8-flash: provider rejects the logprob-evidence
path outright (opaque "Provider returned error" on every attempt) —
excluded, and dense qwen3-32b judged poorly anyway.

**Gemma 4 31B is the champion**: beats Gemini 3.8 Flash at 1/29 the
cost, and it is open-weights — the same model serves locally on an
RTX Pro 6000 (96 GB holds 31B in BF16, let alone 4-bit) through
cardinald/freelane's existing local-engine lanes. Cerebras also hosts
gemma-4-31b on the free lane. Gemini Flash itself is hosted-only.

Consistency + throughput on the champion:
- **Cross-phrasing** (#a/#b/#c): ρ 0.71–0.85 — matches the frontiers'
  own 0.68–0.82. Consensus agreement holds across wordings
  (0.88/0.77/0.73).
- **Concurrency**: `--concurrency 24` runs the 24-comparison budget in
  3.1s wall (vs 6.2s at the default 8) — the rail saturates cheaply;
  wide-and-unpaced is correct for paid/local lanes.
- **Setwise (k=6)**: 6 calls instead of 24, $0.0017, ρ +0.930 vs its
  own pairwise and +0.839 vs frontier consensus; order-flip gauge
  0.022. Decoy slips 8→6 — acceptable, watch it. For stream sorting,
  setwise-on-Gemma is the throughput shape: ~4x fewer calls with ~45
  entity-pair constraints per 12-item batch.

Napkin for one RTX Pro 6000 (prefill-bound: ~650 input tokens and ~7
output tokens per pairwise judgement in logprob mode): 20–40k tok/s
batched prefill ⇒ ~30–60 judgements/s ⇒ 2.5–5M/day per GPU — two
orders of magnitude past the hosted-free-tier arithmetic, consistent
with the local-engine lane thesis in the cardinald commits.

## Next

1. Framing battery (order/label invariance) if full admission is
   wanted before production; the three-wording family is already
   sufficient for a lensed production run.
2. Production run: land the axis via cardinald (`lens=alpha`,
   `axis_key=technologist_alpha#a/#b/#c`) over a real corpus slice;
   freelane lanes make volume free. Cross-phrasing rank agreement on
   real data is the remaining admission evidence per AXIS_RESEARCH.
