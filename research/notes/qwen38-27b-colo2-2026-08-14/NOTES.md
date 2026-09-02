# Qwen3.8-27B-FP8 as a local judge on colo2 (2026-08-14)

First self-hosted heavyweight judge lane for the sorting loop: zero marginal
cost, full logprobs, ~150ms warm single comparisons.

## Serve (colo2, RTX PRO 6000 96GB, co-tenant with rerank + embed fleet)

- Weights: `/data/models/hf` → `Qwen/Qwen3.8-27B-FP8` (28.75 GiB, arch
  `Qwen3_5ForConditionalGeneration`: 64-layer dense hybrid, Gated DeltaNet +
  gated attention, VL — vision zeroed at serve).
- Launch: `/data/models/launch_qwen38.sh` (vllm 0.24 from the voyage venv,
  UUID-pinned to the PRO 6000, `:8023`, served name `qwen38-27b`).
  Boot scar tissue, each one load-bearing:
  - `CUDA_HOME` → the venv's `nvidia/cu13` (GDN kernels want nvcc at JIT);
  - `VLLM_USE_FLASHINFER_SAMPLER=0` (pip nvcc 13.2 vs bundled cccl headers
    mismatch kills flashinfer's JIT'd sampling ops);
  - `VLLM_USE_DEEP_GEMM=0` (DeepGEMM asserts "Unknown recipe" for the
    block-128 FP8 quant on sm_120; CUTLASS block-scaled path works);
  - `--gpu-memory-utilization 0.34 --max-num-seqs 32 --max-num-batched-tokens
    4096` (co-tenant VRAM budget: 3 aux embed shards stopped to make room —
    see Debts);
  - `--max-logprobs 64`, `--limit-mm-per-prompt '{"image":0,"video":0}'`;
  - `--default-chat-template-kwargs '{"enable_thinking": false}'` — THE
    load-bearing line for instrument work, see below.
- Wire ratiometer at it with `OPENROUTER_BASE_URL=http://127.0.0.1:8023/v1`
  (tunnel `ssh -L 18023:127.0.0.1:8023 colo2` from a workstation),
  `OPENROUTER_API_KEY=<anything>`, `--model qwen38-27b`. Unknown-slug pricing
  falls back to the documented estimate; reported $ are fiction here.

## Thinking mode vs the first-token contract

Thinking is on by default (`reasoning_effort` xhigh) and is instrument-fatal:
first token "We", 50s/call. `enable_thinking:false` (or
`reasoning_effort:"none"`) → first token IS the answer letter, 150-200ms/call,
0.80 of first-token mass on the 52-letter alphabet, visible mass 0.93 at
top-20. The serve default bakes this in so unmodified clients get the
non-thinking contract.

## Fitness: 16 animals by adult body mass, ground-truth Spearman

| run | comparisons | wall | Spearman | order flips | order-residual |
|---|---|---|---|---|---|
| ratio_letter_v1 | 64 | 11.5s | 0.774 | 31/31 | 4.55 nats |
| ratio_letter_v1 | 240 | 36.7s | 0.924 | 68/68 | 5.55 nats, cyclic 37% |
| canonical_v2 | 64 | 14.5s | **0.976** | **1/32** | **0.36 nats** |

**Non-thinking Qwen3.8-27B cannot hold the ratio_letter case convention on
close pairs.** Direct both-orders probe (`/data/models/qwen38_symmetry.py`):
on every close pair the first-token mass stays on the same case in BOTH
presentation orders (wolf/fox → "B" both ways; cat/rabbit → "b" both ways) —
the winner-by-case semantics detaches from the entities, so counterbalancing
cancels the signal into noise and the diagnostics scream (100% order flips,
cyclic energy growing with budget). Extreme pairs are fine; magnitude is
roughly right; direction is what it loses. canonical_v2's prose answer
survives because the direction is stated in words, not case.

So for this model: canonical_v2 is the fitness winner at equal budget and
near-equal wall time. ratio_letter's single-token economics only pay off on
models that can bind case→direction without a reasoning pass — worth a small
cross-model fitness battery before trusting it anywhere new.

## Throughput today / levers not yet pulled

~4.4 cmp/s at the client's default concurrency 8 (64 comparisons in 11-15s
end-to-end through the tunnel). Untouched levers: client concurrency toward
the 64 cap (local engine, no politeness budget), engine `--max-num-seqs`
above 32 + bigger `--max-num-batched-tokens` (needs the VRAM debt below
repaid), prefix-cache warmth across runs (system+attribute prefix is shared
by all pairs).

## Debts / open

- Three co-tenant aux embed shards are STOPPED for VRAM (reversible; the
  embed backfill runs on the remaining fleet). Heal = relocate the aux
  shards onto smaller cards (~9GB free each, shard ~7.9GB — tight, measure
  before trusting) or accept the capacity dip. Restarting them while
  qwen38-27b serves will OOM one side.
- Serve is lab-launched (`setsid nohup`), not a systemd unit. Unit-ify
  (UUID pin, `Restart=always`) + a health probe once the GPU budget
  settles.
- Thinking-mode fitness unmeasured: does `enable_thinking:true` fix the case
  convention (at 300x the latency), and does `reasoning_effort:"low"` buy
  direction coherence at acceptable speed?

## Health study: 40 Manifund projects by "existential seriousness" (2026-08-15)

Live subjective-attribute workload (manifund.org /api/v0/projects, title+blurb),
canonical_v2, budget 240, two independent seeds + one paraphrase run:

- **Test-retest ρ = 0.966** (seed 1 vs seed 2, no cache, fully re-judged);
  top-10 overlap 8/10, ranks 1-3 identical. ~52s per run (~4.6 cmp/s), $0.
- Diagnostics per run: order flips 22-27%, order-residual 0.21-0.23 nats,
  cyclic 11-13%, frustration 0.11-0.14 — moderate and sane for a soft
  attribute (vs ratio_letter's 100% flips on this model).
- Paraphrase probe "seriousness about reducing existential risk to humanity":
  pairwise consistency +0.35 (engine calls it shaky); final-rank ρ vs the
  main criterion ~0.78. Reads as construct ambiguity, not engine noise: the
  model genuinely distinguishes existential *seriousness* (gravity of intent)
  from x-risk *reduction* work — arguably correct behavior, but phrase the
  criterion precisely when it matters.
- Items unstable across seeds are mid-pack (Δrank ≤ 8 around ranks 7-33);
  head and tail are rigid.
