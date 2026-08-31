# σε knobs cell — the 2p analysis turn's reasoning is the noise (2026-08-31)

**Question:** the slot-hetero pack attributed the 2p rail's per-call noise
(σε ≈ 0.215 nats on terra) to the stochastic phase-1 analysis. Which knob
reduces it — analysis reasoning effort, and at what quality cost?

**Answer: disable reasoning on the analysis turn.** The verdict-forbidden
WRITTEN analysis carries the 2p quality win; the hidden reasoning behind
it is a measured noise source. Landed as the rail's default the same day
(seriate.rs pins `ReasoningConfig::disabled()` on the analysis call).

## Instrument

`judge --draws` on the evidence rails (landed this morning, e40b0ae):
3 fixed proverb pairs × 8 nonce draws through the REAL 2p path per
condition (`run_condition.sh`; per-condition raw in `<label>.jsonl`).
Pooled σε = rms of per-pair sigma_w (24 draws, ~21 df ⇒ σ estimate
sd ≈ 11%).

## Draws sweep (terra, phase-1 reasoning config)

| condition | pooled σε (nats/call) | cost / 24 draws |
|---|---|---|
| provider-default effort | 0.260 | $0.099 |
| effort low | 0.209 | $0.098 |
| effort minimal | 0.238 | $0.098 |
| **disabled** | **0.181** | **$0.082** |

## Quality gate (8×32 sort cells, paired seeds, --no-cache)

| seed | default: residual / σ_w / frustration / flips / risk / $ | disabled: same |
|---|---|---|
| 7 | 0.290 / 0.257 / 0.256 / 5 / 3.41 / 0.120 | 0.136 / 0.121 / 0.070 / 1 / 1.94 / 0.105 |
| 8 | 0.208 / 0.185 / 0.179 / 4 / 4.06 / 0.122 | 0.139 / 0.124 / 0.088 / 3 / 2.55 / 0.105 |
| 9 | 0.217 / 0.192 / 0.121 / 5 / 4.13 / 0.121 | 0.147 / 0.131 / 0.181 / 2 / 2.80 / 0.105 |

Disabled wins order residual 3/3 (mean 0.238 → 0.141, −41%), σ_w 3/3,
rank risk 3/3, flips 14/48 → 6/48, cost −13%; frustration 2/3 (means
0.185 → 0.113). Note the canonical scoreboard's 2.4% cyclic for default
2p did not reproduce in these fresh cells (12–26%) — cross-run spread on
this 8-item corpus is large; the paired comparison is the evidence here.

## Generalization (luna, same sweep)

baseline σε 0.047, disabled 0.060 — indistinguishable at 24 draws,
slightly cheaper. The noise pathology is terra-magnitude, not
family-universal; the pin is safe where it does not help.

## Bonus finding (from the instrument's own smokes)

Single-phase ratio_letter_v1 on 5.4-mini: σ_w 0.004 — the verdict read
is nearly deterministic across nonces. The 2p noise was always the
analysis turn; now measured directly at both ends.

## Rerun

    ./run_condition.sh <label>                 # terra by default
    SIGMA_EPS_MODEL=openai/gpt-5.6-luna ./run_condition.sh <label>

(Conditions other than disabled require re-patching seriate.rs's
analysis_request reasoning line; disabled is now the shipped default.)

Total spend: ~$1.10 (4 terra sweeps $0.38, 2 luna sweeps $0.02, 6 sort
cells $0.68).
