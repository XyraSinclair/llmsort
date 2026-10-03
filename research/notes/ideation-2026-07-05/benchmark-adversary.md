# Attacking the Judge Coherence Benchmark

Adversarial design review of a proposed "Judge Coherence Benchmark" (JCB):
score judge quality with **no ground truth**, purely from internal
consistency under meaning-preserving transformations. Grounded throughout in
what `cardinal-harness` already measures and how — the Hodge-curl
frustration receipt (`compute_hcr`, `src/rating_engine.rs:832`), the framing
`--spin` susceptibility probe (`src/rerank/spin.rs`), the `--two-sided` /
`--also-by` polarity and paraphrase probes, and the `cardinal calibrate`
null-pair receipt (`artifacts/live/null-calibration-2026-07-04/`) — because
every one of these mechanisms already has a documented exploit surface or an
honest negative on record, and the honest thing to do is attack the *actual*
receipts, not a hypothetical.

The framing that organizes everything below: **a coherence-only benchmark
cannot distinguish "correct judge" from "judge with a stable but wrong or
content-blind decision rule."** Every attack in this document is a variant
of that one fact. Every countermeasure is an attempt to buy back some of the
discriminating power a ground-truth benchmark gets for free, using only
structure (perturbation families, statistics, corpus design) — and every
countermeasure has a residual gap that should be stated, not hidden.

---

## (a) The Goodhart core: coherent-but-worse judges

### The central exploit

Any judge whose decision rule is a **stable, computable function of surface
features that are usually correlated with the target attribute but are not
the target attribute** will ace every coherence axis simultaneously, because
coherence axes test whether the judge's answer is a well-defined function of
content under nuisance transformations — they say nothing about *which*
function.

Concrete shortcut functions that pass everything:

- **Length**: `score(X) = len(X)`. Transitive by construction (real numbers
  are totally ordered) → zero Hodge curl. Antisymmetric by construction →
  perfect reciprocal consistency. Order-invariant if the judge reads both
  slots before answering → perfect swap agreement. Insensitive to *any*
  wrapper text, including a leaning preamble, because it never attends to
  the attribute description at all → **χ = 0, "belief survives spin"
  reported `true`** — the spin probe's own criterion for a belief
  (`src/rerank/spin.rs:212`, direction identical across all three framings)
  is satisfied trivially by not reading the framing. Reverses correctly
  under polarity ("lack of conciseness" → shorter wins) if the negation is
  simple string-prefix negation. Stable under attribute paraphrase, because
  the attribute never mattered. Ties on null pairs, because identical text
  has identical length.
- **Formality/register markers, sentiment polarity, or presence of hedge
  words** — same story, different feature.
- **Lexicographic/hash order of the raw entity string** — see (b) for why
  this one is actually *not* fully safe, which is the one clean win we have.

### Argue it's acceptable

A benchmark that explicitly disclaims ground truth cannot also claim to
detect "wrong taste" — that would require the very labels the design
forswears. Coherence is a **necessary, not sufficient**, property of a good
judge, and a benchmark that only claims necessity is honestly scoped. Labs
already have correctness-oriented evals (RewardBench, human-labeled
preference sets); JCB's job is to answer a different, real question:
*given that a judge agrees with some correctness standard on average, is its
underlying preference a stable function of content, or noise?* That
question is well-posed without labels, and is the question that predicts
whether a judge's behavior will hold up on inputs the correctness eval never
sampled.

### Argue it's not acceptable

The moment JCB becomes a leaderboard, "score well on JCB" becomes an RLHF
target, and consistency-only reward signals are *easier* to hack than
correctness reward models precisely because they have no external anchor at
all — a policy can climb a coherence metric by becoming *more* rigidly
rule-bound (more shortcut-like), which is close to the opposite of what
"better judgment" should mean. Unlike RewardBench, where gaming requires
matching a fixed human label distribution (a real, if imperfect, constraint),
gaming pure self-consistency has no floor except the transformation family
the benchmark happens to test. A marketing claim of "0.94 on Judge
Coherence" will be read by customers as "trustworthy judge," and the gap
between that reading and "judge that has learned a stable proxy" is exactly
the gap this section describes.

### The resolution that actually buys something

Neither argument fully wins; the fix is architectural, not philosophical:
**pair every coherence axis with a content-sensitivity probe that a
shortcut-follower fails and a real judge passes**, specifically:

- **Confound-controlled content swap.** Construct pairs where a cheap
  surface feature (length, register, position of the assertive claim) is
  *held fixed* while the attribute-relevant content is swapped between the
  two entities (e.g. two paragraphs of identical length, one rewritten to
  contain the stronger argument). A judge tracking the real attribute flips
  its answer when the content is swapped; a judge tracking the surface
  feature does not move. This is the one test in the whole design that
  targets mechanism rather than behavior, and it is not free — it requires
  hand-curated or LLM-generated-then-human-checked pairs per domain, which is
  exactly the corpus cost that makes JCB more expensive than it looks in
  (d).
- Treat "coherent and shortcut-following" and "coherent and idiosyncratic
  taste" as **different findings that must be reported separately**, not
  collapsed into one composite number. The confound-controlled probe
  specifically flags the former; nothing in this design, or any
  ground-truth-free design, can fully rule out the latter, and the spec at
  the end says so explicitly rather than implying otherwise.

---

## (b) Trivial exploits per dimension, and their countermeasures

| # | Dimension | Trivial exploit | Why it works | Countermeasure | Residual gap |
|---|---|---|---|---|---|
| 1 | Entity-order swap agreement | Deterministic tie-break on a hash of the two entity strings (favor whichever hashes larger, symmetric rule) | Any total order over `(hash(A), hash(B))` is automatically swap-consistent and content-blind | **Nuisance-perturbation stability**: apply meaning-preserving edits that change the raw bytes but not the meaning (added whitespace, Unicode homoglyphs, re-labeling A/B → X/Y, casing) and require the judgment be invariant. A hash-keyed judge is *not* invariant — the avalanche property of any real hash guarantees a semantically null edit changes the hash-derived answer. This is the cleanest kill available in the whole design. | A judge could hash on a normalized/whitespace-stripped string; countermeasure needs several *independent* nuisance edits, not one, and should be rotated so a specific normalization can't be special-cased. |
| 2 | Reciprocal consistency (antisymmetry) | A pure position-biased judge ("whoever is in slot A wins by a fixed ratio k") | Re-derive it: query (A-first) says "slot A wins by k"; query (B-first) says "slot A [=physically B] wins by k" → in entity-identity terms this is *exactly* reciprocal (A≻B by k, B≻A by k) even though the judge never looked at content | **This is not actually an independent axis** — it is the same measurement as (1) under a different name. `cardinal-harness`'s own counterbalancing computes both from the same two calls (order-residual = reciprocal-consistency = swap-agreement, `src/bin/cardinal.rs` order-flip receipts). If the composite formula weights them separately, a lab can double-dip: fix position bias once, get credit twice. **Merge dimensions 1 and 2 into one axis before weighting**, or explicitly justify why they are not redundant for a specific corpus design (they are not redundant only if "reciprocal" is measured as re-asking about the *same conceptual question* through an independently-worded template rather than a literal slot swap — state which one is meant). |
| 3 | Transitivity / Hodge curl | (i) Any total-order shortcut feature (see (a)) gives exactly zero curl by construction. (ii) **Abstention laundering**: refuse or report near-zero confidence specifically on the contested/near-cyclic pairs; `compute_hcr` weights every edge by post-Huber λ (`src/rating_engine.rs:832-854`), so down-weighting exactly the pairs that would create curl shrinks the numerator (Σλr²) faster than the denominator (Σλμ²), and refusals are dropped from the graph entirely | Refusing on your hardest cases inflates apparent transitivity while being the least useful behavior a judge can exhibit | Track **coverage** (answered / attempted) as a hard gate, *outside* the curl computation, and require ≥95% coverage before a curl number is even reported; refusal-heavy runs should be flagged, not silently folded into a rosy HCR. Also report curl **separately for a "clear" and a "contested" stratum** (see (d)) — near-zero curl is the right target on clear pairs; on contested pairs, some curl may be the *honest* answer (see below), so a policy that forces false transitivity there by picking an arbitrary tie-break is not obviously better. | The clear/contested split itself needs a construction method that isn't circular — see (d). |
| 4 | Framing-spin robustness | (i) A content-blind judge trivially shows χ = 0 (same shortcut problem as (a)). (ii) **Template fingerprinting**: `spin_probe`'s framing text is a fixed literal string (`src/rerank/spin.rs:82-85`, "*I've already looked at both and I'm fairly convinced...*"). A model fine-tuned against a leaked or published version of this exact wording can special-case "detect the canned persuasion-probe phrasing → answer as if neutral," scoring a perfect χ = 0 while remaining fully steerable by any *other* phrasing of the same social pressure | Draw framing wording from a **large, randomized, periodically-rotated template bank** (many tones/registers/languages for the same social move), regenerate per benchmark refresh, and hold a private subset never published at all — the standard held-out-split defense, applied specifically to the wrapper text rather than the entity content. | A large lab with API-level scraping of eval traffic can still reconstruct the template distribution over time; rotation only raises the cost of memorization, it doesn't eliminate it. |
| 5 | Polarity reversal | A judge can pattern-match the syntactic negation marker ("lack of", "absence of") and multiply its positive-framing score by −1, without modeling the negated concept at all | Passes the specific test (anti-correlation with the positive framing) via a syntactic patch that need not generalize to scoped or compound negation elsewhere | Vary polarity via genuine antonyms requiring semantic understanding (clarity ↔ confusion, terse ↔ verbose) rather than a negation prefix on the same string, and include scoped/double negation as a harder held-out probe ("not entirely lacking in X") that a syntactic-negation-flag detector fails and a real polarity-reversal understander passes. | Antonym pairs are not always exact opposites (clarity's antonym is arguably "confusion" *or* "vagueness" depending on register) — corpus curation cost, not a free lunch. |
| 6 | Paraphrase stability | Eval contamination: if the exact paraphrase wordings are published, they can be baked into future RLHF/SFT data, teaching superficial consistency between *those specific strings* without generalizing | Passing the test stops requiring the capability the test was designed to measure, once the test text itself is training data | Never publish literal paraphrase strings; generate them procedurally per run from a paraphrasing model at high diversity, and only publish the seed attributes + methodology. Same discipline as (4)'s template rotation, applied to attribute wording instead of framing wording. | The underlying capability being probed (does rewording change the answer) is not fully separable from "does the model recognize this specific wording" without an arms race of regeneration; this only raises the cost. |
| 7 | Null-pair calibration | Byte-identical text in both slots is trivially detectable by string comparison — no judgment ability required at all | `cardinal-harness`'s own receipt (`artifacts/live/null-calibration-2026-07-04/`) shows **all four tested models at exactly 0.000 bias** — the test caught nothing, because it is nearly free for any modern model to pass. Zero discriminating power among frontier judges as currently specified. | Use **near-identical, not identical**, pairs: two independent translations of the same source, or two equally-strong rephrasings of the same argument — items a byte-diff cannot shortcut, so genuine "these are equivalent" assessment is required. Add a **should-NOT-tie null-negative**: two items that are superficially similar (same length/structure) but a domain panel agrees are meaningfully different, to catch the opposite failure mode — a judge that defaults to "tie" whenever surface form looks similar. | "Near-identical but should tie" pairs need expert curation to confirm they really are equivalent in the relevant attribute — this reintroduces a sliver of ground truth at the corpus-construction step, which is fine (corpora may use ground truth to build the test; the *judge under test* still never sees it) but should be named as such, not hidden behind "no ground truth." |
| S | Signal / discrimination | (i) Always output the maximum ratio on the ladder with high confidence for every non-null pair — huge apparent spread, meaningless calibration. (ii) A judge whose *direction* correlates with content but whose *magnitude* is always pinned to the ceiling passes a naive "variance of log-ratio" check because sign flips with content, even though magnitude carries zero information | Raw output variance conflates "discriminates content" with "outputs extreme numbers regardless of content" | Split signal into (i) **directional signal** — variance of *signed* log-ratio across genuinely distinct pairs (kills constant-tie), and (ii) **magnitude calibration** — correlation between the judge's ratio and an *objectively verifiable* reference ratio on an anchor subset (see (d)) where ground truth on the *magnitude* (not the subjective attribute) is cheap to obtain — word-count ratios, price ratios, decibel descriptions. This anchors calibration without reintroducing ground truth on the subjective judgment itself. | The anchor subset measures numeric-magnitude-following ability, not subjective-attribute magnitude sense; a judge could conceivably be well-calibrated on numbers and still miscalibrated on "how much funnier" — the anchor is a proxy, not a proof. |

---

## (c) Statistical rigor

**Every dimension above is a proportion or a graph statistic estimated from
finitely many probes; none of them are free of sampling noise, and the
repo's own artifacts already show how much that noise matters.**

- **Order-swap / reciprocal (merged axis).** A binomial proportion. To
  resolve a true agreement rate near 0.85–0.95 with a 95%-CI half-width of
  ≈0.03 needs roughly 350–1,050 distinct pairs depending on where in that
  range the true rate sits (worst case at p≈0.5 needs ~1,050; near p≈0.9
  needs ~350). Recommend **≥500 distinct pairs per model per domain**, each
  asked in both presentation orders (1,000 calls), replicated across ≥3
  domains — a single-domain fluke should not move a leaderboard row.
- **Hodge curl / frustration.** This is a **graph** statistic, not a simple
  proportion — its precision depends on the number of independent triangles
  in the comparison graph, not on raw pair count. `cardinal-harness`'s
  default sparse active-planner budget (4·n) does **not** produce enough
  independent cycles to estimate curl with a usable CI; the curl axis needs
  its **own dedicated near-complete round-robin subgraph**, separate from
  whatever sparse graph the other axes reuse. A round-robin set of n=16–24
  items per domain (120–276 pairs, 560–2,024 triangles) is enough for a
  stable point estimate; report a **bootstrap CI over triangles** (resample
  edges with replacement, ≥100 resamples — matching the discipline the
  repo's own test battery uses for its statistical claims,
  `research/notes/TESTING.md`), and additionally replicate across **≥5 independent
  item-sets per domain** so the CI reflects corpus-sampling variance, not
  just one graph's internal resampling variance.
- **Framing-spin susceptibility (χ).** The repo's own live finding is the
  strongest available warning here: on gpt-5.4-mini, χ measured **−0.18 on
  one pair and +0.64 on another** (`research/notes/FIRST_PRINCIPLES.md` §5½,
  spin-probe-2026-07-05 receipt) — a swing of 0.82 nats between two single
  pairs of the *same model*. A leaderboard number built from one or a
  handful of pairs' χ is not measuring the model, it is measuring which
  pairs got sampled. χ must be reported as a **population statistic across
  ≥50–100 distinct pairs per domain**, with its own CI, and "belief survives
  spin" as a **rate** (fraction of pairs where it survives), never as a
  single boolean.
- **Polarity and paraphrase.** Same shape as swap agreement: binomial-style
  consistency rates need comparable pair counts (≥300–500 per axis per
  domain) to resolve differences the size labs will actually be
  hill-climbing over (a few points).
- **Null-pair calibration.** Cheapest axis by far (sub-cent per model per
  the repo's own receipt) but also, per (b)7, currently the least
  discriminating one for frontier models — more pairs do not fix a floor
  effect; the fix is corpus difficulty (near-identical, not identical).
- **Multiple comparisons.** A composite over 7–8 dimensions × many models ×
  many domains is a large garden of forking paths. Concretely: (i)
  **pre-register the composite formula** before computing any model's
  ranking — never select the aggregation that flatters whichever model the
  benchmark's authors are affiliated with; (ii) publish full CIs and treat
  overlapping intervals as ties rather than crowning a single winner on a
  fractional-point gap; (iii) require a minimum effect size (e.g. composite
  delta > 2×SE) before a leaderboard revision claims a rank change; (iv)
  publish a **test-retest reliability** number for the composite itself —
  run the whole battery twice on the same model with independently-drawn
  probe sets and report the correlation — the way MT-Bench-style judge work
  reports inter-rater agreement. If the composite doesn't reproduce itself
  within a few points run-to-run, no individual model comparison built on
  it means anything.
- **Cost floor for one credible per-model run**, summing the above: ≈1,000
  calls (swap/reciprocal) + ≈2×190 calls per round-robin domain × 5
  item-sets for curl (≈1,900/domain) + ≈600 calls (spin: 100 pairs × 3
  framings × 2 orders) + ≈400 calls (polarity) + ≈400 calls (paraphrase) +
  ≈100 calls (null) + ≈200 calls (signal anchor) ≈ **4,600–6,500 calls per
  model per domain**, before multiplying by however many domains the corpus
  spans. This is not a "run it in CI on every commit" cost; it is a
  quarterly-leaderboard-refresh cost, and the spec below prices it in
  dollars.

---

## (d) Corpus design

- **Domain breadth is the difference between a toy and a benchmark.**
  Include domains that span the contestedness spectrum on purpose: (i)
  low-contest / near-objective (numeric-magnitude descriptions, code-bug
  severity, factual-claim confidence) where the anchor subset for the
  signal axis lives; (ii) mid-contest (writing quality, argument strength,
  plan quality — the repo's own worked domain) where most of the
  "judgement" interest lives; (iii) high-contest / culturally variable
  (humor, aesthetic taste, food, fashion, political framing) where genuine,
  human-irreducible disagreement exists and a benchmark must not punish a
  judge for reflecting that disagreement honestly.
- **Clear vs. contested stratification is not optional.** Pre-screen pairs
  within each domain into a "clear" stratum (a small expert panel agrees
  near-unanimously) and a "contested" stratum (the panel itself splits or a
  known Condorcet-style cycle exists among human raters). Report curl,
  swap-agreement, and spin **separately per stratum**. Near-zero curl is
  the right target on the clear stratum; on the contested stratum, some
  curl is the *epistemically honest* signature — the repo's own note that
  the ratio ladder's quantization alone injects a ≈0.13 curl floor even for
  a fully transitive judge (`research/notes/FIRST_PRINCIPLES.md` §5½) means "curl = 0"
  was never the achievable target in the first place; the benchmark must
  calibrate its notion of "good" against a measured noise floor per
  stratum, not against zero. This stratification is the single most
  important corpus decision in the whole design — without it, the
  composite silently rewards judges that impose false consensus on
  genuinely disputed material.
- **Public vs. held-out vs. procedural — use a hybrid, not one choice.**
  (i) A small published diagnostic slice (≈5–10% of the corpus) for
  methodology transparency and public audit. (ii) A majority held-out set,
  refreshed on a fixed cadence, in the spirit of a live rotating eval rather
  than a static file labs can memorize. (iii) The framing-spin wrapper text
  and the paraphrase wordings specifically should be **procedurally
  regenerated every refresh** (per (b)4/(b)6) — these are the cheapest
  parts of the corpus to memorize and the ones where memorization defeats
  the entire point of the axis.
- **Confound-controlled pairs (per (a))** are a distinct, higher-cost corpus
  artifact: matched pairs where a surface feature is pinned and the
  attribute-relevant content is deliberately swapped. These need either
  expert construction or LLM-generation-then-human-verification; budget for
  this as its own line item, not an afterthought.
- **Sizing.** For a credible multi-domain launch: **8–10 domains**, each
  with **40–60 curated items** stratified clear/contested (≈400–600 items
  total), plus a dedicated **16–24-item round-robin subset per domain** for
  the curl axis, plus a **~100-pair objectively-verifiable anchor subset**
  for signal calibration, plus the confound-controlled subset (recommend
  starting at ~50 pairs per domain, expanding once the axis proves its
  worth).

---

## (e) The composite score

Requirements the formula must satisfy, in order of how badly each attack
above breaks a naive choice:

1. Must **not** be gameable by excelling on the cheap axes (null-pair,
   swap-agreement — both shown above to be near-floor-effect for frontier
   models) while neglecting the hard ones (spin, curl-on-contested).
   → rules out a plain **arithmetic mean**.
2. Must **not** reward a constant-tie or content-blind judge just because it
   is perfectly coherent. → requires a **multiplicative signal gate**, not
   an additive bonus.
3. Must **not** let abstention on hard pairs launder into a rosy coherence
   number. → requires **coverage as a hard external factor**, not folded
   into any per-axis coherence term.
4. Must **not** double-count the swap/reciprocal redundancy from (b)2.

Proposed formula:

```
Composite = Coverage × SignalCalibrated × HarmonicMean(
                C_swap_reciprocal,      # merged, per (b)2
                C_curl_clear,           # calibrated against measured noise floor
                C_curl_contested_adj,   # same, contested stratum, floor is HIGHER by design
                C_spin,                 # 1 - susceptibility-rate-weighted incoherence
                C_polarity,
                C_paraphrase,
                C_null,
)
```

where `Coverage = answered / attempted` (a hard multiplicative gate, per
(b)3), `SignalCalibrated` combines directional-signal variance with
anchor-correlation from (b)'s Signal row (also multiplicative — zero content
sensitivity zeroes the whole score regardless of how coherent the shortcut
is), and each `C_*` term is bounded in `[0,1]` with 1 = perfectly coherent
relative to its stratum's own noise floor (not relative to zero).

**Why harmonic mean, not arithmetic mean or hard minimum.** Harmonic mean is
dominated by its smallest terms — much closer to a minimum than an
arithmetic mean — which directly defeats the "ace six axes, bomb one"
exploit. A hard minimum is more brutal but also more fragile: a single
axis's estimate landing low purely from sampling noise (per the χ swing in
(c)) would crash the whole score on a fluke. Harmonic mean gets most of the
minimum's Goodhart-resistance while tolerating the noise floor established
in (c) — provided every `C_*` is itself reported with a CI, and the
composite's own test-retest reliability (per (c)) is published alongside it
so users know how much of the number is signal.

**What this formula cannot do.** It cannot detect a judge with a stable,
internally-consistent, but simply *wrong or idiosyncratic* taste — passing
every transformation family in this design while being calibrated to a
minority aesthetic, a specific culture's norms, or a training-data artifact
that happens to be self-consistent. That gap is named, not solved, in (a);
no coherence-only design closes it, and the composite should ship with that
limitation stated in the same sentence as the number.

---

## (f) Would a lab actually adopt this?

| Existing eval | What it measures | Needs ground truth? | Relationship to JCB |
|---|---|---|---|
| **RewardBench** | Reward-model accuracy against curated human-labeled preference pairs (chat, safety, reasoning, etc.) | Yes | Orthogonal, not competing — RewardBench answers "does the judge agree with humans on average"; JCB answers "is the judge's own preference a stable function of content." A model can score well on one and poorly on the other; that divergence is itself the interesting finding a joint report would produce. |
| **Sycophancy evals** (Anthropic, Perez et al., and similar) | Whether stated views shift to match an expressed user opinion | No (behavioral, self-referential) | Near-1:1 overlap with the framing-spin axis. JCB's contribution is making it **continuous** (χ in nats, not a binary flip) and **integrated** into the same pairwise-ratio instrument used for every other axis, so cross-axis correlations (e.g. "high-curl judges are also high-χ judges") become visible for the first time in one report rather than two separate papers. |
| **MT-Bench / LLM-judge-agreement studies** (Zheng et al. 2023 and follow-ons) | Judge-vs-human agreement, plus ad hoc position- and verbosity-bias checks in an ordinal single-shot setting | Yes, for the agreement number; position-bias check is self-referential | JCB generalizes the position-bias check (ordinal → full ratio-scale, with a measured reciprocal-consistency-in-nats rather than a flip-count) and drops the ground-truth requirement entirely, at the cost of no longer being able to say anything about correctness. |
| **Checklist-style behavioral testing** (Ribeiro et al. 2020) | Invariance under templated perturbations, general NLP | No | Paraphrase- and polarity-reversal axes are direct applications of this much older idea to the pairwise-judge setting; nothing new in the mechanism, the packaging (same instrument, same corpus, same composite) is the contribution. |

**Genuinely differentiated pieces**, in order of how novel they actually
are: (1) ratio-**magnitude** coherence — nothing in the comparison table
measures whether a judge's sense of *how much* better something is survives
presentation order; MT-Bench and RewardBench are ordinal-only by
construction. (2) Hodge-curl as a formally computed, weighted,
cycle-detecting transitivity statistic rather than ad hoc triad-counting —
more statistically principled than most preference-learning literature's
transitivity checks, and it comes with an honest noise floor already
measured in this exact codebase. (3) Susceptibility as a continuous
physical quantity rather than a binary sycophancy flag. (4) Naming and
jointly measuring the signal/coherence tension — most existing bias evals
check one failure mode in isolation and do not simultaneously guard against
the degenerate "constant judge wins" optimum.

**What would actually make a lab adopt it, concretely:**

1. **Cost.** The repo's own receipts (sub-cent null-calibration sweeps,
   $0.05 for a 32-comparison sort) show this class of probe is cheap enough
   to run repeatedly during training — a real practical edge over
   RewardBench-scale human-labeled sets. State the per-refresh cost from
   (c) up front; "cheap enough to run every training checkpoint" is a
   stronger adoption pitch than "more rigorous."
2. **Demonstrated non-redundancy.** Before asking for adoption, run JCB
   against ~15–20 public models and correlate the composite against
   RewardBench accuracy and against an existing sycophancy eval. If JCB
   predicts variance those evals miss — ideally variance that predicts a
   real downstream failure (a documented reward-hacking incident, a
   user-facing inconsistent-judgment complaint) — that is the adoption
   case. If it is redundant with sycophancy evals, adoption interest will
   be low and the honest thing is to say so rather than ship it anyway.
3. **A refresh mechanism that survives contact with a leaderboard.**
   MMLU/GSM8K-style saturation is the default fate of any published
   benchmark; the held-out + procedural-regeneration design in (d) is not
   optional decoration, it is the difference between a benchmark that stays
   meaningful for a year and one that is memorized within a quarter.

---

## The spec I would actually publish

**Name:** Judge Coherence Benchmark v0 (explicitly versioned; expect v1 to
change the composite once test-retest data exists).

**Dimensions (7, with the redundancy in (b)2 resolved):**

1. Order/reciprocal consistency (merged swap-agreement + antisymmetry)
2. Transitivity / frustration (Hodge curl), reported separately for clear
   and contested strata, calibrated against a measured per-stratum noise
   floor
3. Framing-spin robustness (χ population statistic + survival rate)
4. Polarity-reversal anti-correlation (antonym-based, not negation-prefix)
5. Paraphrase stability (procedurally regenerated attribute wording)
6. Null-pair calibration (near-identical, not identical, plus a
   should-NOT-tie null-negative)
7. Nuisance-perturbation stability (new axis, per (b)1 — the one clean kill
   against hash/lexicographic shortcuts)

Plus the **signal gate** (directional-signal variance + anchor-correlation
calibration) and the **coverage gate**, both multiplicative and outside the
per-dimension harmonic mean.

**Formula:** `Coverage × SignalCalibrated × HarmonicMean(dimensions 1–6 per
stratum, 7)`, published with per-term CIs and the composite's own
test-retest reliability, per (e) and (c).

**Corpus:** 8–10 domains spanning low/mid/high contestedness, 40–60 items
each stratified clear/contested (~400–600 items total), a dedicated
16–24-item round-robin subgraph per domain for the curl axis, a ~100-pair
objectively-verifiable anchor subset for signal calibration, and a
confound-controlled subset (starting ~50 pairs/domain) for the mechanism
probe from (a). Diagnostic 5–10% published, majority held out and refreshed
on a fixed cadence, spin/paraphrase wrapper text procedurally regenerated
every refresh.

**Cost per model per full refresh:** ≈4,600–6,500 calls per domain (from
(c)), ×8–10 domains ≈ **40,000–65,000 calls**. At current small-model
OpenRouter pricing (the repo's own receipts run $0.0004–$0.005 per
comparison depending on model) this lands roughly in the **$20–$300 per
model per refresh** range for a small/mid model, scaling up for frontier
models with longer completions and logprob overhead — cheap enough to run
quarterly across a real model roster, not cheap enough to run in per-commit
CI.

**Stated limitation, in the same document as the number, not a footnote:**
this benchmark cannot and does not claim to detect a self-consistent but
substantively wrong judge; it is a necessary-condition instrument, meant to
run alongside, never instead of, a ground-truth correctness eval.
