# The mathematical frontier of cardinal & stable prior elicitation

Synthesis of the 2026-07-05 archive consultation (operator corpus + scry
intent + literature sweep + theory ideation; full documents under
`research/notes/math-frontier-2026-07-05/`, tier caveat: research agents served at
sonnet, all load-bearing claims re-derived or tested before adoption).
Ranked by (mathematical depth × buildability), status per item.
Continuity: every NEXT item is a tracked issue — #45 (orbit group
growth), #46 (sheaf/provenance wire format), #47 (stochastic
transitivity + DL variance floor), #48 (Cooke + Rank Centrality +
CUSUM), #49 (JCB v2 scale-up) — so the frontier survives sessions.

## Compass (operator's archive, verbatim)

> "exhaustively map out the space of type signatures [of elicitation
> primitives], especially statically cacheable ones... then agent
> continuations and algebraic effects."
> — corpus entity "LLM Prior Elicitation Framework"

> "Elos for everything via model pairwise comparisons eliciting cardinal
> latents." — scry INTENT.md

## 1. Full Hodge decomposition — SHIPPED (2026-07-05)

The cyclic residual splits w-orthogonally over the filled triangles:
`cycle space = im(curl*) ⊕ H`. `compute_hodge_split` in
`src/rating_engine/diagnostics.rs` reports `local_curl_frac` (triangle-auditable
disagreement) and `harmonic_frac` (cycles whose closing chords were never
elicited — invisible to every triad audit BY CONSTRUCTION), plus
`harmonic_dim = cycle_dim − rank(curl)`. Pythagoras invariant
`local + harmonic ≈ hcr` holds end-to-end through the IRLS solver
(pinned ≤ 0.02). Two planted extremes pinned: filled-triangle flow → all
local; chordless 4-cycle → all harmonic; the same hcr, opposite splits.

The load-bearing consequence: **elicitation efficiency and auditability
are in tension** — sparse O(n) comparison graphs have mostly-long cycles,
exactly where frustration hides from triad spot-checks. And a design
fact computed before building (exact rational rank): the JCB stride
graph has harmonic_dim = 0 — its triangles span the whole cycle space,
so harmonic diagnostics there are zero by construction, not by judge
virtue. Pinned in `experiments/tests/hodge_split.rs`; any pair-design change that
alters this surfaces. Measuring harmonic structure in real judges needs
a mixed design (triangle-rich block + chordless-cycle block).

## 2. Spectral identifiability diagnostics — SHIPPED (2026-07-05)

`spectral_diagnostics` in `src/rating_engine/diagnostics.rs`, populated in every solve up to
the dense-eigen cap: the **Fiedler value** (algebraic connectivity — the
standing "how well-posed was this solve" number; posterior variance along
the worst-identified direction scales as 1/fiedler) and the **Foster
residual** (Σ_e w_e·R_eff(e) must equal n − components EXACTLY, by
Foster's theorem — a free correctness invariant over the same effective
resistances the planner optimizes; nonzero means broken linear algebra,
not a bad judge). Pinned on hand-computed spectra (P3, weighted triangle,
disconnected graph) and end-to-end through IRLS
(`experiments/tests/program_equivalence.rs`). Still open in this thread: Rank
Centrality (Negahban–Oh–Shah) as a near-minimax spectral cross-check.

## 3. Program equivalence for elicitation types — SHIPPED (2026-07-05)

Elicitation primitives as typed operations `(EntitySet, Attribute,
Instrument, Model) → SufficientStatistics`, statically cacheable exactly
when pure in those arguments (the content-addressed cache already
enforces this — the theory names it). The free structure: a commutative
monoid of sufficient statistics under evidence-fusion; two elicitation
programs are equivalent iff they fuse to identical statistics.

Pinned in `experiments/tests/program_equivalence.rs` — with a CORRECTION to the
theory notes, decided by the machine: arrival-order/batching invariance
holds (≤1e-9), and same-pair weight re-partition is invariant **even
under active Huber clipping** — the notes claimed a boundary there, but
fusion is per-pair BEFORE IRLS and the λ-weighted mean is linear, so the
monoid theorem is stronger at pair granularity than claimed. The real
equivalence boundary is re-ROUTING evidence across distinct edges (a
different program, not a re-partition). Rejected-claim evidence: the
theory's deliberate counterexample failed to be one.

## 3½. The orbit transform — SHIPPED (2026-07-05)

Harmonic analysis on the elicitation group Z₂³ (`src/rerank/orbit.rs`,
`judge --orbit`): belief = trivial-character coefficient, biases =
orthogonal character coefficients, Parseval = exact energy accounting,
coherence = projection ratio. Subsumes the marginal probes as subgroup
restrictions and adds the interaction coefficients they cannot see.
Pinned: perfect judge (all energy trivial), the two position pathologies
separated (order·polarity vs triple — the first derivation conflated
them; the test corrected it). Live: mini's dominant bias is the
order·polarity interaction (22.3% of energy) — a measured instance of
structure invisible to all marginal probes.

## 3¾. Leave-one-out consistency + the design atlas — SHIPPED (2026-07-06)

Two instruments in the classical styles:

- **Sum-over-histories per judgement** (`SolveSummary.loo`): every edge
  tested against the prediction of the rest of the graph — leverage
  h_e = λ_e·R_eff(e) from the spectral pass (trace identity Σh = n − c,
  Foster again), externally-robust studentization with the judgement's
  pre-Huber precision against a MAD scale. Its own first implementation
  failed the planted test: post-Huber weights hide exactly the outliers
  the robustifier crushed — the diagnostic must see what the robustifier
  saw. Pinned: planted corruption flagged at |z| > 3 and nothing else;
  clean data unflagged; bridges counted as unaudited, never scored.
- **Enumerated designs, not intuited ones** (`experiments/examples/design_atlas.rs`,
  docs/DESIGN_ATLAS.md): all circulants at n ∈ {8,10,12} scored by
  (edges, triangles, harmonic_dim, Fiedler). Headline: C₈(1,3,4)
  strictly dominates the hand-picked v1 graph — same 20 edges, same 16
  triangles, same Fiedler 4.0, harmonic_dim 1 instead of 0. One stride
  away from strictly better, findable only by search. Routed to #49 as
  the v2 core graph; profile pinned.

## 3⅞. Judge portfolio theory — SHIPPED (2026-07-06)

The ensemble question made exact (`experiments/src/ensemble.rs`,
`experiments/examples/judge_portfolio.rs`): judges as a portfolio under correlated
errors. Spearman-triad loadings (1904) on the z-scored latent
correlations; full error covariance Ψ = R − llᵀ projected to the PSD
cone; minimum-variance weights Ψ⁻¹l; total precision lᵀΨ⁻¹l; marginal
information I − I₋ᵢ per dollar as the budgeted-roster ranking; effective
error sources = PR of normalized Ψ. Diversification theorem pinned: a
noisier-but-independent judge strictly adds information; a clone shares
one error channel and one weight. Live zero-spend study from the
retest pack: six frontier models carry **2.89 effective error channels**
(the labs share failure modes); deepseek buys ~10× sonnet's marginal
information per dollar; negative GLS weights appear exactly where shared
error gets hedged — Markowitz in judge space. Three estimator designs
failed en route (raw-PR, eigen loadings, un-projected Ψ → a negative
precision live) — each documented and pinned. Open: per-attribute
geometry (does the judge map change with the attribute?), Cooke
calibration weighting composed on top (#48), and portfolio-aware planner
integration (spend the next comparison on the judge with the best
marginal information per dollar for THAT pair).

## 3¹⁵⁄₁₆. The judgment packet — PROTOCOL CORE SHIPPED (2026-07-07)

`src/packet.rs` (issue #46): belief as a medium. A packet = attribute +
template + judge + (entity id, content hash) pairs + observations as
(log-ratio, precision), canonically ordered, identified by blake3 over a
canonical BYTE encoding (length-prefixed strings, f64 bit patterns — no
JSON floats near the identity). `fuse` lands any partition of the same
evidence, in any order, on a posterior **byte-identical** to the
single-party solve — pinned with `to_bits` equality, not tolerance.

The pin forced a solver-level fix with teeth: fuse buckets were HashMap,
whose per-instance random iteration order made identical multisets differ
by ~30 ulps across engines. Buckets are now BTreeMap; the
program-equivalence pin tightened from 1e-9 to bitwise on the bulk path
(the incremental path keeps its honest 1e-9). Determinism of
multiset → posterior is now structural, not statistical.

Also pinned: partial entity-set overlap fuses on the union; one flipped
bit is a different packet; an impersonated entity (same id, different
content hash) refuses to fuse. Open in #46: the seriate-side provenance
chain (packet → capture ids), orbit-spectrum and gain metadata fields,
and the two-party live demo from the pilot map's cache.

## 4. Stochastic transitivity hierarchy — SHIPPED (diagnostic)

Weak/moderate/strong stochastic transitivity (WST ⊂ MST ⊂ SST) on
repeat-sampled choice probabilities catches inconsistency that Hodge
curl structurally cannot (probabilistic intransitivity with zero mean
curl). The diagnostic is implemented (`experiments/src/transitivity.rs`,
WST/MST/SST over repeat-sampled pairs) with a live study pack at
`research/artifacts/live/transitivity-2026-07-08/`. Still open: whether it is
promoted into the JCB composite headline.

## 5. Pooling across judges: log-pooling is forced, not chosen

For Gaussian log-ratio posteriors, logarithmic opinion pooling =
precision-weighted averaging — already what evidence fusion does. The
theory adds: log-pooling is the unique pooling operator that is
externally Bayesian AND commutes with the ratio-scale invariance group
(the check: verify our fusion commutes with reflection/gain
transformations — a property test, not a live run). Cooke's classical
model transfers as calibration-weighted pooling where our null pairs and
known-order anchor pairs play the role of calibration questions — the
principled cross-model aggregation for "communications" use.

## 5½. Nonce draws: cache-priced repeat elicitation — SHIPPED (2026-07-06)

`nonce_draws` / `judge --draws k`: repeat the judgement varying only a
suffix nonce after a byte-identical prefix (pinned) — provider prompt
caching bills the long prefix at the cached rate, and at temperature 0
the spread over nonces is the model's pure context-sensitivity noise:
the within-pair σ_w the DL floor consumes, measured with the actual
contaminant. Live: deepseek billed 95% of input as cached (8 judgements
on a 1300-token prompt, $0.0010) and showed σ_w = 0.54 vs mini's 0.11 on
the same pair — context sensitivity is a model property that discounts
the portfolio ΔI/$ through the floor; between-run mean shift surfaced
the two-level structure unprompted. This is the missing repeat-sampling
instrument §6 gates on; the de Finetti no-shared-context clause is
satisfied by construction (independent calls, fixed presentation).

## 6. Variance components before repeat-sampling ships — ESTIMATOR SHIPPED (2026-07-06)

`repeat_pooling::pool_repeats`: two-level model m = Δ + b_pair + ε with
the DerSimonian–Laird moment estimator adapted to graph fits (Cochran's Q
over weighted solve residuals, df = cycle dimension), then a floored
re-solve with Var(m̄) = σ_b² + σ_w²/k — pooled precision is capped at
1/σ_b² no matter how many draws. Pinned (`experiments/tests/repeat_pooling.rs`):
planted (σ_w, σ_b) recovered; zero heterogeneity yields no phantom floor
and matches the naive solve; and the misranking pin — a 200-draw
frustrated pair flips the naive k/σ² order while the floored solve holds
the truth. The repeat-sampling instrument may now ship without inheriting
the overweighting bug. Still open in #47: the de Finetti exchangeability
clause in the instrument contract (no shared context across draws) and
the WST/MST/SST probe built on top.

## 7. Time as a change-point problem, not a trend

Provider model updates are step changes on unknown dates; a linear drift
test has poor power against steps. The time-drift diagnostic is CUSUM on
partial sums of (m_t − m̄) with change-point location, alongside the
cheap linear slope — the two disagreeing (slope ≈ 0, CUSUM large) is
itself the diagnostic signature of a step. Fills the last ✗ row of the
invariance table when scheduled probes exist.

## 8. Fisher–Rao for PMF drift

Letter-instrument judgments are points on a shared simplex; the
Fisher–Rao metric (2·arccos Σ√(p_i q_i)) is the canonical Riemannian
distance there and should replace ad-hoc JSD where we quantify "how far
a judgment PMF moved under a transformation." Cheap, principled,
drop-in.

## Rejected (rejection is a finding)

- **Optimal transport between rankings**: OT's advantage is mismatched
  support; our alphabets and entity sets are shared — Fisher–Rao wins at
  lower cost. Revisit only for cross-instrument PMF comparison.
- **Tropical geometry of the ladder**: currently a relabeling of the
  already-measured quantization-curl floor (0.00198); earns entry only
  if it yields a closed-form predictive bound on the floor from ladder
  geometry.
- **Preference polytopes/cones**: the natural tool for ordinal-only
  data; we carry Gaussian posteriors everywhere it matters. Revisit if
  best-worst ships and needs cheap confidence regions.
