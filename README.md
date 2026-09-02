# llmsort

[![CI](https://github.com/XyraSinclair/llmsort/actions/workflows/ci.yml/badge.svg)](https://github.com/XyraSinclair/llmsort/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/llmsort.svg)](https://crates.io/crates/llmsort)
[![docs.rs](https://img.shields.io/docsrs/llmsort)](https://docs.rs/llmsort)
[![license](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

You have a list and a fuzzy criterion: fifty grant proposals by
expected impact, a backlog by user pain, ideas by upside. An LLM can
judge these, but asked directly it gives answers you cannot trust:
"rate each 1–10" clusters at 7 with no error bars, and one-prompt
sorts drop items. llmsort asks the model many small pairwise questions
("how many times more of X does A have than B?") and fits the answers
into one consistent set of scores, so you get the order, the size of
every gap, and how sure the model is about each.

It spends each comparison where it buys the most information, stops
when the top-k is certain enough or the budget runs out, and reports
what the run cost: comparisons, tokens, dollars, stop reason, and an
optional per-judgement trace.

```console
$ llmsort sort ideas.txt --by "expected impact on retention"
```

## Install

```console
$ cargo install llmsort          # CLI
$ cargo add llmsort              # library
$ export OPENROUTER_API_KEY=...  # any OpenRouter model slug works
```

The default judge is `openai/gpt-5.6-terra`; the setwise path
(`--setwise`, below) defaults to the cheaper `openai/gpt-5.6-luna`, and
`--model` / `SortOptions.model` takes any OpenRouter slug. For scale: a
measured n=8 sort at the default 4·n budget is 32 comparisons ≈ $0.11
on the default judge (the `--no-cache` quality-gate cells in
`research/artifacts/live/sigma-eps-knobs-2026-08-31/`).

This repo is the one home of the whole effort; every earlier repo
(`cardinal-harness`, `ratiometer`, `llmsorting`, `llmsort-lab`,
`seriate`) redirects here or is grafted into this history. It keeps
three compartments of deliberately different polish:

| Compartment | Polish | Promises |
|---|---|---|
| the crate (root, [crates.io `llmsort`](https://crates.io/crates/llmsort)) | engineered | API stability, CI green, shape mandate |
| [`experiments/`](experiments/) | research code | compiles, tested, never published; instruments graduate into the crate only on evidence |
| [`research/`](research/) | the raw record | none — replayable evidence packs, dated notes, analysis scripts, kept honest rather than pretty |

If a link brought you here from `cardinal-harness`, `ratiometer`, or
`llmsorting`, this is where development continues.

## Why not just ask the model to sort?

| Approach | What breaks |
|---|---|
| "Rate each item 1–10" | Miscalibrated, anchor-dependent; scores cluster at 7–8; no error bars |
| "Sort this list" in one prompt | Position bias, context limits, silently dropped or hallucinated items |
| "Which is better, A or B?" over pairs | Ordinal only — throws away *how much* better; naive schedules cost O(n²) |
| Elo / Bradley–Terry over wins | Better aggregation, but still magnitude-blind and passive about which pair to ask next |

llmsort treats each ratio answer as a noisy log-space measurement, fits
latent scores over the whole comparison graph with a robust solver (IRLS,
Huber loss), reads uncertainty off the posterior, and plans the next
comparison by effective resistance on the graph. Default budget is 4·n
comparisons — O(n), not O(n²).

## CLI

```console
$ llmsort sort ideas.txt --by "expected impact on retention"
$ llmsort sort backlog.txt --by "user pain if unfixed" --top-k 5 --format csv
$ llmsort judge "plan A" "plan B" --by "execution risk"
$ llmsort judge @a.md @b.md --by "clarity" --spin     # does the belief survive framing?
$ llmsort judge @a.md @b.md --by "clarity" --orbit    # order × polarity × wording group
```

`judge` is the audit instrument: one pairwise reading, plus probes that
test whether the judgement is a *belief* (survives presentation order,
polarity, paraphrase, who's asking) or an echo of how you asked. The
probes report the invariant component and every named bias separately.

## Library

```rust,no_run
use std::sync::Arc;
use llmsort::gateway::NoopUsageSink;
use llmsort::rerank::{sort_texts, RerankExecution, SortOptions};
use llmsort::{Attribution, ProviderGateway};

# async fn demo() -> Result<(), Box<dyn std::error::Error>> {
let gateway = ProviderGateway::from_env(Arc::new(NoopUsageSink))?;
let execution = RerankExecution::new(Arc::new(gateway), Attribution::new("app::sort"));

let sorted = sort_texts(
    vec![
        "First essay...".into(),
        "Second essay...".into(),
        "Third essay...".into(),
    ],
    "clarity of explanation",
    execution,
    SortOptions::default(),
)
.await?;

for item in &sorted.items {
    println!("{:>2}. {:.3} ± {:.3}  {}", item.rank, item.latent_mean, item.latent_std, item.text);
}
println!("cost: ${:.4}", sorted.meta.provider_cost_nanodollars as f64 / 1e9);
# Ok(())
# }
```

## How it works

Five nouns: an **attribute** (any nameable dimension) over entities, each
holding a latent **magnitude** (only ratios are observable);
**instruments** (elicitation modes) emit **evidence** in one currency —
(E[log-ratio], honest variance) — which the solver fuses into a
**scaling**: every entity placed on a shared log-ratio scale with a
*reading* (magnitude ± uncertainty). A ranking is a scaling with the
spacing deleted. `docs/ALGORITHM.md` has the rationale; `docs/MODEL.md`
the observation model; `docs/WORKED_EXAMPLE.md` a full walkthrough.

## What is promised

The stability-promised surface is deliberately small: `sort_texts` /
`sort_documents` (library), their setwise siblings `sort_texts_setwise` /
`sort_documents_setwise`, the CLI `sort` and `judge` verbs, and the
judgement-packet format (`src/packet.rs` — content-addressed evidence
that fuses byte-identically). Everything else is exposed for composition
and may change shape.

Use the pairwise sort for list work where "how much better?" carries
information: prompts, research ideas, candidate plans, reviewer notes,
backlog items. Use the setwise sort when an adequate *order* under a
custom criterion is the bar — reranking search results, triaging a
queue — at roughly a third of the pairwise cost (CLI: `llmsort sort
--setwise`); it measures its own
trustworthiness first (the order-sensitivity gauge; thresholds and the
evidence in `src/rerank/setwise.rs` docs and PROGRAM.md E6). Do not use
either for deterministic rankings, scalar metrics, or attributes too
incoherent to compare.

### Choosing a method — measured, not asserted

Every row below was run head-to-head on the same pools, models, and seeds
(PROGRAM.md E12–E14; evidence packs in `research/artifacts/live/`):

| Method | Verdict | Use when |
|---|---|---|
| Pointwise "rate 0–100" (one item per call) | Cheapest per item, but scores collapse into tie blocks (down to 3 distinct values over 16 close-packed items, truth-ρ −0.19); top-k selection through a tie block is a coin flip | Never for top-k. Acceptable for a rough full-list ordering on a strong model when magnitudes and top-k don't matter |
| Single-call listwise ("paste the whole list") | The k=n special case of setwise minus the safety design: fine when the gauge would be clean, one malformed answer loses everything, hard-capped at 26 items | Quick one-shot sorts of small lists you'd eyeball anyway |
| Setwise (`--setwise`, ring k=8, repeats 2) | Matches the pairwise sort's own test–retest band at ~⅓ the cost; the flip-rate gauge is a measured one-sided screen (flip < 0.20 ⇒ ρ ≥ 0.64, every bad cell flagged); for n ≫ k use `rounds: 2` — at n=150 it lands within 0.02–0.07 of the pairwise ceiling at ~⅙ its cost | Default for adequate orders: reranking, triage, queues. Precondition (E15): the gauge certifies pool-level order only — near-duplicates get arbitrary relative ranks (inside joint 2σ); read ±σ before trusting adjacent-pair distinctions |
| Funnel (setwise screen → pairwise `top_k` refine on the top-3k slice) | Brackets the pairwise path's own top-10 reproducibility at 0.3–0.6× its cost; pointwise screens disqualified (tie blocks silently drop up to 70% of the true top-10 at the slice cut) | "Best k of many" — but read the next row first |
| Pairwise ratio (`sort` default) | The flagship: cardinal scores ± σ, counterbalancing, certification. Its own top-10-of-150 reproducibility across seeds is 0.3–0.7 at the default budget — the honest ceiling every cheaper method is judged against | When magnitudes, error bars, or certification matter |

The cross-cutting rule: elicit with an instrument that measures its own
trustworthiness (gauge, counterbalancing, certification), and treat any
top-k claim without a stability number as unmeasured.

## Evidence and experiments

The trade is explicit: this costs more than one-shot scoring, saves
comparisons versus exhaustive pairwise judging, and returns uncertainty
plus evidence instead of only a sorted list. The research side lives
here too: [`experiments/`](experiments/) is a never-published workspace
crate with the experimental verbs, live batteries, and instruments whose
evidence is not yet in — an instrument graduates into the crate only
after its evidence earns it — and [`research/`](research/) is the
measured record itself: method comparisons, planner-regret benchmarks
(which the planner has lost and then won), judge-coherence batteries,
and every published number's replayable pack. [`PROGRAM.md`](PROGRAM.md)
indexes every method as a rung with its pack. The evidence culture
applies to our own planner first.

## Lineage

First `cardinal-harness`, then `ratiometer`, then `llmsorting`, now
`llmsort`. The parked crates.io names keep resolving; every former GitHub
name redirects here, and the full pre-extraction history (plus seriate's)
is grafted into this repo's ancestry, so `git log` reaches all the way
back.

## License

MIT.
