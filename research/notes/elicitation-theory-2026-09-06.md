# What should work: first principles for subjective elicitation

2026-09-06. A derivation, not a survey of results. Each section states a
theoretical claim, what it predicts, and where today's data already
tests it. Companion empirical record:
`../artifacts/live/SYNTHESIS-2026-09-06.md`.

## 1. The estimand is a common factor, and instruments are raters

Under consistency epistemics (no external truth; intra- and inter-model
agreement is the arbiter), the construct "theory_of_change of item i" is
DEFINED as the common factor across competent readers of the rubric —
judge models × instruments are the raters. This is classical
psychometrics, and it says our pairwise ρ tables are the wrong final
shape: with three or more legs, fit one factor model; an instrument's
validity is its loading on the common factor, and its unique variance is
its idiosyncrasy. Two raters can never separate "shared construct" from
"shared quirk" — which is why the OpenAI-family confound is not a
nice-to-have but constitutive: below three sufficiently different
raters, "transport" is not yet a measurement of the construct.
Prediction: with a third leg (gemma), a one-factor fit will beat any
pairwise table at ranking instruments, and pairwise-isolated will show
high uniqueness (mostly noise variance), stickbreak low.

## 2. Subjective judgment is frame-relative; the lineup IS the frame

Psychophysics (adaptation level, range–frequency): magnitude judgments
of attributes with no natural zero are made relative to a reference
distribution the judge currently holds. An isolated pair supplies
almost no frame — so the judge falls back on superficial salience and
produces near-ties (observed: pairwise |m| ≈ 0.03–0.05 nats,
orientation self-contradiction up to 90%). A k-wise lineup supplies its
own frame — so judgments sharpen (observed), AND whatever the lineup
makes visible becomes part of the measurement (predicted by this claim;
observed as ask-salience before the theory was written down — the
theory retrodicts it, which is weaker than prediction but still binds).

The constructive consequence is not "sanitize the context" (an endless
game of whack-a-mole per attribute) but **standardize the frame**:
include a small fixed set of anchor items in every lineup, so every
judgment conditions on the same reference distribution. This buys, in
one move: (a) cross-slate comparability (today we observed the
ask-opposition is slate-dependent; a fixed frame removes slate
dependence by construction); (b) a magnitude gauge (anchors can be
pinned, giving the scale an origin and unit); (c) drift detection
(anchor–anchor edges should be constant across calls — a free
instrument-health channel in every call). Cost: k_eff shrinks by the
anchor count, so information per call drops by a known factor — worth
it whenever comparability across slates or runs matters.
Prediction: anchored lineups transport across slates at near
within-slate levels; unanchored ones do not.

## 3. Token probability is not choice probability

A near-greedy decode gives the winning letter p ≈ 1 whether the
underlying preference is 60/40 or 99/1: PMF sharpness conflates
preference strength with decode determinism. The behaviorally correct
confidence for "i over j" is the flip rate across independent framings
(shuffled presentations, different companion sets) — an actual choice
probability in the Thurstone/Luce sense. The PMF is a one-sample proxy
observed through the decoder's temperature policy.

**Tested today (zero new calls), and the theory survives.** On the
graded judge (4o-mini, both designs pooled; criterion = 4.1-mini
latents; pairs with ≥3 co-occurrences): behaviorally unanimous pairs
transport at **88%** (53/60) vs mixed at **69%** (31/45), while
high-vs-low PMF magnitude separates only 81% vs 77% — and within
behavioral strata, PMF magnitude adds nothing (unanimous: 89% vs 83%;
mixed: 66% vs 75%, inverted). Behavioral unanimity screens off PMF
magnitude. On the near-greedy judge (4.1-mini) the test cannot run:
92% of its pairs are unanimous — its flip channel is empty, which is
itself the point: a greedy judge has no behavioral magnitude channel,
and its 20-nat PMF tails are decode artifacts (all three explorers
converged on this independently; §3 says why).

Engineering consequences, in order of directness: (i) evidence weight
per edge should come from behavioral counts — a Beta/binomial posterior
over presentations — with PMFs at most a smoothing prior, not the
magnitude itself; (ii) presentations are not replicates to average but
draws to count, so more-presentations-at-temperature dominates
sharper-single-decodes (the exopriors nonce-draw mechanism is exactly
this, served cheaply by prefix caching); (iii) judge selection should
prefer graded decoders over greedy ones for any cardinal use.

## 4. Kill position bias by design, not regression

A per-slot additive correction is the wrong functional form for a
sequential readout (slot effects act on conditional distributions given
history, and stick-breaking propagates them multiplicatively). The
classical answer is balanced designs: across a subset's presentations,
rotate items through slots as a Latin square (each item visits each
slot equally often); across the run, balance pair co-occurrence (BIBD)
and keep the comparison graph an expander (the engine already computes
effective resistance — use it as the planner's objective). Then slot
bias cancels in first order before any model is fit, and the
bias_calibration channels become a verification instrument rather than
a correction. Today's design does neither (items do not rotate through
slots; pair cover is incidental). Prediction: a Latin-square
presentation design shrinks the fitted slot betas to noise without any
correction step.

## 5. Measurement identity is a 4-tuple

Everything above collapses into one typing rule: a measurement is
(construct/rubric, response instrument, context distribution, judge).
Today's "anomaly" was two different context distributions sharing a
rubric name. The openpriors Interpretation should carry the context
policy explicitly — anchored(k, anchor_set) | random-companions(k) |
isolated-pair — so records that condition differently cannot silently
pool. This is cheap now and impossible to retrofit later.

## Ranked implications (theory → next actions)

1. Anchored-lineup pilot (§2): strongest single design change; also the
   cheapest path to magnitudes that mean something.
2. Behavioral-count evidence weights (§3): replace k/2 PMF variance with
   binomial posteriors from presentations; verified predictor of
   transport as of today.
3. Latin-square presentations (§4): mechanical, kills the slot question.
4. Third rater + factor model (§1): gemma leg, then one-factor fit.
5. Context policy into the typed contract (§5): one field, now.
