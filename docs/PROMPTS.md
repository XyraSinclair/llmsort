# Prompt Contract

`llmsort` supports five JSON prompt templates — `canonical_v2`,
`canonical_bucket_v1`, `ordinal_v1`, `less_v1`, and `fraction_v1` — plus four
logprob instruments: `ratio_letter_v1`, `ratio_letter_2p_v1`,
`ratio_letter_attrlast_v1`, and `ordinal_letter_v1`.

## Slugs

| Slug | Model output | Use when |
|------|--------------|----------|
| `canonical_v2` | Decimal `ratio` on the canonical ladder range | General pairwise-ratio judgement. This is the default when no `prompt_template_slug` is set. |
| `canonical_bucket_v1` | Integer `ratio_bucket` in `0..16` | Runs that need output-token logprobs mapped directly to the ratio ladder. The bucket index avoids reconstructing multi-token decimal probabilities. |
| `ordinal_v1` | Direction only: `higher_ranked` plus `confidence` | Runs that want plain natural-language "which has more X?" judgements. This is cheaper and often more natural, but strictly less informative because magnitude is discarded. Good as a baseline or control. |
| `less_v1` | `lower_ranked` plus decimal `ratio` ("how many times less") | The group-inverse wording, for the wording-invariance check: a coherent judge must mirror its "times more" answer. The parser lowers the answer to the same (winner, ratio) shape as every other template. |
| `fraction_v1` | `higher_ranked` plus `fraction` in `(0, 1]` | The fractional wording ("what fraction of the greater one's level does the lesser reach"); a coherent judge's fraction must be the reciprocal of its ratio. Same invariance purpose as `less_v1`. |
| `ratio_letter_v1` | ONE letter from a 52-token alphabet (case = winner, letter = ladder rung, `A` = parity, `!` = refuse) | The logprob evidence path: a single completion position's top-k logprobs are the model's full judgement PMF, so the solver weights each observation by measured variance. Rendering, parsing, and mass accounting delegated to seriate. Degrades loudly to sampled mode where a provider hides logprobs. |
| `ratio_letter_2p_v1` | Written analysis, then one ratio letter | Default for reasoning-native judges: the first call analyzes without emitting a verdict; the second reads the letter PMF with reasoning disabled. |
| `ratio_letter_attrlast_v1` | ONE ratio letter, with the attribute after the entities | Research instrument for prefix-cache family sweeps. It saved input cost but reduced measured accuracy, so it is never selected by default. |
| `ordinal_letter_v1` | ONE letter, direction only | Evidence-path counterpart of `ordinal_v1`. Use it for any judge under ~30B: the 2026-09-06 bakeoff (`research/artifacts/live/judge-bakeoff-2026-09-06/RESULTS.md`) found most 7–14B models and even qwen3.7-flash never emit the lowercase half of the ratio alphabet, which reads as a stable sign flip; under this template gemma-4-12b-it agrees with gemma-4-31b at +0.88 and the usable small Qwens at +0.40. |

Unknown slugs are rejected. When `prompt_template_slug` is omitted, llmsort
chooses the measured default for the selected model: the two-phase ratio-letter
instrument for reasoning-native models, the single-phase ratio-letter instrument
for other logprob-capable models, and `canonical_v2` elsewhere.

Counterbalancing is a separate, orthogonal default: every planned pair is
asked in both presentation orders, the per-pair position bias cancels, and
the flip rate is reported in the run summary and trace
(`--no-counterbalance` restores single-order randomization).

## Ratio ladder

```text
[1.0, 1.05, 1.1, 1.2, 1.3, 1.5, 1.75, 2.1, 2.5, 3.1, 3.9, 5.1, 6.8, 9.2, 12.7, 18.0, 26.0]
```

The ladder is approximately geometric in log-space, with extra density near 1.0 for near-ties.

## Output shapes

`canonical_v2` successful judgement:

```json
{"higher_ranked":"A","ratio":2.1,"confidence":0.74}
```

`canonical_bucket_v1` successful judgement:

```json
{"higher_ranked":"A","ratio_bucket":7,"confidence":0.74}
```

In bucket mode, `ratio_bucket` is the zero-based index into the ratio ladder above. For example, bucket `7` means ratio `2.1`.

`ordinal_v1` successful judgement:

```json
{"higher_ranked":"A","confidence":0.74}
```

In ordinal mode, the live judgement records only direction and self-reported confidence. Internally it is converted into the same fixed modest ratio used by the synthetic ordinal evaluator, with unit precision, so the solver receives a directional log-ratio observation without inventing a magnitude or trusting uncalibrated self-assessment.

Refusal for either template:

```json
{"refused":true}
```

## Semantics

- `higher_ranked`: which side has more of the attribute
- `ratio`: how much more, constrained to the canonical ladder range; used by `canonical_v2`
- `ratio_bucket`: zero-based ratio ladder index; used by `canonical_bucket_v1`
- `ordinal_v1`: no ratio field; magnitude is intentionally not elicited
- `confidence`: self-reported confidence in `[0, 1]`; trace metadata, not solver precision
- `refused`: explicit refusal channel for genuinely blocked cases

## Request examples

- Multi-attribute CLI request: [`../examples/multi-rerank-request.json`](../examples/multi-rerank-request.json)
- Simple single-attribute request shape for library/API callers: [`../examples/simple-rerank-request.json`](../examples/simple-rerank-request.json)

Run the multi-rerank example with:

```bash
export OPENROUTER_API_KEY=your_key_here
llmsort rerank \
  --request examples/multi-rerank-request.json \
  --out output.json \
  --trace trace.jsonl \
  --report report.md
```

Research policy examples live under `research/examples/`. For reproducible
routing, pass one explicitly:

```bash
# Quality-only frontier run.
llmsort rerank \
  --request examples/multi-rerank-request.json \
  --policy-config research/examples/model-policy-quality-only.json \
  --out output.json \
  --trace trace.jsonl \
  --report report.md

# Cost-aware/fast run.
llmsort rerank \
  --request examples/multi-rerank-request.json \
  --policy-config research/examples/model-policy-cost-aware-fast.json \
  --out output.json \
  --trace trace.jsonl \
  --report report.md

# Frontier ladder: start with Opus 4.6, step through Gemini 3.1 Pro preview,
# then use GPT-5.4 Mini for low-uncertainty near-tie checks.
llmsort rerank \
  --request examples/multi-rerank-request.json \
  --policy-config research/examples/model-policy-frontier-ladder.json \
  --out output.json \
  --trace trace.jsonl \
  --report report.md
```

The checked-in policy files use live OpenRouter model IDs from the 2026-06 refresh: `anthropic/claude-opus-4.6`, `google/gemini-3.1-pro-preview`, `openai/gpt-5.4-mini`, and `deepseek/deepseek-v4-flash`.
If a model is newer than the local pricing table, reports use OpenRouter's provider-reported upstream cost when available; otherwise they label the local fallback cost as an estimate instead of pretending it is exact.


The current CLI accepts the multi-rerank request shape. The simple request shape is converted through the library API.

## Notes

- Prompt experiments and archived comparisons live under `research/`.
