# The design atlas: comparison graphs chosen by enumeration, not taste

`cargo run -p llmsort-experiments --example design_atlas` enumerates every circulant design
C_n(S) for n ∈ {8, 10, 12} and scores its invariant profile: edges
(budget), filled triangles (support for the LOCAL curl diagnostic),
harmonic dimension (support for the GLOBAL, triad-invisible diagnostic —
zero means that diagnostic is structurally dead), and the Fiedler value
(identifiability). Milliseconds of compute; the point is that design
intuition loses to exhaustive search even at n = 8.

## The headline row (2026-07-06)

| design | edges | triangles | harmonic_dim | fiedler |
|---|---|---|---|---|
| C₈(1,2,4) — the hand-picked JCB v1 graph | 20 | 16 | **0** | 4.000 |
| **C₈(1,3,4) — the atlas winner** | 20 | 16 | **1** | 4.000 |

Identical budget, identical triangle support, identical algebraic
connectivity — and one measurable global cycle instead of none. The
hand-picked design was one stride away from strictly better, and nothing
short of enumeration would have said so: the v1 harmonic block had to be
bolted on as a DISJOINT component (whose scores don't share the main
graph's gauge) precisely because the core graph couldn't host the
diagnostic. C₈(1,3,4) hosts both diagnostics on the SAME entities.

Recommendation (routed to #49): JCB v2's core graph is C₈(1,3,4) per
8-entity domain block. v1.x keeps its frozen design — versioned
instruments don't churn mid-version; the pinned test records both
profiles so any silent change surfaces.

## Other atlas findings

- **Harmonic-heavy instruments exist**: C₁₂(4,5,6) — 30 edges, only 4
  triangles, harmonic_dim 15. A design for measuring global cyclicity
  almost exclusively; useful as a dedicated probe when triad-invisible
  structure is the question.
- **Dual-diagnostic designs are the exception, not the rule**: of 15
  connected circulants at n = 8, exactly three have both triangles ≥ 4
  and harmonic_dim ≥ 1. Dense designs fill their cycle space with
  triangles (harmonic dies); sparse ones lose triangles (curl dies). The
  window is narrow, which is exactly why it must be searched.
- Best identifiability-per-edge at n = 8 with both diagnostics alive:
  C₈(1,3,4) at fiedler/edge = 0.200.

Full tables regenerate deterministically from the example. Extend the
enumeration (larger n, non-circulant block designs, weighted variants)
before choosing any future corpus graph — this file is the precedent.
