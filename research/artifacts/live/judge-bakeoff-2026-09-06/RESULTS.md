# Small-judge bakeoff — interim results (2026-09-06)

Question: which fast 8–14B model is a stable pairwise judge for "signal and
exciting technical alpha, not noise"-class attributes, and how does it compare
with bigger reference judges? Instrument: the production ratio-letter pairwise
prompt (`ratio_letter_attrlast_v1`, answer-position logprob PMF), driven by
`experiments/examples/judge_bakeoff.rs`.

Status: reference tier complete (4 judges) plus two local small judges
(gemma-4-12b-it, Qwen3.5-9B). The remaining nine local models are parked
behind a GPU-slot waiter on the shared box (a co-tenant took the 96 GB card's
free memory mid-sweep; we yield rather than preempt). This file is rewritten
when they land.

## Verdict so far

1. **The instrument, not the models, is the first-order finding.** The
   ratio-letter alphabet is case-sensitive (`A/a` parity, uppercase `B..Z`
   = entity A ahead, lowercase `b..z` = entity B ahead). Several judges never
   emit the lowercase half: qwen3.7-flash answers "entity B ahead" on 1 % of
   calls, Qwen3.5-9B on 0 % (it answers `B` to every call in both orders —
   it is naming the slot, not a ladder rung), deepseek-v4-pro on 7 %. A judge
   that says `B` when it means "entity B wins" is parsed as "entity A ahead by
   1.06×", which is a stable *reversal*: both Qwen judges correlate
   negatively with every other judge (Qwen3.5-9B vs gemma-4-31b −0.32,
   qwen3.7-flash −0.19) while agreeing with each other (+0.16).
2. **gemma-4-31b-it is the anchor.** It is the only judge using both halves
   of the alphabet in balance (46 % A-ahead / 38 % B-ahead), has the best
   pair-level retest (+0.80) and wording robustness (+0.64 / +0.56 against
   the operational and counterfactual wordings), near-zero slot bias
   (+0.04 nats), zero failures, and agrees with deepseek-v4-pro at +0.33 —
   the strongest cross-family agreement in the matrix. On OpenRouter it also
   ran fastest (15 calls/s).
3. **gemma-4-12b-it is the only small judge that works so far.** Retest
   +0.72, slot bias +0.02 nats, visible mass 1.00, agreement with
   gemma-4-31b +0.34 (equal to 31b↔pro) and +0.43 with the leave-one-out
   consensus on technical-alpha, the axis Xyra cares about most. Its
   weakness is timidity: 42 % of answers sit at parity and mean |m| is
   0.03 nats, so it separates pairs by tiny tilts of the PMF rather than
   by committed ladder rungs, and wording robustness is only +0.34 / +0.31.
   It also refuses more on the LessWrong axes (68–87 abstentions of 720 on
   novelty and technical-alpha).
4. **deepseek-v4-pro is decisive but slot-locked.** Mean |m| 1.36 nats, yet
   92 % of answers say entity A is ahead (+0.72 nats slot bias). The
   both-orders design cancels this for the bakeoff, but single-order
   production use would be mostly position. deepseek-v4-flash is cheap and
   balanced but noisy (retest +0.26) and abstains heavily (433 refusals).

Working recommendation while the sweep finishes: gemma-4-12b-it is the
small-judge candidate to carry forward, with gemma-4-31b-it as the
calibration reference. Before adopting any small judge, run an
ordinal-instrument arm (no case-sensitive ladder) on the top candidates to
separate "cannot rank" from "cannot read this alphabet" — the Qwen result
says the alphabet is a real barrier for small models.

## Design

- Cohorts: 30 LessWrong posts (600–2500 words) and 30 funded Manifund
  proposals; ids in `items-ids.json` (texts are not committed).
- Axes (`research/batteries/judge_bakeoff_axes.json`): LessWrong —
  epistemic-rigor, novelty-of-insight, technical-alpha; Manifund —
  epistemic-pollution-restraint, novel-world-expanding-hit,
  theory-of-change. Each in three wordings: a (definitional),
  b (operational), c (counterfactual).
- Pair design: circulant, degree 6 (90 pairs per lens), both presentation
  orders, wording a drawn twice (second draw under a nonce that bypasses the
  cache). 4320 calls per judge (2160 for deepseek-v4-pro: wording a only).
- Local judges: one vLLM at a time on the 96 GB card, top-20 logprobs,
  reasoning disabled. Reference judges via OpenRouter with
  `require_parameters` so only logprob-honouring providers serve them.
- Metrics are computed at the **pair level** (signed log-ratio per pair,
  averaged over both orders). Item-level folds on a degree-6 ring were
  misleading: a signed mean shares a neighbour artifact across judges, and
  least squares integrates noise around the ring, so cross-judge agreement
  read as ~0 while the pair-level view shows the reference cluster at +0.33.

## Per-judge summary (means over the six lens×axis cells)

| judge | calls/s | retest ρ | wording b ρ | wording c ρ | slot bias (nats) | decisive \|m\| | par/A/B % | vis mass | logprob | refused | failed |
|---|---|---|---|---|---|---|---|---|---|---|---|
| Qwen/Qwen3.5-9B | 5.6 | +0.67 | +0.58 | +0.50 | +0.71 | 0.76 | 0/100/0 | 0.97 | 94% | 79 | 160 |
| deepseek/deepseek-v4-flash | 4.2 | +0.26 | +0.28 | +0.26 | +0.15 | 0.19 | 11/70/19 | 0.95 | 90% | 433 | 0 |
| deepseek/deepseek-v4-pro | 5.0 | +0.45 | n/a | n/a | +0.72 | 1.36 | 1/92/7 | 0.79 | 93% | 1 | 0 |
| google/gemma-4-12b-it | 3.4 | +0.72 | +0.34 | +0.31 | +0.02 | 0.03 | 42/49/9 | 1.00 | 96% | 193 | 0 |
| google/gemma-4-31b-it | 15.4 | +0.80 | +0.64 | +0.56 | +0.04 | 0.09 | 16/46/38 | 1.00 | 97% | 112 | 0 |
| qwen/qwen3.7-flash | 8.0 | +0.24 | +0.26 | +0.16 | +0.17 | 0.20 | 23/76/1 | 0.96 | 100% | 8 | 0 |

Notes: retest ρ for logprob-PMF judges measures provider nondeterminism more
than judgement noise (a deterministic local server retests near 1 by
construction), so it separates the OpenRouter judges from each other but
flatters local ones. Qwen3.5-9B's 160 failures are the last wording-c cell
of one axis, lost when a co-tenant's `pkill -f "vllm serve"` took the server
down mid-battery. Local calls/s were measured on a card shared with three
other tenants at 100 % utilisation; the gemma-4-12b smoke on a quieter card
ran at 15 calls/s. Cost columns for local judges in `pack/REPORT.md` are
adapter estimates, not spend; OpenRouter spend for the reference tier was
about $4.5.

## Inter-judge agreement (pair-level Spearman, wording a, draw 0, mean over cells)

| judge | Qwen3.5-9B | ds-v4-flash | ds-v4-pro | gemma-4-12b | gemma-4-31b | qwen3.7-flash | LOO consensus |
|---|---|---|---|---|---|---|---|
| Qwen3.5-9B | — | −0.11 | −0.18 | −0.16 | −0.32 | +0.16 | −0.19 |
| deepseek-v4-flash | −0.11 | — | +0.05 | +0.11 | +0.14 | −0.06 | +0.03 |
| deepseek-v4-pro | −0.18 | +0.05 | — | +0.07 | +0.33 | −0.16 | −0.03 |
| gemma-4-12b-it | −0.16 | +0.11 | +0.07 | — | +0.34 | −0.07 | +0.05 |
| gemma-4-31b-it | −0.32 | +0.14 | +0.33 | +0.34 | — | −0.19 | +0.13 |
| qwen3.7-flash | +0.16 | −0.06 | −0.16 | −0.07 | −0.19 | — | −0.07 |

The leave-one-out consensus column is dragged toward zero by the two reversed
Qwen judges; read the reference columns (vs gemma-4-31b, vs deepseek-v4-pro)
as the cleaner signal. Per-cell, the reference cluster agrees best on
technical-alpha (gemma-4-12b +0.43 and gemma-4-31b +0.40 with consensus) and
novelty-of-insight, and worst on epistemic-pollution-restraint and
theory-of-change, where every judge is near zero — those two Manifund axes
are not yet well-posed for pairwise judging.

## Files

- `pack/REPORT.md` — full per-cell batteries and matrices (generated).
- `pack/records-<judge>.json.gz` — every call: lens, axis, wording, draw,
  pair, order, presented log-ratio mean/var, visible mass, logprob mode,
  refused/failed, tokens, latency. No entity text.
- `items-ids.json` — cohort ids.
- Sweep runner, serve script and incident notes live on the GPU box under
  `/data/judge-sweep/` (README.md there documents claim/preempt etiquette).
