# llmsorting.com — program

Sorting a list well is where judgement becomes action: every allocation,
priority queue, grant screen, moderation decision, and "which matters more"
is a sort. Language models can sort — sometimes, for some attributes, under
some presentations. This program exists to make *when* they can into a
measurement people can read, to make it cheaper to obtain, and to try every
trick in the book with evidence attached.

llmsorting.com is the technical and demonstrational hub. It does not host
the engine, the ledger, or the benchmark; it uses all three:

| Surface | Role | Repo |
|---|---|---|
| llmsort (engine) | pairwise-ratio elicitation, IRLS solve, uncertainty, active planning, evidence packs | this repo (`XyraSinclair/llmsort`, public, GitHub) |
| openpriors.com | the ledger: provenanced judgement records, cardinal scores fitted from ratio judgements, per-model reliability | `exopriors-core` (private; web surface `openpriors`) |
| pairwiseratio.org | the benchmark: Judge Coherence Benchmark — does the judgement survive order swaps, polarity, paraphrase, cycles | `sites/pairwiseratio.org/` in `exopriors-core` (private) |
| **llmsorting.com** | methods, tricks, robustness metrics, demos, evidence packs, and the program itself | `sites/llmsorting.com/` in `exopriors-core` (private) |

Status labels used throughout: **PLANNED** (designed, no run) · **IN FLIGHT**
(running or under review) · **EXECUTED** (observed after-state, evidence pack
committed).

## 1. The book of tricks (methods catalog)

Every method is a point in arity × scale × output-form (llmsorting,
`docs/FIRST_PRINCIPLES.md` §2). What each yields, what breaks, and its cost
shape:

| Method | Yields | What breaks | Cost shape |
|---|---|---|---|
| Pointwise rating ("rate 1–10") | ordinal-ish score per item | anchor drift, cluster at 7–8, no error bars | O(n) |
| Listwise ("sort this list") | an order, in one call | position bias, context limits, silent drops and hallucinated items | O(1) calls, O(n) tokens, unbounded error |
| Pairwise ordinal ("which is more?") | direction per pair | magnitude-blind; naive schedules O(n²) | O(n log n)–O(n²) |
| Comparison-sort with an LLM comparator (merge/quick/tournament) | an order in O(n log n) comparisons | one wrong comparison propagates; no uncertainty; assumes transitivity | O(n log n) |
| Pairwise ratio (llmsorting `canonical_v2`) | log-ratio measurement per pair → cardinal latents ± σ | needs a solver; ratio ladder must be native to the model | O(n) at 4·n default budget, active |
| Counterbalancing (both presentation orders) | position bias measured, not assumed | doubles calls unless planned | ×2, folded into budget |
| Repeat draws / temperature | choice probabilities per pair → stochastic transitivity | cost multiplies by draws | ×k |
| Logprob / PMF evidence (`ratio_letter_v1`, `ordinal_letter_v1`) | the model's whole answer prior in one call (≤ log₂ 52 ≈ 5.7 bits) | not every provider exposes logprobs; reasoning models often don't | same price as a point |
| k-wise nominal ("which is most") | 1 winner among k → k−1 implied dominations | lowering to pairwise loses magnitude | O(n/k · log) |
| Best–worst scaling (MaxDiff) | best AND worst of k → 2k−3 implied pairs per call | ordinal; PMF hard past small k | high info per call |
| **Setwise ratio with cached prefix** (E1) | k−1 independent log-ratios per call; entities cached, attribute swapped | shared-call correlation; provider cache thresholds | prefix paid once per subset, then attribute-only |
| Active pair selection (effective resistance) | spend the next comparison where it buys the most order information | must beat uniform random — measured, not assumed | planner overhead only |
| Attribute health probes (`--two-sided`, `--also-by`) | evidence the attribute even coheres for this judge | extra runs | ×2–3 |

## 2. The grok gauge — when a model sufficiently groks a transitive attribute

Definition. A judge *groks* an attribute over an entity pool when its
judgements behave like noisy readings of one latent scalar: the root claim
`m(A,B) = s(A) − s(B)` in log space holds up to noise. Grokking is a property
of the triple (model, attribute wording, entity pool), never of a model alone.

The gauge is a small set of falsifiable readouts, each with a denominator and
a null. All of them already exist as engine diagnostics in llmsorting; the
program's contribution is a fixed protocol, a public presentation, and
calibration against ground truth.

| Readout | Test | Null (what a non-grokking judge shows) | Engine seam |
|---|---|---|---|
| Order invariance | same pair, slots swapped: direction agreement; mean \|Δ ln r\| | 50% agreement (coin flip) | counterbalanced pairs; openpriors "order agreement" |
| Reciprocity | "how many times more" vs "how many times less": ln r₊ + ln r₋ ≈ 0 | drift ≫ 0 | JCB reciprocity |
| Direction transitivity | over triads with repeat draws: WST/MST/SST violations deeper than 2 SE | cycles beyond sampling noise | `rerank/transitivity.rs` |
| Multiplicative closure | triangles: ln r_ab + ln r_bc + ln r_ca ≈ 0 — cyclic residual fraction (Hodge curl), frustration | curl mass ≫ noise floor | `rating_engine` Hodge split |
| Polarity | negated attribute: correlation of latents ≈ −1 | ≈ 0 (attribute ignored) or > 0 | JCB polarity |
| Paraphrase | reworded attribute: rank correlation ≈ +1 | low | JCB paraphrase / `--also-by` |
| Signal | spread of latents beyond posterior noise | flat | signal gate |
| Null calibration | irrelevant attribute: no phantom structure | structure appears anyway | JCB null |

Composite (JCB): signal × mean coherence, so a constant judge scores 0 and a
decisive incoherent judge scores low.

Legibility for people. Three bands, stated as conventions to be calibrated
(PLANNED, E2): **grokked** (order agreement ≥ 0.9, zero WST violations
beyond 2 SE, curl fraction ≤ 0.1, polarity ≤ −0.8, paraphrase ≥ 0.8),
**partial**, **not**. The bands are earned, not decreed: E2 runs the gauge
on the anchor tier (countries by population, rivers by length — attributes
with known true ratios) where grokking is externally checkable, and adjusts
the thresholds until the gauge agrees with truth there. Only then does it
get applied to attributes without ground truth. Every band shown carries its
denominators (pairs, triads, swapped pairs, runs) next to it.

## 3. Experiment ladder

Each experiment produces one evidence pack (report.json, trace.jsonl,
RESULTS.md with denominators) and one page here.

- **E1 — setwise ratio with a cached entity prefix.** k ∈ {3,4} entities in
  the prompt prefix, attribute in the suffix; k−1 independent log-ratios per
  call lowered to pairwise evidence; counterbalanced by pivot rotation; solve
  and compare against the canonical pairwise sort on the same items; report
  cache_read_tokens fraction, pairwise-equivalent observations per dollar,
  Spearman/top-k agreement. Design in §4. **EXECUTED** 2026-08-15
  (llmsorting `examples/setwise_cached.rs`, pack
  `research/artifacts/live/setwise-cached-2026-08-15/`, llmsorting@412bd3d):
  gpt-4.1-mini, 372/372 calls parsed, $0.209. Caching confirmed — 75–78%
  cached fraction on tail-attribute calls; 5,370 pairwise-equivalent
  obs/$ vs 2,049 pairwise (2.6×). Pathology found: pivot halo — 795/870
  ratios < 1 (mean −0.95 nats), pivot rotation flips 80–93% of implied
  directions (pairwise positional flips: 6–38%). Agreement ρ 0.29–0.81
  by attribute. Next: reciprocal-frame prompt or per-call pivot-effect
  term in the solver before k-wise earns a promotion.
- **E9 — NORTH head-to-head: single-token PMF vs JSON rail.** The 10x-core
  decree's premise, measured (design: docs/NORTH.md). `llmsort sort` on 6
  items x 24 comparisons, gpt-4.1-mini, both templates, pack-local
  replayable caches. **EXECUTED** 2026-08-19 (pack
  `research/artifacts/live/north-e9-2026-08-19/`): at identical cost
  ($0.0048 vs $0.0044), ratio_letter_v1 reads stat error +-0.020 vs
  +-0.464 (23x), order residual 0.034 vs 0.178 nats (5x), rank risk 1.88
  vs 2.79; rankings agree rho 0.886. Flag: frustration HIGHER on the
  letter rail (16.9% vs 11.5%) — instrument sensitivity vs judge
  cyclicity, E10 must separate. Next: E10 family sweep (cached pair
  prefix x {A, A', not-A} x orders → scaling + reliability reading from
  the same calls).
- **E8 — whitespace-jitter repeat probes.** Probe the same structured
  judgement K times, probe k widening 1-3 seed-chosen word gaps in the
  attribute prompt (deterministic, blake3(text,k)) so each probe is a
  distinct cache key: draws accumulate and replay instead of colliding
  into the one cached judgement; pooled by `repeat_pooling` (DL
  heterogeneity floor). **EXECUTED** 2026-08-19 (`cardinal probe`,
  experiments/src/probes.rs, pack
  `research/artifacts/live/whitespace-probes-2026-08-19/`): 6 entities x
  6 probes x ring, 36 calls, $0.003, deepseek-v4-flash. Jitter moves the
  answer on most repeat draws (duplicate rate 17-40% across two runs);
  one pair split on DIRECTION across probes in both runs — single-probe
  elicitation hides that class entirely. sigma_b2 > 0 even at n=6.
  Next: K-vs-precision curve; jitter-vs-plain-resample A/B (no-cache
  rail); opt-in repeat mode in sort.
- **E2 — grok gauge calibration on anchors.** Anchor entity pools with true
  ratios; several models; the gauge's bands tuned where truth exists.
  **EXECUTED** 2026-08-15 (llmsorting `examples/anchor_gauge.rs`, pack
  `research/artifacts/live/anchor-gauge-2026-08-15/`, llmsorting@2927642):
  3 pools x 16 entities x {gpt-4.1-mini, gpt-5.4-nano}, 768/768 calls,
  $0.107. Bands separate models (mini partial everywhere, nano not
  everywhere — matching truth-rho order in every pool) but the composite
  loses component resolution (truth rho 0.965 and 0.350 both "partial");
  no cell reached grokked — order>=0.90 and polarity<=-0.80 too strict at
  a 64-comparison budget. Evidence-backed proposal on file: gate on
  curl<=0.10 AND order>=0.70 (6/6 separation here), polarity demoted to
  diagnostic. Judges compress true log-ratios ~1/3 (slopes 0.55-0.68)
  wherever rank is good. WST unmeasured (no repeat draws) — next
  calibration round adds repeat-draw triads and a higher budget arm before
  bands are adopted.
- **E3 — attribute-swap economics.** A attributes over one fixed entity block:
  measured cost per attribute as A grows, versus pairwise per attribute.
  **PLANNED** (falls out of E1's harness).
- **E4 — comparator-sort zoo.** Merge sort, quicksort, tournament, and the
  active planner over an LLM comparator with measured noise: comparisons to a
  certified top-k, error under intransitivity, on simulation and one live
  pool. **PLANNED.**
- **E5 — listwise vs pairwise vs setwise.** Folded into E6 as its `order`
  arm (2026-08-22): the consumer's decision is best–worst vs plain listwise
  vs pairwise at adequate agreement per dollar, so listwise is E6's
  efficiency denominator, not a separate sweep. **FOLDED.**
- **E6 — best–worst scaling instrument.** The highest-value missing cell in
  the instrument grid. Design climbed 2026-08-22 (parsimony climb, 5 rounds,
  18 accepted deletions; ledger in hill-climb-parsimony@afa53e4). Target use:
  reranking web-search results under a custom user prompt where an adequate
  quality adjustment, not a certified order, is the bar. Shape: a diff to
  `experiments/examples/setwise_cached.rs`, no new file — `--answer
  {ratio,bw,order}`; `ratio` is the existing pivot-ratio arm on the existing
  pair-cover design, untouched; `bw` (two slot letters: best, worst) and
  `order` (full order of the k slots) are point answers, no logprobs, on a
  chunk design derived from the mode: `--presentations` rounds of seeded
  shuffle → even split into ⌈n/k⌉ groups, call count printed up front. One
  parse target `Slots(Vec<usize>)` (length 2 or k, distinct; anything else
  is malformed, never a default); one tier-lowering — tiers
  [[best],[rest],[worst]] for `bw`, singletons for `order` — emitting every
  cross-tier pair as an ordinal `Observation` at the existing
  `FIXED_BUCKET` magnitude into the same `RatingEngine` (2k−3 and k(k−1)/2
  fall out). `SyntheticJudge` gains one branch (perturb latents per slot,
  sort; `order` emits the order, `bw` the two ends). Readouts added:
  `first_by_slot`/`last_by_slot` histogram (position bias, measured) and
  `SolveSummary.components` surfaced — arm flagged `disconnected` when > 1
  (no silent drops; the harness had no such flag). Everything else —
  pairwise baseline arm, Spearman/top-k, SpendMeter cap, trace, pack — is
  the existing harness. Offline synthetic judge first ($0), then live under
  the cap on the DeepSeek V4 Flash lane. Rejected deletion, recorded as an
  invariant: the `order` arm stays — it is the denominator of the
  efficiency claim. **EXECUTED** 2026-08-22 (llmsort@b956445, pack
  `research/artifacts/live/best-worst-2026-08-22/`): deepseek-v4-flash,
  n=24, k=8, three attributes (two rubrics + one plain user-prompt string),
  two pools, m∈{3,6}; 216 setwise calls, 216/216 parsed, all graphs
  connected, $0.27 total. `order` (plain listwise, 9 calls/attribute) agrees
  with the 96-comparison pairwise sort at ρ 0.64–0.92 (median 0.80) for ~¼
  its dollars per item, against a pairwise test–retest ceiling of 0.83–0.94;
  its own test–retest (m=3 vs m=6) is 0.88–0.94. `bw` at the same price:
  ρ −0.18–0.76 (median 0.25), test–retest 0.46–0.85 — refuted as built (13
  obs/call vs 28, worst-pick is weak). Position bias measured: last slot
  ranked last 2.2× fair share (order), 1.9× (bw); first slots under-picked.
  Reading: in the adequate-adjustment regime the listwise arm the climb kept
  as the denominator is the instrument; the "highest-value missing cell" is
  not. Addendum 2026-08-23 (llmsort@328d212, +$0.13): `--repeats` yields an
  **order-sensitivity gauge** — direction-flip rate across shuffled
  re-presentations of the same subset — the first thing to run in a new
  domain; live it separates flaky attributes (impact_per_dollar ~0.34) from
  stable ones (user-prompt ~0.14) and flags exactly the cell where k = 12
  collapsed. k sweep: k = 6–8 is the band; $/item ~flat in k, so take the
  largest k the flip rate tolerates. Robustness matrix 2026-08-23
  (llmsort@78d2a85+37ca9e4+bb41bd6, +$0.87, 9 runs): delimiter
  {xml,bracket,dash} is a free parameter; entity size 400–8000 chars holds
  (~100-token entities are the flakiest cells and the gauge says so);
  the instrument transfers to gpt-4.1-mini and gemini-2.5-flash and to a
  second corpus family (150 arXiv abstracts, paper-native attributes) —
  weak cells move with (model, attribute) and the gauge screens them
  one-sidedly: over all 38 live cells, flip < 0.20 ⇒ ρ ≥ 0.64 (median
  0.79); every ρ < 0.61 had flip ≥ 0.21. gemini emits partial orders on
  one attribute (strict parse rejects; 6/36 calls). Remaining caveats:
  single-run pairwise baselines on the new cells; no PMF arm (E7).
  **Graduation gate** (making README's "graduates only on evidence"
  specific for `order`): the recipe — r = 2 flip-rate gauge first, then
  k = 6–8 listwise → tier-lowering → solver — enters the crate's promised
  surface (a mode beside `sort_documents`) when: ≥ 2 model families at ρ
  inside the same-model pairwise test–retest band at ≤ ½ pairwise $/item
  on the same pool ✅(deepseek ✓; gpt-4.1-mini/gemini adequate but their
  pairwise bands unmeasured — one repeat run closes this); ≥ 2 corpus
  families ✅; an entity-size map ✅ (400–8000, soft at ~400); delimiter
  verdict ✅ (free parameter); the gauge shipped as part of the recipe,
  not an optional extra ⬜ (the crate-side implementation itself). One
  repeat run on a second model plus the crate-side mode closes the gate.
- **E7 — PMF evidence per instrument.** Where providers expose logprobs,
  separation per dollar versus point answers, per instrument. **PLANNED.**

## 4. E1 design: setwise ratio, cached prefix

Prompt geometry (cache-native). Provider prompt caches key on an exact byte
prefix, so the byte-stable part goes first: system instructions, then the
`<entities>` block (k texts under slot letters). The attribute is the last
thing in the prompt. Swapping the attribute never touches the prefix, so
after the first call per (subset, presentation) every further attribute pays
only for the suffix and the answer. Provider facts to lean on and to
measure, never assume: Anthropic caches at explicit breakpoints with a
minimum cacheable prefix (≈1024 tokens; larger for the smallest models),
reads at ~10% of input price; OpenAI caches automatically for prefixes ≥1024
tokens, reads at 10–50% depending on model. Entity texts must clear the
threshold — that is a design input, not an accident.

Answer shape. Pivot slot A; the model returns `{"ratios":{"B":r_B,"C":r_C,
…},"confidence":c}` or `{"refused":true}`. Strict parse; a malformed answer
is a recorded failure, never a default.

Lowering. Each call yields k−1 independent observations `ln r_i` (slot i vs
pivot), weighted like a canonical point judgement. Implied non-pivot pairs
are linear combinations and are not added twice. The shared-call correlation
is an honest caveat, quantified later by comparing posterior widths against
the pairwise path.

Counterbalancing. Every subset is asked in ≥ 2 presentations (rotate pivot
and slot order); pivot-rotation disagreement is the position-bias readout.

Design. n = 8, seeded random k-subsets until every unordered pair is
covered ≥ 2 times; then A = 3 attributes over the same subset list.
Comparator: `sort_texts` with `canonical_v2` at the default budget on the
same items and attribute. Readouts: Spearman ρ, Kendall τ, top-1/top-3
agreement, calls, tokens, cache_read/write tokens, nanodollars, pairwise-
equivalent observations per dollar. Offline synthetic judge first ($0), then
live under a hard cap.

## 5. The Manifund campaign (three months of GPU work, scheduled)

Operator mandate 2026-08-16: the local judges must never starve. The campaign
is a manifest (`research/campaigns/manifund-3mo.json`) walked by a box-resident runner
on colo2 (`research/scripts/campaign_runner.py`, idempotent via `--resume-ledger` —
restarts re-buy nothing). Supply axes, all committed:

| axis | size | file |
|---|---|---|
| attributes | 1,010 Fable-authored subtle attributes (~39 families) | `research/batteries/fable_subtle_1000.txt` |
| entity pool | 40 curated → 1,263 full-corpus proposals | `research/data/manifund.txt`, `research/data/manifund_full.txt` |
| judges | gemma4-31b (live) · qwen38-27b · gemma4-26b-a4b (lanes activate when served) | manifest `base_url` per phase |
| phrasings | bare now; elaborated forms next Fable pass | `research/batteries/fable_subtle_1000_elaborated.txt` (pending) |
| repeat draws | seeds 2–3 with `--no-cache` (independent samples, not cache replays) | manifest phases |

Measured throughput: gemma4-31b 6.4 judgments/s (one 240-budget attribute
≈ 38 s; one full-pool 5,000-budget attribute ≈ 13 min). Ladder ETAs at that
rate: 40-pool passes ≈ 0.4 d each; full-pool passes ≈ 9 d each per judge per
seed. The manifest as committed schedules ≈ 55–75 days on the gemma lane
alone; the qwen and A4B lanes add ≈ 35 d when their serves are up.

What the data is FOR (the fascinating part — each lands as an analysis over
`ratiometer.judgments`, no new elicitation needed):

1. **Attribute quality at scale.** Cross-judge direction agreement over 1,010
   attributes ranks which subtle questions LLMs can actually answer — the
   32-attribute pilot already separated 'technical depth' (0.94) from
   'counterfactual impact' (0.62). Now with denominators in the thousands.
2. **The geometry of judgment space.** 1,010 latent scores per proposal →
   factor structure of what LLM judgment actually spans. Are 'poshness',
   'institutional insiderness', and 'quiet prestige' one axis or three?
   How many effective dimensions does a 31B judge have?
3. **Elaboration effect at scale.** The 12-attribute pilot showed +0.2
   agreement on the vaguest attributes and −0.1 on 'earnestness'. Over 1,010
   attributes this becomes a rule for when elaboration helps, not an anecdote.
4. **Grok-gauge distribution.** Curl, order-invariance, and WST (from the
   seed-2/3 repeat draws) per attribute — the E2 gauge applied over a
   thousand attributes instead of six cells.
5. **Ground truth.** `research/data/manifund/ground_truth.csv` (funding outcomes):
   which subtle attributes predict what actually got funded — and where the
   judges and the funders disagree.

Public surface: **openpriors.com/manifund** (exopriors-core route
`web/src/routes/manifund/+page.svelte`) renders the 40-pool slate re-rankable
by every judged attribute. Its dataset is a committed static snapshot
regenerated by `research/scripts/manifund_page_data.py` (extend `RUN_TAGS` as phases
land, then commit `web/static/manifund/data.json` in exopriors-core and
redeploy the `nucleus-web` scope).

## 6. Deployment

Static site, `site/` → colo2 `/srv/llmsorting`, served by Caddy
(`/etc/caddy/llmsorting.caddy`, `tls internal`, Cloudflare-proxied zone
`llmsorting.com`, zone id `15d09d3f83294e1a59691f1f1ba87f96`). `./deploy.sh`
rsyncs and verifies the live page. No build step; no framework.
