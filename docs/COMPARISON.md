# How llmsorting relates to the rest of the field

The LLM-ranking ecosystem almost universally elicits **binary or ordinal**
preferences ("is A better than B?", "rank these 20") and then either uses the
order directly or fits an **interval-scale** latent strength from binary
outcomes (Bradley–Terry / Elo). llmsorting differs on three axes at
once:

1. It elicits **ratio-magnitude** judgements ("how many times more of X does
   A have than B?") — the Analytic Hierarchy Process / magnitude-estimation
   tradition, not the IR-reranking tradition.
2. It fits **ratio-scale scores with per-item uncertainty**, robustly
   (IRLS + Huber) over the whole comparison graph.
3. It selects the next pair **actively** and stops when the top-k boundary is
   settled within a stated budget.

Lineage in one line: AHP's ratio judgements crossed with LMArena's statistical
discipline (uncertainty + active sampling), delivered as a Rust library/CLI
with a SQLite cache and full cost accounting.

A scale distinction worth being precise about: *ordinal* gives order only;
*interval* (Bradley–Terry, Elo, Thurstone, Rank Centrality) makes differences
meaningful but not ratios — Arena "scores" are interval, you cannot say "A is
twice as good"; *ratio* scales support exactly that claim, and are only
obtainable when the elicitation itself carries magnitude. That is the bet this
repo makes — and note the honest caveat: our checked-in studies show the bet
paying off on some regimes and losing on others (see
[EVALUATION.md](EVALUATION.md)). Ratio elicitation is strictly more
informative *when the judge can actually provide it*; whether a given model
can, for a given attribute, is an empirical question. Our own test battery
pins a regime where the bet loses: under heavy noise and outlier pressure,
direction-only ordinal judgements are more robust than ratio magnitudes
(`tests/method_dominance.rs`). Magnitude is extra signal and extra attack
surface; the honest position is to measure which one your judge provides.

## Prompting regimes

| Regime | Representative work | Shape | Scale | Calls for n items | Characteristic failure |
|---|---|---|---|---|---|
| Pointwise / scalar | "rate 1–10", Likert, query likelihood | item + rubric → score | cardinal in name only | O(n), cheapest | miscalibration; clustering near 7–8; scores not comparable across calls |
| Pairwise preference | PRP ([Qin et al. 2023](https://arxiv.org/abs/2306.17563)) | 2 items → "A"/"B" | ordinal (binary) | naive O(n²); sort/window variants O(n log n)/O(n) | intransitivity; position bias; magnitude discarded |
| Listwise | RankGPT ([Sun et al. 2023](https://arxiv.org/abs/2304.09542)), [RankVicuna](https://arxiv.org/abs/2309.15088), [RankZephyr](https://arxiv.org/abs/2312.02724), [RankLLM](https://arxiv.org/abs/2505.19284) | k items → permutation | ordinal | ~O(n) via sliding window (20/10) | context limits; order sensitivity; no scores, no uncertainty |
| Setwise | [Zhuang et al., SIGIR 2024](https://arxiv.org/abs/2310.09497) | c items → "which is best" | ordinal | between pairwise and listwise | still comparison-only output |
| Tournament / knockout | single-elimination, round-robin + BT | bracket | ordinal → interval if fed to BT | O(n) knockout … O(n²) round-robin | knockout is fragile to a single noisy judgement |
| **Pairwise ratio (this repo)** | AHP tradition, LLM-adapted | 2 items → ratio on a fixed ladder | **ratio, with posterior std** | default budget 4·n, actively allocated | costs more per call than scalar; requires a coherent attribute |

None of the mainstream regimes ask "how many times more". That question is
this repo's entire reason to exist.

## Aggregation math

| Method | Input | Output scale | Per-item uncertainty | Active selection |
|---|---|---|---|---|
| Bradley–Terry (MLE) | binary wins | interval (log-odds) | bootstrap / Hessian CIs | not intrinsic; [LMArena](https://arxiv.org/abs/2403.04132) bolts it on |
| Elo (online) | sequential binary | interval | weak | no; order-sensitive, superseded by BT in Arena |
| TrueSkill | game outcomes | Gaussian skill (μ, σ) | native σ | matchmaking uses σ (implicit) |
| Thurstone (Case V) | binary | interval | variance | no |
| [Rank Centrality](https://arxiv.org/abs/1209.1688) | comparison graph | stationary distribution | theory bounds, not per-item CIs | no |
| AHP (Saaty eigenvector) | **ratio matrix** | **ratio** | global consistency ratio only | no — wants all n² comparisons |
| **This repo (IRLS + Huber on log-ratios)** | **ratio + optional measured precision** | **ratio (log-space latent)** | posterior std per item + top-k boundary error | effective-resistance planner + certified stop |

The active-ranking literature (LMArena's uncertainty-weighted sampling,
[active top-k aggregation](https://proceedings.mlr.press/v70/mohajer17a.html),
budgeted pairwise ranking) is mature in theory and nearly absent from LLM
tooling; the planner and stopping rule here sit squarely in that line.

## Query-relevance rerankers are a different job

Cohere Rerank, Voyage, Jina, BGE cross-encoders, FlashRank, and the
[`rerankers`](https://github.com/AnswerDotAI/rerankers) library answer *"how
relevant is this document to this query"* in one pass — no criterion-based
magnitudes, no uncertainty, no active selection, and no need for them: it is a
retrieval problem. Same word ("rerank"), different problem. If you need
query→document relevance at scale, use one of those; if you need "sort my
shortlist by how much of X each item has, and tell me how sure you are," use
this.

## The nearest tool: nanojudge

[`nanojudge`](https://github.com/nanojudge/nanojudge) (MIT, Rust, hosted
at nanojudge.ai; mined 2026-09-05, notes in
`research/notes/nanojudge-mining-2026-09-05/`) is the closest system to
this repo we know of: adaptive pairwise/k-wise LLM judgements fed into a
Bradley–Terry fit with per-item credible intervals, uncertainty-aware
matchmaking, and an early stop. The load-bearing difference is the
**scale of the elicitation**: nanojudge elicits win probabilities
(interval scale — logprob mass on a verdict token, or a one-hot text
verdict), while this repo elicits ratio magnitudes (ratio scale). Their
own `docs/zero-information-paradox.md` makes the cardinal argument
crisply: binary tournament outcomes carry zero information about *how
much* stronger the winner is; only graded verdicts do.

| | nanojudge | llmsort |
|---|---|---|
| Judgement | P(A beats B): verdict-token logprobs or text winner | log-ratio on a fixed ladder; single-token PMF rail |
| Model | Bradley–Terry + per-judge per-slot bias, Laplace posterior | robust log-linear fit (IRLS + Huber), gauge-pinned |
| Position bias | fitted in the likelihood (β per judge/slot, CI) | counterbalanced per pair + orbit-transform diagnostics |
| Outliers | none (global verdict tempering only) | Huber weights, LOO leverage audit |
| Selection | focal-item anchor weights + windowed info-gain (O(n log n), 100K items) | effective-resistance planner (dense, capped) |
| Stop | P(all items on their side of a rank anchor) | top-k frontier inversion error + certified separation |
| k-wise | stick-breaking Plackett–Luce PMF → Luce edges, df-weighted | setwise full-order text lowering + flip-rate gauge |
| Diagnostics | judge bias CI, panel bias | Hodge curl/harmonic, spectral, WST/MST/SST, JCB, portfolio |
| Provenance | JSONL + item text hashes | content-addressed packets, bitwise-deterministic fusion |

The two designs are convergent on much of the operational layer
(uncertainty-aware matchmaking, refit cadence, evidence seeding,
raw-saved/transformed-scored). What we adopted from the mining pass and
what flows the other way is itemized in the research note.

## The other nearby tools: llm-sort and gwern's seriate.py

[`llm-sort`](https://github.com/vagos/llm-sort) (an `llm` CLI plugin,
[reviewed by Simon Willison](https://simonwillison.net/2025/Feb/11/llm-sort/))
is the lightest general-purpose "sort an arbitrary list by a criterion" CLI
we know of. It feeds binary pairwise judgements into a comparison sort
(`sorted(cmp_to_key(...))`). Same user intent, opposite engineering
philosophy:

| | llm-sort | cardinal sort |
|---|---|---|
| Judgement | binary "which line is better" | ratio ladder; self-reported confidence is trace metadata |
| Aggregation | comparison sort trusts every answer | robust global fit; outliers down-weighted |
| Intransitive judge | thrashes (sort assumes transitivity) | modeled — cycles become residuals |
| Output | reordered lines | reordered lines + mean ± std, z, percentile |
| Uncertainty / stop | none | top-k error estimate, certified stop, budget |
| Cost accounting | none | per-run totals; per-comparison trace; SQLite cache; keyless replay |
| Weight | tiny Python plugin | a Rust engine; heavier by design |

If you just need a quick plausible ordering and don't care about auditability,
llm-sort is less machinery. The moment "how much better?" or "how sure are
we?" matters, the machinery is the point.

[`seriate.py`](https://github.com/gwern/gwern.net/blob/master/build/seriate.py)
(Gwern Branwen, CC-0, ~200 lines; used to order the "similar links"
sections on gwern.net) solves an adjacent but distinct problem:
**seriation**, not criterion sorting. There is no explicit criterion and no
pairwise elicitation at all — the whole list is handed to the model, which
rewrites it into "a best-effort context-dependent logical order" (cluster
similar items, minimize item-to-item distance), iterated to a fixed point
(max 5 passes, cycle detection), then gated by a word-multiset permutation
check so the "sort" provably lost nothing. Sharp contrasts with `cardinal
sort`:

| | seriate.py | cardinal sort |
|---|---|---|
| Question | "what order is natural?" (criterion implicit, chosen by the model) | "how much X does each item have?" (criterion explicit, caller-owned) |
| Elicitation | whole-list rewrite, one completion per pass | O(n·budget) pairwise ratio judgements |
| Output scale | ordinal at best — no scores, no uncertainty | ratio scores, mean ± std, top-k certification |
| Convergence | fixed point of a rewrite map (may cycle; errors out) | posterior uncertainty under a stated budget |
| Integrity check | permutation-of-words gate (lossless reorder) | per-judgement trace, cost accounting, keyless replay |
| Cost shape | cheap: ~passes × list tokens | linear in comparisons; priced per run |

The two compose rather than compete: seriation is the right tool when the
list's own structure should pick the order (link lists, notes, galleries)
and any defensible clustering beats an arbitrary one; cardinal is the right
tool when a caller-owned criterion, magnitudes, and audit trail matter.
seriate's permutation gate is also a pattern worth stealing anywhere an LLM
is trusted to reorder content it must not rewrite.

## Known pathologies and what this design does about them

| Pathology | Evidence | Design answer here | Status |
|---|---|---|---|
| Position bias | [Judging the Judges](https://arxiv.org/abs/2406.07791) | both presentation orders per pair (counterbalanced, default on the sort surface) with a measured flip rate; randomization still available | implemented; live study shows 21.6% flips on a real task |
| Scalar miscalibration / clustering | [judgment-distribution work](https://arxiv.org/abs/2503.03064) | ratio elicitation avoids absolute scales entirely | by construction; comparisons vs Likert are mixed |
| Context limits | listwise sliding windows | two items per call, always | by construction |
| O(n²) pairwise cost | PRP's own motivation | active planner, default 4·n budget, top-k focus | implemented; studies do not yet prove early stopping |
| Intransitivity | [non-transitivity](https://arxiv.org/abs/2502.14074), [LLM-RankFusion](https://arxiv.org/abs/2406.00231), [TrustJudge](https://arxiv.org/abs/2509.21117) | latent-score model fits *through* cycles; Huber loss discounts outliers | implemented |
| Baseline / anchor dependence | fixed-baseline comparisons drift | full comparison graph, global fit, gauge pinning | implemented |
| Attribute incoherence (phrasing- or polarity-sensitivity) | [TrustJudge](https://arxiv.org/abs/2509.21117)-style score/comparison inconsistency | `--two-sided` (opposite of the criterion, weight −1) and `--also-by` (paraphrases) probes with cross-attribute rank-consistency diagnostics | implemented; live study caught a shaky paraphrase (+0.35) |

"Design answer" is not "proven win" — the honest state of the evidence lives
in [EVALUATION.md](EVALUATION.md) and the checked-in study packs under
`research/artifacts/`.
