# Reader-priority attributes: what the battery assumes about the reader, twelve candidates that break those assumptions, and which of them a judge can actually see

Readers: Xyra and agents choosing which attributes to teach the fast judges next. Executed
2026-09-17 13:30–14:35 PT. Artifact: `../artifacts/live/reader-priority-attributes-2026-09-17/`.

## The question and the denominator

The ask: attributes worth rating text by so a human can decide what to read *first* — key, and
differentiated from what we already rate. The denominator is not small. Authored prompts:
`experiments/data/catalog_axes_*.jsonl` 2,014 axes in 508 families (four wordings per family,
three lenses), `research/batteries/` fable_top700 (700), fable_subtle_1000 (1,010), highdim (12),
manifund (32), judge_bakeoff (6) — about 2,200 prompts. Reader-relation attributes are already
among them: actionability, timelessness, archive-durability, delta-to-informed-reader,
importance-of-topic, skimmability, prerequisite-flagging/mapping, canonical-pointing, urgency,
time-sensitivity-truth, audience-service, rereadability. So "differentiated" has to be measured,
not asserted, and against the rated battery, not the prompt list.

The rated battery (host ClickHouse, gemma4-31b, the same 20 entities per lens) has this shape:
lesswrong-posts 714 axes in 183 families, within-family wording r median .48, all-pairs |r|
median .19, PC1 17%, 11 components for 80% of variance — a genuinely multi-dimensional read.
lesswrong-comments 557 axes, within-family .85, PC1 56%, 4 components for 80% — one general
quality factor. manifund-proposals 726 axes, PC1 32%, 9 components. (n=20 per lens: SE of a
correlation is about .22; everything below is read at that resolution.)

## The ideonomy move

`ideonomy draw` forced three lenses on the subject: Lemmology × relax (what does the battery
assume?), Icology × hybridize (cross the object with what it is not), Alleloschemology ×
magnify. The Lemmology read is the productive one. Every existing attribute treats value as a
property of the text alone. The battery assumes, without saying so: (1) value is text-only, not
text × reader × time; (2) reader time is unbounded; (3) reading is linear and complete; (4)
value is positive-additive — no reading cost on the ledger; (5) texts are independent — no
dependency structure between them; (6) the reader arrives with no prior exposure. Gunkel's
importance-detectors add the operator: read what matters off a cost paid, not a claim made.

Relaxing each assumption gives a candidate. Twelve were written (exact prompts in
`cand.criteria.json`), the twelfth an aggregate target to test whether priority can be asked
directly:

| # | attribute | assumption relaxed |
|---|---|---|
| 1 | front-loading | linear complete reading — value after the first fifth |
| 2 | portable-payload | complete reading — what can be carried away in a sentence |
| 3 | read-now-premium | time — what is lost by reading a month late |
| 4 | source-uniqueness | text-only — can the payload be had elsewhere |
| 5 | canonicality | independence — is this the text to send someone to |
| 6 | breadth-of-consequence | one reader — how many kinds of reader act differently |
| 7 | reader-empowerment | positive value — capability gained, not information |
| 8 | misleading-if-trusted | positive value — the cost of trusting it |
| 9 | compounding-value | independence — makes later reading more valuable |
| 10 | effort-to-value | no reading cost — value per effort |
| 11 | discourse-currency | time × community — the text being responded to now |
| 12 | regret-if-missed | the aggregate: regret a year later at never having read it |

## Method

Same teacher and design as the corpora (`google/gemma-4-31b-it`, joint setwise, n=20, k=8,
overlap 2, two presentations, Huber-IRLS latents), rated in four triples on exactly the 20
entities per lens the existing battery was scored on. 12 lists, 72 calls, $0.07, 0.3 min, 0
malformed. Differentiation per candidate and lens: teacher flip (self-consistency), R² on the
existing battery's top 4 principal components, r to the semantically nearest existing families
where those were rated on the same entities, r among the candidates, and leave-one-out
predictive r for the aggregate target. Permutation nulls are essential at this n: the max |r| of
a random vector to *some* one of 700 axes is .64 (95th percentile .75), so "nearest existing axis
r=.75" means nothing and that column is dropped; R² on 4 PCs has null median .20, 95th
percentile .39–.44.

## Results

R²@4PC marked * exceeds the permutation 95th percentile (the candidate is substantially inside
the existing battery's main dimensions). "nearest family" is the composite of the semantically
nearest existing family rated on the same entities.

**lesswrong-posts** (null95 R²@4PC .44)

| candidate | flip | r regret | R²@4PC | nearest family (r) | nearest candidate (r) |
|---|---|---|---|---|---|
| front-loading | .19 | +.67 | .35 | explanatory-compression −.04 | discourse-currency +.72 |
| portable-payload | .13 | +.12 | .32 | generality +.71 | compounding-value +.72 |
| read-now-premium | .23 | +.59 | .54* | timelessness −.49 | discourse-currency +.77 |
| source-uniqueness | .16 | −.05 | .16 | novelty-of-insight +.13 | effort-to-value −.56 |
| canonicality | .26 | +.50 | .37 | novelty-of-insight +.35 | portable-payload +.71 |
| breadth-of-consequence | .24 | +.41 | .44* | generality +.50 | canonicality +.69 |
| reader-empowerment | .12 | −.08 | .32 | actionability +.27 | compounding-value +.75 |
| misleading-if-trusted | .14 | +.27 | .33 | urgency-honesty +.52 | compounding-value −.74 |
| compounding-value | .07 | −.10 | .14 | generality +.67 | reader-empowerment +.75 |
| effort-to-value | .28 | −.44 | .29 | idea-density −.18 | source-uniqueness −.56 |
| discourse-currency | .14 | +.92 | .63* | importance-of-topic +.26 | regret-if-missed +.92 |
| regret-if-missed | .21 | 1 | .55* | delta-to-informed-reader +.44 | discourse-currency +.92 |

**lesswrong-comments** (null95 .39)

| candidate | flip | r regret | R²@4PC | nearest family (r) | nearest candidate (r) |
|---|---|---|---|---|---|
| front-loading | .15 | +.41 | .29 | skimmability +.30 | portable-payload +.56 |
| portable-payload | .21 | +.09 | .36 | — | reader-empowerment +.75 |
| read-now-premium | .11 | +.43 | .35 | timing-relevance +.59 | discourse-currency +.60 |
| source-uniqueness | .15 | −.22 | .20 | canonical-pointing +.06 | compounding-value −.76 |
| canonicality | .47 | +.14 | .04 | canonical-pointing +.03 | breadth-of-consequence +.73 |
| breadth-of-consequence | .34 | +.04 | .09 | — | canonicality +.73 |
| reader-empowerment | .14 | +.12 | .25 | — | compounding-value +.85 |
| misleading-if-trusted | .18 | −.13 | .23 | claim-calibration +.19 | reader-empowerment −.77 |
| compounding-value | .17 | +.25 | .37 | — | reader-empowerment +.85 |
| effort-to-value | .16 | −.53 | .36 | audience-service −.39 | discourse-currency −.59 |
| discourse-currency | .22 | +.88 | .45* | timing-relevance +.69, thread-advancement +.69 | regret-if-missed +.88 |
| regret-if-missed | .21 | 1 | .34 | — | discourse-currency +.88 |

**manifund-proposals** (null95 .43)

| candidate | flip | r regret | R²@4PC | nearest family (r) | nearest candidate (r) |
|---|---|---|---|---|---|
| front-loading | .07 | +.60 | .63* | — | read-now-premium +.85 |
| portable-payload | .27 | +.58 | .58* | — | read-now-premium +.82 |
| read-now-premium | .18 | +.66 | .49* | time-sensitivity-truth +.63 | front-loading +.85 |
| source-uniqueness | .12 | +.74 | .87* | — (research-taste#a +.88) | regret-if-missed +.74 |
| canonicality | .21 | +.53 | .53* | — | breadth-of-consequence +.73 |
| breadth-of-consequence | .15 | +.21 | .54* | interdisciplinary-reach +.41 | canonicality +.73 |
| reader-empowerment | .15 | +.31 | .19 | — | compounding-value +.62 |
| misleading-if-trusted | .15 | −.60 | .38 | — | compounding-value −.71 |
| compounding-value | .10 | +.46 | .19 | — | misleading-if-trusted −.71 |
| effort-to-value | .11 | −.23 | .22 | — | portable-payload +.52 |
| discourse-currency | .27 | +.88 | .45* | urgency +.35 | regret-if-missed +.88 |
| regret-if-missed | .19 | 1 | .47* | — | discourse-currency +.88 |

Leave-one-out prediction of regret-if-missed: from the existing battery's top 4 PCs alone
.53 / .34 / .34 (posts / comments / manifund); from discourse-currency alone .91 / .87 / .88;
adding any other single candidate to the 4 PCs moves it by at most +.2. The twelve candidates
themselves span 3–4 dimensions (80% of their variance), not twelve.

## Reading

**The aggregate target collapses onto currency.** regret-if-missed correlates .88–.92 with
discourse-currency in all three lenses, with no other candidate above .67. On posts the
teacher's top regret items are *Where I agree and disagree with Eliezer*, *What just happened? A
retrospective of AI alignment*, *SolidGoldMagikarp*; its bottom three are the LessWrong album,
*The Company Man*, and *Eight Short Studies On Excuses*. Asked "what would a thoughtful reader
regret a year later never having read", gemma answers "what is central to the alignment
conversation now". compounding-value ranks the *Rationality: A–Z* preface, *The Best Textbooks
on Every Subject* and *Humans are not automatically strategic* on top — the durable set — and is
uncorrelated with regret (−.10). So priority cannot be asked for directly: a single aggregate
prompt is one component wearing the aggregate's name. Priority has to be composed from
component attributes with explicit weights the human owns, which is what a battery is for.

**Where the existing battery is blind.** On comments the battery's four main components predict
regret at .34 leave-one-out while currency alone predicts it at .87; the general quality factor
that dominates the comment battery is not what makes a comment worth reading. The families that
do carry it, timing-relevance and thread-advancement, exist (r .69 to currency) but are two of
141 and sit outside the main components. On posts there is no rated family near currency at all
(importance-of-topic .26).

**What the teacher can see that the battery does not.** Keeping candidates with flip ≤ .20 in
at least two lenses, R²@4PC inside the null in at least two lenses, and no rated neighbour above
.5 on the text lenses: source-uniqueness (top posts *Making Vaccine*, *The Company Man*,
*Rationalism before the Sequences* — first-hand experience, exactly the intended reading; on
manifund it is absorbed by research-taste, R² .87), front-loading (explanatory-compression
−.04, skimmability .30 — the first-fifth read is not the skim read), compounding-value
(generality .67 on posts is the one real overlap), and effort-to-value (negative to regret in
every lens: the teacher's regret goes to long texts; flip .28 on posts is the worst reliability
among the keepers). reader-empowerment and misleading-if-trusted are not separate from
compounding-value to this teacher (r +.75/+.85/+.62 and −.74/−.77/−.71) — but all three were
rated in the same joint call, so the collapse may be the call format as much as the concepts; a
rerun in separate triples is the cheap check before ruling them one family.

**What is covered.** portable-payload is generality (.71 / .78 to its best wording) and
compounding-value (.72); read-now-premium is inverted timelessness (−.49/−.56) on posts,
timing-relevance (.79) on comments, time-sensitivity-truth (.63) on manifund; breadth-of-
consequence is generality (.50) plus interdisciplinary-reach (.54) and unreliable on comments
(flip .34). canonicality is unreliable on comments (flip .47 — the question is ill-posed for a
comment) and correlates with breadth-of-consequence (.73) where it is reliable.

## Coverage denominator

| candidate | label | by |
|---|---|---|
| source-uniqueness | named gap (posts, comments); covered (manifund: research-taste) | measured |
| front-loading | named gap (posts, comments); partly covered (manifund, R² .63) | measured |
| compounding-value | named gap; overlaps generality on posts (.67) | measured |
| effort-to-value | named gap; reliability .28 on posts needs the prompt sharpened | measured |
| discourse-currency | named gap on posts; replayable-unpromoted on comments (timing-relevance, thread-advancement); it is also what regret-if-missed measures | measured |
| reader-empowerment | replayable-unpromoted — one family with compounding-value across separate calls (.74) | measured, pass 2 |
| misleading-if-trusted | unresolved — co-rated with empowerment both times (−.66/−.36/−.72), repeat .31 on proposals | measured, pass 2 |
| reading-pleasure | covered on posts (imagery-vividness .73, humor-effectiveness .61); named gap on comments and proposals; the anti-currency axis | measured, pass 2 |
| mistake-prevention | the usable aggregate on LessWrong (not currency); covered on posts by breadth-of-consequence (.83) | measured, pass 2 |
| challenge-to-priors | covered (steelmanning .64 on posts; mistake-prevention .94 co-rated) | measured, pass 2 |
| portable-payload | replayable-unpromoted (generality) | measured |
| read-now-premium | replayable-unpromoted (timelessness inverted; timing-relevance; time-sensitivity-truth) | measured |
| breadth-of-consequence | covered (generality, interdisciplinary-reach) and unreliable on comments | measured |
| canonicality | ruled out (flip .47 on comments; no gap it fills on posts) | measured |
| regret-if-missed | ruled out as a target (it is discourse-currency) | measured |

Outside this mechanism entirely, and the largest gap: the cost-paid signals Gunkel's
importance-detectors point at — who replied, what cited it, what was built on it, how often it is
linked a year on. These are metadata joins, not text attributes, and no text-only judge can rate
them; they belong in the composition as columns from the corpus, next to the rated attributes.
Likewise text × reader (prior exposure, the reader's own project) needs a reader model, not
another prompt.

## Sharpening check

An independent reader (a Fable subagent given only the twelve prompts and the names of the
existing families, no numbers) was asked, for each candidate's closest existing neighbour, for
one case the two rank in opposite directions, and whether the pair is two attributes or one
with two wordings. All seven pairs separated:

- source-uniqueness vs novelty-of-insight: six months of the author's own sleep data concluding
  late caffeine hurts sleep — unique, not novel; a fresh Goodhart reframing three people posted
  the same month — novel, not unique. Uniqueness is supply of the payload, novelty its age.
- front-loading vs skimmability: thesis, result and evidence in paragraph one then 4,000 words of
  unbroken prose — front-loaded, unskimmable; crisp headings whose thesis lands in the
  conclusion — the reverse. Positional versus structural; they will correlate.
- read-now-premium vs timelessness: a permanent argument about model evaluation that a live
  policy consultation is quoting this week — both high, so inverted timelessness ranks it low
  and read-now ranks it high. A window can be open on non-decaying content.
- discourse-currency vs timing-relevance / thread-advancement: a late comment in a dead thread
  that states cleanly the position a dozen other threads are arguing this month — currency high,
  thread-level attributes low. Community-level versus thread-level.
- reader-empowerment vs actionability: "sign this by Friday" is actionable and transfers no
  capability; the inside-view/outside-view distinction with no directive is the reverse. The
  reader notes empowerment sits close to generality plus explanatory-compression and should be
  named as transferable skill if kept.
- compounding-value vs prerequisite-mapping: a post that coins a term the community uses for
  years with no scaffolding, against a comment that lists what to read first and adds nothing to
  carry forward. Caveat from the reader: compounding is an ecosystem fact about later texts,
  which a text-only rater cannot see — it estimates it from foundationality.
- misleading-if-trusted vs claim-calibration: a post that hedges every sentence correctly and
  omits the one counterexample that flips its conclusion — calibrated and misleading. Omissions
  are invisible to calibration.

Among the twelve, the reader predicted a careful human could not separate: regret-if-missed
from breadth-of-consequence (both "how much does this matter"), canonicality from
source-uniqueness, portable-payload from effort-to-value and from reader-empowerment,
compounding-value from canonicality, and regret-if-missed from nearly everything ("a summary
rating with no independent evidence in the text"). The measured teacher agrees on
portable-payload ↔ compounding-value/reader-empowerment (.72/.75) and on regret being a summary,
but not on which summary — for the teacher it was currency (.9), not breadth (.4/.0/.2); and it
separated canonicality from source-uniqueness (r ≤ .3 in every lens), though canonicality's own
reliability was too poor to count that as evidence.

Asked why regret collapses onto currency, the reader's diagnosis matches the item read above: a
text-only judge has no access to reception, so both prompts reduce to the one cue the text
supplies for "mattering to the community" — recognisability as a central, argument-bearing
alignment document, famous names and titles seen cited in pretraining — which the album, the
fiction and the parables fail even though a human might regret missing those most. The currency
prompt is honest about what it measures; the regret prompt is the mis-worded one, because "a
thoughtful reader in the intended audience, a year later" is a reception forecast in disguise,
and the cheapest proxy for reception is prominence. A regret signal that is not currency has to
be reader-side and counterfactual — what this reader would believe or do wrongly had they not
read it — with no reference to audience or time.

Asked what neither list has that matters more for choosing what to open first, the reader named
three; checked against the 2,200 authored prompts, none exists:

- reading-pleasure — "how much a reader would enjoy the act of reading this, independent of
  anything taken away". Every attribute in both lists is instrumental (the nearest is
  humor-effectiveness); pleasure is the strongest single driver of what people actually open
  first, and it is exactly what the teacher scored at the bottom.
- mistake-prevention — "how likely a reader who never reads this ends up making a specific
  costly decision or holding a specific false belief that this text would have prevented". The
  counterfactual loss regret-if-missed was reaching for, stated without routing through the
  community.
- challenge-to-priors — "how much a reader who currently disagrees with the central claim would
  be forced to update or sharpen their view after reading it". delta-to-informed-reader measures
  new information; steelman-strength measures the author's handling of the opposing case; this
  measures the text as the strongest opposing case for its reader.

## Second pass: the reader's three, and a repeat read

The three were rated the same way at 14:25 PT (6 lists, $0.03, 0 malformed), in one triple, with
reader-empowerment, source-uniqueness and misleading-if-trusted re-rated in a different triple so
the joint-call question gets a repeat read. Repeat agreement with pass 1 (same prompt, different
companions): reader-empowerment .92 / .83 / .75, source-uniqueness .87 / .65 / .90,
misleading-if-trusted .89 / .73 / .31 — the teacher is stable on the first two and unstable on
misleading-if-trusted for proposals. reader-empowerment ↔ compounding-value holds across
separate calls (.74 on posts), so those are one family to this teacher; empowerment ↔
misleading (−.66 / −.36 / −.72) was co-rated both times and stays unresolved.

| attribute | flip (P/C/M) | R²@4PC (null95 .45/.39/.47) | r regret | r currency | reads as |
|---|---|---|---|---|---|
| reading-pleasure | .12/.16/.21 | .70* / .40 / .36 | −.63/−.59/+.10 | −.75/−.73/+.15 | posts: imagery-vividness .73, humor-effectiveness .61; top *The Company Man*, *Orienting Toward Wizard Power*, the album; bottom *Making Vaccine*, the book announcement, *Best Textbooks* |
| mistake-prevention | .21/.10/.12 | .64* / .39 / .37 | +.43/+.50/+.61 | +.17/+.26/+.68 | posts: breadth-of-consequence .83, challenge-to-priors .94 (co-rated); top *Only Law Can Prevent Extinction*, *Schelling fences*, the alignment retrospective |
| challenge-to-priors | .16/.24/.12 | .68* / .36 / .27 | +.36/+.53/+.74 | +.09/+.30/+.56 | posts: steelmanning .64, breadth .73; same top three as mistake-prevention |

reading-pleasure is the mirror of currency on LessWrong (−.75 posts, −.73 comments): the texts
the teacher says a reader would enjoy are exactly the ones it says nobody would regret missing.
That is the cleanest statement of what the battery's "matters" reading leaves out. On posts the
battery already has it under other names (imagery-vividness, humor-effectiveness — R² .70), on
comments and proposals it does not. mistake-prevention does what regret-if-missed was meant to
do — an aggregate that is not currency on LessWrong (.17 / .26) — but on posts it is the
battery's breadth/"matters" dimension (R² .64, breadth .83), and it does not separate from
challenge-to-priors when co-rated (.94); challenge-to-priors adds nothing over steelmanning on
posts and is the weaker of the pair elsewhere.

## Whose judgment

The numbers are gemma-4-31b's reading of 20 entities per lens against its own reading of the
battery; the top/bottom item reads are mine; the sharpening cases below are an independent
reader's (a Fable subagent given the prompts and the existing family names, no numbers).

## The call

Teach the fast judge six attributes it cannot currently see: source-uniqueness, front-loading,
compounding-value (carrying reader-empowerment), effort-to-value, reading-pleasure, and
discourse-currency, with mistake-prevention as the aggregate to calibrate a composition against.
That is one teacher pass
per lens on the v2 recipe (200 lists × 40, about $3 and 5 minutes per lens, then about an hour
of distillation on the shared card), with misleading-if-trusted rated apart from empowerment
in that pass so its one open question closes for free. Retire
regret-if-missed as a target (mistake-prevention is the counterfactual it was reaching for); the priority score for a human is a weighted composition the
human sets, over these plus the cost-paid metadata columns. The pass is new spend (about $9 for
three lenses), so it waits for a word; everything else here is done.
