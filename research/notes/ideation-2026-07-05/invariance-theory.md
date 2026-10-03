# Permutation-respectedness at full depth

## 0. What the question actually is

The repo already treats a judgement as a function

```
J : (attribute, entity_A, entity_B, presentation, context) -> distribution over answers
```

and instruments one transformation group acting on the arguments: the dihedral
swap of `(entity_A, entity_B, presentation)` (order-swap, counterbalance). The
team-lead question generalizes correctly: **the full symmetry group is every
transformation of the five arguments that a rational judge should either (a)
be a fixed point of (invariance) or (b) transform through in lockstep
(equivariance), plus a third, under-discussed case (c): transformations the
judge *should* respond to, where the received quantity is not "is it
invariant" but "does it respond by the right amount, in the right functional
form."**

That third case is where most of the genuinely new material lives. Presentation-order
swap is a pure invariance (the correct answer literally does not depend on
which slot is A). Framing spin is deliberately NOT an invariance test — spin
should move the answer a *little* (real information from a real epistemic
peer moves belief) and a coherent judge is one whose response is bounded and
signed correctly, not zero. The repo's own doc gets this half right already
(`belief_survives_spin` = direction is a fixed point) but doesn't yet ask "is
the *magnitude* of the response the right shape" — see §4.

So: three flavors, not one.

1. **Strict invariance** — the transformed input is semantically identical;
   any measured drift is pure instrument error. (presentation order, entity
   ID/name swap holding content fixed, attribute ID/label swap holding prompt
   fixed, tokenization/whitespace, translation-round-trip.)
2. **Exact equivariance under a known group action** — the transformed input
   *is* semantically different, but by a group element whose action on the
   correct answer is derivable in closed form; violation means the model
   doesn't understand the group structure of its own answer space.
   (polarity/negation, ratio-vs-fraction wording, chain composition through
   a third entity.)
3. **Bounded, signed, structured response** — the transformed input carries
   genuine new information (a persuasive framing, an added alternative in the
   choice set) and a coherent judge's response has a *predicted shape*
   (monotone in the manipulation's strength, saturating or linear, bounded)
   that can be falsified. (susceptibility properly measured, IIA/context-set
   effects, attribute-importance stated-vs-revealed consistency.)

The repo's current grid (`research/notes/FIRST_PRINCIPLES.md` §5) has excellent
coverage of category 1 axes and one instrumented category-3 axis (spin), and
zero coverage of category 2. Category 2 is the cheapest gap to close because
it reuses 100% of existing plumbing — no new entities, no new probe
machinery, just new prompt templates and an arithmetic check.

---

## 1. The transformations, one by one

For each: the group element, the invariance/equivariance statement, the
cheapest receipt, and the analogy — stated only where it is a real
correspondence, flagged as absent otherwise.

### 1.1 Presentation-order swap — *(already shipped, baseline for comparison)*

- **Transformation**: swap which entity occupies slot A vs slot B.
- **Statement**: strict invariance of the belief; the *elicitation call* is
  covariant (answer flips sign) but the *belief* it estimates must not move.
- **Receipt**: shipped — flip-rate + order-residual in nats
  (`src/rerank/multi.rs`, counterbalance_dirs/position_flips).
- **Analogy**: none needed, it's definitional — Z₂ on a labeling with no
  semantic content.

### 1.2 Entity relabeling / anonymization — *(new, high leverage)*

- **Transformation**: attach a label, name, or provenance tag to entity text
  that carries **no content information about the attribute being judged**
  (e.g. append "— written by GPT-4" / "— submitted by Ahmed" / "— Entity Q7"
  to otherwise byte-identical text).
- **Statement**: strict invariance — the judged log-ratio must not move,
  because the attribute prompt is about the text, not its author.
- **Receipt**: cheapest in the whole list. Take N existing entity pairs from
  any receipt pack, wrap `entity.text` with a fixed-length attribution
  suffix drawn from a battery (neutral / prestige-coded / demographic-coded
  names), leave `attribute.prompt` untouched, rerun through the existing
  `compare_pair` path, diff log-ratio against the unlabeled baseline. Zero
  new solver code — it's a data-augmentation harness around
  `PairwiseComparisonEntity.text`.
- **Analogy**: **not physics** — this is the audit-study design from field
  experiments on discrimination (Bertrand & Mullainathan's resume-name
  swap): hold substance fixed, vary only the label, measure outcome drift.
  Import the methodology, not a metaphor.

### 1.3 Attribute relabeling — *(new, same mechanism as 1.2, different axis)*

- **Transformation**: the prompt template renders `{attribute_name}`
  *separately* from `{full_attribute_text}` (`src/rerank/comparison.rs:763`,
  `estimate_pairwise_input_tokens` and its live counterpart both take
  `attribute_name` and `attribute_prompt` as distinct render arguments). This
  means the caller's chosen short label for an attribute — today typically
  `attribute.id`, e.g. `"quality"`, `"excellence"`, `"depth"` — is shown to
  the judge as a literal tag *in addition to* the full descriptive prompt.
- **Statement**: strict invariance — if the full attribute text fully
  specifies the criterion, the short label is decoration and must not move
  the answer. If it *does* move the answer, the harness has an unacknowledged
  leakage channel: multi-attribute rerank users who pick evocative attribute
  IDs (`"brilliance"` vs `"attr_7"`) for the *same* underlying prompt are
  silently injecting a halo effect.
- **Receipt**: fix `full_attribute_text` constant, sweep `attribute_name`
  across a battery of loaded vs. neutral labels, measure drift. As cheap as
  1.2, and it's checking a code path (`attribute_name` vs `attribute_prompt`
  split) that the repo clearly built deliberately but has never used as an
  invariance target.
- **Analogy**: same audit-study framing as 1.2; this is the attribute-side
  twin of the entity-side test.

### 1.4 Scale/unit covariance ("times more" vs "fraction less") — *(new, category 2)*

- **Transformation**: hold entities, slots, and order **fixed**; vary only
  the grammatical form of the ratio question: (i) "how many times more X
  does A have than B" (current `canonical_v2`), (ii) "what fraction of A's X
  does B have" (should answer `1/r`), (iii) "by what percent does A exceed B
  in X" (should answer `r - 1`).
- **Statement**: exact equivariance — the three answers, run through the
  inverse of their respective response transforms, must recover the *same*
  log-ratio. This is a homomorphism check: does the model's ratio judgement
  respect the group structure of (ℝ₊, ×), or does wording act as an
  independent nuisance parameter on top of genuine magnitude perception?
- **Receipt**: three new `PromptTemplate` constants in `src/prompts.rs`
  (no new entities, no swap machinery — the cheapest category-2 probe to
  ship). Compare implied log-ratio across templates on the same pair/order.
- **Analogy**: **real, from psychophysics/numerical cognition, not physics**
  — "ratio bias" / denominator-neglect literature documents that humans
  reliably judge "3× as much" and "1/3 as much" as different-magnitude
  claims even though they're mathematically identical. Directly testable
  whether LLM judges inherit this bias.

### 1.5 Conjugation: negated comparative + swapped answer — *(new, and: unifies with 1.4)*

- **Transformation**: ask "which has **less** X" instead of "which has
  **more** X," same attribute wording, same slots. This is different from
  the shipped `--two-sided` polarity probe, which substitutes a
  *semantically opposite attribute* (e.g. clarity ↔ confusingness) — a
  different concept, not a logical negation, and not necessarily a true
  antonym (confusing ≠ exactly not-clear).
- **Statement**: exact equivariance under group inverse — `higher_ranked`
  must flip and `ratio` must be unchanged (pure logical inverse, not a
  different concept with its own anchor).
- **Adversarial note**: 1.4 and 1.5 are **the same underlying invariance**
  (group-inverse consistency in (ℝ₊, ×)) surfaced through two different
  natural-language mechanisms — one rewords the *question type*, one
  rewords the *comparative direction*. Worth building both because they can
  fail independently: a model might correctly invert "more→less" (modus
  tollens on its own scale) while still showing ratio-bias on
  multiple-vs-fraction framing, or vice versa. Treating `--two-sided`'s
  antonym-swap and this pure-negation swap as the same probe (as the current
  framing risks) would hide *which* failure mode is present when
  `--two-sided` shows inconsistency: antonym-imperfection or logic-failure.
- **Receipt**: one new ordinal/ratio template with "less" substituted for
  "more"; check ratio and direction are exact inverses of the baseline call
  on the identical unswapped presentation.
- **Analogy**: group-theoretic inverse-element check; no physics needed.

### 1.6 Attribute–entity duality: stated vs. revealed importance — *(new, uses AHP plumbing already built)*

- **Transformation**: transpose the judgement matrix. The repo already
  supports comparing *attributes* pairwise as entities under a meta-attribute
  ("importance for goal G") via `cardinal weigh` (`FIRST_PRINCIPLES.md` §7).
  That produces **stated** importance weights. Separately, elicit a single
  **holistic** pairwise judgement ("overall, for goal G, is A or B better")
  on a probe entity pair, and also measure each entity's per-attribute scores
  directly. That produces **revealed** weights (regress the holistic
  log-ratio against the per-attribute score gaps).
- **Statement**: not strict invariance — a *consistency* equivariance: the
  AHP-stated weight vector, applied to the per-attribute score gaps, should
  predict the sign and rough magnitude of the independently-elicited holistic
  judgement. This is the one place the repo can check whether its own
  decomposition (canonical-attribute loop, §8 of FIRST_PRINCIPLES) is
  *load-bearing* — whether attributes-and-weights actually reconstitute the
  judgement they were decomposed from, or are a plausible-sounding fiction.
- **Receipt**: run `weigh` for goal G over the existing attribute set, take
  its weights, take per-attribute scores already computed by a multi-rerank
  run, predict holistic log-ratio for a held-out pair, elicit the real
  holistic judgement, compare. All three calls already exist in the CLI;
  this is a comparison script, not new instrumentation.
- **Analogy**: **real, from economics** — stated-preference vs.
  revealed-preference consistency (conjoint analysis / discrete choice
  literature). Not physics.

### 1.7 Context-set invariance / IIA — *(new, high leverage, closes two gaps at once)*

- **Transformation**: judge A vs. B directly (baseline). Separately, judge
  {A, B, C} as a 3-way set (best-worst or k-wise) for some third alternative
  C — in particular a C designed to be *dominated* by both A and B (a decoy)
  — and extract the **implied** A-vs-B preference from the k-wise answer.
- **Statement**: Independence of Irrelevant Alternatives — the implied A-vs-B
  direction and magnitude from the k-wise call must match the direct pairwise
  call, regardless of C's presence or identity, *when C is genuinely
  irrelevant* (strictly dominated). Predictable, well-documented violation
  mode if it fails: the attraction/decoy effect from consumer-choice research
  (Huber, Payne & Puto 1982) — adding a dominated decoy shifts preference
  between the two real options even though the decoy is never chosen.
- **Receipt design note**: this probe is currently unbuildable cheaply
  because **best–worst / k-wise scaling is the highest-value missing
  instrument in the repo** (`FIRST_PRINCIPLES.md` §2, explicitly flagged
  ✗ and "highest-value missing cell"). Building best-worst *for this probe*
  gets the instrument and the IIA receipt in one motion — this is the
  strongest leverage item in the whole list because it pays for a
  pre-existing backlog item and a new invariance receipt simultaneously.
- **Analogy**: **real, from social choice / decision theory** (Arrow's IIA,
  Luce's Choice Axiom) — the correct citation is choice theory, not physics;
  resist the urge to call this "gauge invariance under adding background
  charge," it isn't.

### 1.8 Composition / chaining beyond triangle curl — *(sharpens an existing receipt)*

- **Transformation**: chain a judgement through an intermediate not used in
  the direct call — either an entity already in the set (the existing HCR
  machinery, aggregated) or an **external anchor** entity (from the
  `calibrate`/null-pair battery) not otherwise part of the ranked set.
- **What's missing today**: `compute_hcr`
  (`src/rating_engine.rs:832`) reports one *global scalar* —
  `Σλr² / Σλμ²` over the whole graph, after Huber down-weighting. The doc
  correctly calls this "curl + harmonic component" together (Hodge language),
  but the repo doesn't separate the two, and conflating them risks
  misdiagnosis: a high HCR can mean either (a) the judge is genuinely
  frustrated (real curl, cyclic disagreement in triads), or (b) the
  comparison *topology* the planner sampled is too sparse/tree-like to pin
  down the harmonic subspace (structural, not judgemental). These have
  different fixes — (a) needs a better judge or Huber tuning, (b) needs a
  denser planner.
- **Cheap disambiguating receipt**: report **cycle rank** of the sampled
  comparison graph (`|E| − |V| + |components|`, trivial arithmetic already
  available where `edges`/vertex count are in scope) alongside HCR, and
  separately report the **max single-triad residual** (not just the
  weighted mean) — a small number of severely frustrated triads hiding
  inside a low mean tells a different story than uniformly-distributed mild
  frustration. Also worth: chain through an entity NOT in the graph (a fixed
  calibration anchor) and check whether the two-hop implied ratio A→anchor→B
  matches the direct A→B ratio — this checks something the "gauge pinning"
  language currently elides (see §3 below).
- **Analogy**: genuine — Hodge decomposition (gradient + curl + harmonic) is
  exactly the right math here; the repo already invokes it correctly. The
  addition is operational: *measure* the harmonic/topology confound the
  decomposition predicts exists, don't just name it.

### 1.9 Language translation invariance — *(real, but lower priority)*

- **Transformation**: translate entity text and/or attribute prompt into
  another language via a fixed MT step, rerun the identical comparison.
- **Statement**: strict invariance of the belief (not of the elicited text).
- **Why it ranks lower**: translation quality is a confound entangled with
  the effect being measured — a drift could be genuine model
  translation-sensitivity or an MT artifact, and separating them needs a
  second judge or back-translation check, which roughly doubles the probe's
  cost for a noisier signal than most of the above.
- **Analogy**: none needed; straightforward invariance claim.

### 1.10 Tokenization / markdown perturbation — *(new, high concreteness, real-world relevant)*

- **Transformation**: apply a meaning-preserving reformatting to one side's
  text only — collapse/expand newlines, add markdown bold/headers, convert
  paragraph to bullet list, add trailing whitespace.
- **Statement**: strict invariance if the attribute is about substance; a
  documented failure mode if it isn't — LLM-as-judge literature has repeated
  findings of **format/verbosity bias** (judges rewarding markdown structure,
  bolding, bullet lists, and length independent of content quality; see the
  motivation behind length-controlled AlpacaEval). This matters concretely
  for cardinal-harness because its actual entities are very often *other
  LLMs' outputs*, which vary in formatting for reasons that have nothing to
  do with the attribute being judged.
- **Receipt**: take existing entity pairs, apply a formatting-only transform
  to one side, rerun, compare log-ratio drift against a content-preserving
  control. Very cheap — no new entities, no new templates, just a text
  transform function.
- **Analogy**: not physics — this is the standard non-semantic-perturbation
  robustness test from adversarial ML (closest correct analogy: certified
  robustness to non-semantic input transforms), and it has direct prior art
  in the LLM-judge-bias literature.

### 1.11 Time-of-conversation position — *(mostly N/A by design; one real sub-case)*

- If every comparison call is an independent, stateless chat completion
  (which the gateway/cache design strongly implies — each call is keyed by
  attribute+entities+template, not by a conversation ID), then "position
  within a conversation" doesn't apply: there is no shared conversation. This
  collapses to a non-finding, and it's worth stating that plainly rather than
  inventing an effect.
- **The one real sub-case**: if the "sample-set" output form (repeat draws at
  temperature 1, mentioned as ◐ in the instrument grid) is ever implemented
  as multiple turns in one conversation rather than independent calls,
  *order within that batch* becomes a real serial-position effect (primacy/
  recency), and it would silently violate the assumption the aggregation
  math needs: that repeated samples are **exchangeable**.
- **Analogy — the one place de Finetti actually belongs in this whole
  investigation**: the sample-set instrument's statistical validity rests on
  the drawn samples having an exchangeable joint distribution (permuting
  which draw is "first" must not change the joint law) — de Finetti's
  theorem is what licenses treating repeated samples as draws from a fixed
  posterior at all. If sample-set calls ever share context, exchangeability
  is the exact thing that breaks, and the correct receipt is a permutation
  test on sample order, not a vibes check. Currently moot because sample-set
  output form is unshipped (◐), but flag it as a design constraint *for
  whoever ships it*: sample-set calls must be architecturally independent
  (fresh context per draw) or exchangeability, and therefore the whole
  aggregation, is unlicensed.
- **Verdict**: mostly ✗-and-honestly-so; one precise constraint worth writing
  down before, not after, sample-set ships.

---

## 2. Table view (leverage × concreteness)

| # | Transformation | Leverage | Concreteness | Buildable this week? |
|---|---|---|---|---|
| 1.7 | Context-set / IIA via best-worst | Very high (closes backlog gap + new receipt) | Medium (needs k-wise call plumbed) | Partial — needs best-worst first |
| 1.10 | Tokenization/markdown perturbation | High (production-relevant bias) | Very high (pure text transform) | Yes |
| 1.2 | Entity relabeling/anonymization | High (leakage channel, real halo risk) | Very high (text transform) | Yes |
| 1.3 | Attribute relabeling | Medium-high (same mechanism, less obviously exploited today) | Very high | Yes |
| 1.4/1.5 | Scale covariance + conjugation (same invariance, two surfacings) | Medium-high (diagnoses `--two-sided` conflation) | Very high (new templates only) | Yes |
| 1.8 | Curl/harmonic separation + max-triad residual | Medium (sharpens existing receipt's interpretation) | High (arithmetic on data already computed) | Yes |
| 1.6 | Stated vs. revealed attribute weight | Medium (validates the "ultimate loop" is load-bearing) | Medium (composes 3 existing CLI calls) | Yes, if scoped to one goal/pair |
| 1.9 | Translation invariance | Low-medium | Low (MT-quality confound) | No |
| 1.11 | Conversation position / exchangeability | Low now, becomes high the moment sample-set ships | High as a design constraint, N/A as a probe today | N/A — write the constraint, don't build the probe |
| 1.1 | Presentation-order swap | (baseline, already shipped) | — | done |

---

## 3. Where the repo's own framing is subtly wrong or shallow

**(a) "Gauge pinning" is claimed as solving anchor dependence, but it solves
a different problem than the one it's credited for.** `research/notes/COMPARISON.md`
lists "Baseline / anchor dependence" as "implemented" via "full comparison
graph, global fit, gauge pinning." That's true for a specific, narrower
claim: the *solver* has a free additive constant in log-score space (any
constant shift leaves all residuals unchanged — this is genuinely gauge
freedom, and Noether's theorem is the correct, non-decorative analogy: the
shift-invariance of the loss function is the symmetry, the invariant
ranking/order is the conserved quantity), and the solver removes that
freedom by convention (pinning one entity or the mean to zero). But that is
a statement about the *fit*, not about the *judge*. It says nothing about
whether the judge's **raw elicited pairwise ratios** — before any fitting —
are anchor-dependent, e.g. whether A-vs-B judged directly differs from
A-vs-B implied by chaining through a specific busy anchor D. Least squares
will always find *some* internally-consistent gauge; that's definitional,
not evidence the underlying judgements were anchor-free. §1.8's anchor-chain
receipt is the actual test of the claim currently being made by citation
alone.

**(b) Is the ratio ladder's non-log-additivity a bug in the ladder, or in
calling the result "quantization curl"? — It's a bug in the ladder,
correctly named.** The ladder (`src/prompts.rs:152-154`) is
`[1.0, 1.05, 1.1, 1.2, 1.3, 1.5, 1.75, 2.1, 2.5, 3.1, 3.9, 5.1, 6.8, 9.2,
12.7, 18.0, 26.0]`. Computing successive log-steps: 0.049, 0.047, 0.087,
0.080, 0.143, 0.154, 0.182, 0.174, 0.215, 0.229, 0.269, 0.288, 0.302, 0.322,
0.348, 0.368 — **monotonically increasing**, not constant. The doc comment
calls this "approximately geometric," which undersells the effect: a truly
constant-log-step ladder makes the index arithmetic exact — `ln(r_i) +
ln(r_j) = ln(r_{i+j})` whenever `i+j` is in range — so for a genuinely
transitive judge whose true log-ratios land near rungs, a chained
prediction and a direct judgement would hit the *same* rung, and
ladder-induced curl would provably vanish (leaving HCR measuring only
genuine structural/judge cyclicity). This repo's ladder does not have that
property because its steps compound unevenly (finer near 1.0, coarser near
26.0 — a deliberate design choice, described as letting the model "express
near-ties," but never checked against its cost to the curl statistic). So:
"quantization curl" is the right *name* for the effect, but the doc's
framing ("no one told us this") undersells that it's a specific, fixable
property of *this* ladder's non-constant step, not an unavoidable fact
about discretization in general. Concrete test: swap in a pure-geometric
ladder (`r_k = 26^(k/16)`, same range, same count, constant step ≈0.204) on
a planted-transitive judge fixture and check whether the measured HCR floor
actually drops. If it doesn't, the "quantization curl" framing was wrong in
a different way — worth knowing either way.

**(c) Susceptibility (χ) is not measured as a susceptibility.** In physics,
susceptibility is a *derivative at (near-)zero field* — the linear-response
slope. The shipped formula (`src/rerank/spin.rs:203-206`) is
`χ = (m_pro − m_con) / 2`: a **secant between two fixed, opposite-direction,
single-intensity treatments**, not a measured slope against a swept field
strength. Two specific problems this creates, worth naming precisely:

1. **No field-strength sweep exists.** There is exactly one persuasive-
   preamble intensity (`spun_criterion`'s fixed wording, `src/rerank/spin.rs:77-86`).
   Whether the response is linear, saturating, or a step-function
   ("ignores mild framing, folds completely past some threshold" — a
   coercion threshold, not a susceptibility) is unknown and unknowable
   from the current instrument. This is exactly the distinction the
   team-lead's prompt asked to check for, and the answer is: not measured.
2. **The formula discards the zero-field reading.** `mean_log_ratio` under
   `SpinFraming::Neutral` is measured and stored but never enters the
   `susceptibility_nats` computation — only `pro` and `con` do. This
   silently assumes the neutral belief sits exactly at the midpoint of the
   pro/con readings (i.e., that the two spun framings are symmetric,
   equal-and-opposite perturbations around the true zero-field point). For
   a belief already near the pinned/certain end of its range, that
   symmetry assumption need not hold, and the secant would then blend real
   susceptibility with an artifact of the framing pair's asymmetry around
   wherever the neutral belief actually sits.

Concrete fix for both: replace the fixed-intensity two-point secant with a
3–5-point intensity ladder (mild → strong preamble, both directions, plus
the existing neutral point as the true zero), fit log-ratio against an
explicit intensity scalar, and report the fitted slope *and* an R² /
linearity residual. A judge that's flagged "belief_survives_spin" today
under the two-point test could still be revealed as a step-function
sycophant once intensity is swept — the current binary verdict cannot tell
those apart from a genuinely low-and-linear responder.

---

## 4. Top 3 buildable this week

### 1. Formatting/anonymization perturbation battery (§1.2 + §1.3 + §1.10, unified)

These three are mechanically identical (wrap `entity.text` or vary
`attribute.id`/name with something semantically null, hold everything else
byte-identical, diff log-ratio against baseline) — build one harness that
takes a battery of perturbation functions and applies each to an existing
receipt pack's entity/attribute set.

- **Inputs**: any existing pack under `artifacts/live/` (e.g.
  `method-comparison-2026-06-30`) — no new entities needed, reuse what's
  already been judged once as the baseline.
- **Perturbations to ship first**: (a) attribution suffix battery (neutral /
  prestige / demographic-coded names) on entity text; (b) attribute-id
  swap battery (loaded vs. neutral labels) holding `attribute.prompt` fixed;
  (c) formatting battery (markdown bold, bullet-vs-paragraph, whitespace
  collapse) on entity text.
- **Metric**: signed log-ratio drift per perturbation vs. baseline, and a
  refusal-rate delta.
- **Pass/fail bar**: any perturbation whose median |drift| exceeds, say, the
  measured order-residual noise floor (already computed elsewhere in the
  repo — use it as the null-effect calibration rather than picking an
  arbitrary threshold) is a finding, not noise.
- **New code needed**: a text-transform module and a small runner script —
  zero changes to `compare_pair`, `multi.rs`, or the solver.

### 2. Ratio-wording homomorphism check (§1.4 + §1.5, unified — the group-inverse probe)

- **New code**: two new `PromptTemplate` constants in `src/prompts.rs` —
  fraction-framing (`"what fraction of A's X does B have"`) and
  negated-comparative framing (`"which has LESS X"`), sharing the existing
  system-prompt scaffolding and JSON contract exactly.
- **Probe**: for N existing judged pairs, same slots/order, run all three
  templates (times-more / fraction-less / less-than), transform each answer
  back to a canonical signed log-ratio (`ln(r)`, `−ln(r)`, `−ln(r)`
  respectively for the three framings under exact consistency), and report
  the pairwise disagreement between the three recovered log-ratios.
- **Why this is the sharpest of the three**: it's the one probe that can
  cleanly separate two failure modes the repo currently conflates under
  `--two-sided`: (i) the model can't invert its own scale (logic failure,
  caught by the less-than framing), vs. (ii) the model shows a genuine
  numerical framing bias between multiplicative and fractional wording
  (caught by the fraction framing) — these need different fixes (prompt
  engineering for i, nothing to fix for ii except knowing it's there since
  it's a documented human bias too).
- **Deliverable**: a receipt table, one row per pair, three recovered
  log-ratios + max pairwise disagreement, same shape as the existing
  paraphrase-consistency receipt so it slots into the existing report
  format.

### 3. HCR topology disambiguation (§1.8)

- **New code**: in `src/rating_engine.rs`, alongside `compute_hcr`, add (a)
  `cycle_rank(edges, n_vertices)` = `|E| − |V| + components` (a few lines,
  standard graph arithmetic, no new dependencies), and (b) track
  `max_triad_residual` (or more generally `max_weighted_residual`) alongside
  the existing mean-based HCR — both are derivable from arrays already
  computed in `solve_weighted_least_squares`'s output (`residuals`,
  `lam_eff`) with no new judge calls at all.
- **Probe**: rerun the existing planted-transitive and planted-rock-paper-
  scissors fixtures (referenced in `FIRST_PRINCIPLES.md` §5½ as already
  built for HCR validation) and check: does cycle rank correlate with HCR
  on the *transitive* fixture (it shouldn't much — a sparse tree-like
  transitive graph should still show near-zero HCR) but predict *inflated*
  HCR when the planner is deliberately starved of redundant edges (a new,
  cheap fixture: same transitive judge, but force a near-spanning-tree
  comparison topology)? If sparse topology alone inflates HCR on a
  genuinely transitive judge, the current single-scalar HCR is measuring
  planner sparsity, not judge frustration, some fraction of the time.
- **Deliverable**: this either (a) shows HCR is already clean of topology
  confounds (good, ship the two extra numbers as a documented robustness
  receipt) or (b) reveals a real conflation the repo should fix before
  citing HCR as "the" frustration number — either outcome is a receipt worth
  having, at the cost of zero new API calls (it's pure arithmetic on
  existing solver internals plus one new synthetic fixture).

All three are gated on no new gateway/cache work, reuse the existing
`PromptTemplate`, `compare_pair`, and `rating_engine` surfaces exactly as
built, and each produces a receipt in the same shape (drift table / disagreement
table / two extra diagnostic numbers) as receipts the repo already ships —
so the reporting/plumbing cost is near zero and the finding is real either way.
