# Arbiter run: stickbreak vs a properly budgeted pairwise reference

**Errata:** none yet.

Single run (n=1), `openai/gpt-4.1-mini` via OpenRouter, seed 17, n=12
Manifund items, 3 attributes, 120 setwise stickbreak calls (k=3/4/5, 4
presentations) vs pairwise at `--pairwise-budget 160`/attribute (149–160
used ≈ 2.3× the 66 pairs, vs 34/66 distinct-pair coverage in the scaled
run's default budget). Total spend $0.216 (cap $3). Instrument 52f5833 +
budget flag.

**The dense reference does NOT rescue agreement:** ρ −0.23…0.44 across the
nine arms — same range as the under-budgeted run. The divergence is not a
reference-coverage artifact.

**Test-retest across the two independent live runs (same protocol, fresh
API draws) localizes it:**

| instrument | retest ρ (3 attrs / 9 arms) |
|---|---|
| stickbreak (each of 9 arms) | **+0.986…+1.000** |
| pairwise (48- vs 160-budget) | +0.74 / +0.78 / +0.92 |

Stickbreak is essentially perfectly repeatable — its disagreement with
pairwise is a stable, systematic offset, not noise. (Caveat: same seed ⇒
same lineups, and the model is near-greedy on order tokens, so retest
reliability here is protocol determinism, not accuracy.) Pairwise's own
0.74–0.92 retest bounds any instrument's achievable agreement with it at
≈0.86–0.96; observed 0.1–0.4 sits far below, so the two instruments rank
these items genuinely differently.

**Prime suspect: slot-position bias in the order elicitation.** First-pick
histograms pull hard toward slot B: k=3 [5,8,3], k=4 [0,8,2,2], k=5
[2,6,2,2,0] (slot A almost never first at k≥4). The stick-breaking fold
inherits whatever the order judgment does, and a systematic B-first pull
survives 4 shuffled presentations as correlated error. This is precisely
the shape `bias_calibration`'s order channels fit — the promotion gate is
now: fit per-slot additive channels over the stickbreak edges (slot of a,
slot of b as signed channels) and re-measure agreement, before any accuracy
claim for the instrument.

**Cost:** stickbreak ≈ $0.0002/item/attribute vs pairwise ≈ $0.005 at this
budget (~25×). One run, one model.
