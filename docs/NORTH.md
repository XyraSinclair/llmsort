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
- **Two-phase where depth is needed**: reason at effort-medium, read the
  one-token PMF at effort-none with the analysis in context — measured
  evidence-tracking, not verdict-copying (LOGPROBS.md).
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
   JSON path demoted to explicit fallback.
4. The deletion campaign, one room at a time, blind-defended
   (parsimony law); ceilings ratchet down as rooms shrink.
5. Reliability reading enters the promised surface (sort output +
   packet v2).
