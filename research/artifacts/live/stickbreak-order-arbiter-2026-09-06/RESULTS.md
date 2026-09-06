# Arbiter run: stickbreak vs a properly budgeted pairwise reference

**Errata:** the "k=5" arm never presented 5-item lineups — the chunk design
at n=12 makes ⌈12/5⌉=3 groups of 4, so it is a second k=4 protocol (found
2026-09-06 by the slot-bias gate, which saw no slot-E channel; same holds
for the scaled pack).

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

**Slot-bias gate (2026-09-06, `examples/stickbreak_slot_bias.rs`): the
positional-bias hypothesis is REJECTED as the explanation.** Fitting per-slot
additive channels over the stickbreak edges (two channels per edge,
β_{slot(a)} − β_{slot(b)}; multi-channel `bias_calibration`; edges
winsorized at the ratio ladder's ±ln 26 so deterministic-PMF tails cannot
dominate the least-squares offset step) finds a real but small primacy
gradient — pooled per attribute, earlier slots read hot and later cold
(e.g. theory_of_change A +0.60 → D −0.65 nats; B positive on all three
attributes, matching the first-pick histograms) — yet correcting it leaves
agreement with the pairwise reference essentially unmoved (pooled ρ:
−0.01→−0.01, +0.36→+0.32, +0.08→+0.09). The stable stickbreak-vs-pairwise
offset is NOT per-slot additive presentation bias; the remaining suspects
are full-lineup context effects (the judge weighs different evidence when
k items are visible than in isolated pairs) and the reference's own 0.74–0.92
retest instability. Fit caveat: the alternation hits its 50-round cap
(coupled channels + robust score step oscillate at small amplitude); beta
sign patterns are stable across per-arm and pooled fits.

**Per-pair probe (2026-09-06, replay of this pack — no new calls): the
pairwise reference carries almost no per-pair signal.** Comparing each
unordered pair's mean signed log-ratio across the two instruments:

| attribute | pairs | sign agree | pw median \|m\| | pw internal flips | sb internal flips |
|---|---|---|---|---|---|
| impact_per_dollar | 46 | 54% | 0.050 nats | 16/39 | 11/41 |
| theory_of_change | 42 | 55% | 0.003 nats | 38/42 | 9/37 |
| team_evidence | 42 | 52% | 0.027 nats | 19/36 | 6/37 |

The isolated pairwise judge contradicts itself across orientations/repeats
of the SAME pair on up to 90% of pairs, and its counterbalanced means sit
at ~0 nats — the reference's latents are noise aggregations, so ~50% sign
agreement is agreement with a coin, not evidence against stickbreak (whose
verdicts sit at the winsorized ceiling with 15–27% internal flips). Which
instrument is *accurate* is undetermined by this data: decisive-and-stable
can be stably wrong. One weak external anchor leans stickbreak's way: the 3
of 12 items that actually raised money rank better than chance (5.5) under
stickbreak in 8/9 arms (mean ≈ 4.0) and under pairwise in 1/3 arms
(mean ≈ 5.7) — anecdote-grade with 3 positives, recorded, not claimed. The
real remaining arbiter is rubric-grade ground truth (human labels or
planted-truth corpora), not more LLM cross-instrument comparison.
