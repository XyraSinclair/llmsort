# Cross-model transport: the consistency arbiter (gpt-4o-mini leg)

**Erratum (2026-09-06, found by an independent astra exploration):** the
impact_per_dollar rubric used by ALL 2026-09-06 packs was defective — it
ended mid-sentence ("Also do not reward the") and opened with
pairwise-specific phrasing ("which of two items") inside k-wise lineups.
The other rubrics are clean. This confounds the opposed-constructs claim
below specifically for impact_per_dollar: a truncated, pair-phrased rubric
can be read differently under lineup vs isolated-pair framing for
instrument-artifact reasons. The rubric is now fixed in
`research/data/manifund/rubrics/impact_per_dollar.md` (sentence completed,
count-neutral phrasing); re-measurement under the fixed rubric is required
before the opposed-constructs interpretation stands.

Single run (n=1), `openai/gpt-4o-mini` via OpenRouter, seed 17 — same 12
Manifund items, 3 attributes, and protocol as the arbiter pack
(`stickbreak-order-arbiter-2026-09-06`, judge gpt-4.1-mini): stickbreak
order elicitation at k=3/4 (84 setwise calls, 4 presentations) vs pairwise
at `--pairwise-budget 160`/attribute (126–160 used). Total spend $0.099
(cap $3). The k=5 arm was dropped (unrealizable at n=12; see arbiter
errata).

**Arbiter framing (operator, 2026-09-06):** these attributes are
subjective — there is no external ground truth, and the system's epistemics
are intra- and inter-model consistency. The arbiter is transport: run the
same instruments under a second judge model; the instrument whose rankings
different models share is measuring the common construct, not model
idiosyncrasy.

## Verdict: stickbreak transports, pairwise mostly does not

Spearman ρ between the two models' latents, per instrument:

| attribute | sb k=3 | sb k=4 | pairwise |
|---|---|---|---|
| impact_per_dollar | +0.72 | **+0.85** | +0.57 |
| theory_of_change | +0.33 | **+0.47** | +0.21 |
| team_evidence | +0.62 | **+0.86** | **+0.01** |

Per-pair sign agreement across models (66 latent-diff pairs for sb;
counterbalanced trace means for pw): stickbreak 59–86% (k=4: 86/67/86%),
pairwise 47–63% — at coin level on two of three attributes. Stickbreak
wins every cell, k=4 uniformly beats k=3 (richer lineups sharpen the
shared construct), and pairwise's team_evidence latents share nothing
across models — consistent with the arbiter pack's per-pair probe showing
those latents are aggregations of self-contradicting noise.

## The impact_per_dollar anomaly: two real, opposed constructs

Within each model, sb and pw ANTI-correlate on impact_per_dollar (4.1-mini
k=4 −0.23; 4o-mini k=4 **−0.87**) — yet BOTH instruments transport across
models (+0.85 / +0.57). Cross-instrument-cross-model confirms it: sb_A vs
pw_B = −0.66, sb_B vs pw_A = −0.52. Both models agree that lineup-elicited
"impact per dollar" and isolated-pair "impact per dollar" rank these items
in opposite orders. The divergence is not noise and not slot bias (both
already rejected) — the two elicitation framings measure two different
shared constructs for this attribute (plausibly relative-cost salience in
a lineup vs absolute-impact salience in isolation). Which construct the
caller wants is a prompt-design question, not an instrument defect.

The other two attributes show the ordinary pattern: pw carries little
transportable signal, sb carries most of it, and sb-vs-pw within-model
agreement (+0.11…+0.44) is bounded by pw's noise.

**Cost:** setwise $0.00014–0.00026/item/attribute vs pairwise
$0.0024 (~10–17×). Mechanism robust in a second model family: 84/84
setwise calls harvested, all arm graphs connected; 4o-mini shows the same
slot-B first-pick pull as 4.1-mini (k=3 [0,9,7]-shaped histograms),
so the primacy gradient is a shared model behavior, not a 4.1-mini quirk.

**Caveats:** one run per model, same seed ⇒ identical lineups (transport
is across models, not across designs); two models, both OpenAI-family
(logprobs via OpenRouter constrain the judge pool); n=12. A third leg
(e.g. gpt-4.1) would separate "OpenAI house style" from "shared
construct". A non-OpenAI leg was attempted same day
(`meta-llama/llama-3.3-70b-instruct`): the model answers the order prompt
cleanly ("B A C") but OpenRouter's serving providers returned top_logprobs
on 1/31 calls — no PMF harvest, leg aborted. Breaking the family confound
needs pinned provider routing (a `provider.order` knob in the instrument)
or a self-hosted logprob engine.
