# TypeSafe Jev as a judge for llmsort (2026-09-16)

Status: desk study, no calls made (Jev is waitlist early access; no key held).
Companion: the Scry-side read is the ChatGPT chat "Evaluate TypeSafe for Scry"
(2026-09-15); this note is the llmsort-side read.

## What Jev is, in this repo's ontology

Jev (typesafe.ai, released 2026-09-15) is a non-generative model: `POST
https://api.typesafe.ai/v1/systemone` with `{state, model: "jev-latest",
questions}`; every question returns a probability distribution over a declared
finite alphabet, never text. Three primitives:

| primitive | request | response | llmsort object |
|---|---|---|---|
| `score` | 2–10 ordered level descriptions | probability per level (+ mean, + "confidence") | the ratio-bucket PMF that `ratio_letter_v1` scrapes from top-k logprobs |
| `choice` | ≤255 named options | probability per option | setwise top-1 (Plackett–Luce first-place likelihood) |
| `noul` | yes/no proposition | P(yes) | `ordinal_letter_v1`'s direction PMF |

Any mix of questions rides one call against one shared state; TypeSafe says
they are evaluated "in parallel and in isolation" with little added latency per
question. Pricing $0.042/M input tokens, output unmetered; 70–500 ms end to
end (their laptops, West Coast). Docs give no state-size limit, no rate limit,
no calibration numbers (the confidence page says so explicitly — "confidence"
is an unspecified concentration statistic of the distribution), and the status
page showed 99.858 % 30-day uptime on 2026-09-15.

The decisive structural fact: **Jev's native output is the object our
evidence rail reconstructs by side channel.** The whole `seriate_logprob_route`
apparatus — `requires_effort_none`, `top_logprobs` support checks,
off-alphabet mass, the Cerebras `reasoning_effort none` scar — exists because
chat models emit tokens and we want a PMF at one answer position. A `score`
question over ratio levels is that PMF as the contract. `PacketObservation
{log_ratio, precision}` falls straight out of `(level probabilities × ladder
values)` with no parser.

## Three instruments, in order of fit

1. **Pairwise ratio via `score`.** State `{attribute, A, B}`; levels are a
   coarsened `RATIO_LADDER` (max 10 levels → e.g. B≫A ≥10×, B ≈3×, B ≈1.5×,
   about equal, A ≈1.5×, A ≈3×, A≫B ≥10×; 7 levels). Read moments in log
   space from our own ladder values, ignore their level-index mean. Cost
   floor: two 3,000-char entities ≈ 1.6K tokens ≈ $0.00007 per comparison,
   against ≈$0.0034 on the default judge (n=8 cell, README) — ~50× cheaper
   before any amortization, at sub-second latency. Open question the
   logprob-efficiency pack can answer offline first: how much split-half τ
   does a 7-level ladder lose against 51 buckets (rerun M/P/S with the PMF
   collapsed onto 7 rungs).
2. **Full pairwise graph over a window in one call.** State = k=8 labelled
   items; questions = C(8,2)=28 `score` ratios (× m criteria for the
   multi-criteria case, all in the same call). The state is paid once, output
   is free, latency claimed flat in question count: a complete ratio graph on a
   window for the price of one setwise call, with no permutation line to
   parse (the teacher corpus dropped 3.1 % of windows as malformed; here that
   class is empty by construction). This is the shape our multi-criteria
   setwise experiment wanted and could not have from a chat model. The
   research risk is the same halo family as NORTH E1 / RESULTS.md: whether
   "evaluated in isolation" survives 28 questions over one state, and whether
   m criteria in one call inflate inter-criterion correlation. Both are
   measured by gauges we already run.
3. **Setwise `choice` as a screen.** "Which item is highest on X" over up to
   255 labelled items → a top-1 distribution (their semantic_find cookbook
   does this over 218 lines). Weaker evidence than an order, but a candidate
   for the funnel's screening stage (design note avenue C) at one call per
   pool. Their own caveat applies: the vector sums to 1, so pair it with a
   `noul` ("is any item strong on X").

Their rerank cookbook is the pointwise formulation (one `noul` per
query–candidate pair, CLERC 40×30, top-1 5→18 %, top-10 38→62 %, $0.0645). Our
E-series evidence disqualifies pointwise screens for top-k work (tie blocks
drop up to 70 % of the true top-10 at the slice cut), so instruments 1–2 are
the ones to run, not their cookbook.

## Why it matters beyond a cheaper judge

- **Judge-agnosticism is the product.** llmsort's claim is that an instrument
  measures its own trustworthiness. Jev's marketing ("can't hallucinate",
  "calibrated") is exactly the claim our gauges exist to test; the docs
  themselves ship no calibration evidence. Running Jev through spin/orbit,
  flip rate, counterbalanced order, and cross-criterion halo yields a
  publishable calibration reading of a model whose vendor published none.
- **A commercial ceiling for the distillation program.** The teacher corpus
  (`mc-teacher-corpus-2026-09-13`, 120K rows) exists to train a small fast
  judge that emits per-criterion latents. Jev is a ready-made fast judge
  emitting PMFs at $0.042/M. If it matches gemma-4-31b joint on the bench,
  the distillation target has a price and a latency to beat — or the answer
  is to buy the lane and spend the corpus on criterion-conditioned
  reranking instead.
- **Scry and llmsort share one bakeoff.** The Scry question (is a single Jev
  pass over a 64-way shortlist as good as qwen3-rerank-8b?) and the llmsort
  question (is Jev's PMF a belief under our gauges?) are answered by the
  same runs on the same cohorts.

## The experiment (once a key exists)

Reuse the multi-criteria judge bench (`0b88e30`: hn_top, lw, arxiv,
hn_comments — four 40-item cohorts, criteria files, gemma-4-31b baselines,
BENCH.md). One experiment binary in `experiments/` hitting `/v1/systemone`
directly; nothing enters the crate until the numbers say so.

Arms per cohort × criterion:
- P1: pairwise `score`, one pair per call, counterbalanced (A/B swapped).
- P28: the 28-question window arm, 7 ring windows × 2 shuffled labellings,
  m criteria joint and separate.
- C: setwise `choice` over the whole 40, plus the `noul` existence check.

Readouts (same bars as RESULTS.md): per-criterion Spearman vs the gemma-4-31b
baseline and the teacher-corpus latents; test–retest flip rate under
relabelling (< 0.20); order mirror — does the P1 PMF reflect under A/B swap
(the spin invariant); cross-question halo = ρ(P28, P1) and inter-criterion
inflation joint − separate (< 0.10); Brier of `noul` direction against
teacher pairwise sign; tokens, $, p50/p95 latency, error and 429/529 rates.
~4 cohorts × 3 criteria × (28×14 + 40×39/2) questions ≈ 12K questions, well
under $1 at list price; wall time is bounded by their rate limit, which is
unpublished.

Decision: adopt as a judge lane if P1 or P28 clears agreement > 0.85 with
flip < 0.20 and the order-mirror holds; P28 additionally needs halo < 0.10.
Fail cleanly otherwise and keep the note as the record.

## Gate

API access is a waitlist (homepage form). No key is held anywhere local
(`~/.config`, acct ledger, env: nothing). Everything above the gate is
paper-ready; nothing runs until a key exists.

## Access log (2026-09-16 05:00–05:10 PT)

- Waitlist joined as `indexablework+typesafe@proton.me` (framer form, 201;
  confirmation "You're on the waitlist for Jev" received 05:03). Ledger:
  `typesafe--indexablework`, state `waitlisted`.
- `console.typesafe.ai` is self-serve magic-link (Stytch) or Google. The
  login.typesafe.ai redirect page is a Stytch device-fingerprint
  interstitial that headless Chromium fails; opening the embedded
  `console.typesafe.ai/auth/callback?…token=…` directly authenticates. The
  console then answers "TypeSafe is currently invite-only — join the
  waitlist". So the key is gated on their invite, not on anything we can do;
  the mechanics to mint it are recorded in the ledger note.

## `harshatheg/Qwen-2.5-1B-RLCD` (HF, uploaded 2026-09-16)

Not a model. The repo holds no weights: it is a Python engine
(`core/engine_torch.py`, `core/engine_mlx.py`) over stock
`mlx-community/Qwen2.5-1.5B-Instruct-4bit`, and the name borrows TypeSafe's
own acronym (RLCD = "Reinforcement Learning for Calibrated Decisions", the
homepage's training algorithm) for something untrained. Mechanism: prefill
the context once, broadcast the KV cache across M schema fields, append one
suffix `"field": "` per field, read logits at that single position, softmax
over the first token of each allowed choice (after a common-prefix strip —
the README's "token tree disambiguation" is not in the torch engine), assemble
JSON in code. "Calibrated" means that softmax; nothing is calibrated against
outcomes. Numbers on M4 Max: 4-field schemas 68–75 ms, a 28-field schema
270 ms, 255-way choice 89 ms.

Read for llmsort: this is `ratio_letter_v1`'s single-position top-logprob
readout, restated with a shared prefix — the mechanism, not the model, is
Jev's. On our judge host the same thing is `max_tokens=1` +
`top_logprobs` over k prompts sharing a cached prefix (vLLM prefix caching),
which the ratio-letter rail already does per pair. What it does not answer
is the part that matters: whether a small model's first-token PMF is a
belief (spin/orbit/flip gauges), which NORTH E1/E10 already say degrades
when entities share a prefix and the attribute moves to the tail. Nothing
to adopt; the useful residue is the confirmation that the whole "System One"
mechanism is one prefill + M single-token reads, i.e. we can run instrument
2 (the 28-pair window graph) on our own judges today with a prompt-layout
change, and gauge it before Jev's invite arrives.

## Mid-turn aside: small diffusion LMs as sorting judges

Masked-diffusion LMs (LLaDA / Dream class, ~1–8B open weights) unmask all
output positions in parallel with bidirectional attention over the prompt.
For setwise elicitation the output is a permutation of k letters, and the
per-position marginals at the last unmasking step are a k×k
position-by-item matrix — a richer evidence object than one sampled order
(closer to Plackett–Luce marginals), and one pass instead of k tokens. The
teacher corpus (120K latents, 765 criteria) is exactly the supervision a
fine-tune would need. Cheapest decisive step, before any training: run one
open dLLM zero-shot as the setwise judge on the four bench cohorts under the
existing gauges (flip rate, dropped/duplicated letters, position bias) against
gemma-4-31b; the AR "one prefill + M single-token reads" trick above is the
control, since it also buys parallel typed decisions without a new model
class. Proposed, not started.

### diffgemma PR #21 / #23 (mmastrac, merged 2026-09-16): the diffusion route made concrete

Matt Mastracci's `diffgemma` (Rust + Metal engine for `google/diffusiongemma-26B-A4B-it`,
Gemma-4 MoE, 3.8B active, discrete block diffusion over a 256-token canvas) now
does Jev's trick natively. PR #21: a JSON question schema as the system message
turns `/v1/chat/completions` into scored denoise forwards with no text
generated — the answer template `id: label` is seeded into the canvas with each
label slot as a noise token, one forward of the denoiser gives the label
distribution at every slot, and all questions in a request share that forward.
Labels are tokenizer-verified single tokens (`yes`/`no`, `A`/`B`/…, `1`/`2`/…),
which is our `ratio_letter_v1` discipline restated. Two findings there matter
to us more than the speedup (3.1 s for four reads vs 9.6–13.1 s generating the
JSON on an M3 Pro):

- **One read conditions on one noise draw and is sharper than the marginal.**
  Hole noise alone flipped borderline answers (16 seeds: 10/6). The reply
  averages reads with different noise and reports `stderr` of the top label's
  probability and `agreement` (share of reads whose argmax matches). This is a
  dispersion term we do not have: our flip rate measures order-permutation
  variance of an AR judge; noise-draw variance is a property of the diffusion
  read itself and belongs in the same place — as a variance inflation on the
  evidence row before the Huber fit.
- **PR #23: entropy gates the extra reads.** `samples:"auto"` takes one read and
  spends up to four only when some slot's first-read row entropy exceeds
  0.1 nats. Held-out 20 tickets / 60 slots at 16 reads: all 5 moving slots
  caught, 0/55 stable ones flagged, 15/20 tickets stop at one read. That is a
  per-decision adaptive budget keyed on the PMF's own entropy — the same
  quantity our precision column is built from, so an entropy-gated re-read is
  the diffusion analogue of "re-ask the low-precision pairs".

Also worth stealing regardless of model class: `label_mass`, the share of the
full-vocabulary mass at a slot held by the allowed labels. Low mass means the
model did not read the slot as an answer — our E10 attribute-in-tail failure,
measured directly instead of inferred from degraded agreement.

The ranking object it yields. A setwise template `A: _  B: _  …  H: _` with
rank labels `1`…`8` in each hole reads, per forward, an 8×8 item-by-rank
marginal matrix from one bidirectional pass — each slot conditions on the
entire template including the other holes' noise. Averaged over reads it is
the Plackett–Luce-ish object the aside above asks for; it is not a
permutation, and the constraint gets imposed after (Sinkhorn to a doubly
stochastic matrix, or Hungarian for the mode). A pairwise ratio instrument is
the trivial case: one hole, 25 single-token ladder letters. Multi-criteria
joint is "one forward, m holes per item", which is precisely the shape where
halo lives (NORTH E1), and here the coupling is structural rather than
sequential — every criterion's hole sees every other's. `fix_definite`
(settled slots pinned for later reads) is halo by construction for our use
and stays off.

**Correction to the earlier assumption that this is Mac-only.** The engine is
Metal-only, but the model is not: `google/diffusiongemma-26B-A4B-it` is
Apache 2.0, in transformers as `DiffusionGemmaForBlockDiffusion` (canvas 256,
max 48 denoise steps), supported by vLLM, with `RedHatAI/…-FP8-dynamic` and
`nvidia/…-NVFP4` quants published. The template-seeding read is ~50 lines over
the transformers model (build canvas, replace label positions with the mask
token, one forward, slice logits at the slots), so it runs on our judge host. Fit:
bf16 ≈ 50 GB, FP8 ≈ 26 GB, NVFP4 ≈ 13 GB, KV negligible at a 256-token canvas.
Tonight the judge host's 96 GB card has ~18 GB free (79/98 GB used) and each 5090 ~4 GB, so
NVFP4 fits without borrowing; FP8 needs a slot under the heretic-rig tenancy
law. Blackwell runs NVFP4 natively. Cost of the model itself: MMLU-Pro 77.6 vs
82.6 for its AR sibling — it is a judge candidate, not a solver, and only the
gauges decide.

Decisive cheap test (judge host, transformers, NVFP4, ~1 GPU-hour): one bench
cohort (`hn_top`, 40 items, ring windows k=8), joint setwise with the seeded
template, four reads per window, gauges as in BENCH.md — agreement with
gemma-4-31b, flip rate under slot permutation, halo across criteria, plus the
two new columns `stderr` and `label_mass`. Bars unchanged (agreement > 0.85,
flip < 0.20, halo < 0.10). If it passes, the fine-tune on the teacher corpus
becomes a live option; if it fails on `label_mass` or halo, the model class is
out and the AR one-prefill-M-reads trick stays the control. Executed the same
day on all four cohorts with fp8 experts instead of NVFP4 — results below.

### Executed 2026-09-16 (22:30–22:50 PT): one-forward slot marginals on the four-cohort bench

Artifacts: `research/artifacts/live/diffusiongemma-setwise-2026-09-16/` (runner `dg_setwise.py`,
four `summary-*.json`, four `trace-*.jsonl` with every 8×8 slot×item matrix).

Setup. `google/diffusiongemma-26B-A4B-it` through transformers on our GPU host, experts cast to
fp8_e4m3 per row (encoder and decoder experts are tied, so one copy: 26.9 GiB resident, 13 s to
quantize). Same cohorts, criteria, prompts, ring design (n=40, k=8, overlap 2, 2 presentations)
and Huber-IRLS fit as `multi_criteria_setwise.rs`. Reader: answer template seeded in the canvas,
label slots filled with uniform-random tokens, ONE denoiser forward, softmax over the eight label
tokens per slot, mean over 4 noise draws, best permutation of the mean log-matrix as the window's
order. Two facts the port had to learn: the model turn opens with a 4-token empty thinking
channel (`<|channel>thought\n<channel|>`), so the template sits at canvas position 4, not 0; and
free `generate` on a magnitude-ordering probe returns the exactly correct order, so the fp8 cast
is sound.

Result against the bars (agreement ρ(separate, joint) > 0.85; halo inflation < 0.10; flips not rising):

| cohort | agreement ρ | halo | flip separate → joint | ρ vs gemma-4-31b (separate arm) |
|---|---|---|---|---|
| hn_top | .83 / .70 / .84 | **+0.226** | .24/.29/.22 → .16/.21/.19 | .70 / .66 / .80 |
| hn_comments | .84 / **.92** / .79 | −0.006 | .18/.15/.29 → .14/.13/.25 | .74 / .76 / .73 |
| arxiv | .58 / .48 / **.90** | +0.016 | .23/.28/.28 → .39/.33/.19 | .73 / .34 / .76 |
| lw | .60 / .84 / .69 | −0.052 | .43/.21/.27 → .28/.22/.25 | .71 / .78 / .67 |

Fails as a drop-in judge: 2 of 12 criteria clear the agreement bar (gemma-4-31b clears them all
on hn_top), hn_top halo is over the bar, and flip rates run roughly 1.5–2× the gemma baseline.
It is nonetheless a real judge — ρ ≈ 0.7–0.8 against gemma-4-31b on 10 of 12 criteria — at
1.5 s per 8-item window for short items (6 s at 5.5k-token prompts, one read per forward on a
shared card) and zero marginal dollars.

Why it falls short, from the per-slot columns (mean over all windows, separate arm):

| slot | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 |
|---|---|---|---|---|---|---|---|---|
| p_top | .82 | .49 | .32 | .26 | .25 | .27 | .28 | .32 |
| entropy (ln 8 = 2.08) | .52 | 1.35 | 1.77 | 1.89 | 1.91 | 1.86 | 1.81 | 1.76 |
| label_mass | .97 | .77 | .76 | .70 | .65 | .55 | .41 | .31 |
| noise-draw stderr | .03 | .05 | .03 | .03 | .02 | .02 | .02 | .03 |

- One forward from noise is a mean-field read. Slot 1 is a confident "which item is first";
  slots 3–8 are near-uniform because each is conditioned on noise where its predecessors should
  be. Argmax across slots of the mean matrix was a valid permutation in 0 of 336 window-lines
  (mean 4.6 distinct letters of 8).
  The matrix carries a sharp top-1 and a soft rank gradient, not eight rank marginals.
- The noise-draw dispersion is tiny (stderr ≈ 0.02–0.05) and agreement across draws is 0.7–0.95.
  So PR #21's `stderr` is NOT the judge's uncertainty; it measures hole-noise sensitivity only.
  The uncertainty that matters is already in the mean matrix. The proposed variance-inflation
  use of it is withdrawn.
- label_mass decays to 0.31 by slot 8: the model wants to end the answer early there (`<turn|>`
  / repeats). The E10 parallel holds — mass off the label set is a validity signal, per slot.
- Joint prompts flatten slot 1 further (p_top .67) and hn_top halo rises: reading three criteria
  lines in one forward couples them, as predicted.

What this points at (not started): (a) use the matrix as it is — a top-1 PMF per window is a
best-of-k observation, which the fit can take directly instead of a forced full permutation;
(b) sequential clamping — fix slot 1, re-forward, read slot 2 — is k forwards per window, still
about 10 s, and turns the mean-field read into a proper Plackett–Luce chain with a PMF at every
stage; (c) read after the sampler's own denoising trajectory rather than from pure noise.
(b) is the honest version of the instrument and the one to run next.

### Executed 2026-09-16 (23:28–23:58 PT): sequential clamping, same bench

Artifacts: `research/artifacts/live/diffusiongemma-setwise-2026-09-16/clamp/` (same runner,
`--reader clamp --reads 2`). Mechanism: encode the prompt once (the decoder never writes into
the encoder KV cache, so one encoder pass serves every stage); at stage s the slots before s hold
the letters already chosen, the slots from s on are noise, and slot s is read as a PMF over the
letters still unused; greedy argmax fixes the slot. k = 8 decoder-only passes per read, 2 reads,
about 6 s per 8-item window on short items and 8 s at 5k-token prompts. On the magnitude probe
it is exact with p_top ≈ 1.0 at every stage.

| cohort | agreement ρ | halo | flip separate → joint | ρ vs gemma-4-31b (separate arm) |
|---|---|---|---|---|
| hn_top | **.92** / .77 / **.92** | −0.009 | .17/.18/.22 → .17/.24/.26 | .81 / .75 / .72 |
| hn_comments | **.88** / **.87** / .76 | −0.083 | .10/.15/.20 → .17/.14/.24 | .78 / .82 / .85 |
| arxiv | .82 / .62 / **.91** | +0.010 | .22/.27/.26 → .29/.32/.17 | .86 / .29 / .87 |
| lw | .85 / .81 / **.94** | +0.017 | .29/.20/.14 → .34/.20/.20 | .75 / .85 / .78 |

Against the one-forward read: agreement clears the bar on 6 of 12 criteria (was 2), halo now
passes on all four cohorts (hn_top +0.226 → −0.009), separate-arm flip rates are at gemma's
level (hn_top .17/.18/.22 vs gemma .15/.24/.12), and ρ against gemma-4-31b rises to .72–.87
everywhere except arxiv clarity (.29 — this model simply reads clarity differently from gemma;
both its own arms agree with each other at .62 only, so it is also its least stable criterion).

Per-stage columns (mean over all windows, separate arm): p_top .82 .73 .74 .75 .74 .76 .84 1.0;
entropy .51 .70 .70 .67 .64 .58 .35 0; label_mass ≥ .97 at every stage (was .31 at slot 8);
read agreement .90–.95. The chain is a proper Plackett–Luce read: every stage is a confident
choice among what is left, and the noise-draw spread (stderr .04–.08) is still small.

What still fails, and it is one thing: the joint arm. Flips rise slightly under joint prompting
on 8 of 12 criteria, and the joint-arm agreement with gemma's joint arm is lower (lw novelty
.49). For gemma the joint prompt is the better instrument; for this model the separate prompt
is. So the decision for this model class is: separate criteria, clamped reads — and then it is
a free local judge at gemma-4-31b's flip level with ρ ≈ .8 to it, at 6 s per window on a shared
card (a dedicated card and batched reads would take that to ~1 s).

Not done: the stage PMFs are stored in the traces (`matrix` = k stage vectors per line), so the
fit can consume the soft chain — each stage as a multinomial over the remaining letters — instead
of the greedy permutation; that is the offline analysis to run before any fine-tune decision.
