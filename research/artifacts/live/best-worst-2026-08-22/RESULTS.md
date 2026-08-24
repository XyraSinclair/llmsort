# E6 — best–worst vs listwise vs pairwise, DeepSeek V4 Flash (k-wise · ordinal · point)

**Errata:** none yet.

Question (the consumer's): reranking ~24 items under a custom user prompt
where an adequate quality adjustment — not a certified order — is the bar,
which instrument gives adequate agreement with the canonical pairwise sort
at the lowest dollars per item? Instrument: `experiments/examples/setwise_cached.rs`
`--answer {bw,order}` (design: PROGRAM.md E6; climbed 2026-08-22).

Setup. `deepseek/deepseek-v4-flash` via OpenRouter, pinned to the five
providers of the manifund-deepseek lane, reasoning disabled, no logprobs.
n = 24 Manifund items (1600 chars each), k = 8, three attributes — two
rubric files (`impact_per_dollar`, `theory_of_change`) and one plain
user-prompt string ("fit for a funder who wants cheap high-leverage AI safety
field-building"). Chunk design: `--presentations` m rounds of seeded shuffle
→ 3 groups of 8; m = 3 ⇒ 9 calls per attribute, m = 6 ⇒ 18. Pools: seed 17
(m = 3 and m = 6, both modes) and seed 23 (m = 3, both modes; a different
24-item pool). Baseline each run: canonical_v2 `sort_documents`, default
budget = 96 comparisons per attribute. Six live runs, 54 + 108 + 54 = 216
setwise calls + 18 pairwise sorts, **$0.2735 total** (cap $1/run). Every
setwise call parsed (0 malformed, 0 refused, 0 errors); every observation
graph connected (1 component). Offline synthetic-judge dry run first
(`offline/`): `order` m = 3 recovered planted truth ρ 0.93–0.96 vs pairwise
0.93–0.96; `bw` 0.72–0.88; the untouched `ratio` arm reproduces.

## Agreement with the pairwise sort (Spearman ρ over 24 items)

| run | impact_per_dollar | theory_of_change | user prompt | $/item (setwise) | $/item (pairwise, 96 cmp) |
|---|---|---|---|---|---|
| order m=3 s17 | 0.84 | 0.78 | 0.85 | 1.6e-4 / 1.2e-4 / 1.1e-4 | 5.8e-4 / 5.1e-4 / 4.4e-4 |
| order m=6 s17 | 0.85 | 0.76 | 0.92 | 3.2e-4 / 2.3e-4 / 1.8e-4 | 5.4e-4 / 5.0e-4 / 4.3e-4 |
| order m=3 s23 | 0.64 | 0.73 | 0.80 | 1.7e-4 / 1.2e-4 / 0.9e-4 | 5.8e-4 / 5.1e-4 / 4.4e-4 |
| bw m=3 s17 | 0.46 | 0.41 | −0.08 | 1.6e-4 / 1.1e-4 / 1.1e-4 | 4.6e-4 / 4.5e-4 / 4.1e-4 |
| bw m=6 s17 | 0.32 | 0.76 | −0.18 | 3.2e-4 / 2.0e-4 / 1.8e-4 | 4.7e-4 / 4.6e-4 / 4.0e-4 |
| bw m=3 s23 | 0.12 | −0.02 | 0.19 | 1.7e-4 / 1.0e-4 / 0.9e-4 | 4.6e-4 / 4.4e-4 / 4.0e-4 |

Denominator for "adequate": the pairwise sort's own test–retest across the
same-seed runs (independent calls, same pool and prompt) is ρ 0.90 / 0.91 /
0.91 (seed 17, 6 pairs each; min 0.83) and 0.84 / 0.94 / 0.83 (seed 23, one
pair). `order` test–retest m = 3 vs m = 6 on the same pool: 0.88 / 0.90 /
0.94 — as stable as pairwise. `bw` m = 3 vs m = 6: 0.85 / 0.53 / 0.46.
`order` vs `bw` in the same run (m = 6): 0.20 / 0.72 / 0.04 — they are not
measuring the same thing on two of three attributes.

Cost. The first attribute of each run pays the prefix once more (no provider
cache evidence on this lane); later attributes ≈ 1.0e-4 $/item for 9 calls
(input-dominated: ~3.0k tokens in, 9 tokens out per `order` call, 3 per
`bw`). The pairwise arm at 96 comparisons ≈ 4.4e-4 $/item. So `order` at
m = 3 costs **~¼** of the pairwise sort and lands at or just under the
pairwise test–retest ceiling on two attributes (0.84 vs 0.90; 0.85 vs 0.91)
and below it on one (0.78 vs 0.91; seed-23 pool 0.64–0.80 vs 0.83–0.94).
`bw` at the same price is not an adequate instrument here.

## Position bias (measured, pooled over the three runs per mode, 108 calls, 13.5 expected per slot)

- `order` first-rank picks by slot A..H: 3, 9, 13, 16, 24, 15, 19, 9 —
  slots A–B under-picked (12/27 expected); last-rank picks: 7, 5, 12, 7, 13,
  20, 14, **30** — the last slot is ranked last 2.2× its fair share.
- `bw` best picks: 8, 6, 5, 20, 15, 22, 19, 13 — the first three slots
  under-picked (19/40.5 expected); worst picks: 8, 16, 6, 9, 14, 10, 19,
  **26** — last-slot again 1.9×.

A primacy-disfavour + recency-worst shape on both. Randomized slot order per
call keeps it from becoming an item effect, but it costs precision; a
slot-offset term or the paired presentation this design deleted would be
the fix if a later regime needs it.

## Reading

1. For the consumer's regime (adequate adjustment, custom prompt, ~24
   items), **plain listwise at k = 8 lowered into the solver is the
   instrument**: 9 calls, ~¼ the cost of the 96-comparison pairwise sort,
   agreement with pairwise at or near pairwise's own reliability, and
   test–retest as good as pairwise. The E5/E6 question "is a new best–worst
   instrument needed" answers **no** in this regime — the listwise arm that
   the climb kept as the efficiency denominator is the winner.
2. Best–worst — the "highest-value missing cell" of the instrument grid
   (docs/FIRST_PRINCIPLES.md §2) — is refuted as built: 13 observations per
   call vs 28 for `order` at the same input cost, and the worst pick is a
   weak, biased signal. The grid entry should carry this pack, not the
   prior.
3. Caveats, honestly: one model, one corpus family, n = 24, k = 8, m ≤ 6;
   the pairwise baseline is itself at ρ ≈ 0.9 reliability so agreement above
   that is not measurable here; no logprob/PMF arm (deleted in the climb —
   E7's question); the user-prompt attribute is one string.

## Addendum 2026-08-23 — order-sensitivity gauge + k sweep (llmsort@328d212, $0.13)

`--repeats 2` re-presents each chunk group in a second shuffled slot order;
the **order-sensitivity readout** is the fraction of entity pairs (ordered by
both presentations of the same subset) whose direction flips between the two
orders — a per-(model, attribute, domain) gauge costing one duplicate pass,
the thing to run FIRST in a new domain before trusting any k-wise sort.
Offline synthetic judge (σ = 0.35 nats): flip rate 0.14–0.22, so the metric
reads noise correctly. Live, deepseek-v4-flash, n = 24, m = 2, r = 2, k ∈
{4, 6, 8, 12} (`live/order-ksweep-r2/`, $0.082; `live/bw-k8-r2/`, $0.043):

| attribute | flip rate by k=4/6/8/12 (order) | ρ vs pairwise by k |
|---|---|---|
| impact_per_dollar | 0.28 / 0.40 / 0.36 / 0.33 | 0.67 / 0.85 / 0.81 / **0.44** |
| theory_of_change | 0.21 / 0.13 / 0.23 / 0.24 | 0.81 / 0.77 / 0.82 / 0.87 |
| user-prompt attr | 0.15 / 0.11 / 0.13 / 0.16 | 0.89 / 0.90 / 0.87 / 0.84 |

Readings. (1) The gauge separates attributes: the judge is order-flaky on
`impact_per_dollar` (~0.34 mean flip rate) and stable on the user-prompt
attribute (~0.14); the one ranking collapse in the sweep (k = 12,
impact_per_dollar, ρ 0.44) happened on the flakiest attribute — the gauge
flags exactly where a bigger k stops being safe. (2) k = 6–8 is the band:
k = 4 wastes calls for no reliability gain, k = 12 is fine on stable
attributes and unsafe on flaky ones. (3) Dollars per item are ~flat in k
(input-dominated: every item is read m·r times regardless of k), so k buys
pairs per call, not savings — take the largest k the flip rate tolerates.
(4) `bw` with repeats stays inadequate (ρ 0.30–0.58), unchanged verdict.
Denominators: flip rates on 63–264 compared pairs per cell (4–12 repeated
subsets); ρ on 24 items — single-run cells, not averaged.

## Addendum 2026-08-23 — robustness matrix: delimiter × size × model × corpus (llmsort@78d2a85+37ca9e4, $0.87)

Nine live runs (`live/run4.sh`), all `order` k = 8, n = 24, m = 2, r = 2,
seed 17, $1/run caps; 9/9 completed, every graph connected. New corpus:
`research/data/arxiv_abstracts.json` — 150 arXiv CS-2025 abstracts
(OpenAlex via Scry; title + abstract, 1.1–2.0k chars) with paper-native
attributes (methodological rigor / novelty / practitioner usefulness).
Cells report flip rate (gauge) and ρ vs same-run pairwise; per-attribute
order: impact_per_dollar / theory_of_change / user-prompt (manifund) or
rigor / novelty / usefulness (arXiv).

| run | flip rate | ρ vs pairwise | setwise $/item |
|---|---|---|---|
| delim-bracket (deepseek) | 0.29 / 0.18 / 0.10 | 0.85 / 0.78 / 0.88 | 1.3–2.1e-4 |
| delim-dash (deepseek) | 0.29 / 0.13 / 0.12 | 0.73 / 0.79 / 0.78 | 1.3–2.2e-4 |
| (xml = ksweep k=8 cell) | 0.36 / 0.23 / 0.13 | 0.81 / 0.82 / 0.87 | — |
| size-400 (deepseek) | 0.36 / 0.32 / 0.27 | 0.64 / 0.61 / 0.88 | 0.6–0.8e-4 |
| size-4800 (deepseek) | 0.21 / 0.17 / 0.18 | 0.74 / 0.68 / 0.82 | 4.2–5.6e-4 |
| size-8000 (deepseek) | 0.23 / 0.32 / 0.21 | 0.85 / 0.94 / 0.67 | 6.3–9.5e-4 |
| model-gpt41mini | 0.32 / 0.14 / 0.11 | 0.47 / 0.64 / 0.75 | 2.0–6.1e-4 |
| model-gemini25flash | 0.24 / 0.17 / 0.00* | 0.32 / 0.79 / 0.85 | ~4.9e-4 |
| arxiv-deepseek | 0.29 / 0.31 / 0.19 | 0.40 / 0.53 / 0.84 | 0.7–1.2e-4 |
| arxiv-gpt41mini | 0.21 / 0.10 / 0.18 | 0.76 / 0.74 / 0.76 | 1.4–3.5e-4 |

\* gemini-2.5-flash returned partial orders (4–5 letters, then stop) on
6/12 calls of the user-prompt attribute; the strict length-k parse
rejected them — a model-behavior finding, not silent damage (ρ 0.85 from
the 6 clean calls; the 0.00 flip rate is over few surviving pairs).

Readings.

1. **Delimiter is a free parameter.** xml / bracket / dash move flip rate
   and ρ within run-to-run noise (ρ spread ≤ 0.09 against a pairwise
   test–retest band of ±~0.06). Keep xml as default; nothing to tune.
2. **Entity size 400–8000 chars: the instrument holds; the gauge tracks
   the soft end.** ~100-token entities are the order-flakiest cells
   (flip 0.27–0.36) with the lowest rubric-attr agreement (0.61–0.64);
   1600–8000 stay in the adequate band. $/item is ~linear in size
   (input-dominated), so short entities are cheap AND flaky — the gauge,
   not the price, should pick the truncation.
3. **The instrument transfers across model families and corpora** —
   gpt-4.1-mini and gemini-2.5-flash on manifund, deepseek and
   gpt-4.1-mini on arXiv all produce usable sorts — but the weak cells
   move with (model, attribute): impact_per_dollar is order-flaky on
   every model (flip 0.24–0.36, ρ 0.32–0.47 on the new models);
   deepseek is flaky judging rigor/novelty from abstracts (ρ 0.40/0.53)
   where gpt-4.1-mini is uniform (0.74–0.76). Single-run pairwise
   baselines on the new cells, so low ρ conflates both instruments'
   noise — the flip rate is the per-cell signal that doesn't.
4. **The gauge is a one-sided screen (38 cells, all live runs):** every
   cell with flip < 0.20 has ρ ≥ 0.64 (median 0.79); every ρ < 0.61
   sits at flip ≥ 0.21. Operating rule: flip < 0.2 → trust the k-wise
   sort; ≥ 0.25 → drop k, average more presentations, or fall back to
   pairwise for that attribute.

Matrix cost $0.866; E6 running total ≈ $1.27.

**Repeat run, gpt-4.1-mini** (`model-gpt41mini-rep2`, $0.14): the
same-model pairwise test–retest band is 0.61 / 0.87 / 0.94 per attribute —
the pairwise baseline itself is unreliable on impact_per_dollar for this
model, exactly the attribute the gauge flags (flip 0.32/0.35 across the
two runs). Order-vs-pairwise ρ against that band: theory_of_change
0.64–0.68 vs 0.87, user-prompt 0.75–0.82 vs 0.94 — adequate at ~⅓–⅒ the
$/item, but genuinely a step below the band, unlike deepseek which sat at
its ceiling. Caveat: same seed ⇒ identical setwise prompts across the two
runs, and gpt-4.1-mini answered near-deterministically (order latents
correlate 0.99) — so the setwise "retest" measures determinism, not
independent stability; the flip rate over shuffled orders remains the
honest stability gauge.

## Addendum 2026-08-24 — gpt-5.6-luna (the search repo's model), $0.23

Three runs (`live/run5.sh`): manifund + repeat + arXiv, same design
(order k = 8, n = 24, m = 2, r = 2, seed 17).

| run | flip rate | ρ vs pairwise |
|---|---|---|
| model-gpt56luna | 0.22 / 0.15 / 0.11 | 0.70 / 0.81 / 0.89 |
| model-gpt56luna-rep2 | 0.17 / 0.11 / 0.12 | 0.74 / 0.81 / 0.92 |
| arxiv-gpt56luna | 0.25 / 0.15 / 0.20 | 0.69 / 0.91 / 0.78 |

1. **Luna's pairwise band is 0.94 / 0.95 / 0.94** — reliable on every
   attribute, including impact_per_dollar where gpt-4.1-mini's baseline
   broke (0.61). Order-vs-pairwise on the user-prompt attribute
   (0.89 / 0.92) sits at that ceiling; theory_of_change (0.81) is near;
   impact_per_dollar (0.70–0.74) below — and impact is exactly luna's
   flakiest gauge cell (flip 0.17–0.22, still the calmest any model
   showed on that attribute). 36/36 calls parsed per manifund run.
2. **Cost:** order ≈ 3.8e-4 $/item vs pairwise 1.0–1.3e-3 (~⅓). The
   rep2 run's setwise arm cost 3.8e-5 $/item — 10× less — because the
   identical prompt prefixes hit OpenAI's provider cache: live evidence
   of the cache-native prompt geometry paying off on repeat sorts.
3. This closes the gate's ≥ 2-model cell in the same sense deepseek
   held it: at/near the same-model pairwise ceiling on stable
   attributes at ≤ ½ the price, gauge flagging the rest.

## Addendum 2026-08-24 — E11 anchored-ring design, live ($0.18)

The disjoint design needs a second round only for graph connectivity; a
ring of cyclic windows (stride k−overlap, last wraps) is connected in one.
Live (`live/run6.sh`, n = 24, k = 8, overlap 2, seed 17, manifund):

| run | calls/attr | flip rate | ρ vs pairwise |
|---|---|---|---|
| ring-m1r2-deepseek | 8 | 0.22 / 0.21 / 0.09 | 0.84 / 0.81 / 0.84 |
| ring-m1r1-deepseek | 4 | — (no repeats) | 0.71 / 0.70 / 0.79 |
| ring-m1r2-luna | 8 | 0.26 / 0.17 / 0.12 | 0.81 / 0.88 / 0.91 |
| (disjoint m2r2 cells) | 12 | see above | deepseek 0.81–0.87, luna 0.70–0.92 |

Structure substitutes for the second round: ring at 8 calls matches the
12-call disjoint agreement on both models; 4 calls is the connected
adequacy floor (ρ 0.70–0.79) but gauge-blind. Consequence shipped: the
crate's `rerank::setwise` defaults to `SetwiseDesign::Ring`, rounds 1,
repeats 2 — verified live through `setwise_api_check`: 8/8 parsed, gauge
0.11, $0.0054 for a 24-item sort (was $0.0097 under the disjoint
default). Caveat: single live run per cell; offline synthetic cells agree
(ring m1r2 0.81–0.94 vs disjoint m2r2 0.89–0.92).

Replay: `report.json` + `trace.jsonl` (+ `pairwise_trace.jsonl`) per run
under `live/<mode>-m<m>[-s23]/`; offline packs under `offline/`. Runner
scripts mirrored in `live/run.sh`, `live/run2.sh`, `live/run3.sh`,
`live/run4.sh`, `live/run5.sh`, `live/run6.sh`.
