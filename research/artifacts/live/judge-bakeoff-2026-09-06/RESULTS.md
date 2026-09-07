# Small-judge bakeoff — results (2026-09-06)

Question: which fast 8–14B model is a stable pairwise judge for "signal and
exciting technical alpha, not noise"-class attributes, and how does it compare
with bigger reference judges? Driver: `experiments/examples/judge_bakeoff.rs`.

Two instruments were run on the same pairs:

- **ratio** — the production ratio-letter prompt (`ratio_letter_attrlast_v1`):
  52-letter case-sensitive alphabet, `A/a` parity, uppercase `B..Z` = entity A
  ahead by a ladder rung, lowercase `b..z` = entity B ahead. Answer-position
  logprob PMF → signed log-ratio.
- **ordinal** — `ordinal_letter_v1`: `A` / `B` / `=` direction only, fixed
  ±1.02-nat bucket. Added after the ratio arm showed most small judges cannot
  read the case-sensitive ladder.

Status: every ≤9B candidate has both arms; the 14B-class tier has its ratio
arm (Qwen3-14B-FP8, Ministral-3-14B, gemma-4-26B-A4B; gpt-oss-20b answered
only parity before a co-tenant SIGTERMed it, Nemotron-3.5-Lightning NVFP4
dies at load on this vLLM build) and gemma-4-12b has its ordinal arm. The
ordinal arm for gpt-oss-20b, Qwen3-14B-FP8, Ministral-3-14B and
gemma-4-26B-A4B is running on a 32 GB card and lands in `pack-ordinal/`.

## Verdict

1. **Use the ordinal instrument for any judge under ~30B.** Under the ratio
   alphabet, seven of the nine small/cheap judges never emit the lowercase
   half: Qwen3-8B-FP8, Qwen3.5-9B, Olmo-3-7B and Qwen3.7-flash answer
   "entity A ahead" on 99–100 % of calls in *both* presentation orders (they
   are naming the slot, or answering `B` = "entity B wins" which the ladder
   parses as "A ahead by 1.06×"), granite answers parity 99 % of the time,
   Ministral-8B is 74/24 with a +0.98-nat slot bias. The result is a sign
   flip: Qwen3.5-9B −0.32 and Qwen3.7-flash −0.19 against gemma-4-31b. The
   same models under the ordinal prompt agree with the reference cluster at
   +0.40 to +0.80.
2. **Reference pair: gemma-4-31b-it ↔ qwen3.7-flash under ordinal, +0.80
   pair-level Spearman**, with retest +0.98 / +0.95 and balanced slot use
   (50/48, 57/42). That is the ceiling any small judge is measured against.
   Per cell they agree with the leave-one-out consensus at +0.71 / +0.79 on
   technical-alpha and +0.65 / +0.76 on novelty. Under ratio the same pair
   is −0.19 because qwen3.7-flash cannot read the ladder; gemma-4-31b alone
   is the only balanced ratio reader (46/38) and remains the ratio anchor.
3. **gemma-4-12b-it on the ordinal instrument is the small judge, and it
   is as good as the 31b.** +0.88 with gemma-4-31b, +0.84 with
   qwen3.7-flash, +0.69 with the leave-one-out consensus (+0.73 on
   technical-alpha, +0.79 on novelty), retest +0.99, slot bias −0.03 nats,
   48/52 A/B, decisive (mean |m| 0.94 of the 1.02 bucket), 15 refusals of
   2160, zero failures, 4.7 calls/s as fp8 on a 32 GB card. Qwen3.5-9B is
   the fallback if a Gemma licence is a problem: +0.62 with qwen3.7-flash,
   +0.40 with gemma-4-31b and gemma-4-12b, LOO +0.39, retest +0.76, but a
   +0.29-nat slot preference (89 % argmax A) that only the both-orders
   design cancels. Qwen3-8B-FP8 is third (+0.35 vs both references) and
   100 % argmax-A — its ranking lives entirely in the PMF tilt, so it needs
   logprobs, never sampled letters. Olmo-3-7B has a balanced alphabet but
   little signal (+0.08 / +0.17, LOO +0.16). granite-4.2-8b (+0.06, 705
   refusals) and Ministral-3-8B (−0.16 / −0.36, 1787 refusals — it answers
   off-alphabet) are out on both instruments.
4. **Under the ratio alphabet gemma-4-12b-it is the only small reader**
   (retest +0.73, slot bias +0.02, +0.34 with gemma-4-31b) but it is timid
   there — 42 % parity, mean |m| 0.03 nats — so the ordinal prompt is what
   unlocks it. The 14B tier does not change the picture: gemma-4-26B-A4B
   is fast (9.9 calls/s) and reads the alphabet but leans 81 % A-ahead and
   agrees with gemma-4-31b at only +0.21; Qwen3-14B-FP8 uses both halves
   (58/18, 30/42 on technical-alpha) yet agrees with nobody (−0.11 vs
   gemma-4-31b), so reading the alphabet is necessary, not sufficient;
   Ministral-3-14B is slot-locked (92 % A, +0.65 nats) like its 8B sibling.
5. **deepseek-v4-pro is slot-locked under both prompts**: 92 % "A ahead"
   under ratio (+0.72 nats), 82 % "B" under ordinal (−0.16 nats), retest
   only +0.36 ordinal, consensus +0.15. Decisive, expensive, and mostly
   position. deepseek-v4-flash is cheap and balanced but noisy (retest
   +0.26 / +0.39; LOO +0.30 ordinal).

Working recipe: ordinal prompt, both presentation orders, logprob PMF, and
gemma-4-12b-it (fp8 on a 32 GB card, ~5 calls/s at 5K-token prompts under
contention) as the local judge, calibrated against qwen3.7-flash (which at
$0.15 per 2160 calls is nearly free) and spot-checked against gemma-4-31b.
Adopt the ordinal instrument for the reference tier as well — it is what
makes qwen3.7-flash usable.

## Design

- Cohorts: 30 LessWrong posts (600–2500 words) and 30 funded Manifund
  proposals; ids in `items-ids.json` (texts are not committed).
- Axes (`research/batteries/judge_bakeoff_axes.json`): LessWrong —
  epistemic-rigor, novelty-of-insight, technical-alpha; Manifund —
  epistemic-pollution-restraint, novel-world-expanding-hit,
  theory-of-change. Ratio arm ran three wordings (a definitional, b
  operational, c counterfactual); the ordinal arm ran wording a only.
- Pair design: circulant, degree 6 (90 pairs per lens), both presentation
  orders, wording a drawn twice (second draw under a nonce that bypasses the
  cache). Ratio: 4320 calls per judge (2160 for deepseek-v4-pro, wording a
  only). Ordinal: 2160 calls per judge.
- Local judges: one vLLM per lane, top-20 logprobs, reasoning disabled.
  ≥20 GB models on the 96 GB card at 16K context; 7–9B models on a 32 GB
  card quantised to fp8 at load, 8K context (prompts are ~5K tokens).
  Ministral-3 needs `--tokenizer-mode mistral` (current transformers loads
  it as a mistral-common backend with no jinja template; the HF path 400s).
  Reference judges via OpenRouter with `require_parameters` so only
  logprob-honouring providers serve them.
- Metrics are computed at the **pair level** (signed log-ratio per pair,
  averaged over both orders). Item-level folds on a degree-6 ring were
  misleading: a signed mean shares a neighbour artifact across judges, and
  least squares integrates noise around the ring, so cross-judge agreement
  read as ~0 while the pair-level view shows the reference cluster at +0.33
  (ratio) / +0.80 (ordinal). Leave-one-out consensus z-scores each other
  judge over the pairs it answered and averages per pair over the judges
  that covered it (≥ half of them), so one high-refusal judge cannot shrink
  the shared pair set.

## Per-judge summary (means over the six lens×axis cells)

### Ordinal instrument

| judge | calls/s | retest ρ | slot bias (nats) | decisive \|m\| | par/A/B % | vis mass | logprob | refused | vs gemma-4-31b | vs qwen3.7-flash | LOO consensus |
|---|---|---|---|---|---|---|---|---|---|---|---|
| google/gemma-4-31b-it | 10.0 | +0.98 | +0.01 | 0.98 | 2/50/48 | 1.00 | 93% | 143 | — | +0.80 | +0.66 |
| qwen/qwen3.7-flash | 6.4 | +0.95 | +0.18 | 0.58 | 2/57/42 | 1.00 | 63% | 799 | +0.80 | — | +0.74 |
| google/gemma-4-12b-it | 4.7 | +0.99 | −0.03 | 0.94 | 0/48/52 | 1.00 | 99% | 15 | +0.88 | +0.84 | +0.69 |
| Qwen/Qwen3.5-9B | 3.4 | +0.76 | +0.29 | 0.24 | 5/89/6 | 0.98 | 93% | 159 | +0.40 | +0.62 | +0.39 |
| Qwen/Qwen3-8B-FP8 | 3.9 | +0.58 | +1.02 | 1.02 | 0/100/0 | 1.00 | 89% | 241 | +0.28 | +0.35 | +0.36 |
| deepseek/deepseek-v4-flash | 4.6 | +0.39 | +0.02 | 0.33 | 11/56/33 | 1.00 | 99% | 12 | +0.27 | +0.32 | +0.29 |
| deepseek/deepseek-v4-pro | 6.1 | +0.36 | −0.16 | 0.30 | 9/9/82 | 1.00 | 97% | 20 | +0.16 | +0.20 | +0.16 |
| allenai/Olmo-3-7B-Instruct | 8.8 | +0.58 | +0.16 | 0.25 | 16/67/17 | 0.96 | 97% | 73 | +0.08 | +0.17 | +0.16 |
| ibm-granite/granite-4.2-8b | 3.2 | +0.33 | +1.02 | 1.02 | 0/100/0 | 1.00 | 67% | 705 | −0.04 | +0.06 | +0.00 |
| mistralai/Ministral-3-8B-Instruct-2512 | 4.1 | +0.08 | +0.17 | 0.21 | 2/96/1 | 0.87 | 17% | 1787 | −0.36 | −0.16 | −0.27 |

qwen3.7-flash's 799 refusals are answer positions whose top-5 logprobs (the
provider's cap) did not contain any alphabet letter; its agreement numbers
are over the pairs it did answer. Qwen3-8B-FP8's slot bias of +1.02 is the
fixed bucket: it never changes argmax, only the PMF mass behind it.

### Ratio instrument

| judge | calls/s | retest ρ | wording b ρ | wording c ρ | slot bias (nats) | decisive \|m\| | par/A/B % | vis mass | logprob | refused | failed | vs gemma-4-31b | vs ds-v4-pro |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| google/gemma-4-31b-it | 15.4 | +0.80 | +0.64 | +0.56 | +0.04 | 0.09 | 16/46/38 | 1.00 | 97% | 112 | 0 | — | +0.33 |
| google/gemma-4-12b-it | 3.4 | +0.73 | +0.33 | +0.31 | +0.02 | 0.03 | 42/49/9 | 1.00 | 96% | 193 | 0 | +0.34 | +0.07 |
| google/gemma-4-26B-A4B-it | 9.9 | +0.62 | +0.40 | +0.28 | +0.05 | 0.05 | 15/81/3 | 1.00 | 94% | 258 | 0 | +0.21 | +0.19 |
| mistralai/Ministral-3-14B-Instruct-2512 | 7.5 | +0.55 | +0.33 | +0.32 | +0.65 | 0.83 | 2/92/7 | 0.82 | 86% | 629 | 0 | +0.08 | +0.08 |
| deepseek/deepseek-v4-pro | 5.0 | +0.45 | n/a | n/a | +0.72 | 1.36 | 1/92/7 | 0.79 | 93% | 1 | 0 | +0.33 | — |
| deepseek/deepseek-v4-flash | 4.2 | +0.26 | +0.28 | +0.26 | +0.15 | 0.19 | 11/70/19 | 0.95 | 90% | 433 | 0 | +0.14 | +0.05 |
| allenai/Olmo-3-7B-Instruct | 4.2 | +0.66 | +0.44 | +0.49 | +0.63 | 1.07 | 0/99/1 | 0.99 | 94% | 284 | 0 | +0.12 | +0.09 |
| mistralai/Ministral-3-8B-Instruct-2512 | 4.0 | +0.39 | +0.35 | +0.50 | +0.98 | 1.07 | 2/74/24 | 0.71 | 96% | 154 | 0 | +0.04 | +0.00 |
| Qwen/Qwen3-14B-FP8 | 4.9 | +0.61 | +0.22 | +0.43 | +0.03 | 0.04 | 24/58/18 | 1.00 | 86% | 609 | 0 | −0.11 | −0.13 |
| ibm-granite/granite-4.2-8b | 6.1 | +0.30 | +0.37 | +0.34 | −0.03 | 0.04 | 99/0/1 | 1.00 | 99% | 31 | 0 | −0.14 | +0.08 |
| qwen/qwen3.7-flash | 8.0 | +0.24 | +0.26 | +0.16 | +0.17 | 0.20 | 23/76/1 | 0.96 | 100% | 8 | 0 | −0.19 | −0.16 |
| Qwen/Qwen3-8B-FP8 | 7.4 | +0.54 | +0.39 | +0.41 | +0.21 | 0.26 | 0/99/0 | 1.00 | 99% | 30 | 0 | −0.30 | −0.10 |
| Qwen/Qwen3.5-9B | 5.6 | +0.67 | +0.58 | +0.50 | +0.71 | 0.76 | 0/100/0 | 0.97 | 94% | 79 | 160 | −0.32 | −0.18 |

Ratio LOO consensus is not reported: with eight of thirteen judges unable to
read the alphabet, the pool is mostly noise and every LOO cell sits within
±0.1. Read the two reference columns instead. Retest ρ for logprob-PMF
judges measures provider nondeterminism more than judgement noise (a
deterministic local server retests near 1 by construction). Qwen3.5-9B's
160 ratio failures are one wording-c cell lost when a co-tenant killed the
server; local calls/s were measured on cards shared with other tenants at
100 % utilisation. Cost columns for local judges in the generated reports
are adapter estimates, not spend; OpenRouter spend for both arms was about
$9.

## Inter-judge agreement, ordinal (pair-level Spearman, wording a, draw 0, mean over cells)

| judge | Qwen3-8B | Qwen3.5-9B | Olmo-7B | ds-flash | ds-pro | gemma-12b | gemma-31b | granite | Ministral-8B | qwen3.7-flash |
|---|---|---|---|---|---|---|---|---|---|---|
| Qwen3-8B-FP8 | — | +0.22 | +0.14 | +0.27 | +0.13 | +0.29 | +0.28 | −0.14 | +0.17 | +0.35 |
| Qwen3.5-9B | +0.22 | — | +0.16 | +0.18 | +0.04 | +0.40 | +0.40 | −0.06 | −0.26 | +0.62 |
| Olmo-3-7B | +0.14 | +0.16 | — | +0.06 | +0.01 | +0.13 | +0.08 | +0.02 | +0.25 | +0.17 |
| deepseek-v4-flash | +0.27 | +0.18 | +0.06 | — | +0.19 | +0.26 | +0.27 | −0.05 | −0.25 | +0.32 |
| deepseek-v4-pro | +0.13 | +0.04 | +0.01 | +0.19 | — | +0.19 | +0.16 | +0.06 | −0.11 | +0.20 |
| gemma-4-12b-it | +0.29 | +0.40 | +0.13 | +0.26 | +0.19 | — | +0.88 | +0.04 | −0.30 | +0.84 |
| gemma-4-31b-it | +0.28 | +0.40 | +0.08 | +0.27 | +0.16 | +0.88 | — | −0.04 | −0.36 | +0.80 |
| granite-4.2-8b | −0.14 | −0.06 | +0.02 | −0.05 | +0.06 | +0.04 | −0.04 | — | −0.11 | +0.06 |
| Ministral-3-8B | +0.17 | −0.26 | +0.25 | −0.25 | −0.11 | −0.30 | −0.36 | −0.11 | — | −0.16 |
| qwen3.7-flash | +0.35 | +0.62 | +0.17 | +0.32 | +0.20 | +0.84 | +0.80 | +0.06 | −0.16 | — |

Per-cell LOO consensus (ordinal), technical-alpha column: qwen3.7-flash
+0.82, gemma-4-31b +0.80, gemma-4-12b +0.73, Qwen3.5-9B +0.49, Qwen3-8B-FP8
+0.36, deepseek-v4-pro +0.34, deepseek-v4-flash +0.32, Olmo +0.16, granite
+0.14.
The two Manifund axes epistemic-pollution-restraint and theory-of-change
remain the weakest cells for every judge on both instruments; they are not
yet well-posed for pairwise judging.

## Files

- `pack/REPORT.md`, `pack-ordinal/REPORT.md` — full per-cell batteries and
  matrices (generated by `judge_bakeoff report`).
- `pack*/records-<judge>.json.gz` — every call: lens, axis, wording, draw,
  pair, order, presented log-ratio mean/var, visible mass, logprob mode,
  refused/failed, tokens, latency. No entity text.
- `items-ids.json` — cohort ids.
- Sweep runner, serve script and incident notes live on the GPU box under
  `/data/judge-sweep/` (README.md there documents claim/preempt etiquette
  and the day's incidents).
