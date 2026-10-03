# Red team: the doctrine itself (2026-07-09)

Target: `research/notes/PRINCIPLES.md` and the cultural rules around it — not the work
they govern. Question: is the operating system refutable, Goodhart-resistant,
and worth its cost? Method: doctrine text vs its own receipts (artifact packs,
commit log, notes). Six findings, ranked. Survivors listed at the end.

Denominators for this audit: 131 commits total; 15 commits whose messages
perform a self-correction; 28 artifact packs under `artifacts/live/`; 3 packs
carry errata; doctrine is 12 principles, ~110 lines.

---

## Finding 1 — §8 dogfooding has no exit: the validation loop closes through one person

**Receipt.** `research/notes/PRINCIPLES.md:72-75`: "Choose judges by our own coherence
benchmark; prioritize our roadmap with our own ANP; audit our own sessions
with our own probes' discipline." The flagship validation,
`artifacts/live/corpus-map-500-2026-07-08/README.md:15`: "validation vs 2026
scores, ambition (fused) | 0.903" — the reference scores are the operator's
own annotations of the operator's own prompts (commit 720bffe: "reproduces
her pre-existing annotations"). `README.md:322-324` already names the gap:
"human or high-budget external reference judgements" is the "next empirical
proof target" — acknowledged since the evidence section was written, never
doctrinal.

**Failure mode, concretely.** Every layer of the strongest claim in the repo
("intellectual ambition and epistemic rigor are independent axes of this
corpus") is self-referential: corpus = operator's writing, reference =
operator's labels, judges = chosen by our own benchmark, fusion = our own
solver. If the operator's annotation style has a latent that the judges also
pick up (both are reading the same rhetorical surface), rho 0.903 measures
shared surface reading, not transmissible structure. §8 makes this loop a
virtue and nothing in the doctrine ever forces the loop open. Dogfooding is
the fastest bug source for the INSTRUMENT; it is structurally incapable of
validating the MAP.

**Minimal edit.** §8, replace the last sentence ("Self-application is
simultaneously the demo, the test, and the fastest source of real bug
reports." — 16 words) with: "Self-application is the demo and the fastest
bug source; it is never the only validation — each map version scores at
least one reference the stack didn't produce and we didn't write." (+13 net
words. The only finding where adding words is the honest move; everything
else here deletes.)

**30-day test.** Does any pack exist by 2026-08-08 whose reference column
was produced by neither the operator nor the stack (external annotator,
public benchmark with published labels, second human)? Binary; currently 0
of 28 packs qualify.

---

## Finding 2 — §9's "Distribution > capability" is decoration: it lost a direct conflict with §10 for three days and nobody logged the loss

**Receipt.** `research/notes/PRINCIPLES.md:84-85`: "Distribution > capability once the
capability exists." `notes/ideation-2026-07-05/differentiation.md:194`: "No
new instrument variants before the artifact layer ships." What actually
shipped between that sentence and the artifact layer (receipt viewer,
974fb36, 2026-07-08): DL floor (c14d966), canonize (e974fe8),
sum-over-histories + design atlas (b773884), portfolio theory (516409e),
nonce draws (8827129), repeat-elicitation stack (175b406), judgment packets
(cb44831), slate (bd4d112), stochastic transitivity (02e61aa) — roughly nine
new capabilities before one artifact.

**Failure mode, concretely.** §10 ("The transformation group is the
roadmap") is a generative principle: it always has a next ✗ row, and each
row is locally virtuous under §1-§7. §9's distribution clause is the only
counterweight, and when the two collided, §10 won 9-to-1 silently — no
erratum, no "we chose to break the moratorium because X." A principle that
can be violated nine times in three days without anyone noticing is not a
principle; it is décor. Note the repo's own differentiation doc calls
toolbox-not-artifact "the single thing that feels off" — so this is the
doctrine failing at exactly its self-identified biggest strategic risk.

**Minimal edit.** Delete "Distribution > capability once the capability
exists." (-7 words). Either the moratorium form binds (in which case it
lives as a frontier issue with a checkable condition, where its violation
is visible) or the slogan goes. Keeping a slogan that demonstrably does not
govern trains readers that the doctrine is aspirational.

**30-day test.** Ratio of new-instrument commits to shipped-artifact commits
over the next 30 days. It was ~9:1 under the slogan. If deleting it (and
routing the binding form to an issue) moves the ratio toward the 2:1-ish
the differentiation doc wanted, the edit mattered; if the ratio is
unchanged, the problem is deeper than wording and §9 needs a real
counterweight, not a better sentence.

---

## Finding 3 — §1's "session health metric: self-refutation count" is an uncounted metric that has already colonized the prose register

**Receipt.** `research/notes/PRINCIPLES.md:12-13`: "**Session health metric:
self-refutation count.**" No session log, pack, or commit anywhere records
that count — the metric has never been computed. Meanwhile 15 of 131 commit
messages perform self-correction as narrative drama: "the test lost the
argument" (02e61aa), "the margin test lost its own argument", "the algebra
corrected itself in test" (1bfe56b), "the first implementation failed its
own planted test" (b773884), "two self-corrections en route".

**Failure mode, concretely.** Both Goodhart directions are open. Direction
one (manufacture): the commit register shows refutations are already
treated as trophies; the step from "dramatize real refutations" to "prefer
work that generates refutation theater" (refuting naive strawmen — e.g.
theory.md:210 refutes "elicitation forms a group," a hypothesis nobody
held) is small and invisible, because nothing distinguishes a load-bearing
refutation (changed shipped code: BTreeMap fuse fix, dead-branch deletion)
from a rhetorical one. Direction two (avoid): a session can stay "healthy"
by only making claims pre-shrunk to survive. Today the refutations are
mostly load-bearing — each of the 15 is attached to a real diff — so the
metric is pre-gamed, not gamed. But it is stated as a METRIC while being
uncountable as one, which is the worst of both: it can't detect direction
two, and it licenses direction one.

**Minimal edit.** Delete the bolded sentence "Session health metric:
self-refutation count." (-6 words). The next sentence — "A session that
never proved itself wrong wasn't testing anything." — carries the entire
qualitative content and is not gameable, because it names an absence, not
a score to maximize.

**30-day test.** Two counts, both cheap from the commit log: (a) fraction
of self-correction mentions attached to a same-commit code/test diff
(load-bearing) vs prose-only; (b) whether commit-message drama density
changes after the metric sentence is gone. If (a) stays ~100% and (b)
drops, the drama was metric-chasing; if both are unchanged, the culture
was never leaning on the sentence and deleting it was free.

---

## Finding 4 — §4 is selectively enforced: stories about models are contamination, stories about our instruments are branding

**Receipt.** `research/notes/PRINCIPLES.md:40-41`: "never the personality
('stiffens', 'follows the asker') ... never as decoration." Versus
`artifacts/live/spin-probe-2026-07-05/README.md:29`: "The magnet analogy
lands exactly" — asserted in the same pack that says "n = 1 pair" about
its own data. `artifacts/live/corpus-map-500-2026-07-08/README.md:30-31`:
"The map knows the difference between reaching, checking, and venting" —
a personality attribution ("knows") of precisely the kind §4 bans, aimed
at our own product. Also "not a halo wearing two names" (same file,
line 24), "two instruments, one conclusion" (judge-portfolio README).

**Failure mode, concretely.** The asymmetry is self-serving in the exact
direction that hurts: flattering narrative about the judged models would
be caught by any reader steeped in §4; flattering narrative about OUR
instruments is where motivated reasoning actually lives, and it is
indulged. "The magnet analogy lands exactly" on n=1 is a claim §3 would
reject if a model vendor made it about their model. To be fair to the
practice: in every cited case the number appears WITH the story, the
story never replaces the number, and the analogies (χ, linear response)
are §4-literal. The practiced norm — number first, narrative anchored to
it — is defensible and arguably the repo's best communication asset. The
written norm ("stories are contamination") is simply not the practiced
one, and a doctrine whose flagship stylistic rule is visibly violated by
every flagship artifact teaches readers to discount the rest.

**Minimal edit.** Retitle §4 from "Mathematics is the register; stories
are contamination" to "Mathematics is the register" (-4 words), and
append to the body: "This binds hardest on our own instruments." (+7
words; net +3). That converts a false absolute into the rule actually
enforced, and points the ban at the spot where it is currently weakest.

**30-day test.** Grep new packs for instrument-personality verbs ("knows",
"lands exactly", "sees") not adjacent to a number-with-denominator. The
edit mattered if such phrases either disappear or acquire their
denominator; it failed if READMEs keep two registers with the doctrine
pretending there is one.

---

## Finding 5 — Receipt theater at the sub-cent scale, and "errata on top" is violated by every pack that has an erratum

**Receipt (theater).** `README.md:312`: "$0.994335 provider-reported cost"
— six significant figures on one dollar. Pack READMEs quote "$0.0026"
(slate), "$0.0010" (nonce), "$0.0135" (transitivity): four-sig-fig costs
on runs three orders of magnitude below any decision threshold, plus
§12's "worst-case pricing before every run" performed on $0.003 runs.
**Receipt (errata).** `research/notes/PRINCIPLES.md:54`: "errata on top, never
rewrites." Actual placement: judge-bench erratum at line 82 of 88
(bottom); v1.1 at 52 of 56 (bottom); retest at 23 of 39 (mid). Zero of
three errata are on top. Nobody noticed, including the sessions that
wrote Principle 6 after the packs existed.

**Failure mode, concretely.** Two flavors of the same rot. (a) Precision
beyond decision-relevance is ritual: no decision differs between $0.99
and $0.994335, so the digits exist to perform the culture — and ritual
receipts train the reader to skim receipts, which is how a broken
denominator (the 138-vs-194 undercount, b4403de) survives skimming. To
be precise about what survives: the denominators that ARE
decision-facing — $70-90/attribute-pair extrapolation, $0.50/board
retest, min_draws honesty — are the best things in the repo; the attack
is on digits, not on denominating. (b) The errata clause shows a rule
detailed enough to specify PLACEMENT going unenforced on 3 of 3
opportunities: the doctrine's most checkable clause is its least
followed. A reader who opens judge-bench-2026-07-05 reads a dead
headline ("nano fails polarity in SIGN") for 80 lines before the
correction.

**Minimal edit.** Doctrine: zero words — §6 is right and the packs are
wrong; move the three erratum blocks to the top of their READMEs (a
mechanical, sub-10-minute fix that makes the clause true). Receipts to
DROP: costs below $0.10 round to one significant figure or "<$0.01";
worst-case pricing is quoted only when the worst case could change the
go/no-go (in practice: runs estimated over ~$1). That is a norms change
inside §12's existing text, not new text.

**30-day test.** (a) All errata physically on top (3 of 3, and any new
ones). (b) New pack READMEs stop quoting sub-cent costs to four figures.
(c) The real test of (b): does anyone MISS the digits — i.e., does a
decision in the next 30 days turn out to need a sub-cent cost at
precision "<$0.01" wouldn't serve? Predicted: no.

---

## Finding 6 — Missing: a stopping rule. Programs close only by completion; nothing in the doctrine can kill an instrument, and §3's retest promise is cheap virtue that will first bind at exactly the moment it becomes expensive

**Receipt.** §10 (`PRINCIPLES.md:87-93`) generates work monotonically
("the ✗ rows are ranked future work by construction"); §5 promotes probes
up the maturity ladder; no principle demotes, retires, or sunsets
anything. The one attempted stop in the repo's history —
differentiation.md's moratorium — had no doctrinal standing and was
overrun (Finding 2). Instrument count over 30 days: spin, curl, orbit,
harmonic, DL floor, nonce, portfolio, transitivity, packets, slate,
canonize, atlas — monotone growth, zero retirements. Meanwhile
`PRINCIPLES.md:33`: "Test–retest is a standing cost of every leaderboard
version, not a one-off" — honored at $0.46/board, and
corpus-map-500 already extrapolates "$70-90 per attribute-pair"
(README:37) where the retest twin doubles the bill.

**Failure mode, concretely.** A doctrine that can only add is a bloat
machine with receipts — §9's own "pay the bloat tax" names the tax for
CODE but no equivalent exists for instruments, axes, or standing cost
commitments. The predictable failure: the first full-corpus map ships
without its retest twin (because $80 is real money where $0.46 was not),
the §3 promise quietly narrows to "boards, not maps," and nothing in the
doctrine registers the retreat — the same silent-loss pattern as
Finding 2. Cheap-to-keep promises that become expensive exactly when
the work scales are the classic form of doctrine rot.

**Minimal edit.** One sentence, and it should replace §10's last
sentence ("The ✗ rows are ranked future work by construction." — 9
words): "The ✗ rows are ranked future work; an instrument no claim has
needed in two versions is a retirement candidate." (+11 net). This is
a criterion, not a process: it makes "kill" a named move with the same
standing as "add," which is all a stopping rule needs to be.

**30-day test.** Two observables: (a) does the first >$10 map ship with
or without its retest twin (the §3 forecast — if without, and no erratum
says so, Finding 6 stands confirmed); (b) does anything get retired or
explicitly marked dormant by 2026-08-08? Current base rate: 0
retirements in 131 commits.

---

## Principles that survive the attack cleanly

- **§2 (scripted pathologies):** every recent instrument shipped with a
  planted adversary that dies by exactly it (transitivity's telescoping
  judge, the diamond-pin mantissa flip); no theater found — the pins are
  in the test suite, not the prose.
- **§7 (adversary before audience):** practiced at every layer including
  this document's commission; the open-attack-list-as-v2-spec pattern is
  real (issues #48-#50).
- **§11 (prompt bytes are physics):** the best receipts-per-word in the
  doctrine — charset mojibake caught twice, apostrophe escaping caught
  live, and each catch changed shipped bytes. Untouched.
- **§5 (probes graduate into estimators):** the ladder is real (spin
  secant → sweep → counterbalancing; probe → DL floor) and the
  stat/systematic split is honored in native units.
- **§3's denominator half** survives; only its retest promise (Finding 6)
  and its precision practice (Finding 5) are attacked.

## Word ledger for all proposed edits

Deletions: -7 (§9 slogan), -6 (§1 metric sentence), -4 (§4 title).
Additions: +13 (§8 external reference), +7 (§4 instrument clause),
+11 (§10 retirement criterion). Net: +14 words across the doctrine, all
of them in the two places the receipts show a genuine hole (external
contact, stopping rule); every rhetorical sentence attacked is a
deletion.
