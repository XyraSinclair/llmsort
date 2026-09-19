# jev-highdim: ranking signal from Jev on hard attributes, one lever at a time (2026-09-19)

Readers: Xyra and agents continuing this. Executed 14:15–14:45 PT. Judge: TypeSafe `jev-latest`. Total Jev spend
$0.36 (468 calls). Fable spend: 8 subagent reads, 518K tokens, once.

Why this pack exists: `jev-bits-2026-09-19` measured instruments on easy criteria and found every readout at the
judge's ceiling, so nothing could separate them. Here the attributes are hard and high-dimensional, the reference
is strong, and Jev starts well under it, so a lever that adds signal shows.

## Cohorts, attributes, reference

24 Manifund grant applications (2,500–6,000 chars) and 24 arXiv abstracts (from `multi-criteria-bench`), seed 19.
12 attributes from `research/batteries/highdim_attributes_elaborated.txt`: self-containedness, high-status,
low-status, poshness, technical calmness, earnestness, intellectual density, legibility to an outsider, coiled
potential energy, craftedness, institutional insiderness, rewards a careful re-read.

Reference: Fable magnitude estimation (ratio scale, median text = 1.0), two independent replicas per cohort, each
with its own item shuffle, neutral labels, and attribute grouping and order; a replica is two reads of six
attributes. `ref.py` pools log magnitudes. Replica agreement (Spearman) is the reference's reliability:

- Manifund: .88–.98 on every attribute, mean .93 (Spearman-Brown for the pooled mean .94–.99).
- arXiv: mean .77. Nine attributes .85–.95; soft on self-containedness .56, earnestness .62, poshness .69, and
  institutional insiderness .24, where abstracts barely differ (log sd .17–.21). Treat arXiv insiderness as unscored.
- The attributes are not one axis: first principal component 44 % (Manifund), 53 % (arXiv). Near-duplicates:
  re-read ~ intellectual density +.93 to +.95; high-status ~ low-status −.78 to −.94.

The Manifund half is public here by operator decision (2026-09-19 14:41 PT: the applications are public and the
judgments are interesting to have alongside them): `manifund.json` carries each item's id `m00`–`m23`, its project
slug and its text; the Fable magnitudes and Jev results are keyed by that id.

## Fixed across steps

Same k=8 windows for every variant (seed 7, 3 rounds, 9 states a cohort), so variants differ only in the questions.
rho = Spearman of the fitted latents against the Fable reference, mean over the 12 attributes.

## Steps

| step | what changed | calls | $ | rho arXiv | rho Manifund |
|---|---|---|---|---|---|
| ceiling | Fable replica vs replica | — | — | .77 | .93 |
| `bare` | attribute NAME only; noul + score9 + rate | 108 | .062 / .083 | .57 / .49 / .47 | .58 / .46 / .49 |
| `elab` | one-paragraph definition beside each question | 108 | .081 / .103 | .65 / .68 / .63 | .70 / .68 / .73 |
| `decomp` | 5 concrete yes/no propositions per attribute, per item | 9 | .005 / .007 | .62 | .67 |
| `elab` + `decomp` | z-mean of the two latents | 117 | .086 / .110 | .69 | .74 |
| `decomp10` | 10 propositions | 9 | .009 / .010 | .60 | .68 |
| `decomp10` pruned | drop item-rest r < .2, no reference used | 9 | same | .67 | .68 |

(three numbers in a cell = noul / score9 / rate.)

1. **The definition is worth +.08 to +.24** (least on the noul, most on the graded fields). Largest on Manifund self-containedness (.24 → .77) and arXiv technical
   calmness (.48 → .74). A bare name is not a criterion.
2. **Polarity again.** Bare "low-status" inverted both graded fields on both cohorts (score9 −.69 / −.51, rate
   −.59 / −.74) while the noul held (+.68 / +.69): "how strong on low-status" reads as status. The definition fixed
   it (+.75 / +.86). Graded fields need the direction spelled out; the noul is the robust one.
3. **Instruments still tie** on hard attributes (within .05 of each other under `elab`), and their z-mean is no
   better than the best single one. The readout is not the lever; this repeats jev-bits finding 3 where it could have failed.
4. **Concrete propositions are the cheap channel.** One call per window carrying 480 item-level nouls ("Item 3
   contains irony, sarcasm or jokes") reaches .62 / .67 for half a cent, about a fifteenth of the abstract pairwise
   arm's tokens, with signs fixed in advance and equal weights. Best where the abstract question was worst:
   Manifund earnestness .32 → .50, self-containedness .77 → .91, insiderness .66 → .82; arXiv poshness .23 → .50,
   re-read .78 → .91.
5. **It carries different errors.** Adding it to the abstract arm gives .69 / .74 (+.03 / +.04), where three
   instruments on the same abstract question added nothing. Distinct questions about an item add; distinct
   readouts of one question do not.
6. **More propositions did not help; better ones do.** Subsets of the first five rise steadily (.37 → .62,
   .47 → .67), but a second hand-written five gave .60 / .68: they were weaker, and an equal-weight sum is hurt by
   a wrong-signed member (arXiv self-containedness .59 → .29). Pruning by item-rest correlation, which needs no
   reference, recovered arXiv to .67 (self-containedness back to .59, coiled potential energy .63 → .81) and left
   Manifund at .68.
7. **Still hard after every lever:** arXiv poshness (.22–.50) and craftedness (.34–.51), Manifund earnestness
   (.32–.62) and poshness (.50–.63). Fable agrees with itself at .69–.94 on these, so the gap is Jev's.

## What to try next, in order

1. Propositions written by Fable from the definition plus a few high and low reference items, instead of by hand
   cold; then the reference-free prune. One read a cohort.
2. Weights for the propositions fitted on one cohort and tested on the other (nothing here was fitted).
3. Two-phase: a short Fable or Jev-choice analysis note per item placed in the state, then the abstract question.
4. A LessWrong comment cohort; gemma as a second reference beside Fable.

Replay: `gunzip -k trace-arxiv.jsonl.gz`, then `ref.py`, `step.py arxiv elab|bare`, `decomp.py arxiv
[decomp10.json]`, `props_curve.py`, `prune.py` need no key; the same with `manifund` after `gunzip -k trace-manifund.jsonl.gz`.
