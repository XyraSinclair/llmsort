# What this is good for, why, and how

The one-page version, for sharing. Every claim below has evidence in this
repository—a test, a checked-in artifact from a live run, or both.

## What we are building (locked 2026-08-12)

The instrument that turns an LLM's felt sense into calibrated measurement.
Sorting is the demo; measurement is the product. The ontology is five nouns:

1. **Attribute** — any nameable dimension, vibes included.
2. **Magnitude** — the latent positive quantity each entity has of it. Only
   ratios of magnitudes are observable; the absolute scale is gauge freedom.
3. **Instrument** — any elicitation mode that yields evidence about magnitude
   ratios (ratio letters, ordinal reads, decimal ledgers, …). Instruments are
   diverse and swappable; none touches the solver directly.
4. **Evidence** — the one currency every instrument must emit:
   (E[log-ratio], honest variance). This is why the instrument zoo stays
   commensurable instead of becoming a mess.
5. **Scaling** — the output. A **ranking is ordinal; a scaling is cardinal**.
   A ranking is a scaling with the spacing deleted. Each entity in a scaling
   carries a **reading**: magnitude ± uncertainty on the shared log-ratio
   scale. A reading keeps the gaps a ranking deletes.

The precedent is star magnitudes: astronomy's rank buckets ("first magnitude
stars") became a measurement system when Pogson fixed the ratio step (2.512×)
in 1856 — composable, comparable across instruments, indefinitely extensible.
The ratio ladder is that move, generalized to any attribute.

Why cardinal is worth the extra machinery: gaps are where decisions live
(prioritization needs "10× more valuable", not "ranked above"); ratios
compose along paths, so sparse comparisons cover a list and yield free
consistency checks; and magnitudes on a shared scale transfer — readings
accumulate into a web of priors (the OpenPriors endgame) instead of every
sort starting from zero.

## What it's good for

Sorting and ranking **short lists of things by judgement calls** — the work
you'd give a careful human with taste: prompts, research ideas, candidate
plans, tweets, abstracts, backlog items. Anywhere "which is better, and by
how much?" is the real question and the list is dozens-to-hundreds, not
millions.

It is NOT for: query-to-document retrieval at scale (use a reranker API),
deterministic metrics (just compute them), or attributes too incoherent
for two judges to agree on (the tool will tell you when that is
happening; see probes; it cannot fix it).

## Why it's good

1. **It asks a stronger question.** Not "rate this 1–10" (miscalibrated,
   evidence in `docs/EVALUATION.md`) and not "which is better" (throws away
   magnitude) — but "how many times more?", whose answers compose additively
   in log space and over-determine a global fit.
2. **It reads the model's whole prior, where possible.** With
   `--template ratio_letter_v1`, one completion token's top-k logprobs are
   the full judgement distribution. Measured effect: ~3× the resolving
   power per dollar versus sampled JSON at equal budget
   (`research/artifacts/live/evidence-path-2026-07-04/`). Where providers hide
   logprobs, it degrades to sampling and says so in the run summary.
3. **It treats every claim about a judge as an empirical question.**
   Position bias: measured per run (order flips + order-residual in nats).
   Pure elicitation artifacts: measured per model (`cardinal calibrate`,
   null pairs — four models measured clean). Attribute coherence:
   measured per attribute (`--two-sided`, `--also-by` probes; a live probe
   caught a shaky paraphrase at +0.35). Logprob trustworthiness: measured
   per provider (the logprob reality map,
   `research/notes/logprob-reality-2026-07-04/`: DeepSeek's logprobs disagree with
   its own sampling at JSD 0.81).
4. **It applies the same skepticism to itself.** The planner's efficiency
   claim was benchmarked against uniform random pair selection, FAILED,
   got fixed (anchor-diverse exploration), and the fix cycle is pinned
   two-sided in `tests/planner_regret.rs` with history. The test suite is
   adversarial; honest negatives are kept, dated, and load-bearing.
5. **Everything is accounted for.** Comparisons, tokens, dollars, stop reasons,
   evidence health, and per-judgement traces that bind the exact solver input
   to a content-addressed engine configuration. SQLite supports zero-provider-call
   reruns when the cache is present, and judgment packets (`src/packet.rs`,
   emitted via `--packets-out`) make raw pairwise evidence content-addressed
   and portable — packets fuse byte-identically, so merging the same evidence
   twice is a no-op. A portable bundle for the *full study record* remains the
   explicit #52 gap. The seriate instrument layer (single-token evidence,
   content-addressed judgement records) lives in `src/seriate/`, folded back
   from the standalone crate 2026-08-11 (`research/notes/seriate-fold-2026-08-11.md`).

## How to use it

```bash
cargo install llmsort --locked
export OPENROUTER_API_KEY=...
# or, with a Claude Code subscription and no API key:
#   --model claude-code/<model>  (21/21 decisive-pair agreement with the
#   API rail at $0 marginal; research/notes/claudecode-vs-api-2026-08-06/RESULTS.md)

# What will this cost? (no network)
llmsort sort ideas.txt --by "expected impact" --estimate

# Sort (pairwise ratio, both orders, active planner, σ per item):
llmsort sort ideas.txt --by "expected impact" --scores

# Bigger list, order is enough (~1/3 the cost, flip-rate gauge on stderr):
llmsort sort ideas.txt --by "expected impact" --setwise

# Best 10 of many (setwise screen → certified pairwise refine):
llmsort sort ideas.txt --by "expected impact" --setwise --top-k 10

# Sharpen the criterion, probe whether it coheres, sort with full-PMF evidence:
llmsort sort ideas.txt --by "expected impact" --elaborate --two-sided \
  --also-by "how much this would move retention" --template ratio_letter_v1

# One pair, audited for framing bias:
llmsort judge @a.md @b.md --by "clarity" --orbit

# Multiple objectives? The response carries the Pareto front and the
# attribute correlation matrix — see docs and the multi_rerank API.
```

The install command pulls the released crate from crates.io; a source
install of current `main` is
`cargo install --git https://github.com/XyraSinclair/llmsort --locked`.
Research verbs (`cardinal calibrate`, `cardinal explain`, the ordinal-letter
instrument) live in the `experiments/` crate and are not part of the
published surface. The method chooser and cost calculator at
<https://llmsorting.com/methods.html> pick the flags for a given list.

Every run prints its evidence summary: comparisons, cost, order flips,
evidence health, stop reason. When something would be uninformative, it refuses
loudly instead of printing garbage.

## Boundaries, stated plainly

- Evidence quality varies by provider; the reality map tells you where
  logprobs are real. Reasoning-class models refuse them outright.
- The synthetic evaluation suite shows regimes where simpler instruments
  (ordinal, even Likert) win — kept, pinned, documented. Ratio elicitation
  is a bet whose payoff depends on the judge and the attribute; the probes
  exist so you can check YOUR case instead of trusting ours.
- Dense solver: comfortable at hundreds of items, not tens of thousands.
