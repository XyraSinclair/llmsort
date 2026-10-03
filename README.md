# llmsort

[![CI](https://github.com/XyraSinclair/llmsort/actions/workflows/ci.yml/badge.svg)](https://github.com/XyraSinclair/llmsort/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/llmsort.svg)](https://crates.io/crates/llmsort)
[![docs.rs](https://img.shields.io/docsrs/llmsort)](https://docs.rs/llmsort)
[![license](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

You have a list and a fuzzy criterion: grant proposals by expected impact, a
backlog by user pain, ideas by upside. Direct 1–10 ratings cluster around 7 and
one-prompt sorts can drop items. llmsort instead asks many small pairwise ratio
questions, fits one consistent score scale, and reports the size and uncertainty
of every gap.

It spends comparisons where they are most informative, stops when the requested
top-k is certain enough or the budget runs out, and reports comparisons, tokens,
dollars, and the stop reason.

```console
$ curl -fsSL https://raw.githubusercontent.com/XyraSinclair/llmsort/main/install.sh | sh
$ export OPENROUTER_API_KEY=...
$ curl -fsSLO https://raw.githubusercontent.com/XyraSinclair/llmsort/main/examples/ideas.txt
$ llmsort sort ideas.txt --by "usefulness as startup advice" --scores
2.186±0.230	Talk to ten users before writing any code
1.883±0.178	Ship a rough version this week and fix it live
1.835±0.193	Charge money from day one
0.486±0.176	Rewrite the backend in a faster language
0.187±0.183	Add a dark mode
0.000±0.230	Design a logo before the product exists
```

Scores are natural-log magnitudes: a gap of 1.0 means about 2.7× as much of the
attribute in the judge's reading; ± is one standard deviation.

## Rows in, rows out

Results go to stdout in the input shape; progress and cost go to stderr. JSONL
and CSV rows keep every field and gain scores:

```console
$ llmsort sort grants.jsonl --by "expected good done per dollar" > ranked.jsonl
$ jq -r '"\(.llmsort.rank)  \(.title)"' ranked.jsonl
1  Deworming in Kenyan schools
2  Malaria nets for Kano State
3  Open-source vaccine cold-chain sensors
4  Office plants for a think tank
5  A podcast about the history of fonts
6  Luxury yacht refit for a donor retreat

$ llmsort sort backlog.csv --by "user pain if unfixed" --field title --field notes
id,title,notes,llmsort_rank,llmsort_latent_mean,llmsort_latent_std,llmsort_z_score,llmsort_percentile
1,Login fails on Safari,"Users cannot sign in; ""blank screen"" after submit",1,4.570811,0.454564,0.674491,0.900000
5,Payments double-charge,Some cards are charged twice,2,4.360191,0.433434,0.388191,0.700000
3,Export to CSV drops rows,Rows with commas vanish silently,3,4.074614,0.411624,0.000000,0.500000
4,Dark mode button misaligned,Off by 2px on settings,4,0.225386,0.436597,-5.232328,0.300000
2,Typo in footer,The word privacy is misspelled,5,0.000000,0.454564,-5.538700,0.100000

$ git log --format=%s -n 40 | llmsort sort --by "risk of breaking users" | head -n 5
```

- A `.csv`, `.jsonl`, or `.ndjson` filename selects that input; otherwise a
  leading `{` means JSONL, `[` means a JSON array, and anything else means one
  item per line. `--input lines|json|jsonl|csv` overrides detection.
- `--field` is repeatable and limits which JSONL keys or CSV columns the judge
  reads. Without it, the judge sees the whole row as `field: value` lines.
- JSONL gains an `llmsort` object. CSV gains `llmsort_rank`,
  `llmsort_latent_mean`, `llmsort_latent_std`, `llmsort_z_score`, and
  `llmsort_percentile`. Re-sorting replaces old scores before judging.
- `--format jsonl|csv` converts row shape, `--format text` prints what the judge
  read, and `--format json` prints the full run report.
- `--top-k 10` focuses the comparison budget on settling the top ten.

## Install

```console
$ curl -fsSL https://raw.githubusercontent.com/XyraSinclair/llmsort/main/install.sh | sh
$ cargo binstall llmsort   # the same prebuilt binary
$ cargo install llmsort    # build from source
$ cargo add llmsort        # use the library
```

The install script places a checksum-verified macOS or Linux binary in
`~/.local/bin`; Windows builds are on the
[releases page](https://github.com/XyraSinclair/llmsort/releases/latest).
`LLMSORT_INSTALL_DIR` and `LLMSORT_VERSION` override the destination and release.

The judge uses [OpenRouter](https://openrouter.ai), so set
`OPENROUTER_API_KEY`. Pairwise sorting defaults to `openai/gpt-5.6-terra`;
setwise sorting defaults to `openai/gpt-5.6-luna`. `--model` accepts any
OpenRouter slug. Judgements are cached locally, so an identical rerun does not
pay for the same calls again.

## Choosing a method

These methods were run head-to-head on the same pools, models, and seeds. See
[PROGRAM.md](PROGRAM.md) E12–E16 and the linked evidence packs for denominators
and limits.

| Method | What the measurements say | Use when |
|---|---|---|
| Pointwise 0–100 | Cheapest per item, but close pools collapse into tie blocks; one 16-item run produced three distinct scores and truth-ρ −0.19 | A rough full-list order where top-k and magnitudes do not matter |
| Single-call listwise | Fast for small lists, but one malformed answer loses the run and the implementation is capped at 26 items | A one-shot sort you will inspect manually |
| Setwise (`--setwise`) | At ring k=8 with two rounds, matched the pairwise path's test–retest band at about one-third the cost; the flip-rate gauge flags unstable pools | Adequate orders for reranking, triage, and queues |
| Funnel (setwise screen, pairwise top-k refinement) | Reached the pairwise path's own top-10 reproducibility at 0.3–0.6× its cost | Finding the best few of many |
| Pairwise ratio (default) | Returns cardinal scores ±σ, counterbalances presentation order, and targets the requested boundary | Magnitudes, error bars, or certification matter |
| Typed ten-level rating in a 24-item window | On hosted Jev, led 29 tested designs across four cohorts and every budget; it is not yet a crate instrument | Research with a typed-probability judge |

Setwise order is not evidence that near-duplicates differ: read adjacent error
bars before trusting their relative positions. The general rule is to choose an
instrument that measures its own trustworthiness and to treat any top-k claim
without a stability number as unmeasured.

## Judge one pair

```console
$ llmsort judge "plan A" "plan B" --by "execution risk"
$ llmsort judge @a.md @b.md --by "clarity" --spin
$ llmsort judge @a.md @b.md --by "clarity" --orbit
```

`judge` exposes one comparison and optional probes for presentation order,
polarity, wording, and requester framing.

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
    println!(
        "{:>2}. {:.3} ± {:.3}  {}",
        item.rank, item.latent_mean, item.latent_std, item.text
    );
}
println!("cost: ${:.4}", sorted.meta.provider_cost_nanodollars as f64 / 1e9);
# Ok(())
# }
```

The promised surface is `sort_texts`, `sort_documents`, their setwise siblings,
the `sort` and `judge` CLI verbs, and the content-addressed judgement-packet
format. Other public modules support composition but may change before 1.0.

## Method and evidence

Each ratio answer becomes a noisy log-space measurement. llmsort fits the
comparison graph with Huber IRLS, derives score uncertainty from the posterior,
and chooses new pairs by their expected value near the top-k boundary.

The cost trade is measured rather than assumed. On 72,813 private production
judgements, with public analysis scripts and no published rows, ratio magnitude
needed 1.3–1.8× fewer calls than direction alone at small budgets; probability
mass from answer logprobs added about 1.4× efficiency. At full measured budget,
direction-only sorting plateaued at split-half Kendall τ 0.16 while PMF moments
reached 0.49. Scope and method are in the
[analysis note](research/notes/logprob-efficiency-2026-09-05/FINDINGS.md).

[ALGORITHM.md](docs/ALGORITHM.md) explains the design,
[MODEL.md](docs/MODEL.md) states the mathematical contract, and
[WORKED_EXAMPLE.md](docs/WORKED_EXAMPLE.md) walks through a complete run.
[PROGRAM.md](PROGRAM.md) indexes every research claim by its replayable evidence
pack. `experiments/` contains instruments that have not graduated into the
published crate; `research/` holds dated notes, analysis, and evidence packs.

## License

MIT.
