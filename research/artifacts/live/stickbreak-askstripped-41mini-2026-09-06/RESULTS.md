# Ask-stripped probe: the impact_per_dollar reversal was the price tag

Paired packs: `stickbreak-askstripped-41mini` + `stickbreak-askstripped-4omini`
($0.086 + $0.032). Same 12 items/seed as the day's other packs but corpus
`full_noask.json` (ask-stripped corpus) (every `ASK:` line deleted), impact_per_dollar only,
k=4, stickbreak + pairwise at budget 160. Probe designed by an independent
Grok exploration, which first identified the mechanism in the existing
packs: stickbreak impact latents tracked −log(min_ask) at +0.70…+0.78
(cheap wins) while pairwise tracked +log(min_ask) (expensive wins) — in
the original AND the rubric-fixed packs, so neither rubric truncation nor
an explicit "cheap is not better" sentence breaks it.

**Prediction confirmed, both models.** With ASK lines deleted:

| quantity | with asks (rubricfix) | asks stripped |
|---|---|---|
| sb vs −log(ask), 4.1-mini / 4o-mini | +0.70 / +0.78 | **+0.12 / +0.19** |
| pw vs +log(ask) | +0.54 / +0.92 | **+0.25 / −0.19** |
| sb-vs-pw within model | −0.46 / −0.78 | **+0.52 / +0.60** |
| cross-model transport, sb / pw | 0.79 / 0.62 | 0.50 / 0.32 |

The "two opposed constructs" reading is retired: the anti-correlation was
ask-line salience read in opposite directions by the two framings — a
lineup with four visible ASK lines rewards the small price tag; an
isolated pair largely ignores the dollar and rewards the grander blurb.
Remove the price tag and the instruments converge on a shared construct
(sb-vs-pw +0.5…+0.6, about as high as pw's noise floor permits), with
stickbreak still transporting better than pairwise (0.50 vs 0.32) on the
de-asked construct. This is a prompt/corpus-design result, not an
elicitation-theory result: for price-sensitive attributes, what the
context window makes visible IS part of the instrument's identity.

**Left open:** neither residual construct is "correct" — the rubric asks
for marginal cost-effectiveness at the min ask, which arguably NEEDS the
ask visible; an instrument that obeys the dollar too literally and one
that ignores it are both miscalibrated to the rubric's intent. Encoding
the intended use of the ask into the rubric (e.g. "use the ask only to
normalize, never as a quality signal in either direction") and
re-measuring is the follow-on.
