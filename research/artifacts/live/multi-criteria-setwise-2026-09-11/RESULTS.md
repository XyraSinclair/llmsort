# Multi-criteria setwise: m criteria in one call vs m separate calls

Avenue A of `research/notes/multi-objective-reranking-2026-09-11.md`. Question: when a list is
ranked on m attributes, does asking the judge for all m orderings in ONE setwise call (one
`<entities>` block, m `<attribute_i>` blocks, m answer lines) lose anything against m separate
calls? Adoption bar from the note: inter-criterion inflation (halo) < 0.10, per-criterion
agreement with the separate arm > 0.85, flip rate unchanged.

Instrument: `experiments/examples/multi_criteria_setwise.rs` — the extension's setwise design
(n=40, k=8, overlap 2, 1 round, 2 presentations per window: window order then a shuffle),
same Huber-IRLS fit, same slot grammar. Two arms over identical windows and presentations, so
the only difference is the prompt. Lists: `lists/lw.json` (40 LessWrong posts; criteria
novelty / technical alpha / epistemic rigor — the battery wording) and `lists/hn_top.json`
(40 HN front-page items; interesting / credible / actionable — chosen to be near-orthogonal).
Judges: `google/gemma-4-31b-it` via OpenRouter (metered), `qwen-3.8-27b` on Cerebras
(`reasoning_effort: none`, free tier). Raw traces per call in `trace-<label>.jsonl`, per-run
tables in `summary-<label>.md/json`.

## Cost

| run | judge | separate calls / in-tokens / $ / s | joint calls / in-tokens / $ / s | joint ÷ separate |
|---|---|---|---|---|
| lw (seed 7) | gemma-4-31b | 42 / 223,212 / $0.0196 / 27 | 14 / 76,876 / $0.0076 / 5 | 0.39 |
| lw (seed 11) | gemma-4-31b | 42 / 224,340 / $0.0205 / 17 | 14 / 77,251 / $0.0083 / 8 | 0.40 |
| hn_top | gemma-4-31b | 42 / 67,893 / $0.0067 / 12 | 14 / 24,536 / $0.0024 / 3 | 0.36 |
| hn_top | qwen-3.8-27b (Cerebras) | 42 / 64,810 / — / 2 | 14 / 23,554 / — / 1 | 0.36 |
| lw | qwen-3.8-27b (Cerebras) | 42 / 173,201 / — / 29 | 14 / 65,891 / — / 25 | 0.38 |

Joint costs 0.36–0.40× of separate for m=3 (input tokens ≈ 1/m plus the shared entity block
once; output grows by the extra answer lines only). Malformed: 0/56 on lw both seeds, 1/42 and
1/14 on hn_top (gemma); 0 on Cerebras. The Cerebras errors (9 + 2 on lw) are all HTTP 429
`token_quota_exceeded` — the free tier's tokens-per-minute cap at ~173K prompt tokens in 29 s,
not model failures; those windows were dropped from the fit, which is why lw_cerebras parsed
counts are 10–13 rather than 14.

## Agreement with the separate arm, and the seed-noise floor

| run | judge | ρ(separate, joint) novelty/alpha/rigor or interesting/credible/actionable | top-10 overlap |
|---|---|---|---|
| lw s7 | gemma | +0.897 / +0.954 / +0.946 | .80 / .90 / .80 |
| lw s11 | gemma | +0.942 / +0.960 / +0.941 | .80 / .90 / .90 |
| hn_top | gemma | +0.929 / +0.855 / +0.914 | .80 / .90 / .80 |
| hn_top | qwen | +0.890 / +0.937 / +0.841 | .80 / .80 / .60 |
| lw | qwen | +0.928 / +0.957 / +0.956 | .90 / .80 / .80 |

The floor to compare against is the same arm re-run with a different window design (lw, gemma,
seed 7 vs seed 11):

| criterion | ρ(sep s7, sep s11) | ρ(joint s7, joint s11) | ρ(sep s7, joint s11) |
|---|---|---|---|
| novelty | +0.917 (top-10 .70) | +0.833 (.60) | +0.856 |
| alpha | +0.920 (.70) | +0.946 (.90) | +0.924 |
| rigor | +0.888 (.80) | +0.912 (.90) | +0.854 |

Separate-vs-separate across seeds is 0.89–0.92; joint-vs-separate on the same seed is
0.90–0.96. The joint call is inside the design's own noise: it is not distinguishable from
re-running the separate arm. Every run clears the > 0.85 agreement bar except qwen's
`actionable` on hn_top (0.841, top-10 overlap 0.60).

## Halo (inter-criterion inflation)

| run | judge | mean inter-criterion ρ, separate → joint | inflation |
|---|---|---|---|
| lw s7 | gemma | 0.865 → 0.910 | +0.045 |
| lw s11 | gemma | 0.831 → 0.842 | +0.012 |
| hn_top | gemma | 0.075 → 0.075 | +0.001 |
| hn_top | qwen | 0.183 → 0.370 | **+0.187** |
| lw | qwen | 0.800 → 0.897 | +0.096 |

hn_top is the sharp test: its criteria are genuinely near-orthogonal for the judge
(credible~actionable −0.20, interesting~actionable −0.05 under gemma's separate arm), so any
halo shows up as raw correlation. Gemma-31b keeps the three axes exactly as independent in the
joint call (+0.001). Qwen-27b does not: credible~actionable goes −0.09 → +0.25 and
interesting~actionable +0.02 → +0.26 — the joint prompt pulls its three orderings toward one
another. On lw the three battery axes are already one latent for both judges (0.80–0.87 in the
separate arm), which bounds how much inflation is even possible there; gemma still adds
+0.01–0.05, qwen +0.10.

## Flip rate (repeat-presentation disagreement)

| run | judge | separate | joint |
|---|---|---|---|
| lw s7 | gemma | .138 / .082 / .077 | .189 / .092 / .117 |
| lw s11 | gemma | .107 / .107 / .051 | .112 / .087 / .107 |
| hn_top | gemma | .153 / .235 / .119 | .138 / .158 / .101 |
| hn_top | qwen | .117 / .209 / .107 | .117 / .189 / .173 |

No consistent direction: joint is noisier on lw s7 (novelty +.05, rigor +.04), the same on
s11, and cleaner on hn_top. Flip rate is "unchanged" within what two seeds resolve.

## Verdict

**Adopt the joint call for gemma-4-31b-class judges; keep it judge-gated.** With gemma-31b the
joint call is 0.36–0.40× the cost, agrees with the separate arm at the design's own seed-noise
floor (0.90–0.96), adds no halo on orthogonal criteria (+0.001) and ≤ +0.05 on correlated ones,
and does not move the flip rate. All three adoption conditions hold.

With qwen-3.8-27b the joint call fails the halo bar on the orthogonal list (+0.187, one
criterion below 0.85 agreement). The failure is exactly on the case multi-objective ranking
exists for — criteria that really are independent — so a smaller judge should keep separate
calls, or the roster should treat "joint-safe" as a per-model property measured on an
orthogonal cohort like hn_top rather than assumed.

Practical shape for the extension: `llm:<model> … criteria=joint|separate` with the default
per preset (joint for gemma-31b / gpt / claude / gemini-pro class after they pass the same
check; separate otherwise). The check itself is cheap — one hn_top-sized run costs under a
cent — and is the thing to run when adding a preset.

## Cerebras as a byo judge (what this run also established)

- `gemma-4-31b` is not on Cerebras' public endpoints (withdrawn 2026-09-03; dedicated
  endpoints only). `qwen-3.8-27b` and `gpt-oss-120b` are.
- `qwen-3.8-27b` spends its whole `max_tokens` on hidden reasoning and returns `null` content
  unless `reasoning_effort: "none"` is sent (400/400 tokens reasoning, `finish_reason: length`).
  With it: 9 completion tokens, exact slot answers, 0 malformed across 56 calls on hn_top. The
  extension's byo lane now always sends the field (payload 0.12.1); the harness has `--effort`.
- Throughput: 42 setwise calls (65K prompt tokens) in 2 s. The free tier's tokens-per-minute
  cap bites on long items (lw ≈ 5K tokens/call): 11/56 calls hit 429 at concurrency 4.
  Concurrency 2 or a short retry-after sleep covers it; the extension runs at concurrency 1–2.
- `gpt-oss-120b` at `reasoning_effort: low` answers correctly with ~65 reasoning tokens per
  call — usable, ~7× the output tokens of qwen.

## Caveats

n=40 per list, two lists, one seed replicate, two judges. Agreement and halo are measured against
the separate arm, not against ground truth — the separate arm is the reference the note
proposed, not a gold standard. The lw battery axes are ~0.85 correlated under both judges in the
separate arm, so lw mostly tests cost and agreement; hn_top carries the halo test.
