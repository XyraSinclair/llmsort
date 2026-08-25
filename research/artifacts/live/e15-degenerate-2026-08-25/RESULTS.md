# E15 — degenerate pools: does the gauge stay one-sided? (2026-08-25)

Question: the order-sensitivity gauge's promise (flip < 0.20 ⇒ trust) was
measured only on pools of genuinely distinct items. Real lists carry
duplicates, near-duplicates, and boilerplate. Does the gauge notice — and
if not, what does the instrument report about the arbitrary order it
produces inside a degenerate cluster?

Frame: arXiv n=150, 105 kept originals (rng seed 15 drops 45 of the
sorted-id list; `data/arxiv150_corruption_map.json`), 45 corrupted items:

- **dup** — verbatim twins of 45 kept items under `-dup` ids
- **para** — deepseek-v4-flash paraphrases (temperature 0, reasoning
  disabled, claims preserved) under `-para` ids, frozen in
  `data/arxiv150_para.json`
- **stub** — 45 near-identical "withdrawn, no abstract" stubs

Cells: ring k=8 rounds=2 presentations=2 (run8/E13 flags verbatim),
deepseek-v4-flash (pinned providers) + gpt-5.6-luna, seeds 17+18,
3 arXiv attributes, `--skip-pairwise`; plus one pairwise σ cell
(dup, deepseek, seed 18, "methodological rigor"). 13 runs, 100
order-calls/attr each, **$1.071 total** (run11.sh; sentinel LIVE11_DONE).

## 1. The gauge does NOT notice degeneracy

Flip rates on corrupted pools sit at or below the clean run8 baseline:

| cells | flip range | clean baseline (run8) |
|---|---|---|
| deepseek dup/para/stub | 0.16–0.24 | 0.22–0.25 |
| luna dup/para/stub | 0.11–0.20 | 0.15–0.21 |

Mechanism: flips are an aggregate over ~1,300 re-presented pairs; ~45
degenerate pairs cannot move the rate. The gauge is a **pool-level**
screen by construction.

## 2. Within-cluster order is seed noise; across-cluster structure survives

| reading | dup | para | stub |
|---|---|---|---|
| twin sign agreement across seeds (45 pairs, chance ≈ 22.5) | 21–28 | 24–29 | — |
| cluster across-seed ρ | — | — | −0.22…+0.17 |
| kept-105 across-seed ρ | 0.73–0.92 | 0.73–0.91 | 0.56–0.86 |
| median twin rank distance within a run (of 150) | 10–17 | — | — |

Identical texts land a median ~13 ranks apart, and which twin ranks higher
flips with the seed at coin-flip rate. Meanwhile the real items' order
reproduces across seeds, and the stubs sort where they should (mean rank
126/150, min 90, on every attribute and both models).

## 3. σ covers it — the failure is confined to rank-only readers

Twin |Δmean| exceeds the joint 2σ band in only 0–4 of 45 pairs per cell
(12 cells, both variants, both models). Mean σ does not widen (setwise
0.226 dup vs 0.227 clean; pairwise 0.55–0.58 dup vs 0.570 clean) — it
doesn't need to: the twins' existing error bars already overlap. The
instrument's cardinal output honestly reports "indistinguishable"; only a
consumer who reads ranks and discards ±σ is misled.

## 4. Anomaly (unattributed)

luna dup cells lost calls to transport/API errors (s17: 23–24/100
errored; s18: 4–12) — `errored`, not refused/malformed. Para and stub
cells were clean (0–2). Cause not established (provider blips vs
identical-text payloads); parse rate among completed calls stayed 100%.

## Verdict

The gauge's one-sided promise survives at the level it was stated: flip
< 0.20 still marks pools whose **across-cluster** order is reproducible.
It acquires a named precondition: **the gauge certifies pool-level order,
never item-level distinctions.** Near-duplicate items receive arbitrary
relative order (~1/10 of the list apart in rank, seed-flipping), silently
— but inside joint 2σ. Doc consequence: read ±σ before trusting any
specific adjacent-pair distinction; rank gaps smaller than the error bars
are presentation, not measurement. No change to the gauge thresholds.
