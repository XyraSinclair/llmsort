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

Script: `probe_cascade.py` fuse mode (port of the original probe_fuse.py,
reproduces its published numbers exactly; runs on the judging host against
its local ClickHouse and embedding endpoints). Runs: jrun_043dfbe9 (#a),
jrun_75bb1bac (#b), jrun_364af0a4 (#c), jrun_24371b89 (#d).

## Wide-variance replication (2026-09-06, later): POSITIVE

Same protocol on 200 RANDOM lesswrong posts (word_count >= 100, sampled from
the ~44K-post pool; lens `lesswrong-posts-rand`; runs jrun_115ebbbf (#a),
jrun_59e85b50 (#b), jrun_0f3c6d7c (#c), jrun_d06c4ff8 (#d)).

Anchor reliability, even better than the curated cohort:
pairwise wording Spearman 0.842-0.911 (mean 0.879), Spearman-Brown 0.967,
split-half 0.915.

Probe (fused target, 5-fold CV):

| lambda | held-out Spearman | Pearson | top-decile overlap |
|-------:|------------------:|--------:|-------------------:|
| 10     | +0.717 | +0.752 | 6/20 |
| 100    | +0.724 | +0.762 | 6/20 |
| 1000   | +0.738 | +0.778 | 5/20 |
| 10000  | +0.718 | +0.760 | 8/20 |

**Verdict: the distillation lever works.** With 160 training anchors the probe
reads ~0.74 of a 0.97-reliability target (validity ~0.75 after disattenuation).
Combined with the top-200 null: the probe propagates coarse structure across
the corpus but cannot discriminate within the elite band — which fixes the
architecture, a cascade:

1. Probe scores the whole corpus at embedding cost (~1000x cheaper than judging).
2. The pinned judge (gemma4-31b) judges only the probe's top slice, where the
   probe is weakest and the leaderboard actually lives.
3. Anchor refresh: periodic N=200 wording-family runs on random cohorts keep
   the probe calibrated; 8x200x4 = 6400 comparisons per axis family.

Top-decile overlap 6-8/20 at the corpus scale means the probe's top slice must
be over-sampled (take probe-top-500 to catch most of the true top-100 band)
before handing to the judge.

## Cascade pilot executed (2026-09-06): discovery leaderboard live

Full cascade ran end-to-end in ~45 min wall-clock:
all 43,954 LW posts (word_count >= 100) probe-scored (~86 posts/s at embedding
cost) -> probe-top-200 (excluding the curated top-200 lens) judged by
gemma4-31b at N=200 (run jrun_6a48f13b) -> public board.

Probe-vs-judge Spearman WITHIN the probe-top-200: +0.456 (n=200). Compare the
~0 within the curated karma-top-200: the probe-defined elite band retains
usable ordering because it spans more true-quality range than a karma-selected
band. The judge stage remains load-bearing for the final order.

Topic concentration, confirmed and ratified: the probe's top slice is heavily
agent-foundations / decision-theory, and the judge's top-15 KEEPS that tilt
(top titles: Optimization at a Distance, Toward a New Technical Explanation of
Technical Explanation, Teleosemantics!, thin logical priors, ...). Open
question: genuine axis signal (dense conceptual novelty) vs shared
jargon-density bias. The #d auditor wording (counterfeit-novelty audit) was
authored to break exactly this tie; run jrun_55ba8ae6 (probetop cohort, #d)
submitted for the check — if the cluster survives an adversarial-novelty
audit, it is signal, not style.

Scorer note: JSONEachRow lines must be split on "\n" only — str.splitlines()
also splits on U+2028/U+2029, which are legal unescaped inside JSON strings
(one post contained a raw LINE SEPARATOR and sheared a row mid-string).

## Auditor check (2026-09-06): the cluster is signal

#d (counterfeit-novelty audit) vs #a (definitional hit) on the probetop
cohort: Spearman +0.860, 12/20 top-20 overlap (runs jrun_6a48f13b /
jrun_55ba8ae6). The auditor wording — written to demote familiar ideas in
fresh vocabulary — keeps the agent-foundations cluster at the top
(Logical counterfactuals for random algorithms, Maxent and Abstractions,
Consequentialist Formal Systems, ...). Within this design the tie is broken:
the axis genuinely rewards dense conceptual novelty and that corner of LW
delivers it. Residual caveat: both wordings share one judge model's priors; a
cross-model-family check is the remaining falsifier if ever needed.

Discovery board stands. Cascade is now the standing recipe per axis family:
6,400 anchor comparisons (4 wordings x N=200, random cohort) -> ridge probe ->
corpus propagation at embedding cost -> judge the probe-top slice -> public
board.
