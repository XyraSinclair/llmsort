# Red team: the mathematical register (2026-07-09)

Adversarial review of the repo's ideas under its own Principle 4
("physics vocabulary is admitted only where the math is literal, never
as decoration") and Principle 3 (no claim without its denominator and
noise class). Six findings, ranked by decision impact. Each carries a
receipt, the objection stated as math, the weaker-but-true replacement,
and the refutation path that would kill the objection. Claims that
survived attack are listed at the end — self-refutations count both
ways.

Reviewer denominators, up front: the Steiger tests in Finding 5 assume
entities are independent draws (the graph solve couples them, so my z
values are slightly inflated) and approximate corr(fused, judge) as
√((1+ρ₁₂)/2) (exact only under equal loadings). Everything else is
recomputed directly from committed receipts.

---

## Finding 1 — The sweep's shape narrative contradicts its own receipts

**Receipt.** `artifacts/live/spin-sweep-2026-07-05/README.md`:

> "gpt-5.4-mini — m(f) ≈ 0.20·f (odd-dominant, linear) … The response
> is monotone in f with no detectable threshold and a near-zero even
> component."

> "claude-sonnet-4.6 … the structure is in the EVEN part:
> (m(+3)+m(−3))/2 − m(0) = +0.31 nats"

and `research/notes/FIRST_PRINCIPLES.md` §5½: "claude-sonnet-4.6 slope −0.014
with R² 0.02 and a positive even component … the response is in the
even part, not the odd part."

**Objection (recomputed from the committed raw JSON,
`contested_gpt-5.4-mini.json` / `contested_claude-sonnet-4.6.json`).**
The instrument defines its even statistic as
`even_response_mean = mean over f∈{1,2,3} of (m(f)+m(−f))/2 − m(0)`
(`src/rerank/spin.rs:264-270`). Recomputing:

| model | even(1) | even(2) | even(3) | **even_response_mean** | narrated as |
|---|---|---|---|---|---|
| gpt-5.4-mini | +0.358 | +0.350 | +0.010 | **+0.239** | "near-zero even component" |
| claude-sonnet-4.6 | −0.148 | −0.056 | +0.310 | **+0.035** | "the structure is in the EVEN part" |

By the instrument's own defined statistic, the two models' even
components are the **reverse** of the narrative: gpt's is the large one
(+0.24) and sonnet's is negligible (+0.035). Sonnet's "+0.31" is the
single f=3 edge value — the largest of three even values whose signs
disagree (−0.15, −0.06, +0.31), which is what noise looks like, not
what an |f|-response looks like. The stored receipts contain
`even_response_mean: None` — the narrated numbers were hand-derived
edge picks, never the instrument's output.

Second contradiction, same pack: "monotone in f with no detectable
threshold" is false of the data — m(−1)=+0.04 > m(0)=−0.23 and
m(+3)=0.33 < m(+2)=0.65. Only the fitted line is monotone, and a fitted
line is monotone by construction.

Third: the live "linear" verdict is R² = 0.81, but the instrument's own
scripted-pathology calibration (`tests/judge_explain_cli.rs:1498,1538`)
pins *linear* at R² > 0.98 and *exposes step* at R² < 0.9. The flagship
live "linear-odd" example sits in the region the repo's own pin
reserves for step judges. The thresholds were calibrated on noiseless
scripted judges; they do not transfer to live data with per-point
residual sd ≈ 0.23 nats (recomputed), and no noise-adjusted threshold
exists.

**Weaker-but-true.** "gpt-5.4-mini: positive monotone *association*
(fitted slope +0.200, residual sd 0.23 nats, n=7 points × 2 draws);
linear vs saturating vs threshold shapes are indistinguishable at this
noise floor. sonnet: no detectable odd response; even statistic +0.035,
indistinguishable from zero; the f=±3 edge elevation is a single-point
observation." Both packs need errata-on-top (Principle 6).

**Refutation.** Show σ_w for this pair (nonce-draw machinery exists) is
large enough that E[R² | truly linear] ≈ 0.8 at k=2 — that would
license "consistent with linear" (not "linear"), and would
simultaneously concede the even-component claims are sub-noise. The
even_response_mean recomputation cannot be refuted; it is arithmetic on
the committed receipts.

---

## Finding 2 — χ is not a susceptibility: the field has no units and
its two directions are different texts

**Receipt.** `src/rerank/spin.rs:216-235` (`spun_criterion_at`): field
"intensity" ∈ {1,2,3} indexes three hand-written preamble wordings;
sign is implemented by quoting a 48-char excerpt of the *favored* item
— `first.1` for f>0, `second.1` for f<0 (`spin.rs:298-305`).
`SpinSweepReport.chi_slope` is documented as "the susceptibility as a
linear-response coefficient (nats per intensity step)."

**Objection.** Linear response χ = ∂m/∂h|₀ requires h on an interval
scale. Here the abscissa is an ordinal wording ladder: the map
(1,2,3) → ("slight hunch", "fairly convinced", "certain, everyone
agrees") has no measured spacing. Reparameterize the same three
wordings as (1,2,4) and both `chi_slope` and `linearity_r2` change;
nothing in the data privileges equal spacing. So R²-as-linearity-
evidence is not a property of the judge — it is a property of the
prompt-writer's arbitrary coordinates. The invariant content is only:
sign of association, monotonicity (a rank statistic), boundedness.

Worse, the field's two signs are not the same perturbation reversed:
+f quotes item A's opening, −f quotes item B's opening. Model the
effective persuasive force as m(+f) = χ·s_A(f) + …, m(−f) = −χ·s_B(f)
+ …; then the even channel picks up χ·(s_A − s_B)/2 — a pure
excerpt-asymmetry artifact — in addition to any genuine |f| response.
The two are algebraically indistinguishable in one sweep. This is
*exactly* the asymmetry critique the ideation note leveled at the
secant (`notes/ideation-2026-07-05/invariance-theory.md` §3(c) problem
2), which `research/notes/FIRST_PRINCIPLES.md` §5½ declares "CLOSED as
instrumentation." The sweep closed problem 1 (no intensity sweep); it
inherited problem 2 and gave the artifact a new name (the even
component).

**Weaker-but-true.** "Framing-pressure probe: response to an ordinal
3-rung pressure ladder, reported as sign + rank-monotonicity + range;
the even channel confounds |f|-response with framing-side asymmetry."
Drop "susceptibility as a linear-response coefficient" until the field
has units.

**Refutation.** Two cheap receipts: (a) swap which entity is "first"
and re-sweep — a genuine |f| response is invariant, the asymmetry
artifact flips sign; (b) give the field units by measuring each
preamble's persuasive strength on a calibration pair battery (making
the abscissa an estimated, interval-scaled quantity), then re-fit. If
(a) shows invariance and (b) shows the hand ladder was near-linear in
measured strength, χ-as-slope is licensed and this finding dies.

---

## Finding 3 — The underwriting formula prices an unspecified estimand

**Receipt.** Issue #50: `P(reversal) ≈ Φ(−|Δ̂| / √(σ_b² + σ_w²/k_claim
+ se(Δ̂)²))`, "the two-level model prices the policy directly."

**Objection, as math.** The two-level model
(`src/repeat_pooling.rs:1-25`) defines b_p as "the pair's STRUCTURAL
offset — the per-pair component of frustration that more sampling can
never remove." Structural means persistent: a fixed effect of
(judge, pair, protocol). Then a re-elicitation under the same protocol
draws m̄' = Δ_p + b_p + ε̄' with the *same* b_p, so

  Var(m̄' | history) = σ_w²/k_claim + Var(Δ_p + b_p | history),

and σ_b² enters the *conditional mean* (through the posterior of
Δ_p + b_p, which the pair's own draws already pin), not as fresh
variance. The quoted formula instead treats b as re-drawn i.i.d. per
elicitation session — an exchangeable random effect. The two readings
price materially differently: under persistence, a pair with small
|Δ̂| but large |b_p| is *cheap* to warrant (the claim reproduces the
same offset), and the formula overprices it; conversely, if the
warranted proposition is sign(Δ_p) — the graph-explained quantity —
then a claim protocol that re-draws only the named pair measures
Δ_p + b_p, a biased estimator of the proposition, and the policy pays
on events that are not noise around the warranted claim at all. You
cannot price b as a risk (random across claims) and describe it as a
defect (fixed) in the same instrument.

Two further gaps an actuary would flag rather than laugh at:
(i) **priced event ≠ trigger event** — the premium prices
P(zero-crossing) but the claim pays only on a crossing certified
beyond 2se, so P(payout) ≈ Φ((−|Δ̂| − 2·se_claim)/·) < Φ(−|Δ̂|/·); the
formula systematically over-collects, which a competitive book cannot
sustain and an honest spec should state as an upper bound.
(ii) **tails** — DL supplies only second moments; Φ assumes Gaussian b.
The repo's own solver runs Huber IRLS *because* it believes judgment
errors are contamination-heavy-tailed; a pricing formula whose tail
mass is exactly where the claims live cannot assume away the tail model
the rest of the codebase rejects.

**Weaker-but-true.** Specify the estimand before the formula: EITHER
(a) coverage defines b as a wording-family random effect — legitimate
if the claim protocol procedurally rotates wordings AND Δ̂ is defined
as the family-mean (then σ_b² in the denominator is licensed) — OR (b)
b is persistent, in which case the reversal probability is
Φ applied to the posterior of Δ_p + b_p with variance σ_w²/k_claim +
posterior-var, and σ_b buys no premium. Present Φ(·) as a
second-moment approximation with an explicit tail-model caveat, and
price the *certified* trigger, not the crossing.

**Refutation.** The MVP backtest in #50 §6 (warranties on judge-bench
v1, retest pack as claims stream) is the correct killer — if realized
loss ratios match the priced premiums under the formula as written,
the exchangeability reading was empirically right and this objection
reduces to a documentation fix. Run it before any pricing language
ships.

---

## Finding 4 — Byte-identity is a single-binary property marketed as a
two-party guarantee; and `fuse` is not idempotent

**Steelman first.** The `==` pin is not a fetish: (a) it caught a real
bug — HashMap bucket order made identical multisets differ by ~30 ulps
across engines (commit cb44831); bitwise equality is a free
nondeterminism detector that 1e-9 tolerance would have slept through.
(b) Adjudication verdicts (#50) are threshold predicates
(PAYS/HOLDS at a 2se margin), and threshold functions are
discontinuous: two parties disagreeing by 1 ulp *at the boundary*
reach different verdicts, so "agree on every byte" is the only clean
sufficient condition for "agree on every verdict." The ambition is
mathematically right.

**Receipt.** `src/packet.rs:6-18`: "the fused posterior is
**byte-identical** to what a single party holding all the evidence
would compute … A CRDT of belief." Issue #50: "Two honest adjudicators
reach byte-identical verdicts — disputes resolve by recompute."

**Objection.** The pin (`tests/packet.rs`, `to_bits` equality) runs in
one process, one binary, one target. Cross-party recompute means
different machines and builds. IEEE 754 makes +,−,×,÷,√ bit-exact
everywhere, but the solve path contains libm transcendentals —
`c.powf(gamma)` in confidence weighting (`src/rating_engine.rs:313`),
`ratio.ln()` in ingestion (`rating_engine.rs:1866,1956`) — and libm
results differ in the last ulp across macOS/glibc/musl/aarch64. No
cross-target CI pins the fuse hash. So the protocol claim ("two honest
adjudicators…") is currently *unwarranted as stated*: byte-identity is
proven per-binary, asserted per-party. Under Principle 6 discipline the
honest form is: verdict-by-recompute requires either a pinned
reproducible binary named in the policy, or a cross-target bitwise CI
pin — neither exists.

Separately, "CRDT" requires an idempotent, commutative, associative
join. `fuse` (`src/packet.rs:174-240`) concatenates every observation
from every packet with no dedup by packet id: fuse([p, p]) doubles p's
evidence and yields a *different, more confident* posterior. Multiset
union is commutative and associative but not idempotent. The actual
CRDT here is the grow-only *set of content-addressed packets* (join =
set union, dedup by packet id) with the posterior as a query function —
true, but only if some layer enforces exactly-once packet inclusion and
packets carry disjoint provenance (a party who re-seals another's
observations inside their own packet double-counts undetectably —
precisely the reinsurer flow #50 §3 proposes). Neither the enforcement
nor the disjointness assumption is in the code or its header.

**Weaker-but-true.** "Fusion is a deterministic function of the set of
content-addressed packets, bit-reproducible within a build; the CRDT
state is the packet set (dedup by id — one `BTreeSet` away), and
cross-build verdict agreement is an open pin." What `==` buys beyond
canonical-encoding+tolerance: nondeterminism detection and
boundary-proof verdicts — both real, both currently scoped to one
binary.

**Refutation.** (a) Add a CI job hashing a fixed fuse on x86_64-linux +
aarch64-darwin; if bitwise-equal, the cross-party claim is pinned and
the scope objection retires. (b) One-line dedup of packets by id in
`fuse` plus a documented disjoint-provenance assumption kills the
idempotence objection.

---

## Finding 5 — "Orthogonal at +0.072 with real power" narrates a point
estimate its own CI does not certify

**Receipt.** `artifacts/live/corpus-map-500-2026-07-08/README.md`: "the
two dimensions remain measured-orthogonal with real statistical power"
(commit b5677fd: "orthogonal at +0.072 with real power").

**Objection.** Spearman ρ̂ = +0.072 at n = 470: se(z) ≈ 1/√467 = 0.046,
so ρ̂ is 1.56 se above zero with 95% CI ≈ [−0.02, +0.16] — under the
repo's own 2se standard (transitivity pack, 02e61aa) that neither
certifies a nonzero correlation nor certifies orthogonality; it
certifies |ρ| ≤ 0.16. And the pilot→scale drift (+0.003 → +0.072) is
exactly what a small true positive ρ ≈ 0.07 looks like. "Real power"
is true only against halo-scale alternatives (ρ ≥ 0.3 is excluded
decisively) — that is the claim that should be printed. Two unmodeled
systematics, directions opposed: both attributes were scored by the
same two judges (shared judge error is common-mode and inflates ρ̂);
per-attribute measurement noise attenuates ρ̂ toward 0 (transmissibility
0.87/0.81 bounds it). Neither correction is computed; the truth could
sit on either side of +0.072. Also: the entities are coupled through
the shared comparison-graph solve, so the independence se is a floor.

**Weaker-but-true.** "ρ̂ = +0.072, 95% CI [−0.02, +0.16] treating
entities as independent; halo-scale correlation excluded with
overwhelming power; exact orthogonality not certified; disattenuated /
shared-judge-corrected estimates not yet computed."

**Explicit survival, same pack.** The claim I expected to kill —
"fused 0.903 beats both single judges (0.890/0.855)" asserted with no
test — *survives* when the missing test is run: Steiger's z for
dependent correlations (with corr(fused, judge) ≈ √((1+0.868)/2) ≈
0.966) gives z ≈ 2.5 vs gemini on ambition and z ≈ 2.2 vs deepseek on
rigor — beyond 2se, four-for-four across pilot+scale. The repo asserted
it without the receipt; the receipt, once computed, backs it.
Transmissibility 0.868/0.811 and validation 0.903 also survive: at
n=470 their CIs are tight (0.903 ∈ [0.885, 0.918]).

**Refutation.** Compute the CI in-pack and show "real power" was
always scoped to the halo alternative; compute the disattenuated and
judge-partialed correlations and show they bracket ≈ 0. Then only the
word "orthogonal" needed softening, and this finding shrinks to
wording.

---

## Finding 6 — Vocabulary audit: monoid, sheaf, Noether (Hodge and the
orbit transform survive)

**Monoid.** Receipt: `src/packet.rs:11-13` "the solver's sufficient
statistics form a commutative monoid (pinned in
tests/program_equivalence.rs)." The pinned theorem is arrival-order /
batching / partition invariance — i.e. the posterior is a well-defined
function on the free commutative monoid of observation multisets. That
is true and valuable, but "sufficient statistics form a monoid" says
something stronger and false: it implies a finite-dimensional
mergeable summary (a monoid homomorphism onto sufficient statistics).
Under Huber IRLS no such compression exists — the weights depend
jointly on all residuals — which is *why* packets must carry every
observation. The design itself refutes the slogan on its own header.
Weaker-but-true: "the posterior is a function of the observation
multiset; the multiset (not any summary) is the fusion state."
Refutation: exhibit finite-dimensional statistics whose merge commutes
with the Huber solve; I claim none exists for the general case.

**Sheaf (#46).** The H¹ half is literal and correctly stated: for the
comparison graph with filled triangles as 2-cells, harmonic classes ≅
H¹(complex; ℝ) — this is Jiang–Lim–Yao–Ye, and `compute_hodge_split`
genuinely computes it. The *sheaf* half is currently branding: "a
judgment record … as a SECTION over its context; gluing sections
across contexts/judges = the sheaf condition" names no site, no
restriction maps, and no gluing axiom — and the issue's own acceptance
criteria (wire format round-trips; two-party fusion byte-identical)
test zero sheaf-theoretic content. "Failure to glue is measurable
(harmonic + gain mismatch + pooling residual)" bundles three residuals
from three unrelated decompositions as if they were one cohomological
obstruction. Constructive fix that would make the word literal: a
Hansen–Ghrist cellular sheaf on the comparison graph with the fitted
per-template/per-judge gains as edge restriction maps — then H⁰ =
globally consistent scores, the sheaf Laplacian generalizes the solver,
and "gain mismatch" really is a gluing failure. Either write SHEAF.md
with those restriction maps or retitle the issue "wire format +
cohomology notes." Refutation: deliver the definitions; the objection
is to the current state and is fully dischargeable by doing the math.

**Noether.** Receipt: `notes/ideation-2026-07-05/invariance-theory.md`
§3(a) "Noether's theorem is the correct, non-decorative analogy."
There is no action functional and no dynamics; the fact in play is
ker(graph Laplacian) = constants per component, i.e. the loss has a
1-parameter symmetry group and the minimizer is a coset. Calling the
shift symmetry "gauge freedom" is fine (it is literally a symmetry
quotiented by convention); invoking Noether is precisely the
decoration Principle 4 bans — no conserved current is ever computed.
One-line fix in a note that is otherwise the repo's best
self-criticism.

**Explicit survivals.** (a) The Hodge decomposition is the real
theorem, correctly used: w-orthogonal split over filled triangles,
Pythagoras pin local+harmonic ≈ hcr, harmonic_dim = cycle_dim −
rank(curl), planted extremes at both ends, and the honest admission
that the JCB graph has harmonic_dim 0 *by construction*. (b) The orbit
transform is literal character theory on Z₂³ — belief as the trivial
character, Parseval as an exact energy budget, position bias split
into two characters — with the algebra machine-corrected. (c)
"Paramagnetic judgement" is structurally faithful (zero spontaneous
direction + positive field response) and labeled n=1. (d) The
frustration/ladder-curl self-refutation and the secant→sweep upgrade
show Principle 1 operating for real. (e) The transitivity pack's
raw-flag vs beyond-2se split is the statistical honesty the packs in
Findings 1 and 5 should be held to.

---

## Meta

The repo's failure mode is not fake math — every structure I attacked
short of the sheaf is genuinely instantiated somewhere. It is
*narration outrunning the instrument on live n=1 receipts*: the sweep
pack states shape verdicts its own defined statistic contradicts
(Finding 1), the map pack prints a power claim its own CI doesn't back
(Finding 5), and the packet header states a compression theorem its own
design refutes (Finding 6). The discipline that fixed the transitivity
pack (certify only beyond 2se; flag the rest) exists in-repo and simply
wasn't applied to the physics-flavored packs. Errata-on-top for
spin-sweep-2026-07-05 and corpus-map-500-2026-07-08 are the two
same-day actions (Principle 6).
