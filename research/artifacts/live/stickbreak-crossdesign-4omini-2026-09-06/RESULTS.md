# Cross-design deconfound: transport is construct, not shared lineups

Probe designed by an independent Kimi exploration: the day's cross-model
transport numbers came from same-seed runs — identical lineups and
item-slot assignments for both models — so "shared construct" was
confounded with "shared presentation." This pack re-runs the 4o-mini leg
on the SAME 12 items (corpus pinned to the seed-17 slate,
`slate17.json`) with a seed-18 design: different subsets, different slot
orders, fixed rubric, $0.10.

**Confound exonerated.** ρ(4.1-mini seed-17 design, 4o-mini seed-18
design) vs the same-design pair:

| attribute | arm | same-design | cross-design |
|---|---|---|---|
| impact_per_dollar | sb k=4 | 0.79 | **0.69** |
| theory_of_change | sb k=4 | 0.40 | **0.34** |
| team_evidence | sb k=4 | 0.81 | **0.76** |
| impact_per_dollar | pw | 0.62 | 0.84 |
| theory_of_change | pw | 0.39 | 0.53 |
| team_evidence | pw | −0.03 | 0.07 |

Stickbreak transport survives design change nearly intact — the shared
construct, not the shared lineup, carries it. Bonus: within 4o-mini,
stickbreak latents reproduce across the two designs at +0.78…+0.91
(k=4), which retires the standing caveat that stickbreak's ~1.000
same-seed retest was mere protocol determinism — across different
lineups the instrument still ranks the same items the same way.

Side observation (`stickbreak-crossseed-4omini-2026-09-06`, the first
attempt at this probe, which accidentally drew a fresh 12-item slate):
on that different slate WITH asks, sb-vs-pw impact agreement is ~0
(+0.06/+0.15), not −0.8 — the ask-salience opposition's magnitude is
slate-dependent (that slate's ask spread: less extreme).
