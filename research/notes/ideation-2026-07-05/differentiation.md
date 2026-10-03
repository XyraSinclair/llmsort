# Differentiation scouting: cardinal-harness / seriate

Read: cardinal README, research/notes/FIRST_PRINCIPLES.md, README.md,
research/notes/COMPARISON.md, seriate README, src/bin/cardinal.rs (full subcommand
surface: sort, weigh, distinguish, calibrate, judge [+spin], elaborate,
explain, cache-export/prune, policy, eval/eval-likert/eval-compare, report,
experiment-expand, validate, rerank).

## The one sentence that matters

Everything genuinely novel in this stack is already built and receipted —
frustration (Hodge curl on the judgement graph), susceptibility (spin-probe
χ), hysteresis (order-residual), the canonical-attribute loop, AHP weighing,
differentiation profiles, a cross-model logprob reality map. The gap is not
capability. The gap is that all of it lives inside stderr printlns, JSONL
files, and a dense theory doc — nothing turns the math into something a
stranger *feels* in ten seconds. This is a distribution and artifact problem
riding on top of a genuinely deep engine, not a capability problem.

## 1. What could this do that NOTHING else on the internet does

The unifying primitive underneath every idea below is already named in
FIRST_PRINCIPLES: a **taste vector** — N judges (human or model) × M
attributes × pairwise-ratio judgements → per-judge latent scores with
uncertainty. `weigh` extracts one taste vector from one goal today. Nothing
in the ecosystem treats "a judge's taste" as a first-class, measurable,
comparable, storable object. Everything interesting downstream is really the
same move pointed at a different judge population or a different consumer.

### 1a. Cross-model belief cartography — a map of machine values
Run the same frozen attribute + entity set across N frontier models (the
machinery already exists: `--model` swap + shared cache), correlate the
resulting latent vectors, and publish it as a browsable atlas: where do
GPT-5.5, Opus, Gemini, DeepSeek agree on "which of these ideas is more
insightful," and where do they diverge sharply? `calibrate` already proves
per-model *bias*; nobody is doing per-model *taste correlation* on shared
canonical attributes as a standing public artifact. This is the single most
"nothing else exists" idea on the list — taste benchmarks (LMArena,
MMLU-style) measure capability or preference-as-vote; nothing measures
*magnitude-scale agreement structure* between models' priors.

### 1b. Personal taste model, fitted and self-audited
Point the exact same probe battery already built for LLM judges — two-sided
consistency, paraphrase consistency, spin/susceptibility, frustration/curl —
at a human. Let someone answer a stream of pairwise ratio judgements about
their own domain (research ideas, dating profiles, career options, whatever),
fit their personal latent taste vector, and then tell them the truth about
their own incoherence: "you show 0.31 frustration curl — you have a rock-
paper-scissors preference cycle among these three" or "your stated criterion
does not survive being asked in the opposite polarity — you don't actually
have a stable preference here, you have a vibe." Nothing self-quantifies
human preference incoherence with this much rigor; palantir-of-the-self,
receipted. This is the most personally viral idea here because the payoff is
about the reader, not about a model.

### 1c. Attribute markets / a canonical-attribute commons
Attributes are already content-addressed entities (`Attribute::new`,
reworded = different, per FIRST_PRINCIPLES §1). The canonical-attribute loop
(§8: propose → measure → correlate → complement) is designed and partially
shipped. The missing move: let good attributes — the ones that pass
two-sided and paraphrase consistency, with receipts — become citable,
versioned, shared objects that other people's sorts reference instead of
re-deriving from scratch ("use the well-audited `insightfulness-for-a-
software-engineer@a3f9c2` attribute, not your own guess at the wording").
This is a package registry, but for judgement criteria instead of code —
genuinely novel, and it composes with 1a/1b/1e for free because attributes
are the shared currency across all of them.

### 1d. Judgement provenance chains as a communication medium
Right now a ranking is a list of numbers. The thesis of this whole stack
("provenance judgment as the future of communication") implies you should be
able to hand someone a ranking *and its entire evidence graph* as one object
they can explore — click any item, see the pairwise judgements that placed
it there, see which pairs flipped under order-swap, see the raw provider
bytes via seriate's provenance walk. The data for this already exists in
every `--trace` JSONL and every rerank response; nothing renders it as
something a recipient can actually inspect rather than just receive. This is
the most feasible idea on the list because it requires zero new judgement
math — it's a renderer over data structures that already exist.

### 1e. Disagreement as content
The frustration/curl metric and the counterbalance flip-rate already
*identify* the maximally-contested pairs in any run. Nobody surfaces "the
judge disagreed with itself hardest right here" as a first-class, inherently
interesting artifact. A "spiciest disagreements" view of any sort — the
pairs where order-flip happened, or where spin susceptibility was highest —
is compelling content almost for free, because contested judgements are
naturally the ones humans want to read first. This folds into 1d rather than
standing alone.

### 1f. Longitudinal drift tracking — a taste seismograph
FIRST_PRINCIPLES explicitly marks "time (same judge, days apart)" ✗. Given
how fast frontier models rev, a standing frozen benchmark re-run against
every new model release, tracking latent-score drift on the same attribute
set over time, would answer a question nobody tracks: does a model's *taste*
drift across versions the way its capability does? Genuinely novel — but it
needs a time series to be a story, which this month cannot produce (one data
point isn't drift). Real, but not a this-month ship.

### 1g. Taste-portfolio feeds with receipts
`weigh` already turns a goal into a normalized weight vector over
attributes; multi-attribute rerank already composes weighted attributes into
one score. The full idea: a person owns a persistent weighted-attribute
portfolio (their fitted taste vector from 1b, or a `weigh` output), and any
list they hand it — papers, backlog items, a feed — gets ranked by receipted
dot-product against that portfolio, with a per-item breakdown of which
attribute did the work. This is the most "categorically different" pitch of
all seven: recommender feeds today are opaque and engagement-optimized; this
is a transparent, user-owned, portfolio-explainable ranking. Feasibility
this month is real but partial — the scoring primitive exists, but a live
feed needs ingestion and a presentation surface that don't exist yet.

## 2. What feels off

**The CLI-first framing is actively hiding the most profound findings.**
χ = +0.64 on a contested pair (a model's belief moving with whoever is
asking, landing at *exactly* zero absent a nudge — a "paramagnetic
judgement"), and the discovery that the ratio ladder itself injects
quantization curl into the frustration metric — these are arXiv-abstract-
grade findings. They currently live as dated bullet points inside
FIRST_PRINCIPLES.md, a file structured as a permutation-table audit for
maintainers, not a place a stranger would ever land. The stack has the
content to make someone say "wait, what?" and it's buried three docs deep.

**It's a toolbox where it should occasionally be an artifact.** The flagship
demo in the README is a tab-separated table on stdout with a stderr summary
line. That is the right shape for a library's smoke test. It is the wrong
shape for the single thing a stranger sees first. Nothing in the repo
produces something you'd screenshot, or open in a browser, or send a
non-technical friend a link to. Given the "communication medium" thesis,
the absence of anything shareable is the sharpest gap in the whole repo.

**Every genuinely novel capability is buried behind flags on `sort`.**
`--two-sided`, `--also-by`, `--spin`, `--prune-below`, `template=
ratio_letter_v1` — the flag surface on `sort` alone is ~20 options. A
first-time reader has to already know these probes exist to ever invoke
them. The "ultimate loop" (§8, propose/measure/correlate/complement to find
the canonical attributes that actually span a decision) is arguably the
single most profound capability here — "map your values, not just sort your
list" — and it is a subsection of a theory document, not a headline, not a
command with its own name.

**cardinal and seriate have soft-overlapping vocabulary.** Both define
`Entity`/`Attribute` concepts; cardinal's README says seriate "will consume"
into cardinal and seriate's says sort/active-selection "belongs to"
cardinal — the boundary is stated in both places but only in prose, not
enforced by any dependency-direction test. Not urgent, but worth a
regression check before either repo's surface grows further, since 1a-1d
above all live at exactly that seam.

**No cross-model, cross-time receipt exists yet despite the machinery
wanting one.** `calibrate` invites "run this per model" but there is no
standing comparison artifact once you've run it twice. The repo is one
small orchestration layer away from turning its own probes into the map
described in 1a, and currently leaves that composition to the user.

## 3. What would make a stranger immediately get it

Not a sorted list — a **contested judgement**, rendered as something you can
poke at. The single most compelling demo: take one real pair from the
existing spin-probe receipts (`artifacts/live/spin-probe-2026-07-05/`) —
gpt-5.4-mini on a contested pair, zero-field belief exactly 0.000, moving to
χ = +0.64 nats under a one-sentence framing nudge — and render it as an
interactive page: the two items, the neutral judgement, then a slider or
toggle for "pro-A framing" / "pro-B framing" that visibly moves the belief
and shows the nats delta live. That is the ten-second "whoa": you are
*watching* an LLM's stated preference get pushed around by who's asking, on
data that's already sitting in the repo. It requires no new judgement math,
no new probes — only a renderer over an existing receipt. It is also the
best possible proof of the whole thesis ("a judgement deserves the name
*belief* only if it survives framings that shouldn't matter") in a form a
non-technical person feels immediately, instead of reading a nats value in
a markdown table.

## 4. What to deliberately NOT build (bloat risks)

- **No hosted multi-tenant SaaS for the attribute registry or taste
  portfolios.** Keep it git/file-based and statically hostable (content-
  addressed JSON + receipts, pullable by URL or path). The moment this
  needs auth, billing, or a database, it has become a different company,
  not a differentiator.
- **No general feed-ingestion pipeline** (RSS parsing, dedup, spam
  filtering) as part of this repo. If 1g gets built, ship the "portfolio
  scores a list you already have" library function only; ingestion belongs
  to whatever consumer app wants a feed, not to the judgement engine.
- **No real-time multiplayer judging UI** (live cursors, websockets). Cool,
  not this repo's edge — the edge is provenance and math, not collaboration
  infra.
- **No fine-tuning / reward-model training on collected judgements.** The
  README already correctly fences this off to `openpriors-research`. Worth
  restating because it's exactly the kind of scope creep a "what should we
  build next" brainstorm tends to reach for, and the existing boundary is
  the right one.
- **No new instrument variants before the artifact layer ships.** Interval
  scale and full k-wise order are already correctly deprioritized in
  FIRST_PRINCIPLES ("low priority," "likely not worth it"). Best-worst /
  MaxDiff is flagged there as the single highest-value *missing instrument*
  — legitimate, but it is efficiency/completeness work, not differentiation;
  it should not jump the queue ahead of making the existing findings visible.
- **No abandoning the receipts culture for polish.** Whatever ships in this
  vein must keep rendering real numbers from real runs — the credibility of
  this whole stack is that every claim is pinned to a receipt. An artifact
  layer that starts faking or smoothing numbers for a better demo would
  torch the thing that makes this different from every other LLM-ranking
  tool.

## Ranked by wow × feasibility-this-month ÷ bloat

1. **Interactive judgement/receipt viewer** (renders existing trace + spin +
   frustration data as an explorable page; the susceptibility-slider demo
   from §3 is its hero view). Wow: high. Feasibility: very high — zero new
   judgement math, pure rendering over data already produced by
   `--trace`, `judge --spin`, and rerank responses. Bloat: very low.
2. **Personal taste model + self-audit.** Wow: high, deeply personal. Feas:
   medium — needs an interactive human-judging loop and a persisted profile,
   but every probe (two-sided, spin, frustration) is already built and
   judge-agnostic. Bloat: low.
3. **Cross-model belief cartography.** Wow: high. Feas: medium — mostly
   orchestration (run N models on one frozen set) plus the viewer from #1
   for the visualization. Bloat: low-medium.
4. **Attribute registry / canonical-attribute commons.** Wow: medium-high,
   genuinely novel framing. Feas: medium if scoped file/git-based. Bloat:
   medium (real risk of platform creep if scoped wrong).
5. **Taste-portfolio feeds.** Wow: highest of all seven ("categorically
   different"). Feas: lower this month (needs ingestion + a presentation
   loop beyond the scoring primitive). Bloat risk: medium-high if ingestion
   creeps in — ship only the scoring primitive now.
6. **Longitudinal drift tracking.** Wow: high, unique. Feas: low this month
   (one data point isn't a drift story). Bloat: low if it stays a scheduled
   append-only log.
7. **Best-worst/MaxDiff instrument.** Legitimately high scientific value
   (self-flagged as the highest-value missing cell) but it's instrument
   completeness, not differentiation — belongs on the roadmap, not this
   sprint.

## The one thing I'd ship next, and why

The interactive judgement/receipt viewer (#1), built around the
susceptibility-slider demo from §3, using data that already exists in
`artifacts/live/spin-probe-2026-07-05/`.

Why this one: it requires no new judgement science — every number it needs
has already been measured and checked in. It directly fixes the sharpest
thing that feels off (toolbox, not artifact; profound findings buried in a
theory doc) with the least possible risk (pure rendering, receipts
untouched). And it is a prerequisite, not a detour, for almost everything
else on this list: the cross-model atlas (#3) wants the same viewer to
display correlations; the attribute registry (#4) wants the same viewer to
show an attribute's consistency receipts before someone trusts it; the
taste-portfolio pitch (#7) wants the same per-item breakdown rendering. Ship
the viewer once, reuse it four times, and the stack goes from "a Rust crate
with excellent tests" to "a thing you can hand someone and watch their face
change."
