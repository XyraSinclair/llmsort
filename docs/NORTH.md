# NORTH — the 10× core

Operator decree (2026-08-19): the core an order of magnitude simpler and
more powerful — exploit every opportunity with logprobs and the prompt
cache in ultra-smart ways, do rapid structured judgements over diverse
similar and opposite attributes, and robustly measure how reliable LLMs
are at judging things by attributes.

This document is the derivation: what the primitive is, why the current
core's complexity is mostly a subsidy for the wrong primitive, what gets
deleted, and the measured gates each step must pass. Nothing here is done
until its pack exists.

## The primitive

The single-token ratio judgement (`seriate::instrument::ratio_letter`,
already in-tree): the 52-letter ratio ladder as the FIRST completion
token, so one token position's logprobs ARE the posterior — a full PMF
over the ratio ladder, E[log-ratio] + honest variance + refusal mass from
ONE output token. No JSON to parse, no confidence field to trust
(`docs/LOGPROBS.md` holds the measured provider matrix: who serves
logprobs, the reasoning gate, the top-k caps, the loud-degradation rule).

## The cost law

Prompt = [cacheable prefix: system + entity pair] + [tail: attribute].
Measured (LOGPROBS.md): warm prefixes hit 12/12 and 5/5 on the two rails,
cached input at 10–19% of fresh price; output is one token. Therefore the
marginal cost of an EXTRA attribute variant on an already-judged pair is
approximately the tail tokens — near zero.

This inverts the economics the current core was built for. Active-pair
agonizing, budget gymnastics, decimal ledgers, JSON parsing and repair —
that is complexity purchased to economize multi-token elicitation. When a
judgement costs ~one output token and the pair-prefix is cache-shared
across the whole attribute family, most of that machinery loses its
reason to exist.

## The native unit of work: the family sweep

For each pair, over one cached prefix, judge by:
- the attribute A,
- a paraphrase A′ (must correlate),
- the negation ¬A (must anti-correlate),
- both presentation orders (must anti-symmetrize),
- (null pairs: identical items, must read ratio 1).

From the SAME calls, two outputs at once:

1. **The scaling** — fuse the PMFs (+A, +A′, −¬A) through the solver
   (`rating_engine`, unchanged: the IRLS/Huber/Hodge math IS the value).
2. **The reliability reading** — paraphrase stability ρ(A,A′), polarity
   ρ(A,¬A), order invariance + reciprocal residual, null calibration,
   cycle frustration. This is the Judge Coherence Benchmark's insight
   promoted from side-benchmark to standard output: every sort ships
   with the judge's measured reliability on THIS attribute over THESE
   items. "How reliable is the LLM at judging this?" stops being a
   research question and becomes a field in the result.

Reliability without ground truth comes from relations between answers —
and the family sweep buys those relations at cache-discounted, one-token
prices. Simpler and more powerful are the same move.

## Where the 10× lives (deletion ledger — indicative, each line needs
its blind defend before it dies)

| Room | Today | Fate |
|---|---|---|
| `rerank/` | 9,575 lines: multi-orchestrator, JSON comparison + repair, decimal ledger, budget machinery | shrink to a thin executor + gates frame; the economics it manages mostly stop existing |
| `trait_search/` | 1,774 lines of active-selection | dense designs + solver do the work at one-token prices; keep top-k stopping |
| `gain_calibration`, parts of `censored_likelihood` | tuned to JSON ratio parsing | re-derive on PMF evidence or delete |
| `prompts.rs` JSON templates | canonical_v2 et al. | one letter template + family expansion |
| `seriate/` | vendored, optional | becomes THE elicit room |
| `rating_engine/`, `packet`, `cache` | | keep whole (solver, identity, replay) |
| `gateway/` | 4,391 lines | slimmer; logprob matrix + cache-key routing become first-class citizens |

Target: core ≤ ~6k lines from ~19k, with MORE capability surfaced
(reliability reading in every result).

## Laws that survive

- **Loud degradation**: where the matrix says no logprobs, the PMF comes
  from resample-K — E8 whitespace-jitter probes give distinct, cached,
  replayable draws (`experiments/src/probes.rs`); `PmfCompleteness` says
  which mode produced the evidence. One currency throughout.
- **Pair prefix, not set prefix**: E1 measured the pivot-halo pathology
  (795/870 ratios < 1, rotation flips 80–93% of directions) — k-wise
  prefixes stay experimental until that is solved. The measured-safe
  shape is pair-in-prefix, attribute-in-tail.
- **Two-phase where depth is needed**: elicit a written analysis with
  the verdict token forbidden AND reasoning off — hidden reasoning
  behind the analysis is measured verdict jitter, not consistency
  (sigma-eps-knobs pack) — then read the one-token PMF at effort-none
  with the analysis in context; the read tracks the analysis text, not
  a copied verdict (LOGPROBS.md).
- Packet identity, cost truth, calibration honesty: unchanged invariants.

## Migration spine (every step lands green with a pack; no step depends
on a later one)

1. **E9 — head-to-head**: ratio_letter_v1 vs canonical_v2, fixed corpus,
   logprob-serving model: cost/judgement, agreement, and per-template
   family-sweep reliability axes. The decree's premise, measured.
2. **E10 — family sweep instrument** (`cardinal family`): pair ×
   {A, A′, ¬A} × orders over a shared prefix; report cached-token
   fraction, pairwise-equivalent obs/$, and the reliability reading.
   Mechanical gate, found 2026-08-19: `ratio_letter`'s user prompt
   renders attribute FIRST, entities after — zero shared prefix across
   attribute variants. E10 starts with an attribute-LAST prompt variant
   (new slug; same alphabet, same PMF read) so the pair-prefix caches.
   E10 measured (2026-08-29): the caching is real (38.5% cached input,
   −29% cost on long entities) but the attr-last shape roughly halves
   truth accuracy and drops paraphrase coherence — the judge attends
   worse when the question follows the entities. The cache-native
   prompt shape is a measured trade, not a free lunch; recovering
   attr-first accuracy at attr-last prices is E10's open problem.
3. Flip `sort` to the PMF rail by default where the matrix allows;
   JSON path demoted to explicit fallback. **LANDED 2026-08-29**: a
   `None` template now resolves per model (`default_template_slug`) —
   `ratio_letter_v1` wherever the measured logprob matrix
   (docs/LOGPROBS.md) serves answer alternatives, `canonical_v2`
   elsewhere; the seriate call clamps `top_logprobs` to the route's cap
   (over-cap on OpenRouter was a silent 200 + `logprobs:null` — the
   whole PMF lost) and pins reasoning off on the 5.x families. Live
   A/B on the default model (gpt-5.4-mini, 8 items × 32 comparisons,
   seed 7): PMF rail stat ±0.019 vs JSON ±0.521 at identical cost
   ($0.0128 vs $0.0123), order residual 0.031 vs 0.192 nats, cyclic
   2.9% vs 10.7%, frustration 0.029 vs 0.107 — E9's one flag
   (higher frustration on the letter rail) does not reproduce on this
   cell. Probes now inherit the criterion's instrument.
   Default judge moved to gpt-5.6-terra the same day (operator: stop
   defaulting to old models). Terra on the same 8×32 cell, PMF vs JSON:
   stat ±0.061 vs ±0.491, rank risk 0.95 vs 6.59, $0.034 vs $0.051 —
   but order residual 0.391 vs 0.120 nats, cyclic 21.6% vs 7.6%,
   frustration 0.216 vs 0.076, visible mass 0.89 at 5 alternatives.
   Reading: the reasoning-off single-token rail exposes terra's own
   inconsistency that 74 tokens of JSON-rail reasoning smooths over; the
   measurement is far more precise and the systematic axes are reported
   instead of hidden. **RESOLVED 2026-08-30 — the two-phase read is
   productized** (`ratio_letter_2p_v1`): turn 1 elicits a reasoned
   analysis with the verdict token forbidden, turn 2 re-sends the
   conversation with the analysis as an assistant message and reads the
   one-letter PMF at effort none. Terra, same 8×32 cell: cyclic 21.6% →
   2.4% (beats its own JSON rail's 7.6%), frustration 0.216 → 0.024,
   stat ±0.050, visible mass 0.97, order residual 0.391 → 0.277 nats
   (counterbalancing cancels it in the fit; it stays the dominant
   reported term), $0.124 vs $0.034. (That cell's 2.4%/±0.050 later
   proved a lucky draw — fresh same-config cells ran 12–26% cyclic, and
   the ±0.050 predates the honest-σ refit; the standing scoreboard is
   in the 2026-08-31 block below.) The default draws the line the
   matrix measures: reasoning-NATIVE families (logprobs only at effort
   none — 5.5/5.6) default to the two-phase read; families where unset
   effort also serves logprobs measured WORSE under it (5.4-mini:
   cyclic 11.7% vs 2.9%, frustration 0.117 vs 0.029 — its analyses
   inject noise) and keep the plain single-token rail; 4.1/4o plain;
   no route → canonical JSON. Each judge gets the instrument that
   measures its best self.
   **RESOLVED 2026-08-30 (slot-hetero pack) — the 0.277 "order
   residual" on the 2p rail is NOT position bias.** Full decomposition
   on the same corpus (28 pairs × 2 orders × 2 repeats through the real
   rail): global slot bias +0.013 nats (sign-unstable across corpora),
   per-pair slot-bias variance 0.0000 — within-call noise σε = 0.215
   nats/call covers the entire residual (noise-only prediction 0.243 vs
   0.277 observed at 16 pairs). Both symmetrization instruments (global
   slot-term fit, slot-name randomization) are dead: there is nothing to
   correct. Naming caveat: on this rail "syst order" in the run summary
   measures per-call analysis stochasticity (present at temperature 0),
   not systematic asymmetry. Counterbalancing stays the default — with
   zero bias it is exactly repeat-averaging plus a free noise gauge, and
   at equal cost it ties single-order repeats (measured ρ 0.938 vs
   0.970, theoretically identical error variance). The successor lever:
   per-call noise/signal = 1.06, so the PMF-internal variance the solver
   weights by understates true per-observation uncertainty — the honest
   σ needs a σ_w (context-noise) term, and repeat-averaging (cache-
   priced) is the cost knob. The JSON rail's lower residual (0.120) is
   lower per-call noise (σε ≈ 0.13), not better symmetry.
   **LANDED 2026-08-30 — the honest-σ refit**: σ_w = residual·√π/2 is
   estimated from the run's own counterbalance diagnostic (no constants,
   no knobs; conservative where real slot bias exists) and folded into
   every evidence observation's variance by an end-of-run re-ingest, so
   posterior stds carry it; `noise sigma_w` joins the error budget and
   `evidence_sigma_w` the meta. Un-counterbalanced runs have no
   estimator and stay un-widened, loudly None.
   **RESOLVED 2026-08-31 (sigma-eps-knobs pack) — the analysis turn's
   REASONING was the noise.** With `judge --draws` extended to the
   evidence rails (nonce through the real comparison path), a phase-1
   effort sweep on terra measured σε 0.260 (default) / 0.209 (low) /
   0.238 (minimal) / 0.181 (disabled); paired 8×32 sort cells across 3
   seeds: disabling analysis reasoning cut order residual 0.238 → 0.141,
   flips 14/48 → 6/48, rank risk 3.9 → 2.4, cost −13%, frustration
   0.185 → 0.113 (2/3 seeds). Luna unchanged within noise (σε 0.047 →
   0.060). The verdict-forbidden written analysis carries the 2p win;
   hidden reasoning on top adds only stochasticity. The 2p analysis call
   now pins reasoning off. Corollary measured at both ends: single-phase
   verdict reads are nonce-deterministic (σ_w 0.004 on 5.4-mini).
   Standing terra 2p scoreboard with the pin (means over 3 fresh seeds,
   8×32, no cache): order residual 0.141 nats, σ_w 0.125 nats/call,
   stat ±0.079 (posterior incl σ_w), cyclic 7–18%, flips 6/48, visible
   0.97–0.99, $0.105 — every axis better than default-effort's paired
   cells, and the reported σ is now believed.
   **CALIBRATED 2026-08-31 — reproducibility ≠ correctness, and the
   surfaces now say which is which.** 11 independent luna reruns of one
   cell measured 74% cross-run pairwise agreement where the posterior
   plug-in consistency line predicted 54–69%: posterior σ mixes the
   judge's REPRODUCIBLE expressed PMF spread (epistemic — it re-renders
   identically on rerun, errors included) with the σ_w noise that
   actually resamples (empirical rerun σ ≈ 0.3× posterior σ_diff). The
   consistency line now rescales posterior stds by κ = σ_w /
   obs_sigma_rms (`evidence_obs_sigma_rms` joins the meta) and predicts
   fresh-vs-this-run call reproduction as Φ(|gap|/(√2·κ·σ_diff)) —
   validated 66% predicted vs 74% measured on a fresh seed, residual
   conservatism from σ_w's own small-sample noise. Correctness stays
   with stat±/resolution; the line says "reproduce", never "right"
   (pack consistency-calibration-2026-08-31).
4. The deletion campaign, one room at a time, blind-defended
   (parsimony law); ceilings ratchet down as rooms shrink.
5. Reliability reading enters the promised surface (sort output +
   packet v2).
