# Operator Queue — decisions only Xyra can close

The one control loop this repo lacked (guardian panel 2026-07-26, Leveson
M1, unopposed): agent-executable findings close at machine speed; decisions
owned by the operator had zero measured throughput and no visibility. This
file is the queue. **Cap: 5 open items.** Any process (panel, red-team,
campaign) wanting to add a sixth must first get one closed or explicitly
dropped. Sessions: read this at start when work touches strategy, doctrine,
or publication; update states in the same commit as the work that changes
them.

## OPEN (3 of 5)

**Q2. Stake reorder + external-contact stake #0.** 07-09 strategy findings:
external-contact event as stake #0, habit loop above the map, 14K scale-up
shrunk pending the 20-entity probe, JCB published as the one asset with a
living external consumer. Partially overtaken: pairwiseratio.org is live;
the contact *event* (a post a stranger reads) has still never happened.
Q4 is its vehicle — decision 2026-08-10: the P2 post IS stake #0's
artifact; its first external reader is the contact event, recorded here
when it happens.

**Q4. Publish P2?** The professed-vs-revealed mechanism inversion
(notes/manifund-campaign-2026-07-13/p2-results.md). **GATE PASSED
2026-07-27**: kimi-k2.6 REPLICATED the inversion (EI weight 0.887, EV/$
0.000, better reconstruction than the original judge) and the polish
confound is EXCLUDED (EI 1.000 vs polish 0.000) —
artifacts/live/manifund-p2-secondjudge-2026-07-27/RESULTS.md, verdicts
mechanical from pre-committed rules; one honest spend overrun quoted
($7.07 vs $5 registered abort line). Venue + framing DECIDED 2026-08-10
(ship directive): EA Forum, LessWrong crosspost after; mechanism-property
framing (neither mechanism the villain) is in the final text, costs
quoted honestly, all links live. Final text:
notes/manifund-campaign-2026-07-13/post-draft.md. The ONE remaining act
is the send — yours alone; nothing posts without your explicit approval
of that exact text and destination.

**Q5. Axis wave 3 arrest gate.** Guardian panel Hamming M3 + Feynman M3:
wave 3 does not run until (a) one smart stranger sees one admitted-axis
top-10 and the arrest (or non-arrest) is recorded, and (b) the small-tier
replication + test-retest baseline runs on the frozen wave-2 probe sets.
**(b) DONE 2026-07-27** (RESULTS-REPLICATION.md: verdict UPGRADED TO
FINDING per frozen rules — 3/6 axes tier-general, retest median +0.965;
corrected headline: haiku-4.5 breaks the price-tier story, the divergence
is a capability-class property). (a) still needs a stranger you pick;
until then wave 3 stays gated. The author-family confound is the honest
next kill before any public tier claim.

## DUE DATES

- **2026-08-08** — the 07-09 doctrine pack's 30-day tests fell due. Run
  2026-08-10, two days late (the miss is itself a data point on the
  queue's throughput). Results:
  - **External-reference pack (F1): PASS** — closed early as C1 below.
  - **Instrument-vs-artifact commit ratio (F2): moved, without the edit.**
    Window 2026-07-09..08-08, 78 commits: 21 instrument-only (src/tests),
    36 artifact/notes/site-only, 6 mixed, 15 other — 0.58:1 vs the ~9:1
    baseline the finding measured. The proposed doctrine edit (Q1) never
    landed, so the wording was epiphenomenal: the ratio inverted anyway.
    F2's premise that the slogan drives the ratio is refuted in the
    favorable direction.
  - **Errata placement (F5a): PASS** — every standing erratum found by
    grep over pack READMEs/RESULTS and notes (judge-bench ×2, spin-sweep,
    kimi-k3-bench, axis-research ×2, best-library ×2, decimal-pmf) sits in
    the top ~10 lines of its file; none buried.
  - **Retirement/dormant marking (F6b): FAIL — Finding 6 stands
    confirmed.** Zero retirements or dormant markings in the window
    (grep retir/dormant/removed over the commit log). First
    retirement-shaped event is 839c7a7 (2026-08-10, dead embed/batch
    subsystem excision) — outside the window, and code-level, not
    instrument-level. Nothing in the doctrine can yet kill an instrument;
    Q3 remains the live fix.
  - The >$10-map twin test (F6a) never triggered: no >$10 map shipped in
    the window (max single-run spend on record: $7.07, P2).

## CLOSED

**Q6. Rename ratiometer -> llmsorting? — CLOSED (confirmed + executed
2026-08-15).** Xyra: "yes, rename" (same day it was raised; overrides the
2026-08-12 ratiometer lock, 505e4ef). Executed in one sweep: GitHub repo
renamed (redirects live), crate `llmsorting` 0.13.0, `ratiometer` parked on
crates.io like cardinal-harness, code/docs/site swept (notes/ untouched —
dated history), llmsorting.com program repo folded in (PROGRAM.md,
experiments/, www/), pairwiseratio.org redeployed. Binaries `cardinal`/
`cardinald`, prompt slugs, and frozen contracts unchanged; colo2 cardinald
rail unaffected (builds from shipped src, names unchanged).

- **C2 (2026-08-10). Q1 closed — all six 07-09 doctrine edits ACCEPTED
  and landed in research/notes/PRINCIPLES.md**, adjudicated on the 30-day test
  evidence recorded above (operator directive: make the calls, ship).
  Per-edit grounds: §8 external-reference clause (F1 passed via C1 — the
  clause codifies what already worked); "Distribution > capability"
  deleted (F2 showed the ratio inverted with the slogan absent, so the
  sentence was epiphenomenal; the live driver is Q2/Q4); self-refutation
  metric sentence deleted (a count invites Goodharting; the surviving
  sentence carries the principle); §4 retitle + "binds hardest on our own
  instruments" (the deleted subtitle was itself a story); §10 retirement
  criterion (F6b FAILED — zero retirements in 131+ commits — so the
  criterion is load-bearing, not decorative); sub-$0.10 sig-figs (§12
  already owned denomination; false precision is the same sin as missing
  denominators).

- **C3 (2026-08-10). Q3 closed — admission/retirement constraint
  adopted**, folded into the §10 edit above: no instrument or axis is
  admitted without a pre-registered rubric and a retirement condition
  named at birth. Grounds: the pattern already worked where applied (axis
  wave 2 admitted 6/69 via rubric + decoys; eps/gamma knobs died by
  pre-named gate) and F6b confirmed nothing dies without it.

- **C1 (2026-07-26). P2 passed doctrine Finding 1's external-reference
  test — 12 days early.** The 07-09 test asked: "does any pack exist by
  2026-08-08 whose reference column was produced by neither the operator
  nor the stack?" (base rate then: 0/28). ACX 2024 funding decisions and
  EA CC crowd outcomes are exactly that, with predictions registered
  before unblinding (ca1928c → 892e0c0 → c0f5cd9). Noticed by the
  guardian panel's Leveson seat; recorded here so a passed test no longer
  looks identical to an ignored one.
