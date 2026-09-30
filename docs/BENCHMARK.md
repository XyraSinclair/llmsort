# The Judge Coherence Benchmark (JCB)

`cardinal bench --models a,b,c` — score LLM *judgement* quality with **no
ground-truth labels**, purely from internal consistency under
meaning-preserving transformations, plus a signal axis so consistency
cannot be faked by refusing to discriminate.

## The argument

Every LLM-judge eval in circulation scores *agreement with a reference*
(human labels, a stronger model, majority vote). That measures conformity,
not judgement. But a judgement has an internal structure that can be tested
without any reference at all: a genuine belief must be a fixed point of the
transformations that shouldn't matter. Swap which item is shown first — the
direction must hold. Ask the reciprocal question — the answer must invert
exactly. Chain comparisons around a cycle — the ratios must compose. Negate
the criterion — the ranking must reverse. Reword it — the ranking must
survive. Lean on the judge — the belief must not follow the leaning. Show
the same item twice — the judge must tie.

None of these require knowing what the *right* answer is. They are the
measurement-theoretic preconditions for the answers meaning anything at
all. A model that fails them is not a judge with different taste — it is
not a judge. That makes this a canonically shaped benchmark for labs to
hill-climb: improving it cannot be done by memorizing answers, only by
making the model's preference structure more coherent — and coherent
preference structure is upstream of every judging application (reward
models, rankers, evaluators, feed curators).

The honest limit, stated plainly: coherence is necessary, not sufficient. A
judge with coherent but alien taste aces this benchmark. JCB measures
whether the instrument is *rigid*; whether it is *aimed well* is a separate
question that does need references. Labs should treat JCB as the
qualification test that makes reference-based evals meaningful: agreement
scores from an incoherent judge are noise about noise.

## Dimensions

194 comparisons per model on a fixed public corpus (v1.2) (8 short texts, judged
by "depth of insight about living well"), all through the ordinary
counterbalanced pairwise machinery:

| # | Dimension | Measures | Raw unit | Subscore |
|---|---|---|---|---|
| 0 | **signal** | mean fused \|log-ratio\| across 20 pairs | nats | 1 − e^(−x) |
| 1 | **order flip** | direction reversals under slot swap (decisive pairs) | rate + Wilson CI | 1 − rate |
| 2 | **order residual** | \|m_fwd − m_rev\|/2, reciprocal antisymmetry | nats | e^(−x) |
| 3 | **frustration** | Hodge curl fraction of the fused comparison graph | curl fraction | 1 − x |
| 4 | **spin survival** | belief direction stable under pro/con asker framings (3 clear pairs × 6 calls) | rate + Wilson CI | rate |
| 5 | **susceptibility** | mean \|χ\| = \|(m₊ − m₋)/2\| over 5 spin pairs | nats/spin | reported, unscored |
| 6 | **polarity** | Spearman ρ of scores vs the negated attribute | ρ | (1 − ρ)/2 |
| 7 | **paraphrase** | Spearman ρ of scores vs a reworded attribute | ρ | (1 + ρ)/2 |
| 8 | **null bias** | mean \|log-ratio\| on identical-item pairs | nats | e^(−x) |
| 9 | **nuisance stability** | drift under semantically-null edits (whitespace, markdown, bullet, prestige-halo) vs the unperturbed call | nats | e^(−x) |
| 10 | **orbit coherence** | fraction of judgment energy in the G-invariant character of the Z₂³ orbit transform (6 pairs × 8 variants) | energy fraction + CI | value |
| 11 | **interaction share** | energy in the interaction characters — bias invisible to every marginal probe | energy fraction | reported, unscored |
| 12 | **harmonic** | triad-invisible frustration on the chordless-cycle block (harmonic_dim = 1 by design) | energy fraction | 1 − x, coverage-gated |

**Composite**: `coherence` = mean of the consistency axes, where **order
flip and order residual enter as ONE reciprocity axis** (their mean) —
they are measured from the same two calls per pair, and counting them
twice would double-credit a position-bias fix. The frustration and
harmonic axes are **coverage-gated**: refused judgements vanish from the
comparison graph, so refusing on hard pairs would launder cyclicity into
apparent transitivity. Gated axes score ZERO below 95% coverage — the
original design dropped them instead, which the cyclic-judge test exposed
as a reward (deleting a bad judge's worst axis raises its mean); refusal
laundering now strictly costs.
**`JUDGE SCORE` = signal-subscore × coherence.** Multiplicative on
purpose: zero discrimination or zero consistency zeroes the headline. A
**harmonic-mean coherence** is reported alongside — one dead axis tanks
it, so pathologies cannot hide inside an average. At v1 corpus size (3
clear spin pairs) a single unlucky pair zeroes an axis, so the harsher
aggregate is evidence, not yet the ranking; it becomes the headline
candidate at v2 scale. The curl axis reports its denominator as **cycle
rank** (|E| − |V| + components = 13 here), because curl is a graph
statistic — a sparse graph cannot estimate frustration, and the
denominator says so.

## Why it is hard to game

The dimensions cross-check — each obvious exploit of one axis is caught by
another:

| Exploit | Aces | Caught by |
|---|---|---|
| always answer "tie" | every consistency axis | signal = 0 → headline 0 |
| deterministic content hash (order-invariant, decisive) | order, residual, null | **nuisance stability** (one whitespace byte scrambles the hash — the direct kill) plus polarity/paraphrase (a hash can't know the negated attribute must reverse) |
| always prefer slot A | decisiveness | order flip = 100%, null bias = ln(ratio), fused signal = 0 |
| locally-consistent cyclic preferences | order, residual | frustration (Hodge curl) |
| agree with whoever's asking | signal + correlations | spin survival = 0, χ large |

**These are not hypotheticals — they are the test suite.** Six scripted
judges (oracle, constant, position-biased, sycophant, cyclic, avalanche-hash)
run the full benchmark in-process (`experiments/tests/judge_bench.rs`); each pathology must land in
exactly the dimension that names it, and the oracle must lead the board.
A benchmark that can't separate scripted pathologies has no business
ranking labs.

## Reading a report

```text
model: openai/gpt-5.4-mini · template: canonical_v2
  signal           +0.812 [+0.6, +1.0] nats (n=20)  → 0.556
  order-flip       +0.050 [+0.009, +0.236] rate (n=20)  → 0.950
  ...
  coherence 0.871 · JUDGE SCORE 0.484 · 194 comparisons · $0.05
```

Every rate carries its denominator and a 95% interval (Wilson for rates,
±1.96 se for means). `signal` is in nats: 0.7 ≈ "typically feels a 2×
difference". Raw per-call records ship in the JSONL output
(`--out reports.jsonl`) so any number in the table can be recomputed from
the judgements themselves.

## Adversarial review (what a lab gaming this would try)

A full red-team of this design lives in
`research/notes/ideation-2026-07-05/benchmark-adversary.md`. The attacks already
neutralized in v1: consistency-without-signal (multiplicative composite),
content-blind hashing (polarity/paraphrase cross-check), position bias
(fused signal cancels + null pairs), local-consistency-with-global-cycles
(curl), reciprocity double-counting (merged axis), refusal laundering
(coverage gate). The adversary's "cleanest kill" — nuisance-perturbation
stability — shipped as axis 9 the same day. The attacks that remain OPEN
and define v2:

- **Template fingerprinting**: the spin preamble and paraphrase wordings
  are fixed strings; a lab could special-case them. Fix: procedurally
  rotated wording banks with a held-out subset.
- **Max-ratio signal inflation**: a judge whose direction tracks content
  but whose magnitude is always pinned at the ceiling scores full signal.
  Fix: a magnitude-calibration anchor subset (pairs with objectively
  verifiable ratios — lengths, counts).
- **Identical-null floor effect**: byte-equal null pairs are free to pass
  (our own measurements show four frontier models at exactly 0.000). Fix:
  near-identical pairs (independent rewordings that should tie) plus
  should-NOT-tie lookalikes.
- **Syntactic polarity**: "shallowness: the absence of..." can be passed
  by flipping on the negation marker. Fix: true antonym attributes and
  scoped negation.
- **Scale**: 20 pairs bounds the order-flip CI at ~±10pt; a leaderboard
  labs would stake reputation on needs ~500 pairs/axis across 8–10
  stratified domains (clear vs contested strata reported separately) and
  test–retest reliability published for the composite itself.

## Scope and honest caveats

- **v1 corpus is small and single-domain** (8 aphorisms, 1 attribute
  family). This bounds the CIs — order-flip on 20 pairs has a ±10pt
  interval. It is a standardized instrument, not a final exam; scale the
  corpus before treating gaps under ~0.1 as real.
- **Coherence ≠ correctness** (see above). Pair with reference-based evals;
  JCB is the precondition, not the replacement.
- **The ceiling is structural**: no behavioral battery — this one included
  — can distinguish an honest judge from an agent maintaining a coherent
  FALSE preference structure across every transformation applied.
  Consistency certifies the rigidity of the reported structure, never its
  sincerity; sincerity is not an elicitable property of input–output
  behavior. A theorem-shaped limit, not an engineering gap — shipped next
  to the score, not in a footnote. (Elicitation-complexity thread in
  `research/notes/math-frontier-2026-07-05/literature.md`.)
- **Frustration includes quantization curl** from the ratio ladder
  (documented in FIRST_PRINCIPLES §5½); the same floor applies to every
  model, so rankings are unaffected, but absolute curl should not be read
  as pure preference cycling.
- **Susceptibility is one field magnitude**, not a full response curve —
  reported, not scored.
- Spin verdicts on contested pairs are legitimately "undetermined" (a tie
  at zero field has no direction to preserve); only clear pairs enter the
  survival rate, and the denominator is printed.

## Scaling: battery-as-data (v2 machinery, 2026-08-19)

The battery is no longer compile-time constants. `experiments/src/battery.rs`
carries the machinery the public version (`docs/PUBLIC_BENCH.md`) needs:

- **`BatterySpec`** — corpus, attributes, and every pair block as a value;
  serializable, so a generated battery ships inside its evidence pack.
  `BatterySpec::v1()` reproduces the fixed v1.2 battery exactly (the
  scripted-pathology suite and all cached v1 judgements are untouched).
- **`EntityPool`** — JSON pools of meaningful entities
  (`research/data/pools/`: anchors-country-population, interventions,
  funders per the PUBLIC_BENCH tier table). Items are listed in rough
  descending prior order; the anchors tier carries `truth` magnitudes.
- **`BatterySpec::generate(pool, scale, seed)`** — deterministic
  procedural generation (splitmix64; no RNG-crate dependency, so the
  mapping is a contract): doubling-stride circulant core graphs stay
  connected and triangle-rich at any n; spin/null/perturb/orbit blocks
  scale at the v1 proportions; the seed rotates the corpus subset, block
  positions, and the paraphrase wording — the anti-memorization mechanism.
  A 16-entity pool lands at 480 calls/model, 20 entities at ~670: the
  600–1,000 band the public design costs out. `validate()` refuses any
  battery that cannot support its own estimators (disconnected core, no
  cycle support, perturb pair without a drift baseline, non-ring harmonic
  block) before a dollar is spent.
- **Magnitude calibration sidebar** — when every corpus item carries a
  ground-truth magnitude, the report adds `truth-slope`: fused judged
  log-ratios regressed through the origin on true log-ratios (1.0 =
  calibrated, <1 compressed, >1 exaggerated). Reported, unscored — closes
  the max-ratio signal-inflation attack from the adversarial review as a
  sidebar, never the headline.

Scores are only comparable within a battery; every report names its
battery slug.

## Reproduce

```bash
cardinal bench \
  --models "openai/gpt-5.4-mini,google/gemini-2.5-flash,..." \
  --no-cache --out reports.jsonl

# public-tier scale: pool-generated battery, replayable spec
cardinal bench \
  --models "..." --pool research/data/pools/interventions-v1.json \
  --battery-seed 1 --battery-out battery.json --out reports.jsonl
```

`OPENROUTER_API_KEY` required; ~$0.05/model on mini-class models. Live
study records (v1.2 battery): `research/artifacts/live/judge-bench-v1.2-2026-07-05/`,
the 2026-07-18 public board in `research/artifacts/live/kimi-k3-bench-2026-07-18/`,
and the 2026-08-07 additions (claude-sonnet-5, claude-opus-4.8) in
`research/artifacts/live/jcb-additions-2026-08-07/`. The superseded v1 study
(114 comparisons, 6 models) remains at `research/artifacts/live/judge-bench-2026-07-05/`.
