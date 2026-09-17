# Reader-priority attributes: what the battery assumes about the reader, twelve candidates that break those assumptions, and which of them a judge can actually see

Readers: Xyra and agents choosing which attributes to teach the fast judges next. Executed
2026-09-17 13:30–14:45 PT. Artifact: `../artifacts/live/reader-priority-attributes-2026-09-17/`.

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
| reader-empowerment, misleading-if-trusted | unresolved — one family with compounding-value to this teacher, or a joint-call artefact | rerun in separate triples |
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

(pending — independent reader)

## Whose judgment

The numbers are gemma-4-31b's reading of 20 entities per lens against its own reading of the
battery; the top/bottom item reads are mine; the sharpening cases below are an independent
reader's (a Fable subagent given the prompts and the existing family names, no numbers).

## The call

Teach the fast judge four attributes it cannot currently see: source-uniqueness, front-loading,
compounding-value, effort-to-value, plus discourse-currency for posts. That is one teacher pass
per lens on the v2 recipe (200 lists × 40, about $3 and 5 minutes per lens, then about an hour
of distillation on the shared card), with reader-empowerment and misleading-if-trusted rated in
separate triples in the same pass so the joint-call question closes for free. Retire
regret-if-missed as a target; the priority score for a human is a weighted composition the
human sets, over these plus the cost-paid metadata columns. The pass is new spend (about $9 for
three lenses), so it waits for a word; everything else here is done.
