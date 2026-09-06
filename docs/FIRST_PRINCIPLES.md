# First principles: the type system of structured LLM judgement

Permute the smallest words and see what the space actually is; then check,
cell by cell, whether the repo occupies it. ✓ = implemented with evidence,
◐ = partial, ✗ = absent (and honestly so).

## 1. The primitives

Seven nouns, everything else is composition:

| Primitive | What it is | Where |
|---|---|---|
| **Entity** | a thing that can be judged (text shown to the judge) | seriate `Entity`, cardinal `MultiRerankEntity` |
| **Attribute** | one way an entity set wants to be ordered | `Attribute` (content-addressed: reworded = different) |
| **Presentation** | which entity in which slot, this call | `Presentation`, reflection algebra |
| **Judgement** | one elicited answer, typed by instrument | `PairwiseJudgement`, seriate `JudgementRecord` |
| **Evidence** | the judgement as a distribution, mass accounted | `AnswerEvidence` + `PmfCompleteness` |
| **Weight** | how much one judgement moves the fit | unit point precision or measured PMF precision, scaled by rater/repeats/temperature |
| **Posterior** | latents ± uncertainty over the set, per attribute | `CompiledPosterior`, rating engine solve |

## 2. The instrument grid

An instrument is a point in **arity × scale × output-form**:

- arity: 1 (pointwise) · 2 (pairwise) · k (setwise) · n (listwise)
- scale (Stevens): nominal (pick) · ordinal (order) · interval (differences
  meaningful) · ratio (ratios meaningful)
- output form: point (one answer) · sample-set (repeat draws) · **PMF**
  (answer-token logprobs — the model's whole prior in one call)

| Cell | Status | Notes |
|---|---|---|
| pairwise · ratio · point | ✓ `canonical_v2` | the founding instrument |
| pairwise · ratio · PMF | ✓ `ratio_letter_v1` | 52-letter alphabet; live study: ~3× separation/dollar vs point |
| pairwise · ordinal · point | ✓ `ordinal_v1` | direction + confidence JSON |
| pairwise · ordinal · PMF | ✓ `ordinal_letter_v1` | 3-token alphabet; cheapest instrument |
| pairwise · interval · any | ✗ | "how much more, additively" — meaningful only for bounded attributes; low priority, ratio subsumes on log scale |
| pointwise · ordinal/ratio (rubric) | ◐ scalar control | seriate `scalar` digit-PMF; a baseline, not a path — anchor drift is the documented disease |
| pointwise · ratio **to fixed anchors** | ✗ | classic magnitude estimation with pinned anchor entities; fixes anchor drift; cheap O(n); worth building |
| k-wise · nominal ("which is most") | ◐ | seriate `kwise` + lowering; not exposed in cardinal |
| k-wise · **best–worst** (MaxDiff) | ◐ measured, refuted as built | `setwise_cached --answer bw`: 2k−3 ordinal pairs per call; live on deepseek-v4-flash (pack `best-worst-2026-08-22`) ρ vs pairwise median 0.25, test–retest 0.46–0.85 — the worst pick is a weak, last-slot-biased signal |
| k-wise · full order of k (point) | ✓ graduated: `rerank::setwise` | `--answer order`: k(k−1)/2 ordinal pairs per call lowered into the solver; same pack: 9 calls/attribute reach the pairwise sort's own test–retest ceiling at ~¼ its dollars per item; last slot ranked last 2.2× fair share (measured). Robustness matrix 2026-08-23: holds across 3 delimiters (free parameter), entity sizes 400–8000 chars, 3 model families, 2 corpora; the r=2 flip-rate gauge is a one-sided screen (flip<0.20 ⇒ ρ≥0.64 over 38 live cells). Graduation gate stated in PROGRAM.md E6 |
| listwise · any (model returns the order of n) | ✗ by design | context limits, position bias, silent drops — documented rejection; the k-wise `order` row above is the lowered, bounded-k form |

**Best–worst scaling was the predicted highest-value missing cell; measured
2026-08-22 it lost to the full order of k at the same input cost** (13 vs 28
observations per call; PROGRAM.md E6). The prediction stands corrected by
its pack.

## 3. What each scale admits (so we don't compute nonsense)

- ordinal: medians, ranks, concordance (tau). Means of ranks are already a
  liberty. ✓ we report tau/rank metrics for ordinal paths.
- interval: differences, means, Pearson. Ratios meaningless.
- ratio (ours, in log space): products/ratios meaningful; geometric means;
  the log map makes ratio evidence additive — the entire reason the solver
  is least-squares on log-ratios. ✓
- Aggregation discipline: PMF moments (mean, var of signed log-ratio) are
  the sufficient statistics we consume; escape mass never averaged in. ✓

## 4. Efficiency: bits, calls, dollars

Theory: a noiseless binary comparison yields ≤1 bit; full order of n needs
Θ(n log n) bits ⇒ Θ(n log n) point-comparisons; top-k identification needs
Θ(n) for fixed k. A PMF answer yields the model's whole conditional prior —
up to log₂(52) ≈ 5.7 bits per call at the same price as a point.

The sharpest form of the case for graded evidence is the **zero-information
tournament argument** (nanojudge, `docs/zero-information-paradox.md`, mined
2026-09-05): run a single-elimination tournament on binary verdicts and the
win/loss *shape* — one undefeated item, one finalist, two semifinalists — has
probability 1 before a single judgement runs, so conditioning on it moves no
posterior over magnitudes. Only the *identities* filling the slots carry
information, and identities are purely ordinal: a 51/49 squeaker and a 99/1
blowout record the same win. Binary tournaments therefore yield a consistent
ordering of the top items with **every gap between them undetermined** — zero
bits about `max(strengths)`, the very quantity top-k selection and stopping
rules lean on. Graded verdicts (a ratio rung, a PMF over the answer alphabet)
break the paradox because the margin itself is data. That is this repo's bet
stated as an impossibility result for the alternative.

| Property | Status |
|---|---|
| per-call cost accounting (nanodollars, tokens) | ✓ everywhere |
| pre-run worst-case pricing | ✓ `--estimate` (per-template honest: $0.011 vs $1.19) |
| budget defaults O(n), not O(n²) | ✓ 4·n |
| planner vs baseline measured | ✓ regret benchmark; wins scarce-budget, pinned two-sided |
| **bits-per-dollar as a formal efficiency metric** | ✗ — we showed 3× separation/dollar informally; entropy-of-posterior-reduction per nanodollar is computable from what we already store |
| best–worst call efficiency | ✗ (see grid) |

## 5. Stability: the invariance group of a belief

Her question, formalized: a judgement deserves the name *belief* only if it
is a fixed point of the transformations that shouldn't matter. The group:

| Axis | Instrumented? |
|---|---|
| presentation order | ✓ counterbalance default + flip diagnostics + order residual (nats) |
| polarity (attribute ↔ its opposite) | ✓ `--two-sided`, consistency diagnostic |
| paraphrase (same attribute, other words) | ✓ `--also-by`, consistency diagnostic |
| null content (pure prior) | ✓ `cardinal calibrate` (4 models measured clean) |
| **framing spin** (persuasive preamble pushing a side) | ✓ `cardinal judge --spin`: the same pair under neutral, pro-first, and pro-second requester framings × both orders; reports susceptibility χ in nats and whether the belief survives — "if you spin it, do they still agree" made measurable |
| **temperature** | ✗ never swept — one JSD data point (t=0 logprobs vs t=1 samples, 0.128 on gpt-5.4-mini) is a hint, not a map |
| **reasoning effort / thinking params** | ✗ never swept; we only know reasoning models refuse logprobs |
| **wording of the ratio question** (times-more ↔ fraction ↔ times-less: the group inverses on (ℝ₊,×)) | ✓ `judge --wordings` + live study: frontier models keep the SIGN (inversion works) but the fraction wording elicits +0.35…+0.92 nats larger magnitudes — human ratio-bias reproduced in machines; magnitudes are wording-calibrated, not absolute |
| **nuisance edits** (whitespace, markdown, bullets, prestige halo) | ✓ bench axis 9 + live measurements (6 pairs × 4 edits, single run per model): Anthropic measured near-blind (0.08–0.09 nats), prestige suffix moved gpt-5.4-mini 0.878 nats — headline-sized effects, headline-unworthy denominators until replicated |
| judge model | ◐ seriate probe compares models; no standing cross-model study in cardinal runs |
| time (same judge, days apart) | ✗ |

The stability axes we cover, we cover with evidence; the remaining ✗ rows
(parameter sweeps, time drift) are the cheapest untouched science in the repo.

The invariance group is now also an INSTRUMENT: `cardinal bench` (the Judge
Coherence Benchmark, docs/BENCHMARK.md) runs order swap, reciprocal
antisymmetry, frustration, spin, polarity, paraphrase, null calibration,
and nuisance perturbation as a standardized 194-call battery per model,
× a signal axis, → one leaderboard number labs can hill-climb without
ground-truth labels. The benchmark validates itself against six scripted
pathological judges in `tests/judge_bench.rs`.

## 5½. The physics of a judge (measurements, not metaphors)

Prior elicitation behaves like a physical system, and each analogy lands on
a computable diagnostic:

| Physics | Judgement meaning | Diagnostic |
|---|---|---|
| **Frustration** (spin glass) | cyclic preference structure no scores can satisfy (A>B>C>A) | ✓ `judgement_frustration_mean` (total) AND the full Hodge split (`SolveSummary.hodge`): triangle-auditable local curl vs harmonic cycles invisible to every triad audit; Pythagoras invariant local+harmonic ≈ hcr pinned end-to-end; harmonic_dim reported like a denominator (the JCB graph's is 0 by construction — pinned) |
| **Hysteresis** | path dependence: judging A→B vs B→A | ✓ order-residual in nats + flip counts |
| **Susceptibility** | response to a small applied field: framing spin | ✓ `judge --spin` (secant) and `judge --sweep` (response function over f ∈ −3…+3: odd slope, linear R², even component) |
| **Temperature/entropy** | PMF spread per judgement; annealing across sampling temperature | ◐ entropy computable from stored PMFs; sweep unmeasured |
| **Ground state** | the solved scores: minimum-energy potential for the field | ✓ the solver itself |
| **Relaxation** | drift of the same judgement re-asked over time | ✗ |

**Finding from shipping susceptibility** (2026-07-05, live,
`research/artifacts/live/spin-probe-2026-07-05/`): χ is state-dependent, not a model
constant. gpt-5.4-mini on a clear pair: χ = −0.18 nats (mild reactance,
belief survives). The same model on a contested pair: zero-field belief
EXACTLY 0.000 and χ = +0.64 — a **paramagnetic judgement**, moving with
whoever asks. gemini-2.5-flash holds a direction on that same pair and
yields only +0.12. The sycophancy question decomposes into: does this
judgement have a spontaneous direction, and how much field moves it?

The secant critique (adversarial self-review) is PARTIALLY closed as
instrumentation: `judge --sweep` measures the response function m(f) over
field f ∈ −3…+3 and reports its decomposition — odd slope χ (linear
susceptibility), linear R², and the even component mean
(m(f)+m(−f))/2 − m(0), which captures dependence on field MAGNITUDE that
no odd/linear model can represent. Caveats that stand (red team
2026-07-09): the field is an ordinal wording ladder, so slope and R² are
not reparameterization-invariant; and ±f quote different item excerpts,
so the even channel confounds |f|-response with framing-side asymmetry —
the sweep closed the secant's shape-blindness, not its wording problems.
Live sweeps (`research/artifacts/live/spin-sweep-2026-07-05/`, n = 1 pair per
model — instrument demonstrations, not model properties, corrected by
the 2026-07-09 erratum in that pack): gpt-5.4-mini odd slope +0.200/step
(R² 0.81, below the instrument's own R² > 0.9 linearity pin; sign not
preserved through zero) with even mean +0.239 — comparable to the odd
slope, not "near-zero"; claude-sonnet-4.6 odd slope −0.014 with R² 0.02
and even mean +0.035, sign-indefinite across f (−0.148, −0.056, +0.310)
— the earlier "response is in the even part" read rested on the f = 3
edge value alone. Distinguishable response shapes, all pinned by
scripted judges where marked: flat, linear-odd, step (R² < 0.9, pinned).

**Finding from shipping frustration** (2026-07-05): a directionally
transitive judge still shows ~0.13 curl — quantization frustration. First
hypothesis blamed the ladder's non-constant log step; the controlled test
(`tests/ladder_curl.rs`, same planted-transitive judge, full ladder)
REFUTED the emphasis: repo ladder floor 0.00198 vs constant-log-step
0.00155 — the uneven step is real but third-order. The ~0.13 floor comes
from **rung usage coarseness**: a judge that expresses everything as two
rungs (1.5-or-3.9) injects two orders of magnitude more curl than the
ladder geometry does. Design consequence: to lower quantization curl,
elicit finer *distinctions* (PMF instruments already do), not finer rungs.
Corrected 2026-07-05, same day — evidence over vibes.

## 5⅝. The orbit transform: the invariance table is a character table

The unification the probes were shadows of: elicit one judgment under
the full group G = Z₂³ (order swap × polarity negation × wording
inversion), pull each answer back through the generator's known
equivariance, and decompose the resulting function m: G → ℝ by the
characters of the group (`judge --orbit`, 8 calls). Then:

- **belief := m̂(∅)**, the orbit mean — the unique G-invariant projection;
- every other Fourier coefficient is a **named, mutually orthogonal
  bias**; the marginal probes (counterbalance, `--two-sided`,
  `--wordings`) are restrictions of this transform to subgroups;
- **Parseval is the energy budget**: mean-square judgment = belief² +
  Σ bias² exactly (residual 0 in live runs) — so
  coherence = belief²/mean-square is not a heuristic composite but a
  projection ratio;
- the **interaction coefficients** (|S| ≥ 2) are invisible to every
  one-axis probe. Live study (`research/artifacts/live/orbit-2026-07-05/`,
  n = 1 pair): gpt-5.4-mini's largest bias component is
  order·polarity (−0.552 nats, 22.3% of energy — its slot preference
  reverses under negation); claude-sonnet-4.6 is 98.7% G-invariant.

Algebra pinned by scripted judges, including a machine-forced
correction: "position bias" is TWO characters — always-favor-slot-A is
order·polarity, always-name-token-A is the triple — indistinguishable to
counterbalancing, separated exactly by the transform (each pinned at
coefficient ln 2 with all others vanishing).

Growth path: adjoin generators as they earn instrumentation — paraphrase
(S_k), framing field (the continuous factor already swept in §5½),
format perturbations — and the diagnostic set extends by character theory
instead of by ad-hoc probe design.

## 5¾. Respecting the group: from probes to estimator

The invariance table above is diagnostic — it measures violations. The
symmetry discipline is respected one level deeper when the violations feed
back into
how beliefs are COMPUTED, the way experimental physics treats symmetry:

1. **Systematics are modeled, not just flagged.** Wordings are instrument
   channels with unknown gains; `gain_calibration::solve_with_template_gains`
   fits per-template gains jointly with the scores (bilinear alternation,
   reference channel pinned — the same gauge move as the additive score
   constant). Live study (`research/artifacts/live/wording-gains-2026-07-05/`):
   gains are per-model constants (sonnet fraction 1.43×, mini fraction
   0.56×), sonnet's less-channel is calibrated to 1.009, and the residual
   payoff column honestly reports where the linear gain model itself runs
   out (gemini: 3% — its wording disagreement is not a pure gain).
   Guarantees: no phantom gains on uniform data; a sign-incoherent channel
   collapses rather than calibrates.
2. **Estimators are group-averaged by construction.** Counterbalancing
   already averages the order orbit; the order residual is now measured on
   EVERY instrument (point answers included, not only PMF means), so each
   run carries its own violation magnitude in nats.
3. **Uncertainty is quoted experimentalist-style**: `cardinal sort` prints
   an error budget — `stat ± (posterior)` · `syst order (nats/pair)` ·
   `syst cyclic (% of energy)` — statistical and systematic components
   side by side in their native units, never silently pooled. A belief
   whose systematic terms dwarf its statistical term is not better
   measured by more sampling; it is telling you which transformation to
   fix.

## 6. Parameters, honestly

Current posture: temperature pinned 0.0 for evidence calls, 1.0 for
agreement samples; `top_logprobs` 20 (providers silently cap at 5);
max_tokens 16 (provider floor). **Nothing has been swept.** The open
questions with direct experiments waiting: does temperature move the PMF or
only the sample? (provider-dependent logit scaling — measurable via JSD
curves); does reasoning effort change judgement stability on models that
allow it in sampled mode? Both are afternoon-sized experiments on the
existing probe machinery.

## 7. AHP: attributes are entities too

The Analytic Hierarchy Process is already expressible in our primitives:
attributes compared pairwise ("how many times more important is *clarity*
than *depth* **for goal G**?") are just entities whose bodies are attribute
descriptions, judged under a meta-attribute ("importance for G"). The
solver's log-latents ARE the log priority vector; softmax of latents gives
normalized ratio-scale weights — the least-squares analog of Saaty's
eigenvector. `cardinal weigh` ships this (see §9): goal in, attribute
weights out, consumable directly by multi-attribute rerank. `weigh
--propose N` automates the decomposition too: the model proposes the
considerations, the weighing measures them — AHP end to end from a single
goal sentence.

The network generalization also ships: `cardinal anp` builds the full
ANP supermatrix — goal → criteria (weigh), criteria → criteria (measured
inner dependence: pairwise contribution-to-each-other), criteria →
alternatives (per-criterion multi-rerank), alternatives → criteria
(feedback via softmaxed per-criterion z-scores, the gauge-free
cross-criterion quantity) — and takes the Cesàro limit of the influence
walk from the goal (windowed averaging; robust to the bipartite
periodicity that plain powers cannot handle, unit-pinned against known
stationary distributions). The diagnostic is the network correction:
limiting minus direct criteria weights, in probability mass — how much
the interdependence structure moves the answer away from the hierarchy.

## 8. The canonical-attribute loop (the "ultimate loop")

Goal: discover the small set of attributes — the latents — that actually
span a multi-objective space. All pieces exist; the loop is composition:

1. **Propose** candidates (`explain --propose`, elaborate) — over-generate.
2. **Measure** all candidates on a probe entity set (multi-rerank).
3. **Correlate** (`attribute_correlations`, shipped): near-duplicates
   (|ρ| high) merge; keep the better-worded one (probe consistency decides).
4. **Complement**: ask for an attribute that *distinguishes items the
   current basis ties* — the residual direction, elicited in words.
5. Repeat until new candidates stop adding rank information (correlation
   with span ≈ 1) — the surviving set is the canonical basis; AHP-weight it
   against the goal (§7) and the multi-objective problem is mapped:
   Pareto front + named latents + weights.

Status: steps 1–3 shipped with evidence; step 4–5's merge operation now
ships as `cardinal canonize` — candidate wordings measured across
MULTIPLE judge models and ranked by **transmissibility** (mean cross-judge
Spearman of the induced latents), with per-judge signal and redundancy
against the accepted set. The criterion is the communication-primitive
property itself: an attribute is canonical exactly when it induces the
same cardinal latent in different minds. This loop is the road from
"sort my list" to "map my space" — and custom feeds are exactly a mapped
space plus a standing AHP weighting.

The focal-item inverse of this loop ships as `cardinal distinguish`
(`differentiation_profile` in the library): given a set and one item,
propose the attributes under which the item plausibly stands out, then
MEASURE all of them over the whole set and report the item's percentile
and z-score per attribute, best direction first. That is the propagation
question made judgeable — "under which attribute does this deserve to
travel far?" — answered by the counterbalanced pairwise machinery, never
by the proposer's say-so.

## 9. The judgement ledger, counted

Are we storing the judgements? Yes — in-repo, replayable. As of
2026-08-07, `research/artifacts/live/` holds **35 dated study packs** (through the
public-board additions and the claude-code rail comparison). The counted
2026-07-05 snapshot, preserved for scale reference:

- **~900 live provider judgements** across the first 9 study packs:
  1,483 trace/cache JSONL lines (sort demos 32+120,
  taste tools 164, evidence path 110, policy benchmark 459 across three
  policies, smoke 2) plus 838 per-call raw evidence files
  (request/response/parsed/usage) in the method-comparison packs.
- Synthetic/eval records under `research/artifacts/eval/` (regenerable
  deterministically; stored as summaries by design).
- Test-time judgements: thousands per `cargo test` run across hundreds of tests —
  simulated, seeded, regenerated rather than stored, deliberately.
- Gap found while counting: the evidence-path pack had unexported caches
  (sqlite gitignored); repaired in the same commit as this document.
