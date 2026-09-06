# Scaled stick-breaking run (n=12 · 4 presentations · k=3,4,5)

**Errata:** none yet.

Single run (n=1), `openai/gpt-4.1-mini` via OpenRouter, seed 17, n=12
Manifund items (1600 chars), 3 rubric attributes, 120 setwise calls
(48/36/36 at k=3/4/5), 4 presentations, chunk design. Total spend $0.110
(cap $3). Escalation of `stickbreak-order-2026-09-06` (same instrument,
52f5833).

**Mechanism robust at scale: 120/120 calls harvested.** Every call yielded
aligned per-slot PMFs; all nine arm graphs connected (components = 1). PMF
sharpness holds its shape: mean top-choice 0.932, 64/312 slots genuinely
graded (<0.9) — proportionally more soft slots than the n=8 run (12/66),
consistent with harder lineups at larger k.

**Agreement comparison is INCONCLUSIVE — the reference is under-determined.**
ρ vs pairwise runs −0.13…0.43 across arms, well below the n=8 run. But the
pairwise arm at n=12 keeps the default 48-comparison budget against 66
unordered pairs and covers only **34/66 distinct pairs** (0.73× budget/pair
vs 1.14× at n=8), with 13/144 refusals on top. A reference that never saw
half the pairs cannot arbitrate a full ranking; the drop indicts the thin
reference at least as much as the instrument. Cost per item: setwise
$0.0002–0.0007 vs pairwise ≈ $0.0018.

**Caveats / next:** promotion needs an arbiter that is not itself budget-
starved: either a pairwise reference budgeted to ≥2× pair cover (≈132+
comparisons per attribute at n=12), an offline planted-truth sweep at this
exact prompt shape, or cross-instrument triangulation (ratio + order +
stickbreak on the same items). One run, one model.
