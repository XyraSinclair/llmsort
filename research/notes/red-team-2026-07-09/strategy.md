# Red team: the strategy, 2026-07-09

Adversarial review of the stake list, not the code. Ground truth gathered
before attacking: README.md, AGENTS.md, research/notes/PRINCIPLES.md,
notes/ideation-2026-07-05/differentiation.md,
artifacts/live/corpus-map-500-2026-07-08/README.md,
artifacts/live/slate-2026-07-07/README.md,
artifacts/live/evidence-viewer-2026-07-08/README.md, issues #46/#49/#50,
the artifacts/live/ directory (28 dated evidence packs since 2026-06-30),
and the repo's external footprint: **1 star, 1 fork, 0 watchers**
(gh repo view, 2026-07-09; crates.io download API refused the query, but a
repo announced nowhere has downloads from nowhere).

Findings ranked by expected decision-change. Verdicts: REORDER / KILL /
SHRINK / HOLDS.

---

## Finding 1 — The binding constraint is not on the stake list: zero external contact, by construction

**The bet as stated.** differentiation.md: "The gap is not capability. The
gap is that all of it lives inside stderr printlns, JSONL files, and a
dense theory doc — nothing turns the math into something a stranger
*feels* in ten seconds." Ranked fix: the interactive receipt viewer, "wow:
high," shipped 2026-07-08. AGENTS.md working norm: "Shareable pages …
are committed HTML under `artifacts/live/` and shown via a local static
server on a free localhost port."

**Steelman.** The viewer was the correct #1 by its own metric: zero new
judgment math, pure rendering, prerequisite for the atlas/registry/
portfolio ideas. Localhost-only serving is deliberate privacy hygiene for
a corpus that is one person's private writing, and the claude.ai-Artifacts
ban has real provenance reasons. Building the artifact before the
audience is the right order.

**Attack.** The differentiation doc diagnosed a *distribution* problem and
the repo responded by building an *artifact* — then wired the artifact so
no stranger can reach it. "Wow for strangers" was the ranking criterion
for a page that, by standing working norm, only ever renders on
127.0.0.1. Meanwhile the external footprint after ~6 weeks of shipping is
one star and zero watchers: the "wow × feasibility ÷ bloat" ranking
optimized the wrong variable, because with zero distribution every wow
score multiplies against an audience of ~1 (the operator plus their own
agents). Principle 9 already states the rule being violated:
"Distribution > capability once the capability exists." The capability
exists — 28 receipt packs prove it. Every one of the five stakes is
supply-side; not one produces an external-contact event. Note the ban
covers claude.ai Artifacts specifically; committed HTML on GitHub Pages
(same bytes, same receipts, same regression pin in
tests/live_artifact_pages.rs) violates no working norm.

**Cheap probe (<$1, days).** Publish two already-finished pages — the
receipt viewer and the judge-bench v1.2 leaderboard — via GitHub Pages
from the repo (no new code; the viewer is self-contained by design), and
post the viewer link exactly once in one venue where the audience thinks
about LLM judges (HN, LW, or X). Measure: unique visitors, time-on-page,
one reply asking a real question. This is the existence proof for
"stranger who cares," which every downstream stake silently assumes.

**Verdict: REORDER.** A sixth stake — one external-contact event — belongs
at position #1, above the map. It costs a day and gates the expected
value of stakes 1, 2, 4, and 5. If the best wow-artifact gets no
engagement after one honest post, that is itself decisive information the
$150-per-attribute-pair map cannot buy.

---

## Finding 2 — Stake ordering: the habit loop dominates the map and should be #1 of the build stakes

**The bet as stated.** Stake #1: "The full map: 14K entities through
intake … ~$150/attribute-pair at scale." Stake #3: "Intake habit loop:
voice note → slate → canonized attribute → map position → packet, minutes
end-to-end." Receipt for the loop's front half:
artifacts/live/slate-2026-07-07/ ("11 entries, $0.0026").

**Steelman for map-first.** The map had to exist before intake meant
anything: canonize needed measured-orthogonal attributes to merge into,
and the 470-run proved transmissibility (0.87/0.81), fusion superiority
(0.903), and orthogonality (+0.072) with real power. You cannot habit-loop
into an unmeasurable space.

**Attack.** That argument was true and is now spent — the 470 map
finished it. From here the two stakes are asymmetric in what they can
refute. The 14K map tests no hypothesis the 470 didn't (Finding 3); the
habit loop tests the one hypothesis everything else depends on: *will
even its author reach for this daily?* The overarching thesis is
"provenance judgment as the future of communication" — a communication
practice its own inventor doesn't practice is refuted at n=1, cheaply.
And the loop generates the map as a byproduct: every intake adds
entities and packets, so the map grows toward 14K as exhaust rather than
as a batch job. The loop also exercises the full pipeline
(slate → canonize → measure → packet) where the map exercises only the
measurement stage. Strictly more information, strictly more subsystems
under test, near-zero marginal cost, and the failure mode is the single
most valuable fact available: if the operator logs 0–1 organic uses in a
week, the medium thesis needs revision *before* any protocol or bounty
work.

**Cheap probe (<$1, 7 days).** One-line usage log on `cardinal slate`
(timestamp + initiator). Count operator-initiated (not agent-initiated)
invocations over 7 days. ≥5 = practice hypothesis alive; ≤1 = the
"minutes end-to-end" loop is not a habit even for its author, and stake
#2's "two parties" has no first party.

**Verdict: REORDER.** #3 above #1. The map becomes the loop's exhaust,
not a stake of its own.

---

## Finding 3 — The 14K scale-up is a scale-up with no refutable claim attached

**The bet as stated.** Stake #1, and corpus-map-500 README: "the full
14K-entity corpus at this design density ≈ $70–90 per attribute-pair with
two judges," "Every pilot conclusion survives at scale."

**Steelman.** It is Principle 8 dogfood at maximal scale; it produces the
"first real book" of packets that #50 says will pull bounty
implementation; it is astonishingly cheap per judgment; and the 470-run's
face-validity paragraph shows the instrument distinguishing reaching from
checking from venting — a real result.

**Attack.** Two independent hits. (i) *No hypothesis at risk.* The
pilot→scale replication already happened (120→470, 4×, every number
held). What does 470→14K (30×) refute? Nothing is named. Principle 1
says a session that never proved itself wrong wasn't testing anything;
a $350–450 five-attribute run whose conclusion is "the numbers held
again" is receipts-flavored comfort work. (ii) *No consumer.* Validation
is ρ=0.903 against the operator's *pre-existing* annotations — the map's
headline number certifies that it reproduces judgments its only user
already had. The genuinely new information (rigor axis, validation only
0.66; orthogonality) is already in hand at n=470. Who acts differently
because entity #9,412 has a map position? No decision, workflow, or
reader is named anywhere in the stake. A map of one person's corpus with
an audience of one is self-quantification; the receipts culture makes it
rigorous self-quantification, which is still self-quantification.

**Cheap probe (<$5, 2 days).** Consumer test on the map that already
exists: pull the 20 most position-surprising entities from the 470 map
(large |z| gap between the two axes, or biggest rank disagreement with
the 2026 reference scores), and have the operator record for each,
blind-then-revealed: "did this position tell me something I didn't
know / change anything I'll do?" ≥5/20 yes = the map has decision value
for its maker and scaling has a customer of one, which is at least one.
<3/20 = archival value only; 14K buys 30× more archive.

**Verdict: SHRINK.** Keep the 470 map as the standing demo and the
attribute set as canonized infrastructure. Do not run 14K until (a) the
Finding-2 probe shows the loop is alive (the loop will then produce the
corpus incrementally anyway) or (b) the consumer test passes.

---

## Finding 4 — The JCB is the only asset with a natural external consumer, and it is the under-invested stake

**The bet as stated.** Stake #4: "JCB v2 (#49)." Issue #49: "Ship as the
version labs stake reputation on; keep v1.x as the cheap standardized
instrument." Meanwhile the week's marquee spend (11,200 judgments, two
narrative commits, the scale-up artifact) went to the personal map.

**Steelman for the current allocation.** v1.x is shipped, validated on
scripted pathologies, cheap ($0.05/model), with test–retest receipts —
the responsible move is to let v2 wait until the wording banks and strata
exist, and the map exercises the same instruments as dogfood in the
meantime. Premature leaderboard-pushing with v1's known noise floor
(nano polarity ±0.4) would burn credibility.

**Attack.** "Which frontier model is the least sycophantic, most
order-stable judge" is a question thousands of practitioners actively
have, that no one else measures, that self-validates without ground truth
— it is the repo's one asset whose consumer exists *today* and whose
distribution channel (leaderboards travel on their own) is free. The
market signal you'd want before betting the year on judgment-as-medium is
exactly the signal a published JCB board would generate: do labs/devs
engage, cite, complain, hill-climb? The current ordering spends the
scarce resource (operator weeks) on the asset with an audience of one and
starves the asset with an audience of thousands. The noise-floor caveat
is answerable by publishing what PRINCIPLES already demands anyway: per-
axis noise classes and CIs on the board itself — v1.2's honesty *is* the
differentiation.

**Cheap probe (~$0, days).** The v1.2 board and retest pack are on disk.
Render them with the viewer's polish (the differentiation doc already
notes the viewer is reusable for exactly this), publish per Finding 1,
and watch for one lab-adjacent stranger engaging within two weeks. That
engagement — not the map — is the demand evidence for the whole
"attribute registry / taste vector" superstructure.

**Verdict: REORDER.** JCB publication (v1.2 as-is, honestly caveated)
moves above the 14K map; v2 implementation gets pulled by external
engagement rather than pushed.

---

## Finding 5 — The protocol has one speaker; a medium with no second correspondent is a diary format

**The bet as stated.** Stake #2 / issue #46: "a two-party fusion demo
reproduces the single-party posterior byte-identically"; "distributed
provenance-carrying judgment with convergence guaranteed"; deliverable
docs/SHEAF.md + wire format. Thesis frame: "provenance judgment as the
future of communication."

**Steelman.** The math is real and already partially shipped (Hodge split,
monoid theorem pinned at 1e-9); byte-identical fusion is a legitimately
strong property (CRDT semantics — adjudication by recompute), and #50's
claims machinery genuinely depends on it. Specifying a wire format early
is how protocols avoid retrofitted identity bugs.

**Attack.** Name the second correspondent. The acceptance test's "two
parties" are two processes of the same operator — a clone, not a
correspondent. Candidates within 90 days: (a) another of the operator's
own agents — same party; (b) a human collaborator — none in evidence (no
external issues, PRs, or discussion; 1 fork, 0 watchers); (c) a lab — labs
would arrive via the JCB (Finding 4), not via a sheaf doc. If no external
party parses a packet within 90 days, stake #2 was infrastructure for an
exchange that has no counterparty, and its formal layer (H¹, sections,
gluing) is mathematical register doing the work of product validation —
exactly the failure PRINCIPLES 4 warns about in the other direction:
math as decoration on a product claim. The wire format itself is cheap
and needed internally (map packets, #50 adjudication); the sheaf
formalization is not on any critical path.

**Cheap probe (~$0, <2 weeks).** Send one real packet + the one-page wire
description to one named human (a specific researcher acquaintance, not
"the community") with a single ask: parse it and tell me one thing the
receipts convinced you of. One external parse event = the medium has a
second speaker. Silence = stake #2 demotes to internal infra.

**Verdict: SHRINK.** Ship the wire format as a section of the packet
docs (it is needed by #50 and seriate regardless); demote the sheaf
formalization and two-party demo below stakes #3 and #4 until an
external parse event exists.

---

## Finding 6 — Bounties (#50): sound actuarial math, no insured party; run the free backtest, relabel the rest

**The bet as stated.** Issue #50: "warranted judgments, not cheap ones,
are the product"; premium from variance components; "implementation gets
pulled when the map's packets (the first real book) create a consumer."

**Steelman.** This is the most original product framing in the repo: the
uninsurability of contested pairs is a genuinely correct market
translation of the instrument's "undetermined" verdict, the variance-
components pricing is not metaphor but the literal DerSimonian–Laird
machinery, and the MVP is correctly gated ("spec'd now … implementation
gets pulled"). The backtest design (price warranties on the v1 board,
run the retest pack as the claims stream, compare realized loss ratio to
priced premium) is a real falsification test of the pricing model using
data already on disk.

**Attack.** A warranty has value only to a buyer who bears a cost when
the judgment reverses — someone *acting* on the ordering with money or
reputation staked. Today every judgment in every pack ranks the
operator's own prompts, proverbs, and skills; there is no party with
decision risk, hence no demand for underwriting, hence no premium anyone
would pay. Asked concretely — who is the first counterparty? — the honest
answer routes through Finding 4: a lab or eval team staking reputation on
a JCB score is the nearest entity with reversal risk, and they are a
consumer of #49, not #50. Until such a party exists, #50 is an
insurance company whose only policyholder is its own actuary. The issue
half-admits this with its pull-gate, but the gate is wrong: it waits on
"the map's packets" (an audience-of-one book, Finding 3) rather than on
an external party with skin in the game.

**Cheap probe (~$0, on-disk recompute).** Run MVP step 3 alone: price
warranties on every judge-bench v1 board entry from the variance
components, adjudicate against the existing retest pack, report realized
loss ratio vs premium. This validates or kills the *pricing model* for
free without pretending a market exists. Separately, the market probe is
Finding 4's: the first external JCB citation is the first candidate
policyholder.

**Verdict: HOLDS as spec — with the gate corrected.** Keep it on disk
unbuilt; change the pull-trigger from "the map's packets" to "first
external party with reversal risk" (realistically: JCB engagement), and
run the free backtest now since it tests the math, not the market.

---

## The compressed verdict

The five stakes are all supply. The demand side has one measured value —
1 star, 0 watchers — and no stake moves it. The cheapest permutation with
strictly more information than the current ordering:

1. **External-contact event** (publish viewer + JCB v1.2 board, one post) — new stake, ~1 day, gates everything.
2. **Habit loop (#3)** with a usage log — tests the practice hypothesis, grows the map as exhaust.
3. **JCB (#49)** pulled by whatever engagement (1) produces.
4. **Wire format** shipped as docs (the useful core of #46); sheaf layer parked pending one external parse event.
5. **14K map**: parked pending the consumer probe or the loop producing it incrementally.
6. **#50**: spec stands; run the free backtest; gate re-pointed at an external counterparty.

Total probe budget: under $10 and under a week, and every probe can lose
— which is the standard this repo set for itself (Principle 1).
