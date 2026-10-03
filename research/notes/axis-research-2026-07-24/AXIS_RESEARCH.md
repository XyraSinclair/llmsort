# Axis Research — the lens catalog as the research object

2026-07-24. The OpenPriors thesis update: the scarce asset is not the row
machine (MEAV plumbing exists) but a small canon of judgement axes so
well-chosen that rankings under them are inherently arresting. This note
starts the autoresearch program for finding them.

Register examples, from the operator: `connection_to_the_end_of_time`,
`ultrawattagedness`. What distinguishes this register from the v1
vocabulary (corpus_v3's 46 axes: actionability, epistemic_rigor,
shareability, …) is **big-model-smell**: a frontier model has dense latent
structure for the judgement; a small model decoheres into a cheap proxy
(vocabulary, length, sentiment, performed intensity).

## Admission rubric — six measurable criteria

An axis enters the canon when it clears all six. Every criterion maps to
an instrument verb that already ships.

1. **Transmissibility** (`cardinal canonize`): the axis induces the same
   cardinal latent in different frontier minds — high cross-judge Spearman
   among big models. An axis only one model can see is a curiosity, not a
   canon entry.
2. **Tier divergence** (the big-model-smell signature, this note's probe):
   frontier↔frontier rank agreement high, frontier↔small agreement low,
   with the divergence CONCENTRATED ON DECOYS — items engineered to score
   high on the cheap proxy and low on the real thing. Smell = the gap.
3. **Orthogonality** (`attribute_correlations`): low |ρ| against the cheap
   basis (quality, clarity, length, popularity, recency, sentiment) on a
   probe set. Differentiation is measurable as residual rank information.
4. **Invariance** (the framing battery): survives swap, polarity, spin —
   belief, not echo. Priced in nats like everything else.
5. **Phrasing coherence** (axis families `#a/#b/#c`): rank agreement across
   independent wordings of the same intent — already the exopriors
   admission evidence shape.
6. **Arrest** (editorial, the only human gate): the top-10 list under this
   axis stops a smart stranger. No metric substitutes for this; it is the
   operator's taste applied last, after the machine gates.

## The autoresearch loop

The canonical-attribute loop (FIRST_PRINCIPLES §8) pointed at the axis
space itself:

1. **Over-generate** candidates from disjoint lenses (deep-time/eschatology,
   wattage/agency, epistemic depth, demand-side) — fanned-out big-model
   generation, each candidate carrying its own predicted confounder.
2. **Screen on paper**: kill duplicates (candidate↔candidate correlation
   run), kill anything whose definition is secretly "quality".
3. **Probe empirically**: for each survivor, a 12-item decoy-planted micro
   sort across one small + two frontier models → criteria 2 and 3 for the
   price of ~72 comparisons per axis.
4. **Battery**: survivors get the invariance battery + phrasing variants +
   canonize transmissibility.
5. **Editorial**: operator reads top-10s; arrest or death.
6. Repeat with the residual prompt: "an axis that distinguishes items the
   current canon ties."

Each admitted axis lands in the ledger with its admission evidence — the
axis page IS its provenance (definition, hash, phrasings, coherence,
invariance nats, tier-divergence profile, orthogonality matrix).

## Probe 1 — tier divergence, pre-registered (2026-07-24)

12-item set (`probe-set.txt`) with planted decoys:

- item 2: **cosmic slop** ("grand tapestry of the cosmos…") — high cosmic
  vocabulary, zero real end-of-time content.
- item 4: **performed intensity** ("4:47am cold plunge…") — high intensity
  vocabulary, zero actual wattage.
- items 1/5/7/11: quiet-eternal (allotted hydrogen, Reed-Solomon/Voyager,
  Euclid, proton decay) — real end-of-time content without grandiosity.
- items 3/8: real wattage in plain clothes (kernel-lock review, couch-payroll
  founder note).
- items 6/10/12: controls (pump manual, corporate slop, porch light).

Axes: `end_of_time`, `ultrawattagedness`, `clarity` (control).
Models: claude-opus-4.6, gpt-5.6-sol (frontier), gpt-5.4-mini (small).
Budget 24 comparisons per run, 9 runs.

**Predictions, before results:**

- P1: On both deep axes, frontier↔frontier Spearman > frontier↔mini
  Spearman by a wide margin (≥0.25).
- P2: Divergence concentrates on the decoys: item 2 rises ≥3 rank places
  under mini vs frontier on `end_of_time`; item 4 rises ≥3 places under
  mini on `ultrawattagedness`.
- P3: On `clarity`, all three models roughly agree (Spearman ≥0.7 across
  every pair) — the control shows the divergence is axis-specific, not
  model noise.
- P4: Frontier `end_of_time` and `clarity` rankings are weakly correlated
  (|ρ| < 0.4) — orthogonality.

Results: see `RESULTS.md` (written after the runs; predictions above are
frozen).

## Prior art this builds on

- `research/notes/FIRST_PRINCIPLES.md` §8 — canonical-attribute loop; `canonize`
  transmissibility (cross-judge Spearman) already shipped.
- `notes/ideation-2026-07-05/differentiation.md` — cross-model belief
  cartography; taste vectors as first-class objects.
- corpus_v3 (`~/projects/archive/p1-xyra-sh/corpus/db/corpus_v3.db`) —
  14,013 entities × 46 axes with provenance: the v1 register, useful as
  the "cheap basis" to measure orthogonality against and as evidence of
  which axes the operator actually reached for over a year of curation.
