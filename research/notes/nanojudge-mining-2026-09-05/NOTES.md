# Mining nanojudge (2026-09-05)

Source: https://github.com/nanojudge/nanojudge @ 57d9113, read in full
(core crate line-by-line; CLI/bench surveyed). MIT. A direct parallel
effort: adaptive pairwise LLM ranking, Rust, three-crate workspace
(pure-math core / CLI / synthetic bench), hosted product at nanojudge.ai.
Their frame is Bradley–Terry on win probabilities (interval scale); ours
is ratio elicitation into a robust log-linear fit (ratio scale). The
overlap is close enough that every divergence is information.

## Verdict

Five pieces of genuine alpha, ranked; the rest is convergent evolution or
places we are ahead. The single most valuable import is **slot bias as a
fitted parameter in the likelihood** — it is also the already-diagnosed
fix for our E1 pivot-halo pathology. The second is their **linear-time
Laplace path** (matrix-free Newton-CG + control-variate Hutchinson),
which is the concrete sparse answer our own SCALING.md says we need.

## The alpha (adopt / adapt)

### 1. Per-judge, per-slot positional bias fitted inside the likelihood

`laplace_bt.rs`: every edge carries `(slot1, slot2, judge)` and the model
is `d = θ_i − θ_j + γ_judge[slot_i] − γ_judge[slot_j]` with a Gaussian
prior on γ (τ² = 2, logit space), highest slot pinned as reference.
Pairwise gets one β per judge; k-slot lineups get k−1 free advantages.
Bias is *corrected in the fit* and *reported with a CI* (sigmoid-mapped,
delta-method panel aggregate) — no call doubling.

Why it matters to us:
- Our counterbalancing cancels order bias at 2× calls; their fit removes
  it at 1× and returns the bias estimate as a free reliability readout.
  The two compose: counterbalanced data makes β identifiable fast; the
  fitted β then lets us *drop* counterbalancing adaptively once β is
  pinned (spend the saved calls on new pairs).
- E1's pivot halo (795/870 ratios < 1, pivot rotation flipping 80–93% of
  directions) is exactly a slot effect; PROGRAM.md already names the fix
  "a per-call pivot-effect term in the solver". In our linear model this
  is trivial: an extra design column per (judge, slot) with a Gaussian
  prior — one more term in the IRLS row, still closed-form.
- Their engine's slot bookkeeping (`item1_edge_counts` +
  Laplace-smoothed `position_probability` = (first+1)/(edges+2) ratio
  balancing) keeps the β column well-conditioned without exact ABBA
  pairing. Cheap trick, worth copying into any single-order mode.

### 2. Linear-time Laplace machinery (the sparse solver we said we need)

Our dense Cholesky path is 0.5 s at n = 250 (debug) and SCALING.md
concedes "larger production runs need sparse linear algebra or smaller
active frontiers." nanojudge's `fit_linear` is that answer, working:

- MAP by damped Newton-CG with Hessian-vector products only — O(#edges)
  per CG step, no matrix ever formed; log-posterior concave so Newton is
  safe; halving line search guarantees monotone ascent.
- Marginal variances by Hutchinson probes with refinements:
  (a) **control variate** `T diag(A)⁻¹ Tᵀ` (diagonal-Fisher through the
  mean-centering transform T) known in closed form, so probes only
  estimate the correction; (b) **deterministic probe signs**
  (splitmix-style hash); (c) variances computed *through the gauge
  transform* so reported stds are stds of the identified quantity.
  **CORRECTION (2026-09-06, execution attempt)**: (a) does NOT transfer
  to our solver as-is. For Rademacher z, z⊙(D⁻¹z) = D⁻¹ exactly
  (z_i² = 1), so a *diagonal* control variate cancels identically
  per-probe — zero variance reduction (proved and measured: max
  estimator difference 1.1e-16 on a planted n=400 SPD system). The
  trick is non-vacuous for nanojudge only because their baseline passes
  through T: T D⁻¹Tᵀ is non-diagonal, so the baseline co-fluctuates
  with the target while its expectation stays exact. Our
  `hutchinson_diag` estimates diag(L⁻¹) directly in gauge-pinned
  coordinates — no transform, nothing to cancel. The import becomes
  live only if we either (i) report mean-centered covariance (adopting
  their gauge), or (ii) use a non-diagonal control variate whose exact
  inverse diagonal is computable (e.g. a banded approximation of L).
  Production code deliberately unchanged.
- Matchmaking refits use a cheaper MLE (minorize-maximize BT with a
  ghost player) between full Laplace fits — a two-tier refit cadence.
- Their windowed matchmaking (sort by rating, consider only the ~100
  nearest opponents, tombstone linked-list for O(1) removal) turns
  pairing O(n log n); measured 100K items in ~50 ms. Our planner's
  effective-resistance scoring is more principled per-pair but scores a
  capped candidate list; the rating-window prefilter is the right
  candidate generator at large n (info gain decays fast in rating
  distance, so the window loses almost nothing).

### 3. Setwise stick-breaking PMF → Luce fold with degrees-of-freedom weights

Their lineup mode (k ≤ 9, single-letter labels A–I) asks for a full
ranking as k lines "First place is Option X…", then reads the letter
token of the first k−1 rank lines as **conditional distributions and
reconstructs the whole winner distribution by stick-breaking**
(Plackett–Luce): `q[letter] = residual · slot.dist[letter] /
Σ_unplaced`, last unplaced option absorbs the residual (parse.rs
`parse_lineup`). The k-vector q then folds into k(k−1)/2 edges via the
Luce ratio P(i beats j) = q_i/(q_i+q_j) — and, critically, each
surviving edge is weighted **df/m** where df = k−1 (logprobs) or 1
(text) and m = #edges that survived. One call's edges never count as
more than its actual information content. Cap at 9 exists precisely so
every label is one token; the template's placeholder letters (X, Y, Z,
W…) are drawn from the far end of the alphabet so they can never collide
with an option letter, and letter validation is size-aware ("D" is prose
in a 3-lineup).

Why it matters to us:
- Our setwise instrument (E6) asks for the full order in text — ordinal,
  no PMF. The stick-breaking read is the k-wise sibling of
  `ordinal_letter_v1`: cache-native (entities in the prefix), nonce-draw
  compatible, k−1 letter tokens each carrying its own PMF — continuous
  signal per rank vs our k(k−1)/2 hard ordinal lowerings.
- The df/m weight is a clean first-order answer to the shared-call
  correlation problem E1/E6 flagged; our current lowering weights are
  the analogous place to install it.
- Distinct-permutation gate: repeats or an incomplete trailing block
  throw out the whole judgement; a rank line putting zero mass on every
  still-unplaced option aborts the fold. Loud degradation, no invented
  mass — same doctrine as our abstain/off-alphabet accounting.
- Our 52-token alphabet could go further: encode (position, rung) or
  best–worst (most AND least ≈ 2(k−1)−1 df). Instrument candidates for
  the ladder.

### 4. The anchor machinery: fractional anchor_index + order-statistics prior

Top-heavy selection resolves a *rank boundary*, not "the top" —
`anchor_index = 9` means "certify the top ten", fractional values put
the boundary between ranks. Selection weight per item is the uncertainty
ratio min(A,1−A)/max(A,1−A) with A = Φ((μ_i − target)/√(σ_i² + σ_anchor²))
— the anchor's own variance widens the split, so an uncertain anchor
keeps neighbours in play, annealing as it firms. Then:

- **Order-statistics target blend**: early on, the observed leader's
  mean is unreliable (few edges; in binary mode wins never pin
  magnitude), so the target blends in the *prior-predicted* k-th order
  statistic of n draws from N(center, τ²) via Blom's plotting position
  Φ⁻¹((n − r + 0.625)/(n + 0.25)), weighted as `target_prior_edges`
  pseudo-edges against the anchor's real edge count. Prior-shaped
  exploration that fades automatically. Clever and portable to our
  planner's frontier targeting.
- **Proportional-fair coverage pull**: base weight / edge_count^c —
  resolved items shed weight instead of carrying stale "owed" edges.
- **Stop statistic**: ln P(every item on its posterior-favoured side of
  the *observed* anchor) = Σ ln max(A,1−A), independence deliberately
  optimistic-in-form but conservative-in-effect (side events positively
  correlated through the shared anchor ⇒ product underestimates ⇒ stop
  fires late). Documented trap they hit and fixed: measuring the stop
  against the *blended* target fires spurious stops — the exploration
  target and the certification target must be different quantities. Our
  frontier-inversion stop should keep that separation explicit.
- Matchmaking info gain integrates over rating uncertainty with the
  logistic-probit bridge κ = √(1 + π(σ_a²+σ_b²)/8) — closed-form
  "pairs that MIGHT be close are informative", exact fallback to the
  plug-in gain at σ = 0.

### 5. The zero-information paradox (an argument we should publish)

`docs/zero-information-paradox.md`: a single-elimination tournament's
win/loss *shape* has probability 1 before any judgement runs, so binary
outcomes carry exactly zero information about `max(strengths)` — only
identities move, never magnitudes; a 51/49 and a 99/1 edge record the
same win. Graded (logprob/ratio) verdicts break the paradox because the
margins carry the magnitude. This is the cleanest first-principles
argument for our entire cardinal program, written by a competitor whose
default mode is the binary one it indicts. Steal the argument (with
attribution) for llmsorting.com / FIRST_PRINCIPLES.md; our ratio ladder
and PMF rail are the constructive answer.

## CLI-layer alpha (from the full parse/rank/bench sweep)

- **Tokenizer-agnostic marker scanning**: verdict parsing concatenates
  all token strings into one flat byte buffer with an offset→token map,
  lowercases, and searches the *text* for "verdict"/"option" — so
  DeepSeek's `V+erd+ict` 3-token split, Gemma's `1+st`, and split
  `Opt+ion` anchors all parse identically; property-tested over 200
  random 1–4-byte chunkings. Keep-the-LAST-match handles prose mentions
  before the real verdict. Our letter instruments dodge this by
  construction (single-token alphabet), but any JSON/text rail we parse
  from multi-provider tokenizers should adopt the flat-buffer scan.
- **Spawn-order prefix harvesting on cancellation**: on Ctrl-C they
  collect results in spawn order, never completion order — harvesting
  only finished tasks would select for fast/short responses and bias
  the fit. Subtle integrity property our orchestrator's partial-run
  paths should honor.
- **Raw saved / transformed scored split**: JSONL stores raw verdict
  distributions; tempering is applied at scoring/load time so evidence
  can be re-tempered later. Convergent with our raw-trace + solve-time
  weighting doctrine — good to see independently derived.
- **Cumulative largest-remainder judge apportionment**: per-refit judge
  assignment targets `cumulative_total · weight − cumulative_assigned`,
  floor + largest fractional remainder — panel weights stay honest
  across uneven batch sizes. Neat mechanism if our multi-judge
  orchestrator ever schedules by weight.
- **Seeding discipline** (`--load-judgements`): text-hash identity keys,
  hard errors on logprob-mode mixing and unknown judges (with a
  "weight 0 to reuse without new work" hint), skips always reported,
  and a priming scoring pass so top-heavy selection sees loaded
  evidence before the first pairing. If seeded evidence already meets
  stop_confidence, the run stops before spending anything.
- **Bench harness ideas** for our synthetic battery: shuffle the item
  list handed to the engine so input order carries zero ground-truth
  signal (stable tie-breaks otherwise inflate early accuracy);
  `actual_tau2` vs `prior_tau2` as an explicit prior-misspecification
  knob; simulated logprobs as empirical frequencies from
  `samples_per_judgement` categorical draws (1 = one-hot, ∞ = true
  PMF — a dial between text mode and logprob mode); a real
  OpenAI-compatible axum server driving the shipped binary as a black
  box, with deterministic per-encounter seeds.

## Worth noting, smaller

- **Verdict tempering** q ← q^(1/T) per judge (log-odds ÷ T), default
  T = 3 when reasoning is on, 1 when off — their patch for post-reasoning
  logprob collapse (reasoning commits, verdict tokens go one-hot, docs/
  logprobs-problems.md). Our two-phase `ratio_letter_2p_v1` dissolves the
  collapse at the instrument level instead of deflating it statistically;
  our per-template gain calibration (bilinear fit) is the principled
  generalization of a hand-set T. But the *default posture* — never take
  a reasoning-mode one-hot verdict at face value — is right, and a
  per-judge fitted gain on the ordinal rail would give it to us without a
  magic constant.
- **"While X…" death knell** (docs): the contrastive concession commits
  the verdict before reasoning; unpromptable-away, a judge-selection
  property. A cheap JCB-adjacent probe: flag judgements whose reasoning
  opens with a concession and measure their flip/agreement rates
  separately.
- **Temperature jitter** (per-prompt multiplier N(1, s) clamped [0.8,
  1.2]): style diversity as poor-man's panel. Our nonce draws +
  portfolio theory cover the same need with actual measurement; skip.
- **min_logprob_coverage** per judge (trust a verdict only if ≥ 95% of
  verdict-token mass is visible in top_logprobs): our seriate
  visible/abstain/off-alphabet mass accounting is strictly richer; no
  import needed — but their per-judge threshold knob is a reasonable UX.
- **Benchmark presentation**: BIRCO/RELIC with pool dilution 1×→32×,
  showing pointwise Score-O collapsing (0.39→0.08 nDCG@10) while
  pairwise degrades gracefully (0.60→0.30). The dilution-curve format is
  compelling and cheap; a strong candidate shape for pairwiseratio.org
  (our anchor tiers + dilution ladders, judged at fixed budget).
- **Two-stage run structure named cleanly**: uniform floor
  (`min_uniform_edges` per item, staged random → nearest-neighbour →
  info-gain as min edge count hits 0/1/2) before any top-heavy
  concentration. Our planner mixes these concerns; the explicit staging
  is a legible discipline.
- **weight-0 judges** with `--load-judgements`: seed a run from prior
  evidence without assigning the judge new work. Our packets subsume the
  mechanics; the *affordance* (replay a judge's evidence while benching
  the judge) is worth exposing on the packet CLI.

## Where we are ahead (no action, for the record)

- Scale: they fit interval-scale win probabilities; we elicit ratio
  magnitudes with measured PMF variance. Their own zero-information doc
  is the argument for our side.
- Robustness: no outlier model at all — one deranged verdict enters the
  likelihood at full weight (tempering is global, not adaptive). We have
  Huber/IRLS, LOO leverage audits, planted-corruption pins.
- Diagnostics: nothing like Hodge curl/harmonic split, orbit transform,
  spectral identifiability, Foster residual, WST/MST/SST, JCB.
- Provenance: JSONL with truncated SHA-256 item hashes and honest
  "re-pass your verdict_temperature or the ranking won't reproduce"
  caveats vs content-addressed packets with bitwise-deterministic fusion.
- Panel theory: static config weights + jitter vs measured error
  covariance, GLS weights, marginal information per dollar.
- Elicitation instruments: one prompt family vs a typed instrument
  catalog with wording-invariance probes, two-phase PMF reads,
  nonce-draw repeat pooling, cache-native geometry (measured discounts).
- Their bench is synthetic-only ground truth + one public IR task; ours
  carries live evidence packs with denominators.

## Proposed routing

1. **Slot-bias term in the solver** (fixes E1 pivot halo; enables
   adaptive counterbalance shedding) — solver change, small, high value.
2. **Hutchinson control variate + deterministic probes + windowed
   candidate generation** — the large-n path, staged behind the existing
   planner cap.
3. **k-wise winner-letter PMF instrument** (`most_letter_v1`?) with
   Luce fold + df/m lowering weights — new instrument on the seriate
   rail, E-ladder entry.
4. **COMPARISON.md**: add a nanojudge row/section (nearest-neighbour
   tool, closer than llm-sort; binary+logprob BT vs ratio+robust fit).
5. **FIRST_PRINCIPLES.md / llmsorting.com**: absorb the zero-information
   tournament argument with attribution.
