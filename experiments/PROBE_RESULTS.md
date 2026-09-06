# Embedding-probe distillation: measured verdict (2026-09-06)

Question: can a ridge probe on voyage4-nano (2048-dim) embeddings reproduce
gemma4-31b's ranking on `novel-world-expanding-hit`, enabling ~1000x corpus-wide
propagation without per-entity judging?

## Anchor reliability (the prerequisite)

Four wording variants (#a definitional, #b operational, #c counterfactual,
#d auditor) each judged at N=200 on lesswrong-posts (top-200 cohort),
8N comparisons each, gemma4-31b, canonical_v2.

Pairwise Spearman between wordings: 0.763-0.911, mean 0.835.
Spearman-Brown reliability of the 4-wording mean: 0.953.
Split-half (a+b vs c+d): 0.942.

So at N=200 budget the anchor is highly reliable. (The earlier 0.426
test-retest was N=20 vs N=200 — the N=20 side is the noise. N=20 runs are
loop-probes, not targets.)

## Probe result (fused target, 200 entities, 5-fold CV)

| lambda | held-out Spearman | top-decile overlap |
|-------:|------------------:|-------------------:|
| 10     | +0.023 | 0/20 |
| 100    | +0.038 | 0/20 |
| 1000   | +0.058 | 2/20 |
| 10000  | +0.072 | 5/20 |

**Verdict: negative on this cohort.** The target is reliable (0.94+) and the
probe still reads ~nothing. Controls: embed endpoint sane (cos(identical)=1.0,
cos(unrelated)=0.24); a log-length control is invalid here because 89% of
texts clip at the 6000-char embed truncation.

## What this does and does not kill

Does not kill the lever in general; kills it for THIS configuration:
- Range-restricted cohort (top-200 LW posts — already selected for hit-ness;
  within-cohort variance is exactly the subtle part).
- Small embedder (voyage4-nano) + linear probe + n=200 samples for 2048 dims.

Next falsifiable steps, in order of information/cost:
1. Wide-variance cohort: 200 RANDOM LW posts (not top-200), same protocol.
   If the probe works there, distillation propagates coarse structure and the
   31b judge refines only the top slice — still a big win.
2. Learning curve: judge +200 more entities, see if probe Spearman moves.
   Flat => representation ceiling; rising => data-starved.
3. Bigger embedder (voyage-4-large) only after 1-2 justify it.

Script: `probe_fuse.py` (runs on the judging host against its local ClickHouse
and embedding endpoints). Runs: jrun_043dfbe9 (#a), jrun_75bb1bac (#b),
jrun_364af0a4 (#c), jrun_24371b89 (#d).
