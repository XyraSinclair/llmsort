# FINAL TEXT (2026-08-10) — venue decided: EA Forum, LessWrong crosspost after.

Wording, links, and venue are final per the 2026-08-10 ship directive; every
number traces to a committed evidence pack and all URLs below are live.
SENDING remains Xyra's alone — this file is the exact text awaiting her
explicit approval to post. Drafting is not permission to transmit.

---

# Two funding mechanisms, asked what they value, answered with their money

The EA community's stated first virtue in grantmaking is expected impact
per dollar. I measured what two real allocation mechanisms on the same
platform actually reward. One of them follows the stated value. It is not
the one you would guess.

## The measurement

Take the 83 proposals from ACX Grants 2024 and the 78 from EA Community
Choice (both hosted on Manifund). Score every proposal on four attributes
using pairwise ratio judgments from an LLM judge — thousands of "how many
times better is A than B on this attribute?" comparisons, solved into
cardinal scores with uncertainty (the machinery is open:
[pairwiseratio.org](https://pairwiseratio.org)). The four attributes,
worded precisely:

1. plausibility of the causal path from activities to claimed impact
2. expected impact per marginal dollar at the stated ask
3. verifiable track-record evidence the team can execute
4. epistemic integrity of the write-up: honest failure modes, quantified
   claims, falsifiable milestones

Then ask, separately for each mechanism: which weighted combination of
these four best reconstructs the realized dollar order? The predictions
were committed to a public git history before any comparison against
ground truth — the registration is the commit log
([ACX](https://github.com/XyraSinclair/llmsort/commit/ca1928c70be4d575426495b4548006398633c7d4),
[EA CC](https://github.com/XyraSinclair/llmsort/commit/892e0c0827de384601abeb5534a2a765f714e18f),
[second judge](https://github.com/XyraSinclair/llmsort/commit/e304a5dc666fa838aefc16beb0745c88e3d1a47e)),
not a promise. The original two-cohort measurement cost $1.43; the
second-judge replication and polish control added $7.07 (one judge
produced 4.5× the expected reasoning tokens — the overrun is quoted in
the pack, not hidden).

## The inversion

**The juried order (ACX Grants, one decision-maker plus advisors):
expected impact per dollar carries zero fitted weight.** Not small — 0.000,
under two judges from different model families (deepseek: EI 0.794, EV/$
0.000; kimi replication: EI 0.887, EV/$ 0.000). What reconstructs the
juried dollars is the epistemic integrity of the write-up — the attribute
the community's own discourse ranks last of these four. Alone it
correlates with the dollar order at ρ ≈ 0.36–0.37; EV/$ alone at ρ ≈ 0.05.

**The crowd order (EA Community Choice, quadratic matching over hundreds
of donors): impact per dollar is the strongest single predictor** (AUC
0.682 — stronger than any attribute achieves on the juried order). The
crowd walks its talk. The juried process rewards something else.

A control run: add "overall writing quality and polish of the prose" as a
fifth attribute. It takes zero weight and predicts almost nothing
(ρ 0.034). The juried order is not rewarding pretty writing that happens
to look like epistemic virtue. It is rewarding the specific content —
named failure modes, quantified claims, falsifiable milestones.

## What I think this is

Not a scandal. A juror who has learned that EV/$ claims are cheap talk,
and that honest self-assessment is the hard-to-fake signal of a mind that
will notice when its plan is failing, is arguably doing sophisticated
allocation — trusting revealed epistemics over professed arithmetic. And
a crowd distributing small sums by stated cost-effectiveness is doing
exactly what quadratic funding asks of it. Neither mechanism is the
villain. The finding is that **the gap between professed and revealed
values is a property of the mechanism, and it is now cheaply measurable.**
Mechanism design has argued about this in the abstract for decades. It
costs a few dollars to instrument it.

## Noise class, stated plainly

- One platform, and the two mechanisms were measured on two different
  proposal cohorts — mechanism and cohort are confounded. The
  within-cohort test (same proposals, regrantor dollars vs crowd dollars
  separately) is the registered next step.
- The "professed" ranking is itself an LLM's rendering of community
  discourse under an allocation prompt — soft, and flagged as such. The
  hard, twice-replicated claim is the cross-mechanism flip in what
  predicts realized dollars, which needs no professed leg.
- Funded-only dollar order is n = 41 (ACX). Judge position-flip rates:
  25% (deepseek), 16.7% (kimi), quoted in the packs.
- LLM attribute scores are a measurement instrument with its own priors.
  Both judges agree on the inversion's direction; that is two instruments,
  not ground truth.

## Everything is inspectable

Every judgment, prompt, trace, cost, pre-registration commit, and the
analysis scripts fixed before unblinding:
[original pack](https://github.com/XyraSinclair/llmsort/tree/main/research/artifacts/live/manifund-p2-2026-07-19),
[second-judge pack](https://github.com/XyraSinclair/llmsort/tree/main/research/artifacts/live/manifund-p2-secondjudge-2026-07-27).
The engine — prompts, robust solver, uncertainty, caching — is open at
[pairwiseratio.org](https://pairwiseratio.org)
([source](https://github.com/XyraSinclair/llmsort)). If you
think the wordings are doing hidden work, rerun with your own; the
wording is the axis, and that is half of what this instrument is for.

If you fund things — or design the mechanisms that do — I would genuinely
like to know what your mechanism reveals. It costs less than a coffee to
find out, and it is one command on your own longlist:

    cargo install llmsort
    llmsort sort proposals.txt --by "expected impact per dollar" --top-k 10

One proposal per line; you get the top ten with error bars and the
dollar cost printed underneath. If you would rather hand me the list
and the criterion, I will run it and send you the packet.

