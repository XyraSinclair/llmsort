# Algorithm and design decisions

This document explains not just *what* llmsort does, but *why* each design choice was made.

This is the core engine document. Dated derivations and experiment records live
under `research/`; this page keeps only the rationale needed to use the engine.

## Why pairwise comparisons instead of direct scoring?

When you ask an LLM to "rate this essay 1–10", the scores are:
- **Miscalibrated**: a 7 for one essay might mean something different than a 7 for another.
- **Anchor-dependent**: the first item scored sets an arbitrary reference point.
- **Inconsistent**: the same item can get different absolute scores in different contexts.

Pairwise comparisons ("which is better, and by how much?") can reduce these failure modes when the attribute is coherent and the two items give the rater a useful reference point. They are not magic: the evaluation record should be read as local evidence for specific synthetic regimes, not as proof that pairwise prompts universally beat direct scoring.

## Why ratios instead of just "which is better?"

Pure ordinal comparisons (A > B) waste information. If A is *barely* better than B, that's very different from A being *vastly* better, but a binary comparison throws away the magnitude.

Ratio judgments ("A is 2.5× clearer than B") capture magnitude on a natural scale. In log-space, these become additive: ln(A/B) + ln(B/C) = ln(A/C), which means observations compose cleanly and the system of equations is well-behaved.

## The ratio ladder: 1.0 .. 26.0

The LLM chooses from a fixed set of ratio values: `[1.0, 1.05, 1.1, 1.2, 1.3, 1.5, 1.75, 2.1, 2.5, 3.1, 3.9, 5.1, 6.8, 9.2, 12.7, 18.0, 26.0]`.

This ladder is approximately geometric (roughly evenly spaced in log-space), which means each step represents a similar perceptual increment. The range caps at 26× because extreme ratios are unreliable — if two items are that different, the comparison is already decisive and precision beyond 26× doesn't help. The fine gradations near 1.0 (1.05, 1.1) allow the model to express near-ties with nuance.

## The solver: why IRLS with Huber loss?

Given a set of noisy pairwise log-ratio observations, we need to recover a consistent global set of scores. This is a weighted least-squares problem on a graph, where each comparison is an edge with a measured difference and either explicit measured precision or unit precision.

**Why robust regression?** LLMs are noisy judges. Some comparisons are outliers — the model misunderstands, hallucinates, or produces an inconsistent judgment. Ordinary least squares would let one bad comparison distort all the scores. Huber loss provides a smooth transition between L2 (for small residuals) and L1 (for large residuals), automatically downweighting outliers without discarding them entirely.

**Why IRLS specifically?** Iteratively Reweighted Least Squares turns the robust regression into a sequence of ordinary weighted least-squares problems, each solvable via Cholesky decomposition in the current implementation. This is numerically stable and well-understood for the small and medium active sets covered by the checked-in scaling report; larger production frontiers need sparse linear algebra or smaller active sets.

**Why Huber over other robust losses?** Huber loss (with k=1.5) is the standard choice in robust statistics. It's less aggressive than Tukey's biweight (which can zero-out observations entirely) but more robust than plain L2. The k=1.5 threshold means observations with residuals beyond 1.5 standard deviations get downweighted but never fully discarded.

## Observation precision

Model-reported confidence is retained in traces as a statement by the judge,
not treated as calibrated precision. Point observations receive unit precision.
Evidence paths that measure a response distribution may instead attach an
explicit positive `precision`; invalid measured precision causes the
observation to be skipped rather than silently converted into a default.
Rater efficacy, repeats, and attribute temperature then scale that precision:

```text
edge weight = rater efficacy × measured-or-unit precision × repeats
              / attribute temperature
```

`censored_likelihood` is a separate experimental ordered-probit path. It treats
each ladder rung as an interval-censored response under a declared noise scale
and weak prior. It is not wired into production execution: the clean synthetic
channel passes its comparison gate, while contaminated-channel robustness and
coverage remain required before any cutover.

## Gauge pinning

Latent scores are relative — you can shift all scores by a constant without changing any pairwise difference. The solver "pins" one node per connected component to zero to break this symmetry. This is standard in pairwise comparison models (analogous to setting a reference voltage in an electrical circuit).

## Uncertainty estimation

After solving, the system computes posterior variances from the inverse of the Hessian (the information matrix). For small problems (≤256 items), this is computed exactly via Cholesky; for larger problems, the diagonal is estimated via Hutchinson's stochastic trace estimator.

These variances tell you how uncertain each item's score is — items with few comparisons have wide uncertainty, items with many consistent comparisons have tight uncertainty.

## Dynamic query planning

Given current scores and uncertainties, the planner proposes the next pair to compare. It targets pairs that would maximally reduce uncertainty about the **top-K ranking** specifically — not overall score uncertainty.

The planner uses **effective resistance** from spectral graph theory: the expected variance reduction from observing edge (i,j) is proportional to the effective resistance between nodes i and j in the comparison graph. Pairs with high effective resistance (poorly connected in the comparison graph) and high relevance to the top-K boundary are prioritized.

## Top-K uncertainty and stopping

The system doesn't just track individual score uncertainties — it estimates the probability that the current top-K ranking is wrong. Specifically, for each item near the K-boundary, it computes the **frontier inversion probability**: the probability that an item currently ranked just below K actually belongs above K (or vice versa), using a Gaussian approximation of the score difference.

The global top-K error is the sum of these boundary flip probabilities. The system stops when this error falls below `tolerated_error`, or when it achieves a **certified separation bound** — meaning the estimated gap between item K and item K+1 is large enough relative to their current joint uncertainty that the modeled boundary is stable.

The `stop_sigma_inflate` parameter (default 1.25) inflates the uncertainty estimate for the stopping check to be conservative — the system would rather ask one extra question than stop prematurely with an incorrect ranking.

## Why these evaluation metrics?

The evaluation suite deliberately reports several metrics because they answer
different questions about the same run.

### 1. Global order agreement

- **Kendall tau-b** is the primary "did we get the ranking right?" metric.
  It counts concordant versus discordant item pairs and handles ties correctly.
  This matches the core object the system learns from: pairwise comparisons.
- **Spearman rho** complements tau-b by measuring correlation between rank
  positions. It is useful when a small number of items move a long distance,
  which can leave pairwise agreement fairly high while still producing a visibly
  distorted ranking.

These are complementary, not redundant. Tau-b is better for pairwise order
correctness; rho is better for rank displacement.

### 2. Top-K recovery

- **Top-K precision** asks whether the returned top-K includes impostors.
- **Top-K recall** asks whether the true top-K items were missed.

These matter because the product is usually used for selection, not for caring
about the exact ordering of the entire tail.

### 3. Uncertainty calibration

- **Coverage @95% CI** checks whether the reported posterior intervals are
  honest. If true latent scores fall inside nominal 95% intervals much less than
  95% of the time, the system is overconfident. If they fall inside far more
  often, the system is conservative.

This is distinct from ranking quality. A system can rank well but still report
bad uncertainty.

### 4. Top-heavy ranking quality

- **nDCG@K** rewards getting the top of the list right, with position discount.
- **CURL** measures pairwise concordance with extra weight on high-ranked items.
- **Weighted rank reversals** gives an interpretable count-like penalty for
  reversals near the top and for large displacement.
- **Bayesian regret** translates ranking mistakes into lost decision utility.

These are useful because not all ranking errors are equally costly. Swapping #1
and #2 matters more than swapping #49 and #50.

### 5. Planner-facing boundary risk

- **Frontier inversion probability** is not just an evaluation summary. It is
  the operational risk signal used by the planner and stopping rule. It asks the
  question the system is actually trying to control: "how likely is the K / K+1
  boundary to be wrong?"

That is why it can disagree with global metrics. A run can have mediocre global
tau while still having low boundary risk, or vice versa.

## Multi-attribute composition

When scoring on multiple attributes (e.g., clarity *and* depth *and* originality), each attribute gets its own independent rating engine. Attributes are then combined into a single utility score:

1. **Normalize** each attribute with robust scales (MAD — median absolute deviation, which is outlier-resistant unlike standard deviation).
2. **Weight** according to user-specified attribute weights.
3. **Apply gates** — optional hard filters that exclude items below a threshold on any attribute (e.g., "must be above the 25th percentile on safety").

The planner operates on the combined utility, targeting pairs that reduce uncertainty about the top-K of the *combined* ranking — so it naturally allocates more comparison budget to attributes where the top-K items are poorly separated.

## Research layers

ANP-typed contexts, training/export workflows, and orchestration layers stay in
the experimental and research parts of the repository. The published crate
remains focused on the pairwise-ratio engine.

## Caching

Pairwise judgments are cached in SQLite, keyed on (model, prompt template, attribute, entity text hashes). This means:
- Re-running the same request reuses all prior LLM calls.
- Adding a new entity to the set only requires comparisons involving that entity.
- Changing the model or prompt invalidates the relevant cache entries (correctly).
- Cache hits are treated as observations with the same weight as fresh LLM calls.

## Judgment packets

Above the per-call cache sits the portable evidence layer (`src/packet.rs`):
a judgment packet is a content-addressed bundle of raw pairwise judgements
with their rendered-prompt provenance. Packets from different runs, machines,
or judges fuse idempotently — fusing the same evidence twice is byte-identical
to fusing it once — so evidence can be shipped, merged, and re-solved without
double-counting. The `judge --consortium` verb emits one packet per complete
judge orbit and computes its verdict by fusing them.
