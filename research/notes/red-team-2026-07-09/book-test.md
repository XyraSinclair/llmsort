# The Book test: is each core design the inevitable form, or an epicycle?

Adversarial review of the repo's IDEAS, 2026-07-09. Lens: for each core
design, name the proof-from-The-Book candidate and judge whether the
current design is it, approximates it, or diverges. Ranked by
depth × feasibility. Where the current design already IS the Book form,
that verdict is stated in one line and left alone.

Read for this review: research/notes/PRINCIPLES.md, research/notes/MATH_FRONTIER.md,
docs/MODEL.md, research/notes/FIRST_PRINCIPLES.md (§5–5⅝), README.md,
notes/ideation-2026-07-05/{invariance-theory,differentiation}.md, module
headers of src/{rating_engine,repeat_pooling,packet}.rs,
src/rerank/{spin,ensemble,orbit}.rs, src/prompts.rs, issues #43–#50.

---

## Finding 1 — The ladder is a quantizer being treated as a ruler; the Book form is an interval-censored likelihood. [THE ONE DEEPER JUMP]

This is the single deepest jump the repo has not conceived. Verified by
grep: "probit", "censored", "cut-point", "graded response" appear nowhere
in docs/, notes/, or src/; Thurstone appears only as a row in the
comparison table (research/notes/COMPARISON.md:57).

**(a) Current form.** A rung answer is converted to a point observation:
`y = ln(rung)` with Gaussian noise (docs/MODEL.md:19, and the clamp at
docs/MODEL.md:34 "ratio values are clamped onto the finite ladder before
becoming log edges"). Three consequences currently carried as separate
facts:

1. `ORDINAL_OBSERVATION_RATIO = 2.1` (src/prompts.rs:162) — a direction-only
   judgment is injected as a *fake magnitude* because the solver only eats
   point log-ratios.
2. The quantization-curl program: a planted-transitive judge shows ~0.13
   HCR from rung-usage coarseness; the controlled test
   (`tests/ladder_curl.rs`) refuted the ladder-geometry hypothesis —
   repo-ladder floor 0.00198 vs constant-log-step 0.00155, coarse rung
   *usage* injects two orders of magnitude more
   (research/notes/FIRST_PRINCIPLES.md:145–155).
3. The pinned honest negative: "ordinal beats ratio under heavy noise"
   (README.md:320).

**(b) Book form.** The judge holds a latent read `x ~ N(Δ_ij, σ²)` and
reports rung `k` exactly when `x ∈ [c_k, c_{k+1})`. The likelihood of a
rung is `Φ((c_{k+1}−Δ)/σ) − Φ((c_k−Δ)/σ)` — an ordered-probit / graded-
response model with the ladder's log-midpoints as cut-points. Then:

- **Ordinal** is the same likelihood with a single cut at 0
  (`P(A higher) = Φ(Δ/σ)` — literally Thurstone Case V). The 2.1 pseudo-
  ratio disappears; direction-only data enters with its true information
  content.
- **The bucket-PMF path** (logprob instrument) is the same likelihood with
  the full posterior over bins observed instead of one sample from it —
  the point/bucket/ordinal template trichotomy (README prompt-surface
  table) becomes one likelihood family under three censoring patterns.
- **Estimation stays IRLS.** EM for interval-censored Gaussians replaces
  each observation with its truncated-Gaussian conditional mean in the
  E-step; the M-step is exactly the existing weighted least squares.
  Huber composes on top unchanged. The stack survives; only the point
  where a rung becomes a number moves.

**(c) What it dissolves (five mechanisms).**

1. The `ORDINAL_OBSERVATION_RATIO` hack — deleted, replaced by the
   correct one-cut likelihood.
2. The quantization-curl floor as a *phenomenon* — a judge that uses two
   rungs is a judge with two wide bins; the interval likelihood absorbs
   coarseness into honest observation variance instead of laundering it
   into fake cyclic residual. The 0.13 floor is an artifact of treating a
   bin membership as a point at the bin center.
3. The ladder-geometry question — bin edges may be arbitrary and uneven;
   a censored likelihood doesn't care. The "express near-ties" fine
   rungs near 1.0 become a free design choice with zero cost to any
   downstream statistic.
4. The template trichotomy — one likelihood, three censoring patterns.
5. Very plausibly the "ordinal beats ratio under heavy noise" negative.
   Under a correct likelihood, ratio data *strictly contains* ordinal data
   (the sign of the bin), so ratio can never lose in expectation; the
   current loss is the signature of magnitude noise polluting direction
   information through the point-observation treatment. The pinned
   negative is evidence *for* this finding, not against ratio elicitation.

One honest cost, which is a feature: interval-censored observations are
not closed under (mean, precision) Gaussian fusion, so the packet
sufficient statistic changes from two floats per observation to the
observation itself — (edge, bin) — a multiset. The monoid (Finding 4)
survives trivially (multisets ARE the free commutative monoid); the
Gaussian-moment compression was the special case, not the theorem.

**(d) Cheapest confirming experiment.** Zero API spend, one module:
implement the EM refit and (i) rerun the `tests/ladder_curl.rs`
planted-transitive fixture — the HCR floor should collapse toward the
solver's numerical noise; (ii) rerun the eval-compare heavy-noise regime
— the ordinal-beats-ratio gap should shrink or invert. Either result is a
receipt; both moving together confirms the unification is real, not
aesthetic.

---

## Finding 2 — One master identity: three commuting projections on edges ⊗ group ⊗ repeats. DL, Hodge, and test–retest are block norms; stochastic transitivity is the model's goodness-of-fit test, not a fourth marginal.

**(a) Current form.** Four instruments shipped separately: DerSimonian–
Laird pooling (src/repeat_pooling.rs:1–26), Hodge split
(`compute_hodge_split`, src/rating_engine.rs:1118), orbit character
energies (src/rerank/orbit.rs:11–27), WST/MST/SST
(src/rerank/transitivity.rs). The repo already *conjectures* a corner of
the identity: "σ_b² estimated here IS the per-pair reading of the
frustration energy the Hodge machinery measures at the field level — the
two views must agree in order of magnitude"
(src/repeat_pooling.rs:22–25).

**(b) Book form.** The complete data object is an array
`y ∈ ℝ^E ⊗ ℝ^G ⊗ ℝ^k` (edges × group elements × repeat draws). Three
projections act on different tensor legs and therefore commute exactly:

- `P_rep` — averaging over repeats (leg 3),
- `P_∅` — the trivial-character projection (leg 2; the orbit transform),
- `P_grad` — the gradient projection of the Hodge decomposition (leg 1;
  orthogonal in the converged-IRLS λ-metric, exactly as the shipped
  Pythagoras pin `local + harmonic ≈ hcr` already uses it).

Total sum of squares splits into 2×2×2 = 8 orthogonal blocks, and every
shipped inconsistency statistic is a block norm or a ratio of blocks:

| statistic | block |
|---|---|
| σ_w² (nonce draws, test–retest) | ‖(I−P_rep)y‖²/df |
| orbit bias energies / coherence | (I−P_∅) vs P_∅ on the P_rep slice |
| HCR | ‖(I−P_grad)P_∅P_rep y‖² / ‖P_∅P_rep y‖² |
| local curl / harmonic | the further Hodge split of that block |
| DL σ_b² | the df-corrected *excess* of the (I−P_grad) block over its σ_w-expected value — Cochran's Q is the shrinkage bridge between two blocks, not a fifth object |

**The honest boundary:** WST/MST/SST does NOT reduce to this — it is a
sign/quantile functional of the choice-probability distribution, not a
second moment. But its Book placement is better than "marginal": under
the Gaussian location family that licenses everything above, SST holds
*identically*. Therefore an SST violation is evidence against the
location-family itself — the ST hierarchy is the goodness-of-fit test of
the whole generative model. The keystone pin from commit 02e61aa (a judge
with exactly telescoping means, hcr < 1e-9, violating SST) is precisely a
case invisible to every second-moment block — which is what a GoF test is
*for*. Forcing it into the variance decomposition would be aesthetic
vaporware; placing it as the model's falsifier is the theorem.

**(c) What it unifies.** Four instruments → three projections + one GoF
test; the repeat_pooling order-of-magnitude conjecture becomes an exact
identity; the doctrine gains one sentence that generates the whole
diagnostic suite: *"decompose the judgment array by the three legs; every
inconsistency has an address."* New rows of the invariance table become
new characters (leg 2) rather than new machinery.

**(d) Cheapest confirming derivation.** Pure arithmetic, zero spend: one
synthetic array with planted (σ_w, σ_b, bias characters, curl); compute
the 8 block norms once; recompute every shipped statistic from blocks and
pin equality against the existing implementations (`pool_repeats`,
`compute_hodge_split`, `orbit_transform` math). One fixture, one test
file, the whole zoo certified as marginals.

---

## Finding 3 — JCB coherence is the diagonal block of the judge-portfolio Gram matrix evaluated on the orbit of one judge. Two formalisms are one instrument, and the off-diagonal blocks are an unmeasured receipt.

**(a) Current form.** Two separate formalisms: ensemble portfolio theory
(src/rerank/ensemble.rs:1–36 — z-scored latents, correlation R,
one-factor loadings, Ψ = R − llᵀ, Markowitz weights) and orbit coherence
(src/rerank/orbit.rs:19–27 — per-judge Parseval fraction
m̂(∅)²/mean-square).

**(b) Book form.** Build the matrix `M ∈ ℝ^{(J·|G|) × pairs}` whose rows
are the pulled-back orbit-member latent vectors of every judge (judge i
under group element g, equivariance-corrected). One Gram/correlation
matrix on the rows. Then:

- **Within-judge diagonal block** (8×8 for Z₂³): its consensus share is
  the portfolio-geometry reading of that judge's coherence. An ideal
  judge's orbit rows are identical → consensus share 1 ↔ coherence 1.
- **Cross-judge blocks at fixed character**: whether two labs share the
  *same named bias* (e.g. both carry the order·polarity channel) —
  invisible to both current instruments separately, and exactly the
  refinement the 2.89-effective-error-channels finding
  (MATH_FRONTIER §3⅞) is begging for: *which characters* carry the
  shared error.

One necessary honesty clause, which is where the unification earns its
keep rather than being a slogan: the two shipped statistics differ by a
*quotient*. The ensemble z-scores every row (per-judge gain and gauge
quotiented out; ensemble.rs:6–8 says so explicitly); Parseval coherence
does not — and per-orbit-member gain mismatch (e.g. compressed magnitudes
under the negated attribute) is itself a bias the unquotiented version
correctly counts. So the unified statement is: **every coherence- and
portfolio-statistic in the repo is a function of the Gram matrix of M
after a declared gauge/gain quotient; JCB coherence and Markowitz weights
are diagonal and off-diagonal readings under two different declared
quotients.** The doctrine gains: always name the quotient next to the
number.

**(c) What it unifies.** Two formalisms → one object; plus one genuinely
new zero-spend measurement (bias sharing by character across labs). The
portfolio "diversification theorem" and the coherence "belief fraction"
become the same Schur-complement statement at two block scales.

**(d) Cheapest confirming experiment.** Orbit runs already exist in cache
for ≥2 models (mini's order·polarity 22.3% receipt, MATH_FRONTIER §3½).
Assemble M for two judges from cached orbits over the same pairs, compute
one Gram matrix, check the within-block consensus share reproduces
shipped coherence under the declared quotient, then read the cross-judge
character blocks — the first shared-bias-channel receipt, possibly for
$0.

---

## Finding 4 — The packet monoid already implies the sheaf. Descope #46 to the wire format plus transport; the Čech vocabulary is decoration.

**(a) Current form.** src/packet.rs:11–18 states the theorem: sufficient
statistics form a commutative monoid; fuse canonicalizes the multiset;
posterior is byte-identical. Issue #46 plans docs/SHEAF.md with
"0-cochains = scores, 1-cochains = judgments, 2-cochains = triad audits;
sections; gluing; failure to glue is measurable; H¹."

**(b) Book form.** The deepest statement is exactly one sentence:
**sufficient statistics form a commutative monoid and the posterior map
is a function of the fused statistic; everything else is transport.** The
sheaf apparatus is then derivable, not additional:

- The *gluing axiom* (compatible local sections glue uniquely) for
  evidence over an entity cover is already shipped and pinned: partial
  entity-set overlap fuses on the union (packet.rs pin list). Additivity
  of sufficient statistics + union canonicalization IS the sheaf
  condition — a two-paragraph proof, not a new document.
- The *obstruction* (H¹) is already computed: harmonic classes of the
  comparison complex (`compute_hodge_split`, MATH_FRONTIER §1). Nothing
  in the Čech language predicts a receipt this doesn't.
- What is *genuinely additional* in #46, and should be the whole issue:
  (i) the wire format (spectrum + provenance chain + gain metadata), and
  (ii) the transport maps between DIFFERENT contexts — cross-judge
  gain/gauge alignment, where fusion is NOT free. That seam is the
  ensemble/Cooke machinery (#48), not sheaf theory.

**(c) What it dissolves.** Three planned doc constructs (sections,
gluing axiom, H¹-as-new-math) shown redundant with two shipped theorems
(monoid + Hodge). The issue shrinks to its two real deliverables. This is
parsimony as work-avoidance with a proof attached.

**(d) Cheapest confirming derivation.** Write the two-paragraph gluing
proof. Then apply the test the repo already uses on formalisms
(MATH_FRONTIER's rejected list): does the sheaf language predict any
receipt not computable from (fuse, hodge_split, ensemble)? If none —
demote to a remark in the wire-format doc. Note the interaction with
Finding 1: if censored observations ship, the packet statistic becomes
the observation multiset itself — the monoid survives (multisets are the
free commutative monoid), which shows the monoid sentence, not the
Gaussian-moment compression, was the theorem all along.

---

## Finding 5 — The generative core: one group-structured Thurstone model with random pair effects. Mostly Book already; the confidence map is the last hand-tuned epicycle.

**(a) Current form.** Solver = IRLS/Huber on point log-ratios
(docs/MODEL.md:26–38). Confidence enters via
`g(c) = eps + (1−eps)·c^γ` with two config knobs
(src/rating_engine.rs:67–74, applied at rating_engine.rs:313).
Counterbalancing is bespoke machinery; probes run as separate
diagnostics.

**(b) Book form.** There is a single generative model and the repo is
visibly converging on it:

```
y(e, g, t) = χ_∅(g)·(ds)_e + Σ_{S≠∅} b_{S,e}·χ_S(g) + u_e + ε_{e,g,t}
u_e ~ (0, σ_b²),   ε heavy-tailed,   observed through the ladder censoring (Finding 1)
```

Under this one model: the IRLS/Huber solve is MAP under the heavy-tailed
ε (already Book — Huber IS a likelihood choice, not a hack);
counterbalancing is design balance in the G factor (its generalization,
the orbit transform, is shipped); every probe is a score test on a `b_S`
coefficient; the JCB coherence is the Parseval R²; DL is the u_e random
effect; the probe→estimator maturity ladder (PRINCIPLES §5) terminates in
fitting the b_S jointly instead of testing them one at a time.

**The residual epicycle:** the confidence map. Two free parameters
(eps, γ) that nobody estimates, mapping a self-report to a precision. The
logprob path already dissolves it — "weights each observation by its
measured variance instead of a stated confidence" (README.md:98) is the
Book form of the confidence channel. On the sampled path the map should
be a *fitted* calibration curve or nothing.

**(c) What it dissolves.** Two config knobs → measured variance or a
fitted curve; counterbalancing as special-cased machinery → a design
property of the group factor; the probe suite → score tests of one
model. Verdict: **approximates the Book form, trajectory correctly
aimed**; nothing structurally diverges except the confidence map and the
censoring (Finding 1).

**(d) Cheapest confirming experiment.** Fit (eps, γ) by maximum
likelihood on any existing receipt pack (stated confidence vs realized
squared residual). If the fitted curve is flat, or far from the config
defaults, the knob was never load-bearing — delete it. One script over
committed JSONL, zero spend.

---

## Finding 6 — Probe-zoo verdict: the orbit transform IS the Book form for invariance and equivariance; spin must stay outside the group; JCB v2 should be scored as an energy budget, not a product of rates.

**(a) Current form.** src/rerank/orbit.rs is the character transform on
Z₂³ with the subsumption stated in its header (orbit.rs:24–27: marginal
probes are subgroup restrictions; interaction coefficients invisible to
all of them). Spin/sweep is separate with an odd/even decomposition
(src/rerank/spin.rs:259–266). The benchmark (src/rerank/bench.rs) still
scores marginal-rate axes multiplied together.

**(b) Book form and verdict, in one line each.**

- For strict invariance and exact equivariance (flavors 1–2 of the
  repo's own taxonomy, invariance-theory.md §0): **the orbit transform is
  already the Book form** — belief = trivial character, biases =
  orthogonal characters, Parseval = the norm on deviation from
  equivariance. Verdict: IS.
- Paraphrase: #45's caveat is correct and should be defended against
  aesthetic overreach — rewordings have no inverses, so the object is a
  fiber (orbit mean + within-fiber variance as its own energy term), not
  a group factor. Do not force S_k.
- Spin: NOT an equivariance and must never be scored as one — it is
  linear response to a real field; the received quantity is the odd slope
  at zero field with a linearity residual (already built:
  spin.rs `spin_sweep`). The unified object is: compact factor
  (group + paraphrase fiber) where deviation-from-invariance is the
  norm, × an ℝ factor where bounded-and-signed response is the norm.
- The one doctrine change with teeth: **JCB v2 (#49) should be scored as
  energy fractions of this decomposition**, retiring the marginal-rate
  axes to derived projections. A product of correlated rates
  double-counts the interaction channels the orbit sees once; the
  Parseval budget can't. (#45's acceptance clause "BENCHMARK axis
  upgraded to the extended group" already points here — this finding says
  make it the spine, not an axis.)

**(c) What it unifies.** Seven probe surfaces → one decomposition with
two declared norm types; the benchmark score becomes a single energy
accounting identity instead of a product of per-axis rates.

**(d) Cheapest confirming experiment.** Recompute an existing JCB
leaderboard's consistency axes from orbit energies where cached orbit
data exists, and check leaderboard rank stability under energy-fraction
scoring. If ranks move, the product form was double-counting correlated
axes — a finding either way.

---

## Summary table

| # | Design | Verdict | Dissolves | Cheapest receipt |
|---|---|---|---|---|
| 1 | Ladder as point observation | DIVERGES — Book form is interval-censored likelihood (the one unconceived jump) | 5 mechanisms incl. quantization-curl floor and the ordinal-beats-ratio negative | refit ladder_curl + heavy-noise eval, $0 |
| 2 | DL + Hodge + retest (+ ST) | APPROXIMATES — three commuting projections, 8-block Pythagoras; ST is the GoF test, not a marginal | 4 instruments → 3 projections + 1 falsifier | one synthetic array, pin all stats as block norms, $0 |
| 3 | Portfolio vs JCB coherence | APPROXIMATES — one Gram matrix on (judges × G), two declared quotients | 2 formalisms → 1, plus a new shared-bias-channel receipt | Gram of cached orbits, ~$0 |
| 4 | Packet monoid vs #46 sheaf | monoid IS Book; sheaf language is derivable decoration | 3 doc constructs; #46 shrinks to wire format + transport | two-paragraph gluing proof |
| 5 | Elicitation core generative model | APPROXIMATES, correctly aimed; confidence map is the last epicycle | 2 config knobs; counterbalancing as bespoke machinery | fit (eps, γ) on committed receipts, $0 |
| 6 | Probe zoo | orbit IS Book for flavors 1–2; spin correctly outside; benchmark scoring should become the energy budget | 7 probe surfaces → 1 decomposition | rescore a cached leaderboard by energy fractions |
