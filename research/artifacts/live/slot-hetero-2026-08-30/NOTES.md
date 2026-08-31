# Slot-heterogeneity cell — the 2p order residual is noise, not position bias (2026-08-30)

**Question (guardian-two gate):** terra's ratio_letter_2p_v1 rail reports
order residual 0.277 nats/pair as its dominant error term (0.96 of signal
scale). Is that a global slot bias (fit-and-subtract → single-order runs at
half cost), a heterogeneous per-pair slot bias (slot-name randomization), or
neither?

**Answer: neither. It is within-call sampling noise of the stochastic
phase-1 analysis, present at temperature 0.** No slot bias exists on this
rail — global or per-pair. Both proposed symmetrization instruments are dead.

## Errata / incident (read first)

The first two runs (`run_cell.py` → `raw.jsonl`, `run_smoke_sigma.py` →
`raw_smoke.jsonl`) invoked `judge --draws --template ratio_letter_2p_v1`,
and `nonce_draws` SILENTLY fell back to `DEFAULT_PROMPT` (canonical_v2 JSON,
json_mode, provider-default reasoning) because `prompt_by_slug` knows no
evidence slugs. Those two cells therefore characterize the terra **JSON**
rail with a nonce suffix at temperature 0 — not the 2p rail. The fallback
is now a loud error (src/rerank/sampling.rs, same commit). The third run
(`run_true2p.py` → `raw_true2p.jsonl`) uses bare `judge` single calls —
the identical code path to sort's comparisons (compare_pair → seriate 2p,
temperature 0, no nonce) — and is the load-bearing measurement.

## Design (true-rail cell)

All 28 pairs of the exact 8-item smoke corpus (`smoke_corpus.txt`, the
corpus whose sort run reported 0.277), criterion "usefulness as advice",
openai/gpt-5.6-terra, ratio_letter_2p_v1, both presentation orders × 2
repeats = 112 comparisons (224 gateway calls), $0.43. Presentation-frame
signed log-ratio m (+ = slot-A favored); per pair b = (m̄_fwd + m̄_rev)/2
(slot term), s = (m̄_fwd − m̄_rev)/2 (signal). Within-call noise σε pooled
from same-order repeat differences (56 df). var_obs(b) = var(β) + σε²/4.

## Measured

| quantity | true 2p rail | JSON rail (24-item cell / smoke-corpus cell) |
|---|---|---|
| σε (nats/call) | **0.215** | 0.141 / 0.128 |
| global slot bias | +0.013 | −0.025 / +0.015 (sign-unstable ⇒ ≈ 0) |
| hetero var(β) | **0.0000** (noise = 103% of var_obs) | 0.0000 / 0.0025 (weakly resolved) |
| 1-draw order-residual analog 2·E\|b\| | 0.200 (noise-only prediction 0.243) | 0.158 (prediction 0.147) |
| signal sd(s) | 0.202 | — |
| per-call noise/signal | **1.06** | — |

The sort smoke's 0.277 sits within 16-pair sampling spread of the 0.243
noise-only prediction. Nothing systematic remains to attribute to order.

Equal-cost backtest (24-item JSON cell, 2 calls/pair, Spearman vs 4-draw
counterbalanced truth): counterbalance 1-draw/order ρ=0.938; single-order
2-draws raw ρ=0.970; +global correction ρ=0.961. With var(β)=0 these are
theoretically tied (both are noise averaging; correction subtracts ≈0);
counterbalancing additionally keeps the residual diagnostic, so it stays
the default.

## Consequences

1. **A-a (global slot-term fit) and A-b (slot randomization): dead.**
   There is no slot bias to correct on either rail.
2. **"syst order" is a misnomer on the 2p rail**: mean |m_AB+m_BA| there
   measures per-call noise (≈ 2σε·√(1/π)·… ≈ 0.24 at 1 draw), not
   systematic asymmetry. NORTH carries the naming caveat.
3. **The gauge under-reports on the 2p rail**: per-call noise 0.215 vs
   PMF-internal spread that yields reported stat ±0.050 — the honest
   per-observation variance needs a σ_w term (the DL-floor concept already
   in the codebase). This replaces symmetrization as the top open lever,
   with repeat-averaging (cache-priced) as the cost knob. Design seed:
   with zero slot bias, the run's own counterbalance diagnostic is a
   self-calibrating σ_w estimator — b ~ N(0, σε²/2) at 1 draw/order gives
   σε = (order residual)·√π/2 ≈ 0.886·residual (validated here:
   0.886 × 0.277 = 0.245 vs 0.215 measured directly). No hardcoded
   constants; per-run, per-rail. Caveat: only valid where the bias term
   is ≈ 0 (measured true for terra 2p/JSON; NOT verified for judges with
   real global bias, where residual ≠ pure noise and inflation would
   double-count).
4. The JSON rail's lower residual (0.120–0.158) reflects lower per-call
   noise (σε ≈ 0.13), not better symmetry.
5. **Landed same day (honest-σ refit):** consequence 3's design seed is
   in the core — σ_w = residual·√π/2 from the run's own counterbalance
   diagnostic, folded into every evidence observation's variance by an
   end-of-run re-ingest. Live re-run of the same smoke cell: residual
   0.290 → sigma_w 0.257, stat ±0.050-class → ±0.133 labeled
   `posterior incl sigma_w`, rank risk honestly 3.4 (adjacent ranks
   within 1σ at this budget, said out loud). $0.12.

## Rerun

    python3 run_true2p.py && python3 analyze_true2p.py   # load-bearing cell
    python3 run_cell.py && python3 analyze.py            # JSON-rail 24-item cell (see errata)
    python3 run_smoke_sigma.py && python3 analyze_smoke.py

Total spend this pack: $1.14 across the three cells.
