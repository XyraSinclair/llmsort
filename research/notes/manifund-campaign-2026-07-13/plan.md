# Manifund Judgment Campaign

Goal: demonstrate cardinal-harness value to Manifund (Austin) with a campaign
of LLM judgment over the full Manifund corpus. Stakes: ~$100k Manifund grant.
Crawl approval: Austin, verbal, 2026-07-13. Data source: the public API
(`manifund.org/api/v0/{projects,comments}`) — no crawler needed; exopriors-core
has no manifund source registered (checked 2026-07-13; adding one to
`sources.toml` as `paged_json` is a separate, cheap follow-up that would also
give scry a Manifund relation).

Budget ceiling: **$50 OpenRouter** (operator decision 2026-07-13). Residual
analysis runs **both directions** (underrated and overrated) — the tool must
not read as a flattery machine.

## Corpus (fetched 2026-07-13, denominators)

1,266 real projects (3 dummies dropped): 109 live proposals, 708 never funded,
423 with USD raised (p50 $10k, max $437k), 168 complete, 140 certs.
Cohorts with human decision ground truth:

| cohort | n | funded | truth signal |
|---|---|---|---|
| ACX Grants 2024 | 83 | 41 | Scott's process picked winners |
| ACX Grants 2025 | 41 | 41 | survivors only (selection, weak) |
| EA Community Choice | 78 | 57 | crowd quadratic matching |
| live slate | 109 | — | the actionable frontier |

Raw data: `data/manifund/{projects,comments}.jsonl` (rebuild:
`scripts/manifund_fetch.py`). Judgment inputs: `data/manifund/items/*.json`
(+ `ground_truth.csv`) via `scripts/manifund_prep.py`; item text capped at
8,000 chars with explicit `[truncated]` marker.

## What is to be judged: the target ontology

Judgment targets, ordered from atomic to composite. Each row names the entity
set, the latent worth carving, and the ground truth (if any) that makes the
judgment refutable.

1. **Projects** (the page: title+blurb+ask+description).
   Latents: theory of change, impact/$ at min ask, team evidence,
   tractability, counterfactuality, epistemic integrity, downside risk (gate).
   Truth: funded-or-not per cohort; dollars raised; stage=complete.
2. **Funding decisions** (project, amount) pairs — judge the *decision*, not
   the project: "was $X the right size?" Latents: over/under-funding at the
   observed amount. Truth: none direct; residuals against judged impact/$ are
   the product (underrated/overrated lists, both directions).
3. **Creators** (143 with 2+ projects) — track-record entities assembled from
   their project histories. Latents: execution follow-through (do their
   'active' projects reach 'complete'?), ambition calibration. Truth: observed
   completion rates.
4. **Regrantor/commenter reasoning** (comments corpus) — the quality of the
   *evaluative discourse*. Latents: does the comment surface a decision-
   relevant consideration; does it change the correct valuation? This judges
   Manifund's evaluation layer itself — the most novel target on the list.
5. **Attributes themselves** — `weigh` (importance for the goal), `canonize`
   (which wording transmits the same latent across judge models). The
   campaign's rubric is measured, not asserted.
6. **Cause-area portfolios** — aggregate judged quality vs aggregate dollars
   by cause; where is Manifund's marginal dollar best/worst deployed?
7. **Cohort standards drift** — same attributes measured across ACX 2024 vs
   2025 vs EA CC: did the bar move?

The headline analysis crosses (1) and (5): `weigh` gives the community's
*professed* values; `explain` against the realized funding order gives its
*revealed* values. The gap, and the projects stranded in it, is the story no
Likert rubric can tell.

## Low-level toolkit: verb → epistemic move

For agents lost in the sauce: each verb is a cheap, transparent move that
carves the latent value space. All run against `data/manifund/items/*.json`.
`BIN=./target/release/cardinal`.

| verb | move | Manifund invocation |
|---|---|---|
| `judge` | one pairwise ratio, full prompt visible — the microscope | `$BIN judge @a.txt @b.txt --by "impact per marginal dollar" --show-prompt` |
| `elaborate` | terse criterion → precise rubric (1 call, composes) | `$BIN elaborate --by "counterfactuality of the funding ask"` |
| `sort` | one attribute, one cohort, best-first list | `$BIN sort data/manifund/items/acx2024.json --by "theory of change plausibility"` |
| `distinguish` | why does THIS project stand out? percentile+z per attribute | `$BIN distinguish data/manifund/items/live.json --focus <slug> --propose 5` |
| `slate` | stakeholder-blind attribute proposal for one project | `$BIN slate @project.txt --stakeholders "regrantor,donor,beneficiary,field expert"` |
| `weigh` | AHP: professed importance of attributes for the goal | `$BIN weigh --goal "allocate Manifund's marginal $10k for maximal impact" --propose 8` |
| `canonize` | which attribute wording induces the same latent across judges | `$BIN canonize data/manifund/items/eacc.json --by "epistemic integrity" --judges <m1>,<m2>` |
| `explain` | which attributes reconstruct a believed/realized order | `$BIN explain funded_order.json --candidate ... --propose 4` |
| `anp` | criteria network with feedback; limiting vs direct weights | `$BIN anp data/manifund/items/live.json --goal ... --propose 5` |
| `rerank` | the full multi-attribute engine with top-k stopping + traces | `$BIN rerank --request data/manifund/requests/live-slate-4attr.json --out ... --trace ...` |

Draft campaign request (validated offline):
`data/manifund/requests/live-slate-4attr.json` — 109 live proposals × 4
attributes (theory_of_change .30, impact_per_dollar .30, team_evidence .25,
epistemic_integrity .15), top-20 @ tolerated_error 0.15, budget 1,600
comparisons.

## Phases and spend gates

Each phase gates the next; measured cost replaces estimates at every step.

- **P0 — comfort, ≤$2.** Single `judge` calls with `--show-prompt` on
  hand-picked Manifund pairs (a funded/unfunded ACX pair; an obvious
  quality gap; a hard tie). Read the prompts and answers. `elaborate` each
  campaign attribute; keep the rubrics. Sanity: do ratios and stated
  reasoning survive inspection?
- **P1 — attribute layer, ~$5.** `weigh --goal` (professed weights),
  `slate` on 2 projects (stakeholder attribute proposals), `canonize` the
  top 3 attribute wordings on the EA CC cohort with 2 cheap judges. Output:
  the measured rubric for P2+, with transmissibility evidence.
- **P2 — retrodiction gauntlet, ~$15.** ACX 2024 cohort (83): multi-rerank
  on the canonized attributes; AUC + Spearman vs funded/not and vs dollars.
  `explain` with the realized funding order to get revealed weights.
  Disagreement shortlist both directions. Replicate cheap on EA CC (78).
- **P3 — live slate, ~$15.** The 109 open proposals, 4 attributes, top-20
  precision. Output regrantors can act on today, every score tracing to
  judgment packets.
- **P4 — atlas, ~$10 + $0 render.** Committed HTML under
  `artifacts/live/manifund-atlas-2026-07-13/` (corpus-map pattern): slate
  leaderboard, retrodiction scatter, professed-vs-revealed weights, residual
  lists (both directions), cost ledger + denominators on the page, errata
  block on top. Local server only — never a claude.ai Artifact (repo ban).

Total ≤ $47, inside the $50 ceiling with measurement replacing estimates at
each gate.

## Discipline (per research/notes/PRINCIPLES.md)

- Every judged score links to its packet/trace; no naked numbers in the atlas.
- Denominators on every claim (n judged / n corpus, comparisons spent, $).
- Retrodiction is registered *before* looking: predict ACX 2024 funded set,
  then unblind. Report the AUC whatever it is.
- Truncation is visible to judges (`[truncated]` marker) and reported.
- Downside risk is a gate, not a weighted term — flagged, never averaged away.
