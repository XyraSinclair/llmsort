# Evaluation Evidence

This page is the checked-in evidence surface for the offline synthetic harness, the live OpenRouter cardinal-policy study pack, and the live structured-judgment method comparison. The offline record shows what the current code and deterministic simulator produce. The live studies prove current provider integration and reporting paths work against real traffic. None shows a universal win over scalar ratings.

## Reproduce the study

Run from the repository root. These commands are offline; they use the deterministic simulator and do not require `OPENROUTER_API_KEY`.

```bash
mkdir -p research/artifacts/eval

cargo run -p llmsort-experiments --bin cardinal -- eval \
  --out research/artifacts/eval/synthetic_eval.jsonl \
  --curve-csv research/artifacts/eval/synthetic_curves.csv

cargo run -p llmsort-experiments --bin cardinal -- eval-likert \
  --out research/artifacts/eval/likert_eval.jsonl \
  --curve-csv research/artifacts/eval/likert_curves.csv

cargo run -p llmsort-experiments --bin cardinal -- eval-compare \
  --mode ratio \
  --out research/artifacts/eval/comparison_summary.json

python3 research/examples/offline_eval_delta.py \
  --cardinal research/artifacts/eval/synthetic_eval.jsonl \
  --likert research/artifacts/eval/likert_eval.jsonl \
  --csv research/artifacts/eval/offline-workflow/cardinal_vs_likert_delta.csv \
  --summary research/artifacts/eval/offline-workflow/cardinal_vs_likert_summary.txt
```

Checked-in outputs:

- `research/artifacts/eval/synthetic_eval.jsonl`: raw cardinal pairwise synthetic results, one JSON object per case.
- `research/artifacts/eval/likert_eval.jsonl`: raw Likert/scalar baseline results, one JSON object per case.
- `research/artifacts/eval/comparison_summary.json`: compact cardinal-vs-Likert deltas and win/loss/tie counts from the built-in comparator.
- `research/artifacts/eval/synthetic_curves.csv`: cardinal trajectory data.
- `research/artifacts/eval/likert_curves.csv`: Likert trajectory data.
- `research/artifacts/eval/offline-workflow/cardinal_vs_likert_delta.csv`: script-friendly per-case, per-metric deltas from `research/examples/offline_eval_delta.py`.
- `research/artifacts/eval/offline-workflow/cardinal_vs_likert_summary.txt`: short text summary of the same CSV.

Optional generated aid:

- `cargo run -p llmsort-experiments --bin cardinal -- eval-compare --mode ordinal --out research/artifacts/eval/comparison_summary_ordinal.json` compares the same active-comparison loop using ordinal "which item is higher?" judgements instead of ratio magnitudes.

Do not compare the two curve CSV `error` columns directly. `synthetic_curves.csv` records the cardinal model's estimated top-k boundary error and can exceed 1. `likert_curves.csv` records observed `1 - topk_precision`. They are trajectories with different semantics, not a shared y-axis.

## Live OpenRouter study pack

Source directory: `research/artifacts/live/openrouter-benchmark-2026-06-30/`.

The live pack was generated through `research/examples/live_openrouter_benchmark.py` with `OPENROUTER_API_KEY` set. It runs `cardinal rerank` on three project-relevant case families, preserving each policy's request JSON, response JSON, trace JSONL, markdown report, cache export, `summary.json`, and `summary.md`.

Aggregate study record:

| Policy | Comparisons | Cached | Refused | Cost USD | Top public evidence | Top routing policy | Top release risks |
|---|---:|---:|---:|---:|---|---|---|
| `quality_only_opus_46` | 153 | 0 | 0 | $0.482300 | worked_example, readme, evaluation_doc | frontier_ladder_2026_06, qwen_3_7_max | no_large_live_receipt, baseline_breadth, provider_metadata_drift |
| `frontier_ladder_2026_06` | 153 | 0 | 0 | $0.482175 | readme, worked_example, evaluation_doc | frontier_ladder_2026_06, qwen_3_7_max | no_large_live_receipt, baseline_breadth, provider_metadata_drift |
| `cost_aware_fast_deepseek_v4_flash` | 153 | 0 | 0 | $0.029860 | evaluation_doc, worked_example, readme | frontier_ladder_2026_06, quality_only_opus_46 | no_large_live_receipt, cache_provenance, provider_metadata_drift |

Across the three policy runs, the pack used 459 fresh provider comparisons, 0 cached comparisons, 0 refusals, 234,299 provider input tokens, 80,983 provider output tokens, and $0.994335 provider-reported cost. It proves the OpenRouter gateway, model-policy JSON, prompt parsing, trace writing, markdown report generation, and cache export work against live provider traffic for these cases. It is not a scalar/Likert benchmark, and it does not replace a larger task-family suite with external baselines and held-out prompts.

Re-run one policy:

```bash
python3 research/examples/live_openrouter_benchmark.py \
  --out-dir research/artifacts/live/openrouter-benchmark-2026-06-30/quality_only \
  --policy-config research/examples/model-policy-quality-only.json
```

Refresh `combined-summary.json` and this directory's `README.md` after re-running policy directories; they are aggregate reports, not source data.

## Live structured-judgment method comparison

Source directory: `research/artifacts/live/method-comparison-2026-06-30-suite-v1/`.

The method comparison was generated through `research/examples/live_method_comparison.py` with `OPENROUTER_API_KEY` set. It runs six frozen, attribute-weighted case families through scalar matrix scoring, whole-list sorting, ordinal pairwise judging, and cardinal pairwise-ratio judging, then compares each method with a separate live pairwise-ratio reference model.

Aggregate study record:

| Case | Best candidate agreement with reference | Cardinal pairwise-ratio agreement | Notable disagreement |
|---|---|---|---|
| `public_artifact_work` | list sort and cardinal tie at Kendall tau 0.800 / top-k Jaccard 0.500 | Kendall tau 0.800 / top-k Jaccard 0.500 | ordinal pairwise trails at Kendall tau 0.400 |
| `judgment_method_properties` | list sort reaches Kendall tau 0.800 / top-k Jaccard 1.000 | Kendall tau 0.600 / top-k Jaccard 1.000 | scalar matrix misses the same top-k set; ordinal and cardinal recover it |
| `model_policy_options` | list sort reaches Kendall tau 0.800 / top-k Jaccard 1.000 | Kendall tau -0.200 / top-k Jaccard 0.500 | cardinal pairwise-ratio ranks the policy options in the opposite direction on several pairs |
| `first_user_path` | list sort reaches Kendall tau 0.800 / top-k Jaccard 0.500 | Kendall tau 0.000 / top-k Jaccard 0.200 | both pairwise regimes underperform scalar/list prompts on first-run onboarding priorities |
| `benchmark_design_rigor` | list sort and cardinal tie at Kendall tau 0.600 / top-k Jaccard 0.500 | Kendall tau 0.600 / top-k Jaccard 0.500 | all methods agree on the first item but differ on the middle of the list |
| `public_release_risks` | scalar matrix reaches Kendall tau 1.000 / top-k Jaccard 1.000 | Kendall tau 0.800 / top-k Jaccard 1.000 | cardinal recovers the risk set but swaps the first two risks relative to the reference |

Across the six cases, the comparison used 552 OpenRouter calls, 111,282 prompt tokens, 50,809 completion tokens, and $0.806604 provider-reported cost. Every row used exact provider-reported or local pricing metadata (`cost_is_estimate = false`). Budget-normalized aggregates are intentionally mixed: list sort has the highest mean agreement score (0.817) with 6 calls and $0.003072, scalar matrix reaches 0.650 with 6 calls and $0.009421, cardinal pairwise-ratio reaches 0.667 with 180 calls and $0.052380, and ordinal pairwise reaches 0.558 with 180 calls and $0.043618.

The reference is still an LLM regime, not human ground truth or a hidden exhaustive oracle; low agreement is evidence of prompt/regime brittleness on these cases, not proof that the candidate model cannot perform the task.

Re-run the comparison:

```bash
python3 research/examples/live_method_comparison.py \
  --out-dir research/artifacts/live/method-comparison-2026-06-30-suite-v1 \
  --candidate-model openai/gpt-5.4-mini \
  --reference-model anthropic/claude-sonnet-4.6 \
  --max-usd 10
```

`summary.json` is the machine-readable aggregate. `summary.md` and `README.md` are generated views of the same data. `research/examples/live-method-suite.json` is the frozen suite input. Each case directory also preserves `case.json`, one JSON result per method, and per-call request/response/parsed/usage records under `calls/`.

## Method

The suite runs the same synthetic cases through two deterministic evaluators:

1. `eval` uses the cardinal pairwise-ratio path. Synthetic pairwise judgements feed the reranker, which fits latent scores, estimates uncertainty, and records the final rank metrics.
2. `eval-likert` uses a scalar baseline. It spends the same number of model-call slots as the cardinal run, samples per-item ratings on a 10-point scale, infers utility scores from those ratings, and records the same headline rank metrics where possible.
3. `eval-compare` runs both suites with `levels = 10` and `budget_multiplier = 1.0`, then emits mechanical deltas in `comparison_summary.json`.
4. `research/examples/offline_eval_delta.py` reads the two JSONL study records directly and emits an auditable CSV/text comparison. It is useful when you want to compare artifacts generated by different commits or flags.

The current comparison gives equal call counts to cardinal comparisons and Likert ratings. That is a simple reproducible regime, not a claim about equal token cost: pairwise prompts contain two items, while scalar prompts rate one item. It also does not model live-provider variance, prompt sensitivity, refusal behavior, or pricing differences.

## Metrics

`comparison_summary.json` compares five metrics for each case:

| Metric | Meaning | Better direction |
|---|---|---|
| `topk_precision` | Fraction of predicted top-k items that are truly in the top-k set. | Higher |
| `topk_recall` | Fraction of true top-k items recovered by the predicted top-k set. | Higher |
| `kendall_tau_b` | Pairwise rank-order agreement with ground truth, tie-aware. | Higher |
| `coverage_95ci` | Fraction of true scores covered by the model's reported 95% intervals. | Higher |
| `comparisons_used` | Cardinal comparisons used versus Likert ratings used. | Lower |

For quality metrics, `delta` means cardinal minus Likert. For `comparisons_used`, `delta` is still cardinal minus Likert, but `higher_is_better` is `false` and the `outcome` field applies the lower-is-better direction.

## Current cardinal-vs-Likert study

Source: `research/artifacts/eval/comparison_summary.json`.

| Case | Cardinal top-k P/R | Likert top-k P/R | Cardinal tau-b | Likert tau-b | Cardinal 95% cov. | Likert 95% cov. | Calls | Case W/L/T |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `clean_ordering_10` | 1.000 / 1.000 | 1.000 / 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 40 / 40 | 0 / 0 / 5 |
| `noisy_ordering_50` | 0.600 / 0.600 | 0.600 / 0.600 | 0.641 | 0.735 | 1.000 | 0.800 | 200 / 200 | 1 / 1 / 3 |
| `multi_attr_weighted_20` | 0.800 / 0.800 | 1.000 / 1.000 | 0.895 | 0.905 | 1.000 | 0.750 | 240 / 240 | 1 / 3 / 1 |
| `clustered_scores_30` | 1.000 / 1.000 | 1.000 / 1.000 | 0.830 | 0.868 | 1.000 | 0.033 | 120 / 120 | 1 / 1 / 3 |
| `scale_compression_40` | 1.000 / 1.000 | 0.200 / 0.200 | 0.821 | 0.224 | 0.975 | 0.000 | 160 / 160 | 4 / 0 / 1 |
| `outlier_robustness_25` | 0.400 / 0.400 | 0.800 / 0.800 | 0.567 | 0.830 | 1.000 | 0.360 | 100 / 100 | 1 / 3 / 1 |
| `gated_feasibility_30` | 0.800 / 0.800 | 1.000 / 1.000 | 0.667 | 0.831 | 1.000 | 0.250 | 120 / 120 | 1 / 3 / 1 |
| `inconsistent_cycle_12` | 0.800 / 0.800 | 0.800 / 0.800 | 0.636 | 0.848 | 0.250 | 0.167 | 200 / 200 | 1 / 1 / 3 |

Aggregate over 8 cases and 5 compared metrics per case: cardinal wins 10, Likert wins 12, and 18 are ties.

## What the study says

- Cardinal clearly wins `scale_compression_40`: an extreme outlier collapses 10-level Likert ratings for the non-outlier frontier, while ratio comparisons recover the true top-k and a much stronger Kendall tau-b at the same call count.
- Cardinal ties Likert on top-k precision/recall for `clean_ordering_10`, `noisy_ordering_50`, `clustered_scores_30`, and `inconsistent_cycle_12`; it beats Likert on coverage in the latter three under this simulator.
- Likert wins important ranking metrics. It beats cardinal on top-k precision/recall and tau-b for `multi_attr_weighted_20`, `outlier_robustness_25`, and `gated_feasibility_30`, and it beats tau-b for `noisy_ordering_50`, `clustered_scores_30`, and `inconsistent_cycle_12`.
- No method wins on resource use in this study. Every case uses the same number of cardinal comparisons and Likert ratings, so `comparisons_used` is a tie in all eight cases. Equal call count is not equal token cost.
- Gated top-k precision and recall are gate-aware: predicted top-k over predicted-feasible items is compared with true top-k over true-feasible items. Gate false negatives do not shrink the target set and make a run look better than it is.
- All current cardinal synthetic runs stop at `budget_exhausted`. Do not describe the current artifact as proof of early stopping, lower cost, or strict accuracy dominance.

## What this proves

The repo has a deterministic local evaluation surface with checked-in raw artifacts, a compact comparison summary, and reproducible commands. It proves that claims can be checked and falsified locally. It does not prove that cardinal reranking is generally superior to Likert or scalar rating baselines.

## Known gaps

- The live method comparison now includes scalar, list-sort, ordinal, and cardinal regimes, but its reference is another LLM method rather than human ground truth or an exhaustive non-LLM oracle.
- The baseline is a deterministic 10-level scalar simulator, not a tuned family of scalar prompts.
- The current resource control is equal call count, not equal tokens, equal latency, or equal dollars.
- The current synthetic cardinal runs all end with `budget_exhausted`; they do not demonstrate cost-saving convergence.
- The case suite is small. It is good enough to catch regressions and counter overclaiming, not good enough to settle the method.

## Next empirical proof target

The next public-grade study should turn the live comparison into a larger frozen benchmark suite:

1. more task families and held-out prompts;
2. repeated runs or model swaps to separate method behavior from one model's quirks;
3. equal-call, equal-token, and equal-dollar budget views;
4. a higher-budget or externally judged reference where feasible;
5. stratified reporting by task family, budget type, and failure mode.

For each regime, preserve request JSON, response JSON, trace JSONL, generated report, cache export when used, model IDs, token/cost accounting, and the exact comparison budget policy. The credible claim is still not "cardinal wins"; it is "under this budget and task family, this pairwise-ratio policy improves these metrics while losing or tying these others."
