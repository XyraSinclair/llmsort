# Stickbreak day synthesis — 2026-09-06

Eleven live packs, one instrument arc, three independent explorer reviews
(astra, Kimi, Grok — open-ended briefs, soft recommendations), and the
probes their reviews demanded, all run same-day. Epistemics per operator:
subjective attributes have no external ground truth; the arbiter is
intra- and inter-model consistency.

## What stands at end of day

1. **Stickbreak transports; pairwise mostly does not.** Cross-model
   (gpt-4.1-mini ↔ gpt-4o-mini) sb k=4 ρ ≈ 0.7–0.86 on two of three
   attributes vs pairwise ≈ 0.0–0.6; per-pair signs 59–86% vs 47–63%.
   Replicated under a repaired rubric and — decisively — under a
   DIFFERENT design (different subsets/slot orders, same items): sb k=4
   0.69/0.34/0.76. The shared-lineup confound (Kimi) is exonerated, and
   within-model design-change reproducibility (+0.78…+0.91) retires the
   "retest was protocol determinism" caveat.

2. **The impact_per_dollar "two opposed constructs" anomaly is resolved
   as ask-line salience** (Grok found the mechanism; the ask-stripped
   probe confirmed its prediction on both models): lineups reward the
   small price tag (sb vs −log(ask) +0.7…+0.8), isolated pairs reward
   the grander blurb (pw vs +log(ask) up to +0.92); deleting ASK lines
   collapses both and flips sb-vs-pw to +0.52/+0.60 agreement. Neither
   rubric truncation (astra's find, fixed) nor an explicit "cheap is not
   better" sentence breaks it; the slate's ask spread modulates it.
   Context visibility is part of instrument identity — Kimi's
   framing-typing point survives in this modified form: openpriors
   instrument records should carry framing/context-visibility as
   first-class fields.

3. **Ordinal claims are solid; magnitude claims are not.** All three
   explorers converged here independently: the live fold's 1e-9 floor
   makes near-greedy PMFs produce ±20-nat edges (80% of live 4.1-mini
   edges exceed ln 26; 88/216 hit the engine's ±10 clip; latent sd
   ~200× pairwise's). The extreme scores are built by the clamp, not
   measured. Spearman-level findings survive; treat stickbreak latents
   as ranks with provisional magnitudes until the anchor test (below).

## Explorer findings actioned same-day

- astra: truncated + pair-phrased impact rubric → fixed, erratum
  committed, re-measured both models (anomaly survived; truncation
  exonerated as cause).
- Grok: ask-salience mechanism → ask-stripped probe run, both models,
  prediction confirmed, headline rewritten.
- Kimi: shared-seed transport confound → cross-design probe run
  (slate-pinned corpus + new design seed), confound exonerated.

## Open forks (ranked)

1. **Magnitude fork (Kimi #2, decides what stickbreak IS):** run
   stickbreak k=4 on the perturbation-spectrum anchor pools (true
   ratios known). Slope ≈ 1 ⇒ magnitude instrument, promote the fold;
   slope ≫ 1 (predicted) ⇒ ordinal instrument — winsorize at harvest
   time and type its openpriors currency Ordinal + PMF moments, not
   LogRatio.
2. **Family confound:** both judges are OpenAI-family. gemma-4-12b
   (judge-bakeoff: retest +0.72, slot bias +0.02 nats, logprob-solid,
   already self-hosted on the GPU box) is the third leg with zero new
   infrastructure; OpenRouter provider routing for llama returned
   top_logprobs on 1/31 calls.
3. **Fold ablation as promotion gate (Grok B / astra):** replay
   existing packs as live-fold vs winsorized-at-harvest vs first-PMF vs
   hard-rank-gap; astra's OLS pass found no uniform winner (first PMF
   best on impact, sb/hard best on team) — if hard order matches, it
   wins on model availability. Zero calls.
4. **Planted per-slot recovery (Kimi #3):** plant slot β in the
   synthetic judge and verify the two-channel fit recovers it from
   stickbreak edges — the slot-bias REJECTED verdict currently stands
   only in the winsorized projection. Offline, needs a small Rust edit
   (build host).
5. **Instrument hygiene (Rust, build host):** chunk assert misses remainder
   groups (n=8 k=3 ran 2-item lineups); persist a presentation index
   and letter-misalignment count per trace row; report.json caveats
   still describe the ratio/pivot instrument on stickbreak arms.

Explorer notes (local-only, machine paths):
`research/data/explorer-notes-2026-09-06/`.
