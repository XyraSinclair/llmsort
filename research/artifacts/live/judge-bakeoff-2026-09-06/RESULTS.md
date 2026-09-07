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

Status: complete. 13 judges on the ratio arm, 14 on the ordinal arm
(Nemotron-3.5-Lightning NVFP4 dies at load on this vLLM build and is the
one candidate with no data).

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
   Per cell they agree with the leave-one-out consensus at +0.86 / +0.83 on
   technical-alpha and +0.85 / +0.86 on novelty. Under ratio the same pair
   is −0.19 because qwen3.7-flash cannot read the ladder; gemma-4-31b alone
   is the only balanced ratio reader (46/38) and remains the ratio anchor.
3. **gemma-4-12b-it on the ordinal instrument is the small judge, and it
   is as good as the 31b.** +0.88 with gemma-4-31b, +0.84 with
   qwen3.7-flash, +0.79 with the leave-one-out consensus (+0.82 on
   technical-alpha, +0.88 on novelty), retest +0.99, slot bias −0.03 nats,
   48/52 A/B, decisive (mean |m| 0.94 of the 1.02 bucket), 15 refusals of
   2160, zero failures, 4.7 calls/s as fp8 on a 32 GB card. gemma-4-26B-A4B
   is its twin (+0.87 / +0.86, LOO +0.79, retest +0.98, 46/51) but ran at
   2.5 calls/s as fp8 on the same card and refuses more (115), so the 12b
   wins on cost. Qwen3-14B-FP8 is the best non-Gemma option (+0.56 /
   +0.62, LOO +0.49, +0.72 on technical-alpha, retest +0.88) with a strong
   +0.80-nat slot preference (88 % argmax A) that only the both-orders
   design cancels; Qwen3.5-9B is the smaller fallback (+0.40 / +0.62, LOO
   +0.42, +0.29 nats slot). Qwen3-8B-FP8 (+0.28 / +0.35) is 100 % argmax-A
   — its ranking lives entirely in the PMF tilt, so it needs logprobs,
   never sampled letters. Ministral-3-14B agrees where it answers (+0.42 /
   +0.63) but refuses 55 % of calls; gpt-oss-20b refuses 80 % (it wants
   its reasoning channel); Olmo-3-7B has a balanced alphabet but little
   signal (+0.08 / +0.17); granite-4.2-8b (+0.06, 705 refusals) and
   Ministral-3-8B (−0.36 / −0.16, 1787 refusals — it answers off-alphabet)
   are out on both instruments.
4. **Under the ratio alphabet gemma-4-12b-it is the only small reader**
   (retest +0.73, slot bias +0.02, +0.34 with gemma-4-31b) but it is timid
   there — 42 % parity, mean |m| 0.03 nats — so the ordinal prompt is what
   unlocks it. The 14B tier does not change the picture: gemma-4-26B-A4B
   is fast (9.9 calls/s) and reads the alphabet but leans 81 % A-ahead and
   agrees with gemma-4-31b at only +0.21 (vs +0.87 under ordinal);
   Qwen3-14B-FP8 uses both halves (58/18, 30/42 on technical-alpha) yet
   agrees with nobody (−0.11 vs gemma-4-31b; +0.56 under ordinal), so
   reading the alphabet is necessary, not sufficient; Ministral-3-14B is
   slot-locked (92 % A, +0.65 nats) like its 8B sibling.
5. **deepseek-v4-pro is slot-locked under both prompts**: 92 % "A ahead"
   under ratio (+0.72 nats), 82 % "B" under ordinal (−0.16 nats), retest
   only +0.36 ordinal, consensus +0.17. Decisive, expensive, and mostly
   position. deepseek-v4-flash is cheap and balanced but noisy (retest
   +0.26 / +0.39; LOO +0.31 ordinal).

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
| google/gemma-4-12b-it | 4.7 | +0.99 | −0.03 | 0.94 | 0/48/52 | 1.00 | 99% | 15 | +0.88 | +0.84 | +0.79 |
| google/gemma-4-26B-A4B-it | 2.5 | +0.98 | −0.05 | 0.95 | 2/46/51 | 1.00 | 95% | 115 | +0.87 | +0.86 | +0.79 |
| google/gemma-4-31b-it | 10.0 | +0.98 | +0.01 | 0.98 | 2/50/48 | 1.00 | 93% | 143 | — | +0.80 | +0.75 |
| qwen/qwen3.7-flash | 6.4 | +0.95 | +0.18 | 0.58 | 2/57/42 | 1.00 | 63% | 799 | +0.80 | — | +0.79 |
| Qwen/Qwen3-14B-FP8 | 4.3 | +0.88 | +0.80 | 0.93 | 1/88/11 | 1.00 | 94% | 126 | +0.56 | +0.62 | +0.49 |
| Qwen/Qwen3.5-9B | 3.4 | +0.76 | +0.29 | 0.24 | 5/89/6 | 0.98 | 93% | 159 | +0.40 | +0.62 | +0.42 |
| mistralai/Ministral-3-14B-Instruct-2512 | 5.5 | +0.75 | +0.48 | 0.51 | 0/98/1 | 0.97 | 45% | 1191 | +0.42 | +0.63 | +0.54 |
| openai/gpt-oss-20b | 12.2 | +0.82 | n/a | 1.02 | 0/100/0 | 1.00 | 20% | 1726 | +0.40 | +0.39 | +0.55 |
| Qwen/Qwen3-8B-FP8 | 3.9 | +0.58 | +1.02 | 1.02 | 0/100/0 | 1.00 | 89% | 241 | +0.28 | +0.35 | +0.35 |
| deepseek/deepseek-v4-flash | 4.6 | +0.39 | +0.02 | 0.33 | 11/56/33 | 1.00 | 99% | 12 | +0.27 | +0.32 | +0.31 |
| deepseek/deepseek-v4-pro | 6.1 | +0.36 | −0.16 | 0.30 | 9/9/82 | 1.00 | 97% | 20 | +0.16 | +0.20 | +0.17 |
| allenai/Olmo-3-7B-Instruct | 8.8 | +0.58 | +0.16 | 0.25 | 16/67/17 | 0.96 | 97% | 73 | +0.08 | +0.17 | +0.16 |
| ibm-granite/granite-4.2-8b | 3.2 | +0.33 | +1.02 | 1.02 | 0/100/0 | 1.00 | 67% | 705 | −0.04 | +0.06 | +0.01 |
| mistralai/Ministral-3-8B-Instruct-2512 | 4.1 | +0.08 | +0.17 | 0.21 | 2/96/1 | 0.87 | 17% | 1787 | −0.36 | −0.16 | −0.26 |

qwen3.7-flash's 799 refusals are answer positions whose top-5 logprobs (the
provider's cap) did not contain any alphabet letter; its agreement numbers
are over the pairs it did answer. Qwen3-8B-FP8's slot bias of +1.02 is the
fixed bucket: it never changes argmax, only the PMF mass behind it.
Ministral-3-14B and gpt-oss-20b agreement numbers are over the minority of
pairs they answered (per-cell consensus is n/a where fewer than ten pairs
survive).

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

| judge | g-12b | g-26B | g-31b | q3.7-flash | Q3-14B | Q3.5-9B | Mini-14B | oss-20b | Q3-8B | ds-flash | ds-pro | Olmo | granite | Mini-8B |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| gemma-4-12b | — | +0.91 | +0.88 | +0.84 | +0.52 | +0.40 | +0.55 | +0.42 | +0.29 | +0.26 | +0.19 | +0.13 | +0.04 | −0.30 |
| gemma-4-26B-A4B | +0.91 | — | +0.87 | +0.86 | +0.55 | +0.44 | +0.49 | +0.32 | +0.31 | +0.27 | +0.14 | +0.13 | −0.00 | −0.32 |
| gemma-4-31b | +0.88 | +0.87 | — | +0.80 | +0.56 | +0.40 | +0.42 | +0.40 | +0.28 | +0.27 | +0.16 | +0.08 | −0.04 | −0.36 |
| qwen3.7-flash | +0.84 | +0.86 | +0.80 | — | +0.62 | +0.62 | +0.63 | +0.39 | +0.35 | +0.32 | +0.20 | +0.17 | +0.06 | −0.16 |
| Qwen3-14B-FP8 | +0.52 | +0.55 | +0.56 | +0.62 | — | +0.30 | +0.27 | +0.42 | +0.19 | +0.15 | +0.11 | +0.02 | +0.05 | −0.27 |
| Qwen3.5-9B | +0.40 | +0.44 | +0.40 | +0.62 | +0.30 | — | +0.31 | +0.12 | +0.22 | +0.18 | +0.04 | +0.16 | −0.06 | −0.26 |
| Ministral-3-14B | +0.55 | +0.49 | +0.42 | +0.63 | +0.27 | +0.31 | — | +0.89 | +0.41 | +0.05 | −0.03 | +0.20 | −0.07 | +0.15 |
| gpt-oss-20b | +0.42 | +0.32 | +0.40 | +0.39 | +0.42 | +0.12 | +0.89 | — | +0.03 | +0.13 | +0.17 | +0.36 | +0.24 | −0.25 |
| Qwen3-8B-FP8 | +0.29 | +0.31 | +0.28 | +0.35 | +0.19 | +0.22 | +0.41 | +0.03 | — | +0.27 | +0.13 | +0.14 | −0.14 | +0.17 |
| deepseek-v4-flash | +0.26 | +0.27 | +0.27 | +0.32 | +0.15 | +0.18 | +0.05 | +0.13 | +0.27 | — | +0.19 | +0.06 | −0.05 | −0.25 |
| deepseek-v4-pro | +0.19 | +0.14 | +0.16 | +0.20 | +0.11 | +0.04 | −0.03 | +0.17 | +0.13 | +0.19 | — | +0.01 | +0.06 | −0.11 |
| Olmo-3-7B | +0.13 | +0.13 | +0.08 | +0.17 | +0.02 | +0.16 | +0.20 | +0.36 | +0.14 | +0.06 | +0.01 | — | +0.02 | +0.25 |
| granite-4.2-8b | +0.04 | −0.00 | −0.04 | +0.06 | +0.05 | −0.06 | −0.07 | +0.24 | −0.14 | −0.05 | +0.06 | +0.02 | — | −0.11 |
| Ministral-3-8B | −0.30 | −0.32 | −0.36 | −0.16 | −0.27 | −0.26 | +0.15 | −0.25 | +0.17 | −0.25 | −0.11 | +0.25 | −0.11 | — |

Per-cell LOO consensus (ordinal), technical-alpha column: gemma-4-26B
+0.86, gemma-4-31b +0.86, qwen3.7-flash +0.83, gemma-4-12b +0.82,
Qwen3-14B-FP8 +0.72, Ministral-3-14B +0.60 (partial), Qwen3.5-9B +0.53,
deepseek-v4-flash +0.37, Qwen3-8B-FP8 +0.36, deepseek-v4-pro +0.34, Olmo
+0.15, granite +0.12. The Gemma trio and qwen3.7-flash sit at +0.72 to
+0.88 on every cell, including the two Manifund axes.
Under ratio the two Manifund axes epistemic-pollution-restraint and
theory-of-change were the weakest cells for every judge; under ordinal the
reference cluster resolves them (+0.62 to +0.80), so the earlier "not
well-posed" reading was an instrument artifact too.

## Throughput: gemma-4-12b-it bulk elicitation on one 5090 (2026-09-07)

Question: how many ordinal judgments per second can one 32 GB Blackwell
card deliver, and what actually limits it. Harness: `bench/bench.sh` on
the GPU box — one vLLM 0.26 server per config (fp8 weights at load,
8K context, images/audio off), 1,080 calls per client run (same 60-item
cohort, six lens×axis cells, wording a, one draw, both orders), fresh
cardinal cache per run, counters read from vLLM `/metrics`. Quality is the
pair-level Spearman of each run against the bakeoff's own gemma-4-12b
ordinal pack (draws 0 and 1; the judge's retest agreement is +0.99).

| server | client | calls/s | prompt tok/call | prefix hit | refused | vs ref |
|---|---|---|---|---|---|---|
| 16 seqs, bf16 KV (the sweep's shape) | concurrency 12 | 4.90 | ~3.1K | — | 6 | +0.99 |
| 16 seqs, bf16 KV | concurrency 64 | 4.90 | ~3.1K | — | 6 | — |
| 128 seqs, fp8 KV, 32K-token prefill chunks | concurrency 64 | 5.25 | ~3.1K | ~28 % | 11 | — |
| same | concurrency 128 | 5.10 | 3.1K | 28 % | 11 | +0.98 |
| same | 128, `--order entity` | **6.57** | 2.93K | 44 % | 11 | +0.98 |
| same | 128, entity order, `--max-chars 4000` | **10.82** | 1.99K | 46 % | 27 | +0.96 / +0.95 |
| same | 128, entity order, `--max-chars 2000` | **20.65** | 1.09K | 50 % | 48 | +0.90 |

What the numbers say:

- **The card computes a constant ~12K prompt tokens/s for this model**
  (12.2K / 11.9K / 11.9K / 12.1K non-cached tokens per second across the
  four instrumented runs, GPU busy 90–96 %). Throughput is therefore
  12K ÷ (tokens the server must actually compute per call), and every
  lever is either fewer computed tokens or a faster kernel.
- **Client concurrency was never the limit** (12 vs 64: identical 4.90);
  the server's 16-sequence cap was, and opening it to 128 sequences with
  fp8 KV only buys 7 %, because the work is prefill-bound: ~2 generated
  tokens per call (letter + end), `max_tokens` 16 is never reached.
- **Ordering calls by the presented entity A** turns the prefix cache from
  28 % to 44–50 % hits (the ceiling is 50 %: system + attribute + entity A
  is shared, entity B must be read fresh each time) — +29 % at no quality
  cost. The production orchestrator already issues batches in this order
  (`src/rerank/multi/orchestrator.rs` sorts by attribute, first entity).
- **Entity length is the big lever.** 4,000 chars per entity halves the
  computed tokens (2.2× calls/s) and costs 0.03 of agreement with the
  full-length judge (+0.96 vs a +0.99 retest) with 27 refusals of 1,080;
  2,000 chars is 4.2× but drops to +0.90 with 48 refusals. Default to
  4,000 for bulk passes; keep full text for the reference tier.
- **Attention runs on the Triton backend for gemma-4 on this card.**
  gemma-4 has 512-wide global-attention heads; on the 5090 (SM120) vLLM
  has only FlashAttention 2, which stops at 256, and FA4 is gated to
  SM90/100/110, so vLLM forces `TRITON_ATTN`. Attention is ~3 % of the
  FLOPs at 3K tokens, so this bounds the loss at maybe 10–15 %;
  FlashInfer lists head size 512 as supported and is the candidate fix.
  The fp8 GEMM path is `CutlassFP8ScaledMMLinearKernel` (per-tensor
  online quant); 12K tok/s × 24 GFLOP/token ≈ 290 TFLOP/s, roughly a
  third of the card's dense fp8 peak.
- Beyond one card the honest lever is data parallelism: three 5090s at
  10.8 calls/s each is ~32 calls/s, ~2.8M judgments a day at 4K chars.

Round 3 (kernel round: FlashInfer attention, 8K/64K prefill chunks, bf16
KV, bf16 weights) is queued on the GPU box as `bench/plan3.sh` and will be
appended here.

## Files

- `pack/REPORT.md`, `pack-ordinal/REPORT.md` — full per-cell batteries and
  matrices (generated by `judge_bakeoff report`).
- `pack*/records-<judge>.json.gz` — every call: lens, axis, wording, draw,
  pair, order, presented log-ratio mean/var, visible mass, logprob mode,
  refused/failed, tokens, latency. No entity text.
- `items-ids.json` — cohort ids.
- Throughput harness: `bench/bench.sh`, `bench/plan.sh`, `bench/plan3.sh`,
  `bench/sidecar.sh` (metrics sampler), `bench/compare.py` (quality vs the
  reference pack), `bench/bench.tsv` (one row per run), all on the GPU box
  under `/data/judge-sweep/bench/`.
- Sweep runner, serve script and incident notes live on the GPU box under
  `/data/judge-sweep/` (README.md there documents claim/preempt etiquette
  and the day's incidents).
