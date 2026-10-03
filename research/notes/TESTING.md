# The test battery

`cargo test --workspace` runs the engine integration suites under `tests/`
plus the research suites in `experiments/tests/` (never published, held to the
same green-CI bar). The core suites were authored and then adversarially reviewed as
a deliberate exercise: every assertion had to be falsifiable by a plausible
implementation bug, every statistical claim had to hold over seeded
ensembles (never a single draw), every suite had to survive a reviewer
whose only job was to break it — hunt tautologies, force flakes with triple
runs, re-derive the closed-form math behind each mechanism and check the
assertions against ground truth rather than against the code under test.

The review found real bugs in `src/` (fixed, pinned by regression tests)
and several honest negatives (documented below, not painted over). That is
the point of the battery: it is strong enough to find things.

## The adversarial suites

| Suite | Attacks | Highlights |
|---|---|---|
| `property_solver` (15) | IRLS+Huber recovery claims | Planted-truth Kendall-tau floors over 40–120-seed ensembles; adversarially reversed observations at 5%/15% with bounded rank displacement; robust-vs-naive fit comparison (tau 0.709 vs 0.556 under 15% corruption); gauge/shift invariance to 1e-6; strict monotonicity across all 17 ratio-ladder rungs |
| `censored_likelihood` (1) | Experimental ladder-channel model | Seeded clean-channel comparison: the ordered-probit ladder channel must beat or match point centers on its own planted channel |
| `metamorphic_invariance` (11) | Full sort path invariances | Input-order and relabeling invariance; duplicate texts score near-equal; weak IIA (adding a clearly-worst item does not invert the top three); sorting by X and by "lack of X" produce reversed orders |
| `calibration_coverage` (12) | Uncertainty honesty | Gauge-aligned 95% CI coverage over 200-seed ensembles; posterior std shrinks as observations double; top-k error tiny on huge planted gaps and >0.2 on planted coin flips; `p_flip` semantics pinned |
| `adversarial_judges` (12) | Pathological-judge taxonomy | Pure position bias → diagnostics show 100% flips; intransitive A>B>C>A → solver averages through the cycle; scale-compressed (ratio always 1.05) → order still recovered; refuser → run metadata counts it, rest ordered; gaslighter (confidence 0.99, direction seeded-random) → robust fit limits damage; format vandal → failed calls surface, order survives |
| `planner_efficiency` (13) | Active planning and stopping | ≥70% of exploitation proposals touch the boundary band; pruning changes `explore_pruned_count` but never the top-k set; answered proposals reduce top-k error; the critical straddling pair is proposed first; certified stopping fires well below n(n−1)/2 observations |

The comparative research batteries — method dominance (cardinal vs Likert
vs ordinal), planner regret vs uniform-random selection, and the
judge-coherence benchmark — run from `experiments/tests/`; their evidence
packs live in `research/artifacts/live/`.

## A bug the battery found (fixed)

**Huber MAD degeneracy collapse** (`src/rating_engine/math.rs`). The outlier
scale used `mad(residuals)` with an absolute `<= 1e-18` zero-guard. When
most residuals are tied up to floating-point noise (~1e-12, e.g. duplicate
anchor observations), the MAD passes the guard, `delta = huber_k × (fp
noise)` clips **every** edge, and the whole fit collapses toward zero —
3–4 orders of magnitude below the closed-form answer in the repro. Fixed
with a relative degeneracy floor (MAD ≤ 1e-8 × max-abs residual falls back
to the max-abs scale). Regression:
`property_solver::huber_mad_scale_collapses_on_near_tied_residuals`,
asserted against the hand-solved normal equations.

## Current empirical boundaries

- **Coverage is conservative, not exact.** Gauge-aligned 95% intervals
  cover ~99.7% on the tested ensembles — miscalibrated in the safe
  direction (intervals honest but wide). Pinned so a drift toward
  overconfidence fails loudly.
- **Informational**: sparse hub-and-spoke graphs are numerically fragile
  when the lone bridge edge to the gauge node gets Huber-downweighted;
  `global_diff_var_safe` uses the worst-case `(σᵢ+σⱼ)²` bound rather than
  the exact difference variance. Both are deliberate/known and written
  down.

## Discipline

- Fixed seeds only (`StdRng`); `thread_rng` is banned from assertions.
- Statistical claims assert on ensembles (40–200 seeded replicas), with
  margins verified non-flaky by instrumented review, then triple-run.
- Each suite runs in seconds; the full battery in a few seconds.
- A test that discovers a real `src/` bug becomes an `#[ignore]`d repro
  with the mechanism documented — never silently accommodated — until the
  fix lands and un-ignores it.
