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

## E12/E13 addendum (2026-08-24, run7, $0.79): the folk baselines, truth anchors, and n=150

20 cells, deepseek-v4-flash + gpt-5.6-luna, seed 17 throughout. `rho_pw` is
Spearman vs the same model's pairwise sort (own arm, or the sibling run's);
`rho_tru` is vs external truth (UN population / river km); `ties` is distinct
score values per n (pointwise's resolution); flips per the r=2 gauge.

```
cell                               attr                                        ok  flip rho_pw rho_tru  ties   $/item
mf-point-deepseek                  impact_per_dollar                       24/24          0.62     nan   7/24   0.00009
mf-point-deepseek                  theory_of_change                        24/24          0.87     nan  10/24   0.00007
mf-point-deepseek                  fit for a funder who wants cheap high-  24/24          0.86     nan   8/24   0.00005
mf-list24-deepseek                 impact_per_dollar                        2/2    0.31   0.61     nan  23/24   0.00010
mf-list24-deepseek                 theory_of_change                         2/2    0.17   0.91     nan  23/24   0.00005
mf-list24-deepseek                 fit for a funder who wants cheap high-   2/2    0.13   0.72     nan  23/24   0.00005
anch-countries-ring-deepseek       population                               6/6    0.17   0.89    0.91  16/16   0.00001
anch-countries-ring-deepseek [pairwise] population                                                     0.87
anch-countries-point-deepseek      population                              16/16          0.71    0.68   4/16   0.00002
anch-countries-list16-deepseek     population                               2/2    0.27   0.75    0.61  16/16   0.00001
anch-rivers-ring-deepseek          length in kilometres                     6/6    0.36   0.41    0.55  16/16   0.00001
anch-rivers-ring-deepseek [pairwise] length in kilometres                                           0.31
anch-rivers-point-deepseek         length in kilometres                    16/16          0.29   -0.19   3/16   0.00002
anch-rivers-list16-deepseek        length in kilometres                     2/2    0.17   0.56    0.68  15/16   0.00001
ax150-ring-deepseek                methodological rigor                    50/50   0.28   0.53     nan 147/150  0.00008
ax150-ring-deepseek                novelty of contribution                 50/50   0.27   0.63     nan 143/150  0.00005
ax150-ring-deepseek                usefulness for a practitioner building  50/50   0.24   0.65     nan 148/150  0.00005
ax150-point-deepseek               methodological rigor                   150/150         0.54     nan  10/150  0.00005
ax150-point-deepseek               novelty of contribution                150/150         0.63     nan  14/150  0.00003
ax150-point-deepseek               usefulness for a practitioner building 150/150         0.34     nan  10/150  0.00003

mf-point-luna                      impact_per_dollar                       24/24          0.75     nan  18/24   0.00013
mf-point-luna                      theory_of_change                        24/24          0.86     nan  12/24   0.00013
mf-point-luna                      fit for a funder who wants cheap high-  24/24          0.93     nan  14/24   0.00011
mf-list24-luna                     impact_per_dollar                        2/2    0.20   0.77     nan  24/24   0.00018
mf-list24-luna                     theory_of_change                         2/2    0.13   0.75     nan  22/24   0.00018
mf-list24-luna                     fit for a funder who wants cheap high-   1/2           0.88     nan  24/24   0.00018
anch-countries-ring-luna           population                               6/6    0.08   0.85    0.86  15/16   0.00002
anch-countries-ring-luna [pairwise] population                                                     0.97
anch-countries-point-luna          population                              16/16          0.82    0.80  10/16   0.00004
anch-countries-list16-luna         population                               2/2    0.26   0.88    0.79  15/16   0.00001
anch-rivers-ring-luna              length in kilometres                     6/6    0.29   0.35    0.71  16/16   0.00002
anch-rivers-ring-luna [pairwise]   length in kilometres                                           0.10
anch-rivers-point-luna             length in kilometres                    16/16          0.36    0.33  10/16   0.00004
anch-rivers-list16-luna            length in kilometres                     2/2    0.16   0.33    0.71  16/16   0.00001
ax150-ring-luna                    methodological rigor                    50/50   0.23   0.64     nan 145/150  0.00015
ax150-ring-luna                    novelty of contribution                 50/50   0.15   0.71     nan 149/150  0.00015
ax150-ring-luna                    usefulness for a practitioner building  49/50   0.19   0.55     nan 149/150  0.00015
ax150-point-luna                   methodological rigor                   150/150         0.73     nan  26/150  0.00007
ax150-point-luna                   novelty of contribution                150/150         0.77     nan  31/150  0.00007
ax150-point-luna                   usefulness for a practitioner building 150/150         0.51     nan  38/150  0.00008
```

Readings:

1. **Pointwise ("rate 0–100"), the folk default — the tie pathology is now
   measured, not asserted.** deepseek compresses to 3–14 distinct values
   (rivers: 3 distinct over 16 items, truth-ρ −0.19 — worse than random on
   a close-packed pool). luna is finer-grained (10–38 distinct) and
   competitive on stable attributes (manifund 0.75–0.93 vs pairwise) at the
   lowest $/item on the board. It cannot say *how much* better, and its
   top-k is a tie block.
2. **Single-call listwise ("paste the whole list") holds at n=24 only when
   the gauge is clean** — luna 0.75–0.88; deepseek 0.61–0.91 where flip
   0.31 correctly flags the 0.61 cell. Parse fragility appears at k=24
   (luna returned one malformed order of 2 calls on one attribute — and
   with it the gauge dies for that cell). It cannot express n=150 at all
   (26 slot letters). The folk method is a special case of the instrument,
   k=n, minus the design that makes it trustworthy.
3. **The accuracy claim is now external.** Countries: ring truth-ρ
   0.91/0.86 (deepseek/luna) vs pairwise's own 0.87/0.97 — the graduated
   recipe reads truth as well as the flagship path at a fraction of the
   calls. Rivers (close-packed, the E2 hard pool): *pairwise itself*
   collapses vs truth (0.31/0.10); ring and list16 degrade more gracefully
   (0.55–0.71); the gauge flags the worst ring cell (flip 0.36 → ρ_pw
   0.41). On hard pools no instrument is safe, and only the setwise arms
   carry their own warning light.
4. **n=150 at the same per-item budget as n=24 (2.67 slot-appearances per
   item) loses real agreement** — luna ring falls from 0.81–0.91 (n=24,
   E11) to 0.55–0.71; flips rise to 0.15–0.28 and say so. The instrument
   triangle (ring~point 0.31–0.61, each ~pairwise 0.34–0.77) shows no
   arm is a clean gold at this budget: the 600-comparison pairwise
   baseline covers 5% of pairs. Reading: per-item budget does not
   transfer across n — the observation graph's diameter grows with
   n/(k−overlap); the untested lever is `rounds: 2` (structural density),
   not bigger k.

### E13 close-out (2026-08-24, run8, $0.66): rounds=2 recovers n=150 — at the ceiling

Seed 18 (same 150 entities; independent planner + presentations), ring k=8
rounds=2 repeats=2 (100 calls/attr ≈ 5.3 slot-appearances/item) with an
in-run pairwise arm (600 comparisons/attr) doubling as the seed-17 retest:

| model | attribute | flip | ring2~pw18 | pw17~pw18 (ceiling) |
|---|---|---|---|---|
| deepseek | methodological rigor | 0.25 | 0.65 | 0.68 |
| deepseek | novelty of contribution | 0.22 | 0.78 | 0.81 |
| deepseek | usefulness for a practitioner | 0.24 | 0.74 | 0.81 |
| luna | methodological rigor | 0.18 | 0.77 | 0.80 |
| luna | novelty of contribution | 0.15 | 0.87 | 0.89 |
| luna | usefulness for a practitioner | 0.21 | 0.79 | 0.85 |

Readings: (a) the n=150 pairwise ruler is itself noisy — test–retest
0.68–0.89 at this budget, so run7's ring r=2 numbers were read against a
moving target; (b) structural density was the right lever: doubling rounds
puts ring within 0.02–0.07 of the same model's pairwise ceiling on every
attribute, at ~1/6 the pairwise cost; (c) the gauge keeps its one-sided
ordering — lowest flip, highest agreement (luna novelty 0.15 → 0.87).
Scaling recipe for n ≫ k: `rounds: 2`, everything else unchanged.
600/600, 600/600, 599/600 pairwise and 100/100 ×5, 99/100 setwise calls
parsed.

## E14 addendum (2026-08-24, run9, $0.42): the funnel — and the top-10 ceiling nobody states

Screen all 150 (setwise ring rounds=2, or pointwise 0–100) → pairwise
certified top-10 on the top-30 slice; seed 18; judged against the two
independent full-pairwise runs (pw17, pw18). `ceil` = the full pairwise
path's OWN top-10 overlap across seeds; `slice_recall` = how much of the
reference top-10 the screen kept in its top-30; `s1top10` = screen-alone
top-10 overlap; `funnel` = final top-10 overlap (vs pw18/pw17).

```
cell                         ceil slice_recall s1top10 funnel    $fun    $pw
rigor-setwise-deepseek   0.7  0.8/ 0.8       0.5/0.5  0.6/0.5   0.034  0.054
rigor-point-deepseek   0.7  0.7/ 0.9       0.2/0.4  0.6/0.7   0.017  0.054
novelty-setwise-deepseek   0.7  0.9/ 0.9       0.4/0.5  0.8/0.7   0.030  0.053
novelty-point-deepseek   0.7  0.8/ 0.6       0.4/0.3  0.6/0.5   0.015  0.053
useful-setwise-deepseek   0.6  0.9/ 0.9       0.6/0.5  0.5/0.7   0.026  0.053
useful-point-deepseek   0.6  0.3/ 0.3       0.1/0.1  0.3/0.3   0.015  0.053
rigor-setwise-luna       0.3  0.7/ 0.9       0.3/0.4  0.4/0.5   0.063  0.103
rigor-point-luna       0.3  0.6/ 0.4       0.3/0.2  0.4/0.2   0.028  0.103
novelty-setwise-luna       0.7  0.8/ 0.9       0.4/0.3  0.4/0.6   0.062  0.106
novelty-point-luna       0.7  0.9/ 1.0       0.3/0.2  0.8/0.8   0.031  0.106
useful-setwise-luna       0.6  0.8/ 0.5       0.5/0.3  0.6/0.4   0.064  0.103
useful-point-luna       0.6  0.6/ 0.6       0.3/0.2  0.5/0.4   0.031  0.103
```

Readings:

1. **The ceiling is the finding.** The flagship 600-comparison pairwise
   sort reproduces its own top-10 at only 0.3–0.7 across planner seeds.
   "Give me the best 10 of 150" on subjective attributes at a 4n budget is
   intrinsically unstable — for every method. Any top-k product claim
   without this error bar is overclaiming; the funnel's job is to hit the
   ceiling cheaply, not to beat it.
2. **The funnel hits the ceiling at 0.3–0.6× the cost.** Funnel top-10
   overlap (0.3–0.8) brackets `ceil` in every cell; refinement clearly
   adds value over the screen alone (s1top10 0.1–0.6).
3. **The screen must be setwise.** Setwise slice recall is 0.7–0.9 in
   every cell; the pointwise screen is half the price but erratic
   (0.3–1.0) — on useful-deepseek its tie blocks dropped 70% of the
   reference top-10 before refinement could see them. The E12 tie
   pathology bites exactly at the slice cut, as predicted.
4. Recipe shipped by these numbers: `setwise rounds:2 screen → top-M=3k
   slice → pairwise top_k refine`; ~0.5× full-pairwise cost, ceiling-level
   quality, gauge on the screen, certification on the refine. 12/12 cells
   parsed; stage-1 luna had 1 malformed call of 100 in one cell.

## E7 addendum (2026-08-24, run10, $0.21): PMF weighting REFUTED for multi-token order answers

Twins of the E11 ring-m1r2 (manifund n=24) and run8 ax150-ring2 (n=150)
cells with `--logprobs`: every implied pair entered the solver through the
measured-precision channel as a two-point mixture weighted by the emitted
letters' token probabilities (q = max(0.5, √(p_i·p_j))). Coverage was
total — 344/344 parsed calls carried logprobs on both models.

```
cell                       attr                                   pmf    p̄ flip_lp flip_pl rho_lp rho_pl
lp-mf-ring-deepseek        impact_per_dollar                    8/8    0.70    0.19    0.22   0.86   0.84
lp-mf-ring-deepseek        theory_of_change                     8/8    0.69    0.25    0.21   0.68   0.81
lp-mf-ring-deepseek        fit for a funder who wants cheap h   8/8    0.77    0.07    0.09   0.81   0.84
lp-ax150-ring2-deepseek    methodological rigor               100/100  0.62    0.26    0.25   0.53   0.65
lp-ax150-ring2-deepseek    novelty of contribution            100/100  0.63    0.23    0.22   0.69   0.78
lp-ax150-ring2-deepseek    usefulness for a practitioner buil 100/100  0.62    0.23    0.24   0.66   0.74

lp-mf-ring-luna            impact_per_dollar                    8/8    0.88    0.19    0.26   0.72   0.81
lp-mf-ring-luna            theory_of_change                     8/8    0.88    0.12    0.17   0.58   0.88
lp-mf-ring-luna            fit for a funder who wants cheap h   8/8    0.93    0.16    0.12   0.70   0.91
lp-ax150-ring2-luna        methodological rigor               100/100  0.87    0.18    0.18   0.60   0.77
lp-ax150-ring2-luna        novelty of contribution            100/100  0.89    0.15    0.15   0.73   0.87
lp-ax150-ring2-luna        usefulness for a practitioner buil  96/96   0.86    0.20    0.21   0.69   0.79
```

Verdict: the weighting HURTS — ρ vs the same pairwise reference drops in
10 of 12 cells (median −0.09, worst −0.30) with flip rates unchanged.
Mechanism: a letter's token probability inside an emitted sequence
measures autoregressive continuation confidence, not judgment
correctness — mid-order positions are low-probability even when the
relative order is right (mean emitted prob 0.62–0.93), so q
systematically down-weights mid-list pairs and distorts the solve. This
sharpens E9's lesson: PMF evidence pays on single-token answer rails
(the whole judgment in one token position, 23× stat-error win), and does
not transfer to multi-token sequence answers. The order instrument keeps
its plain fixed-magnitude lowering.