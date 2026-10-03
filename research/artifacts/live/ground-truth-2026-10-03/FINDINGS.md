# Ground-truth benchmark: 2026-10-03

**Errata:** None yet. Corrections belong here; raw model responses and shipped-binary outputs remain unchanged.

## Result

llmsort does not win every order cell: mountains seed 17: listwise had the highest order rho (1.00); rivers seed 17: listwise had the highest order rho (0.99); rivers seed 29: listwise had the highest order rho (0.98).
The pairwise latent/log-truth Pearson r spans 0.61–1.00; this is the direct test of recovered gaps, not only order.

## Protocol

Three n=20 public-fact pools, four methods, two seeds (17 and 29), one judge (`openai/gpt-5.6-terra`). Pointwise uses one 1–10 call per item; listwise uses one whole-pool call; pairwise and setwise are the shipped v0.15.0 CLI with their algorithmic defaults, an explicit common model, and seed. Pairwise disables cache so both seeds are live. Missing listwise ids, if any, tie below returned ids; duplicated ids after the first are ignored and counted. Spearman uses average tie ranks and Kendall is tau-b.

## Denominators and artifacts

24 cells = 3 pools × 4 methods × 2 seeds; 120 item-level pointwise calls, 6 listwise calls, plus the CLI calls/comparisons recorded per cell. Raw direct responses are in `raw/`; CLI JSONL, stderr, inputs, and pairwise traces are in `llmsort/`; `results.json` is the machine-readable reduction.

## Caveats

- These are factual-memory pools of bare names, not subjective sorting tasks; model familiarity is part of what is measured.
- River length depends on source/tributary/mouth convention. The cited table is fixed benchmark truth; Amazon/Nile claims are especially contested.
- Two seeds measure planning/presentation spread, not a model-wide error distribution.
- Pointwise has no native uncertainty. Setwise latent scores are an ordinal fusion scale, so its Pearson r is descriptive; pairwise r is the cardinal claim.
- CLI dollars come from the shipped binary's provider accounting and are printed to four decimal places; direct-call dollars use OpenRouter's response usage.

## Reproduce

```console
python3 research/scripts/ground_truth_bench.py --binary ./llmsort
```
