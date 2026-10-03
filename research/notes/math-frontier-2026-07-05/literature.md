# Math frontier sweep, 2026-07-05

Literature survey for cardinal-harness: what the broader math of cardinal
elicitation and stable preference/prior estimation has that this repo has not
yet absorbed. Cross-referenced against `research/notes/FIRST_PRINCIPLES.md` (primitives,
instrument grid, invariance table) and `research/notes/PRINCIPLES.md` (refutability,
denominators, register discipline). Each thread ends with the precise result,
its citation, and one buildable receipt.

---

## 1. LLM-Prior: knowledge-driven prior elicitation and aggregation

**Paper**: Yongchao Huang (Univ. of Aberdeen), "LLM-Prior: A Framework for
Knowledge-Driven Prior Elicitation and Aggregation," arXiv:2508.03766
(Aug 2025). [abs](https://arxiv.org/abs/2508.03766) ·
[html](https://arxiv.org/html/2508.03766v1)

**The result, precisely.** LLM-Prior is not "ask the model for a number" — it
is an *operator* `LLMPrior: context → tractable distribution`, architecturally
factored as an LLM coupled to an explicit generative model (a Gaussian
Mixture Model), forming what the paper calls an **LLM-based Mixture Density
Network**: the LLM's job is to emit the *parameters* of a GMM (component
weights, means, variances) conditioned on unstructured context (text, data,
figures), not to emit a sample or a point estimate directly. This
guarantees the output is a valid, tractable, differentiable density by
construction — the tractability is architectural, not asserted post hoc.

For multi-agent aggregation the paper doesn't use a linear pool. It uses
**Logarithmic Opinion Pooling**: given expert densities `p_1, …, p_k` and
weights `w_i`, the pooled density is
`p(θ) ∝ ∏_i p_i(θ)^{w_i}`
(product-of-experts, renormalized), and the paper's **Fed-LLMPrior** is a
federated algorithm that aggregates distributed, context-dependent LLM
priors this way, choosing log-pooling specifically because it is the
*externally Bayesian* pooling operator (see §5 below) — meaning
"pool-then-condition-on-new-data" and "condition-then-pool" commute, so a
federation of judges can each privately update and the fusion stays
coherent, unlike linear pooling.

**What this repo has not absorbed.** Cardinal-harness's `cardinal calibrate`
already treats null-content elicitation as "the model's whole prior in one
call" (PMF instruments, FIRST_PRINCIPLES §2), but the *aggregation* across
models/prompts/templates is currently ad hoc (gain-calibration is a linear
bilinear fit, not an opinion pool with a stated coherence property). The
repo has never asked whether its own cross-model or cross-template
aggregation is externally Bayesian.

**Buildable receipt.** Take the existing gain-calibration data
(`gain_calibration::solve_with_template_gains`, wording channels per model)
and check external Bayesianity directly: pool the per-template posteriors
two ways — (a) log-pool the raw per-template PMFs then update on a held-out
judgement, vs (b) update each per-template PMF first then log-pool the
posteriors — and report the KL divergence between (a) and (b). Log-pooling
predicts zero; linear pooling of the same data would not. This is a half-day
receipt using data already in the repo's calibration packs, and it upgrades
"probes graduate into estimators" (PRINCIPLES §5) with a formal coherence
property for the aggregation step, not just the elicitation step.

---

## 2. HodgeRank: what's beyond the curl fraction

**Paper**: Xiaoye Jiang, Lek-Heng Lim, Yuan Yao, Yinyu Ye, "Statistical
Ranking and Combinatorial Hodge Theory," Math. Programming 127 (2011);
arXiv:0811.1067. [pdf](https://arxiv.org/pdf/0811.1067)

**The result, precisely.** Given pairwise comparison data as an edge flow
`Y` on a graph (here: log-ratio judgements on the entity graph), the graph
Helmholtzian gives an *orthogonal three-way* Hodge decomposition of any edge
flow:

`Y = grad(s) ⊕ curl*(Φ) ⊕ h`

where `grad(s)(i,j) = s(j) − s(i)` is the gradient of a potential (the fully
consistent, globally-rankable part — this is exactly the log-latent field
the repo's IRLS solver fits), `curl*(Φ)` is the image of the curl adjoint
restricted to triangles (locally cyclic inconsistency: A≻B≻C≻A within a
3-clique), and `h` is the **harmonic** component: flows that are
simultaneously divergence-free (no node is a net source/sink) *and*
curl-free on every triangle, yet nonzero — meaning the inconsistency is not
detectable from any local triangle but only from a longer cycle around a
"hole" in the comparison graph's topology (nontrivial first cohomology).
Concretely: curl flags "these three items you compared directly disagree
with each other"; harmonic flags "there is no local witness triangle, but
walking a longer loop through the graph doesn't come back to zero" — a
structurally different failure mode requiring a *global* topological
obstruction (the graph's cycle space isn't spanned by triangles), not just
noisy triangles.

**What this repo has not absorbed.** `judgement_frustration_mean`
(FIRST_PRINCIPLES §5½, §5¾) reports **only** `Σλr²/Σλμ²` — the total
non-gradient energy — and calls it "the Hodge curl fraction." Per the
actual theorem this conflates curl and harmonic into one number. The two
have different causes (local noisy triangle vs. global topological
inconsistency) and, per Jiang-Lim-Yao-Ye, different remedies: curl shrinks
with more comparisons on the *same* triangles; harmonic requires adding
comparisons that *close different cycles* (changing the graph topology, not
just resampling existing edges).

**Buildable receipt.** Split `judgement_frustration_mean` into its two
orthogonal parts: compute the curl-projection energy and the
harmonic-projection energy separately (both are linear projections
computable from the existing weighted graph Laplacian machinery the solver
already builds for IRLS). Report `curl_fraction` and `harmonic_fraction`
side by side, and validate on two new scripted pathological judges: one
with genuine 3-cycles only (should show curl≫harmonic), one built on a graph
with a non-triangulated cycle (e.g., a 5-cycle comparison structure with no
chords) carrying a planted inconsistency (should show harmonic≫curl, curl≈0
since there's no triangle to detect it in). This is the direct generalization
of the ladder-curl experiment (FIRST_PRINCIPLES §5½) and gives the planner a
concrete, actionable signal: harmonic energy says "add comparisons that
close a different loop," which the effective-resistance planner (§4 below)
could act on directly.

---

## 3. Bradley-Terry/Thurstone/Plackett-Luce: ties, cardinal extensions, spectral methods, sample complexity

**Survey**: "Recent advances in the Bradley–Terry Model: theory,
algorithms, and applications," arXiv:2601.14727 (2026).
[html](https://arxiv.org/html/2601.14727v1) — this is the single best
entry point; below are its load-bearing threads plus the two classic
tie-extension papers.

- **Ties**: Rao–Kupper (1967) models `P(i ties j) = (θ²−1)λᵢλⱼ /
  [(λᵢ+θλⱼ)(θλᵢ+λⱼ)]`; Davidson (1970) is the alternative
  win/tie/loss parametrization compared head-to-head against Rao–Kupper in
  the original paper
  ([tandfonline](https://www.tandfonline.com/doi/abs/10.1080/01621459.1970.10481082)).
  Both add exactly one scalar tie-propensity parameter `θ`/`ν` to
  Bradley-Terry.
- **Cardinal extension**: Shah et al. (2016), "the paired cardinal model" —
  directly the mathematical genre cardinal-harness lives in, cited in the
  2026 survey as the canonical cardinal generalization of BT.
- **Sample complexity / minimax**: Chen et al. (2019) and Han et al. (2020)
  pin the sparsity threshold for uniform MLE consistency at
  `(log n)/n` edge density on Erdős–Rényi comparison graphs (below this, no
  estimator is uniformly consistent); Gao et al. (2023) give the first
  optimal asymptotic normality result for *both* the MLE and the spectral
  estimator under sparse regimes; Han et al. (2023) generalize error bounds
  to arbitrary (non-Erdős–Rényi) graph topologies via **graph-based
  chaining**, tying estimation error directly to the graph's Cheeger
  constant / Laplacian spectral gap — i.e., how well-connected the
  comparison graph is, not just how many edges it has.
- **Spectral estimators**: Rank Centrality (Negahban–Oh–Shah,
  arXiv:1209.1688) builds a random walk whose transition probability
  `i→j ∝` (empirical frequency j beat i); its stationary distribution is
  the score estimate. Its finite-sample guarantee: under the BTL model,
  the ℓ2 error between Rank Centrality scores and the true BTL parameters
  is bounded at a rate matching the Cramér–Rao lower bound up to log
  factors — spectral, not iterative, and near-minimax without solving a
  convex program. Later refinements (Luce Spectral Ranking, Accelerated
  Spectral Ranking) close the remaining gap to exact minimax-optimality.
- **RLHF connection**: the survey explicitly frames modern reward modeling
  as `P(y_i ≻ y_j) = σ(r(x_i) − r(x_j))` — literally Bradley-Terry with a
  neural `r` — citing Zhu et al. (2023), Zhan et al. (2024) for the
  theoretical foundations of preference-based fine-tuning at this scale.

**What this repo has not absorbed.** The repo's ratio ladder and IRLS solve
are already a form of the cardinal BT/Thurstone genre, but (a) it has no
tie/indifference channel — a judge saying "these are equal" is currently
just a ratio near 1, not a first-class outcome with its own likelihood term
the way Rao-Kupper/Davidson treat it; (b) there is no standing spectral
estimator as a cross-check against IRLS, despite spectral methods being
provably near-minimax and much cheaper to compute; (c) no receipt exists
connecting the *planner's* graph topology to the Cheeger-constant-driven
error bound from Han et al. (2023) — the repo's effective-resistance
planning (§4) is doing something adjacent by instinct but has never been
checked against this literature's exact statement.

**Buildable receipt.** Add Rank Centrality as a second, independent
estimator computed from the same comparison data the IRLS solver already
consumes (it's a stationary-distribution computation on a graph the solver
already builds — cheap). Report `||irls_scores − rank_centrality_scores||`
as a model-agnostic cross-check receipt: on well-conditioned graphs (dense,
high Cheeger constant) the two should agree tightly; the gap should predict
where IRLS's parametric assumptions (or the planner's chosen comparisons)
are under strain, giving an early-warning receipt that requires zero new
LLM calls, only recomputation on stored judgements.

---

## 4. Optimal experimental design for paired comparisons

**Paper**: "Optimal Designs for Discrete Choice Models Via Graph
Laplacians," arXiv:2208.08926 (2022/2023),
[pdf](https://arxiv.org/pdf/2208.08926),
[PMC](https://www.ncbi.nlm.nih.gov/pmc/articles/PMC12274245/).

**The result, precisely.** For Bradley-Terry-type paired-comparison
designs, the Fisher information matrix of the experiment is exactly a
weighted graph Laplacian `L` (weights = design frequencies on each pair).
D-optimality — maximizing `det` of the information matrix restricted to
the identifiable subspace — becomes maximizing a cofactor of `L`, and by
**Kirchhoff's Matrix–Tree Theorem** that cofactor literally counts
weighted spanning trees of the comparison graph. The dual problem has a
clean description via the **Cayley–Menger determinant of the Farris
transform** of `L` (an embedding of the comparison structure into a
Euclidean point configuration where D-optimality becomes a geometric
volume-maximization problem). This is a genuine complexity reduction: it
turns a combinatorial search over which pairs to query into continuous
optimization over edge weights (a gradient-based search for locally
D-optimal designs), and it is a *direct* line item citation-match for the
repo's own effective-resistance planner (also Laplacian-based, also using
graph-theoretic quantities to decide what to compare next).

**What this repo has not absorbed.** FIRST_PRINCIPLES §4 says the planner
uses "greedy effective-resistance planning" without ever citing or checking
against the D-optimality/spanning-tree criterion, which is provably the
right target for BT-type designs and is *not the same object* as minimizing
total effective resistance (effective resistance minimization is a
different, if related, graph criterion — Ghosh-Boyd — closer to
A-optimality than D-optimality). The repo doesn't currently know which
optimality criterion its planner is implicitly chasing, or whether
D-optimal would out-perform it.

**Buildable receipt.** Implement the spanning-tree-count / `det(L)`
criterion as an alternative planner scoring function alongside the existing
effective-resistance planner, and run the repo's own regret benchmark
(FIRST_PRINCIPLES §4: "planner vs baseline measured") with three arms
instead of two: baseline, effective-resistance-greedy, D-optimal-greedy.
This directly answers, with the repo's own existing benchmark harness,
whether the planner is chasing the wrong graph invariant — a receipt the
repo is one scoring function away from having.

---

## 5. Bayesian prior elicitation proper: SHELF, Cooke's classical model, opinion pools

**Papers**: Colson & Cooke, "Expert Elicitation: Using the Classical Model
to Validate Experts' Judgments," Rev. Environ. Econ. Policy 12(1) (2018),
[journals.uchicago.edu](https://www.journals.uchicago.edu/doi/full/10.1093/reep/rex022);
comparison study "A Comparison of Prior Elicitation Aggregation using the
Classical Method and SHELF"; opinion-pooling formalism per Genest &
McConway-style results summarized via
[philarchive: Probabilistic Opinion Pooling](https://philarchive.org/archive/DIEPOP).

**The result, precisely.** Cooke's **classical model** scores each expert
on **calibration questions** — items from the expert's own domain with
known true values, answered *before* the target questions are asked, in the
same elicitation format (usually quantiles). Calibration score is (roughly)
a likelihood-ratio-style statistical test of whether the expert's stated
quantiles actually contain the true value at the stated rate (a
goodness-of-fit test against the uniform-p-value null); an **information
score** separately measures how tight/informative the expert's intervals
are. The final aggregation weight per expert is calibration × information
(with a threshold: badly-calibrated experts can be weighted to ~zero), and
empirically this **performance-weighted pool beats both an equal-weighted
pool and SHELF's group-consensus method** on held-out seed variables in
Colson & Cooke's comparison — but there is a documented ceiling: beyond
some number of seed/calibration variables, the classical model's advantage
over equal weighting stops growing. On pooling operators: **linear pooling
satisfies the marginalization property but is not externally Bayesian**
(pool-then-update ≠ update-then-pool); **logarithmic pooling is uniquely
externally Bayesian** but sacrifices marginalization coherence — the two
desiderata are provably in tension, you cannot have both simultaneously
(this is the formal grounding for LLM-Prior's choice in §1).

**What this repo has not absorbed.** The Judge Coherence Benchmark
(FIRST_PRINCIPLES §5) scores judges on the invariance battery (order,
polarity, paraphrase, spin, curl, nuisance) but has **no calibration
questions with known ground truth** in Cooke's sense — the benchmark
measures self-consistency, never accuracy against a seed variable with a
verifiable true answer. That is a completely different, complementary axis:
a judge can be perfectly self-consistent and still miscalibrated (confident
and wrong, consistently). Cooke's model is precisely the missing piece for
turning the coherence benchmark's leaderboard into a *performance-weighted
pool* across judge models, rather than treating all judges' PMFs as equally
trustworthy inputs to aggregation.

**Buildable receipt.** Add a small battery of **seed comparisons with known
ground truth** (e.g., numeric-fact pairs where the true log-ratio is
externally verifiable — populations, prices, physical constants at a fixed
date) to `cardinal bench`, score each judge model's calibration (do its
stated PMF quantiles/ratio distributions actually contain the true ratio at
the nominal rate?) and information (how tight), and report a Cooke weight
per judge model. Then re-run any existing multi-judge aggregation (the
CARE-adjacent gap in §8) once weighted this way vs. equal-weighted, as a
receipt of whether performance-weighting changes the aggregate answer
materially — directly extending "eat the dogfood at every level"
(PRINCIPLES §8: "choose judges by our own coherence benchmark") to
accuracy, not just self-consistency.

---

## 6. Elicitability: which functionals of a belief can be elicited at all

**Papers**: Lambert, Pennock, Shoham (2008) foundational elicitability;
Frongillo & Kash, "Elicitation Complexity of Statistical Properties,"
Biometrika 108(4):857-879 (2021), arXiv:1506.07212
([abs](https://arxiv.org/abs/1506.07212)); Fissler & Ziegel (2016),
"Higher order elicitability and Osband's principle," Ann. Statist. 44(4)
([pdf](https://projecteuclid.org/journals/annals-of-statistics/volume-44/issue-4/Higher-order-elicitability-and-Osbands-principle/10.1214/16-AOS1439.pdf)).

**The result, precisely.** A statistical functional `T` (mean, quantile,
variance, mode, …) is **elicitable** iff there exists a strictly consistent
scoring function `S(x, y)` such that `T(F) = argmin_x E_{Y~F}[S(x,Y)]` —
i.e., iff `T` can be recovered as the unique minimizer of *some* proper
loss under the true distribution. Moments, quantiles, expectiles, and
ratios of moments are elicitable; famously **variance is not directly
elicitable** (you need the mean as an auxiliary/two-dimensional property
first — this is Osband's principle, and it generalizes: many properties
are only *indirectly* elicitable via a higher-dimensional elicitable
vector), and **Expected Shortfall / mode / modal interval are not
elicitable at all**, not even indirectly, under mild regularity (backed by
"why scoring functions cannot assess tail properties," Fissler-Ziegel
lineage). Frongillo-Kash's **elicitation complexity** is the formal
sharpening: instead of a binary elicitable/not, it asks the *minimum
dimension k* such that `T` is a coordinate of some elicitable
`k`-dimensional vector property, with tight bounds for the broad class of
Bayes risks (linear properties have complexity exactly matching their
natural dimension; more exotic properties provably require strictly more
auxiliary dimensions than their own).

**What this repo has not absorbed.** The repo elicits ratio-scale point
estimates and PMFs (FIRST_PRINCIPLES §2) but has never asked the
elicitability question about its *own* target: is "the cardinal log-latent
score" an elicitable property of the judge's true belief distribution over
outcomes, under the ratio-ladder scoring mechanism actually used? The IRLS
loss is a least-squares-on-log-ratios loss, which is a strictly proper
scoring rule for the *mean* of the log-ratio under Gaussian noise — that
part is fine and standard — but the repo's PMF-based instruments
(`ratio_letter_v1`, FIRST_PRINCIPLES §2) are extracting logprobs over a
discrete letter alphabet and treating derived moments as if any moment
were freely elicitable from that alphabet's induced distribution. Some
moments (mean, variance of the log-ratio, entropy) are legitimately
elicitable this way; others (e.g., anything mode-like, or the tails/escape
mass framing used for `PmfCompleteness`) are exactly the class the
Fissler-Ziegel result says may not be — this has never been checked.

**Buildable receipt.** Audit every derived quantity the repo currently
reports from a PMF instrument (mean log-ratio, variance, entropy,
`PmfCompleteness`/escape mass) against the elicitability taxonomy: for each,
state explicitly whether it is (a) directly elicitable under the ratio-letter
scoring scheme, (b) indirectly elicitable (needs an auxiliary dimension —
name it), or (c) not elicitable and therefore reported as a *descriptive*
statistic of the sample, never as "the model's true X." This is a
half-day paper-and-pencil receipt (no new experiments) that converts a
silent assumption into a stated, falsifiable claim per quantity — exactly
the register PRINCIPLES §4 demands ("never the personality... if it cannot
be stated as a number with units, it is not ready" — extend to "and a
proof it's elicitable at all").

---

## 7. Stochastic transitivity hierarchies and revealed-preference tests

**Papers**: Shah, Balakrishnan, Bradley, Parekh, Ramchandran, Wainwright,
"Stochastically Transitive Models for Pairwise Comparisons: Statistical and
Computational Issues," arXiv:1510.05610; classical GARP/Afriat
consistency-with-utility-maximization theory (Afriat 1967; summarized e.g.
[eclass.aueb.gr lecture notes](https://eclass.aueb.gr/modules/document/file.php/DEOS105/lectures/revealed%20preference.pdf));
"Preference Elicitation For General Random Utility Models," arXiv:1309.6864.

**The result, precisely.** Three progressively weaker consistency
conditions sit strictly between full transitivity (a total order) and
arbitrary noise, defined via the pairwise win-probability matrix
`p_{ij} = P(i beats j)`: if `p_{ij} ≥ 1/2` and `p_{jk} ≥ 1/2`, then
**Strong** stochastic transitivity (SST) requires
`p_{ik} ≥ max(p_{ij}, p_{jk})`; **Moderate** (MST) requires
`p_{ik} ≥ min(p_{ij}, p_{jk})`; **Weak** (WST) requires only `p_{ik} ≥ 1/2`.
Every additive random-utility model (Thurstone/BTL/any model of the form
`u(i) + ε`) automatically satisfies SST, so SST is the natural nonparametric
superset that nests all standard parametric ranking models without
committing to any specific noise distribution. Shah et al.'s central,
somewhat surprising result: **the flexible nonparametric SST/MST class can
still be estimated at the *same minimax rate* as the fully parametric
BTL/Thurstone models** — generality is statistically free at the rate
level, though the *rate-optimal estimator is computationally nontrivial*
(a naive singular-value-thresholding estimator is consistent but
rate-suboptimal; achieving the minimax rate requires more careful
shape-constrained — isotonic-style — estimation). Separately, GARP/Afriat
gives an exact, finite, checkable (via a cycle condition on a graph called
the "revealed preference" digraph) necessary-and-sufficient test for
whether a *finite* dataset of choices is consistent with utility
maximization at all — a strictly different, deterministic falsification
tool sitting one level below the stochastic-transitivity hierarchy.

**What this repo has not absorbed.** FIRST_PRINCIPLES §5 tests cycles
(via Hodge curl/harmonic) but has no notion of *which stochastic
transitivity class* a judge's comparison matrix belongs to. This is a
strictly finer diagnostic than cycle detection: a judge could be
cycle-free (zero curl/harmonic energy in the aggregate log-ratio field, by
construction of how the solver treats the data) yet still violate SST or
even MST at the level of raw win-probabilities if the judge is
systematically inconsistent in a way that averages out in log-space but
shows up in the *win-probability* structure (e.g., a judge with heavy but
symmetric noise on close pairs and none on lopsided ones can still violate
SST while contributing zero net curl). This is a genuinely different
"is this a real preference?" test from anything currently in the repo, and
it composes naturally with existing PMF instruments since those already
expose graded win-probabilities, not just point ratios.

**Buildable receipt.** From the existing PMF-instrument judgements
(`ratio_letter_v1`, `ordinal_letter_v1`), derive the empirical
win-probability matrix `p_{ij}` per attribute (already recoverable — a PMF
answer implies a probability the model favors i over j) and directly test
SST/MST/WST violations as a new invariance-table row: report the fraction
of triples violating each transitivity level, with the planted
"cyclic"/"frustration" scripted judges from `tests/judge_bench.rs` extended
with a new scripted "SST-violating but curl-free" pathology (symmetric
noise concentrated on close pairs) to validate the new probe catches
something the Hodge machinery structurally cannot.

---

## 8. LLM-specific: judge bias correction, confounded aggregation, and the elicitation-impossibility ceiling

**Papers**: "CARE: Confounder-Aware Aggregation for Reliable LLM
Evaluation," arXiv:2603.00039; "Judging the Judges: A Systematic Evaluation
of Bias Mitigation Strategies in LLM-as-a-Judge Pipelines," arXiv:2604.23178;
"SkillAggregation: Reference-free LLM-Dependent Aggregation,"
arXiv:2410.10215; "The Impossibility of Eliciting Latent Knowledge,"
arXiv:2606.12268.

**The results, precisely.**
- **CARE** treats correlated judge errors as arising from a shared **latent
  confounder** (rather than assuming judge errors are conditionally
  independent given the true quality — the standard, usually false,
  assumption behind naive majority-vote/weighted-vote ensembling). It gives
  identifiability conditions for when the confounder structure can be
  recovered from the observed judge-decision matrix alone, and a
  confounder-aware aggregation rule that provably outperforms
  independence-assuming pooling when the identifiability condition holds.
  This is the multi-judge analogue of §5's Cooke weighting, but targeted at
  *correlated* rather than merely *unequal* judge reliability.
- **Position/verbosity bias corrections** (per the bias-mitigation survey
  literature): swap-and-aggregate for position bias (already the repo's
  own default counterbalancing per FIRST_PRINCIPLES §5), and explicit
  length-normalization / "do not prefer longer answers" rubric injection
  cuts measured verbosity inflation roughly in half — a cheap, stated,
  checkable intervention the repo's templates could adopt and then measure.
- **The impossibility result** (arXiv:2606.12268): under the natural
  assumption that a model *can* have latent beliefs distinct from its
  reported output and there is no privileged access to internal state
  beyond behavior, **no general elicitation procedure can be guaranteed to
  recover true latent belief rather than strategic self-consistent
  misreport** — for any elicitation scheme, an adversarial reporting
  policy exists that passes every external behavioral check while
  concealing the true belief. This is a hard ceiling on what *any*
  behavioral instrument (including every instrument in this repo's grid)
  can certify, independent of how clever the invariance battery gets.

**What this repo has not absorbed.** The repo's entire epistemic stance
(PRINCIPLES §1, "refutability is the product") is behavioral: a judgement
"deserves the name belief" if it survives the transformation group
(FIRST_PRINCIPLES §5½). The impossibility result says this is the *most*
that behavioral testing can ever certify — surviving every invariance test
in the battery is necessary but can never be sufficient evidence against a
sufficiently adversarial misreporting policy. This isn't a reason to stop;
it's a reason to state the ceiling explicitly rather than let "coherent
under our battery" quietly imply "true." Separately, CARE is the
formally correct generalization of the repo's current cross-model
comparison (FIRST_PRINCIPLES §5: "◐ seriate probe compares models; no
standing cross-model receipt in cardinal") into an actual aggregation rule
with an identifiability condition, rather than an eyeballed comparison.

**Buildable receipt (two, since this thread is where the repo has the
most missing surface).**
1. Add verbosity-length as a controlled nuisance-edit axis to the existing
   nuisance battery (FIRST_PRINCIPLES §5, "nuisance edits" row already
   covers whitespace/markdown/bullets/prestige — length is conspicuously
   absent from that list despite being the single most-cited LLM-judge
   bias in the literature): pad one side of a pair with verbose-but-content-
   free elaboration, measure the shift in nats, same protocol as the
   existing prestige-halo receipt.
2. State the impossibility ceiling as a documented limitation in
   `research/notes/FIRST_PRINCIPLES.md` §5½ (one paragraph, one citation): "surviving
   the invariance battery is necessary, not sufficient, evidence of a true
   belief; arXiv:2606.12268 gives the formal reason no behavioral battery
   can close this gap." A one-paragraph addition that upgrades the repo's
   epistemic honesty for free — pure PRINCIPLES §4 register discipline,
   zero new code.

---

## Top 5, ranked by (mathematical depth × buildability)

1. **§2 Hodge curl/harmonic split.** Deepest math already half-implemented
   (the Laplacian machinery exists for IRLS); the split is a change to an
   existing computation plus two new scripted pathological judges, directly
   extends the repo's own most-cited physics analogy, and feeds the
   planner. Highest depth-to-effort ratio of the eight threads.
2. **§3 Rank Centrality cross-check.** Near-minimax spectral estimator,
   computable for free from data the solver already holds, gives a
   model-agnostic sanity receipt with a real theorem behind it (Cheeger
   constant → error rate) that the repo's planner has never been checked
   against.
3. **§7 Stochastic transitivity probe.** A genuinely new invariance-table
   row, not a refinement of an existing one — catches inconsistency
   patterns Hodge curl structurally cannot (symmetric-on-close-pairs noise),
   and slots directly into the existing scripted-pathology discipline.
4. **§4 D-optimal (spanning-tree) planner arm.** Real theorem (Kirchhoff/
   Cayley-Menger), directly answers whether the existing planner is
   chasing the right graph invariant, and the repo's own three-arm regret
   benchmark is the exact instrument to settle it.
5. **§5 Cooke calibration weighting.** Adds an axis the benchmark
   currently lacks entirely (accuracy vs. ground truth, not just
   self-consistency) with a well-established, empirically validated
   aggregation rule; ranked below the top four only because it requires
   curating new ground-truth seed items rather than recomputing on data
   already in hand.

(§1 LLM-Prior's log-pool external-Bayesianity check, §6 elicitability audit,
and §8's verbosity-nuisance-edit + impossibility-ceiling paragraph are all
cheap and worth doing, but score lower on depth: they check/document
properties of the existing machinery rather than adding new mathematical
structure to it.)
