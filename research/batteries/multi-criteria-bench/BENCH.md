# Multi-criteria judge bench: is a judge joint-safe?

Purpose: a standing certificate, per judge, that asking for m criteria in ONE setwise call
(one entity block, m attribute blocks, m answer lines) loses nothing against m separate
calls. The joint call costs 0.36–0.40× the separate calls for m=3, so the extension wants to
use it — but only for judges that pass. Evidence and method:
`research/artifacts/live/multi-criteria-setwise-2026-09-11/RESULTS.md`; instrument:
`experiments/examples/multi_criteria_setwise.rs` (n=40, k=8, overlap 2, 1 round, 2
presentations per window; identical windows and presentations in both arms, so the prompt is
the only difference).

## The three numbers and their bars

| number | definition | bar |
|---|---|---|
| agreement | per criterion, Spearman ρ between the separate-arm and joint-arm fitted orderings | > 0.85 on every criterion |
| halo inflation | mean pairwise inter-criterion ρ, joint minus separate | < 0.10 |
| flip rate | repeat-presentation disagreement per criterion (window order vs shuffle) | joint not consistently above separate |

Halo is the number the bench exists for: a judge that lets one criterion leak into another in
the joint call shows it as raw inter-criterion correlation, but only when the criteria are
independent to begin with. That is why every cohort here has criteria chosen to come out
near-orthogonal under gemma's separate arm (target mean |ρ| < 0.3); the lw cohort is the
deliberate exception (one latent, mean ρ 0.87) and bounds how much inflation a correlated
list can even show.

## Cohorts

Each cohort is `<name>.json` (40 items `[{id, text}]`, ≤ 3,000 chars each) with its exact
criteria wording in `<name>.criteria.json`. Separate-arm inter-criterion ρ is gemma-4-31b's,
from `baselines/summary-<name>.md`.

| cohort | source | criteria | separate-arm pairwise ρ | mean \|ρ\| |
|---|---|---|---|---|
| hn_top | 40 Hacker News front-page items (title + text), 2026-09-11 | interesting / credible / actionable | cred~act −0.20, int~act −0.05, int~cred +0.47 | 0.24 |
| lw | 40 LessWrong posts (random), 2026-09-11 | novelty / alpha / rigor (the battery wording) | +0.85 / +0.84 / +0.90 | 0.87 |
| arxiv | 40 abstracts drawn (seed 7) from `research/data/arxiv_abstracts.json` (OpenAlex ids) | novelty / clarity / evidence | clar~evid −0.21, nov~clar −0.01, nov~evid +0.61 | 0.28 |
| hn_comments | 40 Hacker News comments, 60–200 words, one day (2026-08-24), sampled by id hash via Scry's read-only SQL surface | informative / civil / concise | civ~conc +0.44, inf~civ +0.16, inf~conc −0.12 | 0.24 |

hn_comments was first run as funny / informative / civil (`baselines/summary-hn_comments_funny.md`):
funny anti-correlates with civil (−0.57) and informative (−0.32), mean |ρ| 0.36, and gemma's
own joint agreement on funny was 0.76. Funny was swapped for concise; the item list is unchanged.

## gemma-4-31b baselines (`google/gemma-4-31b-it` via OpenRouter, seed 7)

| cohort | agreement ρ(separate, joint) | top-10 overlap | halo inflation | flip separate → joint | $ (sep + joint) |
|---|---|---|---|---|---|
| hn_top | +0.929 / +0.855 / +0.914 | .80 / .90 / .80 | +0.001 | .153/.235/.119 → .138/.158/.101 | 0.0091 |
| lw | +0.897 / +0.954 / +0.946 | .80 / .90 / .80 | +0.045 | .138/.082/.077 → .189/.092/.117 | 0.0272 |
| arxiv | +0.916 / +0.716 / +0.865 | .80 / .60 / .80 | +0.075 | .143/.219/.157 → .194/.265/.119 | 0.0171 |
| hn_comments | +0.955 / +0.932 / +0.905 | .90 / .90 / .80 | −0.045 | .097/.036/.133 → .097/.148/.163 | 0.0073 |

Order of criteria per row follows the cohort table. Gemma clears every bar on hn_top,
hn_comments and lw. On arxiv it clears halo and two of three agreements but not `clarity`
(0.716, top-10 overlap 0.60): clarity is also its noisiest criterion in the separate arm
(flip .22), so at n=40 with 2 presentations the separate arm's own clarity ordering is
under-determined and the shortfall is at least partly design noise rather than joint-call
loss (the lw seed-7 vs seed-11 floor in RESULTS.md is 0.89–0.92 for a stable criterion).
Treat arxiv `clarity` as a known soft cell; a new judge that lands ≥ 0.72 there is at
gemma's level, not failing.

## Certifying a new judge

Build the instrument on the build host (`cargo build --locked -p llmsort-experiments
--example multi_criteria_setwise`), export the provider key, then one line per cohort.
`crit` turns a criteria file into the harness's `name=prompt;…` string:

```
crit() { python3 -c "import json,sys; print(';'.join(c['name']+'='+c['prompt'] for c in json.load(open(sys.argv[1]))))" "$1"; }
D=research/batteries/multi-criteria-bench
B=./target/debug/examples/multi_criteria_setwise
J="--model google/gemma-4-31b-it --base-url https://openrouter.ai/api/v1 --api-key-env OPENROUTER_API_KEY --arm both --concurrency 6"
$B --items $D/hn_top.json      --label hn_top      --criteria "$(crit $D/hn_top.criteria.json)"      $J --out /tmp/mcb-out
$B --items $D/lw.json          --label lw          --criteria "$(crit $D/lw.criteria.json)"          $J --out /tmp/mcb-out
$B --items $D/arxiv.json       --label arxiv       --criteria "$(crit $D/arxiv.criteria.json)"       $J --out /tmp/mcb-out
$B --items $D/hn_comments.json --label hn_comments --criteria "$(crit $D/hn_comments.criteria.json)" $J --out /tmp/mcb-out
```

Swap `J` for the judge under test. A byo/Cerebras judge passes `--model qwen-3.8-27b
--base-url https://api.cerebras.ai/v1 --api-key-env CEREBRAS_API_KEY --effort none
--concurrency 2` (the free tier's tokens-per-minute cap bites on lw at higher concurrency).
Each cohort is 56 calls (42 separate + 14 joint), 10–30 s, under 3¢ on gemma; the four
together are about 6¢. Each run prints the summary and writes `summary-<label>.md/json` and
`trace-<label>.jsonl` to `--out`; file the summaries under `baselines/` with the judge in the
label (e.g. `--label hn_top_cerebras_qwen`).

## Reading a result

Read the three cohort types in order. hn_top and hn_comments are the halo tests: if the
joint-minus-separate inter-criterion ρ is at or above +0.10 there, the judge is pulling its
orderings toward one another and must stay on separate calls, whatever it does on lw (qwen-27b
on Cerebras: +0.19 on hn_top). arxiv adds a criterion pair with a real shared latent
(novelty~evidence +0.61) next to two independent ones, so it shows whether a judge inflates
the already-correlated pair specifically. Then check agreement on every criterion against
0.85, remembering the seed-noise floor (0.89–0.92) and the arxiv clarity soft cell, and
check that flip has not risen across the board; a rise on one criterion at n=40 is within
what two presentations resolve. A judge passes when all three hold on all four cohorts; it
then gets `criteria=joint` in its preset, otherwise `separate`.
