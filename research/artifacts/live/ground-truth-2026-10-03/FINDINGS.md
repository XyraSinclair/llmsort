# Ground-truth benchmark: 2026-10-03

**Errata:** None yet. Corrections belong here; raw model responses and shipped-binary outputs remain unchanged.

## Result

- Pointwise 1–10 fails: mean ρ 0.57 / 0.34 / −0.37 (countries / rivers / mountains), with 60–90% of items sharing the modal score. This confirms the README claim.
- The one-prompt listwise sort orders all three pools at ρ 0.98–1.00 for about $0.002, but it dropped 6 and 7 of 20 countries. The drop claim holds (2 of 6 runs); the claim that listwise orders poorly does not hold on these well-known facts.
- Pairwise `sort` recovers magnitudes where it works (latent vs log-truth r 0.99 countries, 0.98 mountains) but loses on rivers (ρ 0.51/0.72, r 0.61/0.77), below both listwise and setwise, at ~100× listwise cost. Rivers is an open failure, not explained here.
- `sort --setwise` costs $0.005 and lands between listwise and pairwise (ρ 0.67–0.99).

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
