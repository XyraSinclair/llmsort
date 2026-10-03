# Judge Coherence Benchmark — live leaderboard (2026-07-05)


## Erratum (2026-07-05 test–retest)

The finding "nano fails polarity in SIGN (+0.38)" did NOT replicate: a
fresh identical run measured −0.40. The stable claim is that nano
cannot produce a reproducible polarity relation at all (±0.4 swing at
temperature 0, vs ≤0.10 for every other model). See
`../judge-bench-retest-2026-07-05/`.

Six models, 114 comparisons each, canonical_v2 point instrument, fixed
public corpus (8 aphorisms × "depth of insight about living well" + its
negation + a paraphrase), full battery per `research/notes/BENCHMARK.md`. Total cost
**$0.46**. Raw per-call receipts: `reports.jsonl` (one line per model,
every judgement included). Console stats blocks: `run.stderr`. The
composite here is the v1 formula (reciprocity axes merged, curl
coverage-gated).

| # | Model | JUDGE | signal (nats) | coherence | flip [95% CI] | residual | curl | spin | χ | polarity ρ | paraphrase ρ | null | cost |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | gemini-2.5-flash | **0.602** | 1.015 | 0.945 | 0.15 [.05,.36] | 0.274 | 0.039 | 3/3 | 0.31 | −0.81 | **1.00** | 0.000 | $0.021 |
| 2 | claude-sonnet-4.6 | 0.552 | 0.834 | **0.975** | **0.00** [0,.16] | 0.108 | **0.005** | 3/3 | **0.12** | **−0.88** | 0.93 | 0.000 | $0.209 |
| 3 | claude-haiku-4.5 | 0.504 | 0.756 | 0.950 | 0.10 [.03,.30] | **0.084** | 0.032 | 3/3 | 0.34 | −0.81 | 0.83 | 0.000 | $0.165 |
| 4 | deepseek-v4-flash | 0.463 | 0.687 | 0.932 | 0.20 [.08,.42] | 0.250 | 0.055 | 3/3 | 0.49 | −0.76 | 0.95 | 0.000 | $0.011 |
| 5 | gpt-5.4-mini | 0.390 | 0.680 | 0.790 | 0.20 [.08,.42] | 0.264 | 0.044 | **1/3** | 0.93 | −0.69 | 0.64 | 0.000 | $0.045 |
| 6 | gpt-5.4-nano | 0.212 | 0.484 | 0.553 | 0.40 [.22,.61] | 0.310 | **0.326** | **0/3** | 0.90 | **+0.38** | 0.33 | 0.000 | $0.012 |

## Findings

1. **The board separates on different axes, which is the point.**
   gemini-2.5-flash wins on *signal* (1.01 nats — it feels ~2.8×
   differences where sonnet feels ~2.3×). claude-sonnet-4.6 is the most
   *rigid* instrument measured: zero order flips in 20 pairs, curl 0.005
   (near the transitive floor), χ = 0.12, the strongest polarity reversal.
   A lab hill-climbing JUDGE must close its own gap: gemini needs nothing;
   sonnet needs decisiveness, not discipline.
2. **gpt-5.4-nano fails polarity in sign**: ρ = **+0.38** between "depth of
   insight" and "shallowness: the absence of insight" scores. It ranks the
   set the same way under an attribute and its negation — a halo score, not
   a judgement. The same model shows curl 0.33 (real preference cycles) and
   0/3 spin survival. Every structural pathology the benchmark names, in
   one model.
3. **OpenAI smalls follow the asker**: mini keeps only 1/3 clear beliefs
   under framing spin (χ = 0.93 — beliefs move ~2.5× per lean); nano keeps
   none. Every other lab's model here keeps 3/3. This replicates the
   paramagnet finding from `spin-probe-2026-07-05` at benchmark scale.
4. **Null bias is a solved axis for all six** (0.000 across the board) —
   consistent with the adversary's floor-effect critique: byte-identical
   nulls no longer discriminate frontier models; v2 needs near-identical
   pairs.
5. **Everyone agrees on the corpus tails, disagrees in the middle.** All
   six put "Monday is the first day of the work week" last with latent 0;
   the contested middle (measured/managed vs early-to-bed) is where flips
   and χ concentrate — consistent with the clear/contested stratification
   the v2 spec calls for.

CIs are wide (20 pairs) by design — this run is the standardized instrument
demo, not the reputational leaderboard; see `research/notes/BENCHMARK.md` §Adversarial
review for the v2 scale-up spec. Reproduce:

```bash
cardinal bench --models "google/gemini-2.5-flash,anthropic/claude-sonnet-4.6,..." \
  --cache bench-cache.sqlite --out reports.jsonl
```

(`bench-cache.sqlite` in this pack lets a rerun replay all 684 judgements
without spend.)

## Harmonic coherence (post-run receipt, same raw dims)

The game-resistant aggregate (one dead axis tanks it), recomputed from the
stored dimension values after the harmonic column shipped:

| Model | harmonic coherence |
|---|---|
| anthropic/claude-sonnet-4.6 | 0.974 |
| anthropic/claude-haiku-4.5 | 0.948 |
| google/gemini-2.5-flash | 0.939 |
| deepseek/deepseek-v4-flash | 0.925 |
| openai/gpt-5.4-mini | 0.688 |
| openai/gpt-5.4-nano | 0.000 |

Under the harsher aggregate the board's top four barely move, but both
OpenAI smalls collapse — mini to 0.69 (spin drags the harmonic down far
harder than the mean) and nano to exactly 0.000 (spin survival 0/3 is a
dead axis). The two aggregates agreeing on the top and disagreeing on the
bottom is itself evidence the composite isn't an artifact of the averaging
choice.

