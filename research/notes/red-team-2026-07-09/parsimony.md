# Red-team: parsimony audit (2026-07-09)

Adversarial review of the conceptual layer against the repo's own standard
(PRINCIPLES §9: every capability a composition of the seven primitives;
low residue, boring primitives). Six findings, ranked by stake. Each
carries a receipt, the cheaper form, and its own refutation condition.
Banned remedies (docs, renames, process) were not used; every remedy here
deletes or merges surface.

---

## 1. The orbit transform is doctrine's unification — and ships as the sixth parallel probe

**Severity: MERGE** (the deep consolidation jump)

**Receipt.** `src/rerank/orbit.rs:24-27`:

> "The one-axis-at-a-time probes (counterbalance, two-sided, wordings)
> are restrictions of this transform to subgroups; the interaction
> coefficients (S with |S| ≥ 2) are invisible to all of them."

The repo has already proved, in its own module header and in
FIRST_PRINCIPLES §5⅝ ("the marginal probes … are restrictions of this
transform to subgroups"), that one abstraction subsumes the rest. Yet the
code keeps six sibling implementations of "evaluate the judgment
functional on a transformation orbit":

- `src/rerank/spin.rs` — `spin_probe` (discrete field, 3 framings × 2 orders) AND `spin_sweep` (continuous field grid), two report types in one module
- `src/rerank/wordings.rs` — `wording_invariance`, its own report
- `src/rerank/orbit.rs` — the Z₂³ transform itself, `OrbitReport`
- `src/rerank/sort.rs:277-313` — `sort_with_probes` (two-sided = polarity restriction, `--also-by` = paraphrase axis), `SortProbe` receipts
- counterbalancing in the comparison path — the order-subgroup average, with its own flip receipts

Four independent booleans on one subcommand (`judge --spin | --sweep |
--orbit | --wordings`, `src/bin/cardinal.rs:419-451`), five report
vocabularies for one mathematical object.

**Cheaper form.** One orbit engine parameterized by (subgroup ⊆ G,
field grid ⊆ ℝ): spin = restriction to the field factor at f ∈ {−1,0,+1};
sweep = same factor on a grid; wordings = the gain-channel axis of the
same evaluation (already the plan — gains are fitted in
`gain_calibration`); two-sided = polarity restriction; counterbalance =
order restriction. `spin.rs` and `wordings.rs` become thin calls into
`orbit.rs`; their report structs collapse into named coefficient/energy
rows of one `OrbitReport` extended with the continuous factor. Issue #45
("Orbit group growth: paraphrase factor (S_k) and continuous …") already
points here — the finding is that ABSORPTION of the standalone probes
must be in #45's done-criteria, otherwise the orbit becomes an (N+1)th
probe instead of the probe.

**Refutation.** Show that the per-set probe path is structurally a
different instrument from the per-pair orbit: `sort_with_probes` feeds
the polarity restriction back into the SOLVE as weight −1 attributes
over the whole entity set (estimator use), which a single-pair 8-call
orbit cannot express. If unifying per-pair and per-set paths costs more
code than the five implementations it deletes, the correct merge is only
the per-pair family (spin/sweep/wordings/orbit → one), and this finding
downgrades to that smaller merge. It does not downgrade to zero: the
per-pair family alone is four report types for one functional.

---

## 2. The seven primitives are not seven: weight is derived, judge is missing

**Severity: MERGE** (doctrine edit; net concept count does not grow)

**Receipt.** FIRST_PRINCIPLES §1 defines the primitive:

> "**Weight** | how much one judgement moves the fit | `g(c)` or measured PMF precision"

Both given forms are functionals of the evidence record — `g(c)` of the
judgement's stated confidence, precision of the measured PMF. The
canonical exchange object agrees: `PacketObservation` (`src/packet.rs:38-43`)
is `{i, j, log_ratio, precision}` — evidence and weight are ONE struct in
the wire format the repo calls its proudest composition. Meanwhile the
solver's actual weight (`src/rating_engine.rs:196-203`) is
`beta × (precision | g(c)) × reps`, where `beta` is RATER trust
(`RaterParams`, line 169-176) — and "rater/judge" appears nowhere in the
seven primitives despite being first-class in `JudgmentPacket.judge`,
`ensemble.rs` (an entire portfolio theory of judges), and `canonize`
(transmissibility across judges). §7 supplies the other merge itself:
"attributes compared pairwise … are just entities whose bodies are
attribute descriptions" — attribute is a role of content-addressed text,
not a distinct type.

**Cheaper form.** Six primitives, each an irreducible struct that
actually exists: **content** (entity|attribute as roles — §7's own
theorem), **presentation**, **judgement**, **evidence** (whose precision
IS the per-judgement weight), **judge** (owning trust/beta and cost —
already what ensemble/packet/canonize manipulate), **posterior**. Delete
weight from the list; it is `evidence.precision × judge.trust`.

**Refutation.** Show a shipped path where per-judgement weight is set as
a free design parameter independent of both the evidence record and the
judge — `Observation.reps` is the candidate (caller/planner-assigned
multiplicity). If reps is used anywhere as an active design DOF rather
than bookkeeping for pooled duplicates, "weight" survives as design
weight and the honest fix is instead to REDEFINE the primitive as that
(and still admit judge). Either branch shrinks the mismatch between the
list and the code.

---

## 3. "A CRDT of belief" is mathematically false as shipped

**Severity: DELETE-NOW** (one phrase; the repo's own Principle 4 is the executioner)

**Receipt.** `src/packet.rs:18`: "A CRDT of belief." A state-based CRDT
requires idempotent merge (join-semilattice). `fuse`
(`src/packet.rs:174-276`) performs multiset union of observations with
NO dedup by packet id — `fuse(&[p.clone(), p])` double-counts every
observation and even lists the same id twice in `fused_packet_ids`
(line 266-267). Commutative, associative, NOT idempotent. PRINCIPLES §4:
"Physics vocabulary is admitted only where the math is literal." The
literal, pinned theorem is already named two sentences earlier:
commutative monoid on observation multisets (tests/program_equivalence.rs).

**Cheaper form.** Delete the CRDT sentence; the monoid claim is the
stronger one precisely because it is proved. (The alternative — make
fuse idempotent by deduping on packet id, earning the label — is more
surface for a marketing noun; only take it if idempotent delivery is
actually needed by a transport.)

**Refutation.** Exhibit the semilattice: if the intended CRDT state is
"set of packet ids" with fuse defined over the deduped set, show the
code path that dedupes. There isn't one today; the label is ahead of the
implementation, which is exactly what §4 forbids.

---

## 4. AGENTS.md Key Areas names two modules deleted by the narrowing commit

**Severity: DELETE-NOW**

**Receipt.** AGENTS.md Key Areas: "`src/pipeline.rs` and `src/pipeline/`:
generate -> rank -> synthesize flows" and "`src/commander/`: strategic
codebase evaluation workflow". Neither is declared in `src/lib.rs:16-26`;
`git ls-files src/commander src/pipeline src/pipeline.rs` returns zero
tracked files; commit `91af6a8` "narrow repo to canonical pairwise
engine" removed them. Two empty untracked directories remain on disk.
The self-declared source of truth ("Read AGENTS.md. It is the source of
truth.") describes a wider repo than exists — the narrowing was done in
code and not collected in doctrine.

**Cheaper form.** Delete the two Key Areas lines; `rmdir src/pipeline
src/commander`. Pure deletion.

**Refutation.** A branch about to restore these modules would justify
the lines — `git branch -a` shows none carrying them, and README's Scope
section explicitly exiles research workflows to `openpriors-research`.

---

## 5. Two implementations of cross-judge agreement: transmissibility vs judge geometry

**Severity: MERGE**

**Receipt.** `src/rerank/canonize.rs:8-10`: "**transmissibility**: mean
pairwise Spearman between the latent vectors different judge models
produce for the same wording." `src/rerank/ensemble.rs:1-8`: "J judges
each produce a latent vector over the same entities under the same
attribute. Z-score each vector … the correlation matrix R of the
z-scored vectors is the measured geometry." Same input object (J latent
vectors over one entity set, one attribute), same question (inter-judge
agreement structure), two independent computations that do not reference
each other — canonize hand-rolls mean Spearman
(`canonize.rs` imports `spearman` from sort), ensemble builds R,
loadings, and optimal weights.

**Cheaper form.** Transmissibility becomes a named statistic OF
`judge_geometry` (mean off-diagonal of R computed with rank correlation,
or the loading product — the module already handles the J = 2 fallback
canonize needs). One cross-judge geometry function; canonize consumes
it. Bonus dissolution: ensemble's optimal weights then apply directly to
canonize's per-judge latents, so "which wording" and "which judges, how
weighted" stop being separate theories.

**Refutation.** If Spearman-vs-Pearson is load-bearing (rank agreement
deliberately quotients out gain differences that z-scoring does not
fully remove) AND parameterizing `judge_geometry` by correlation kind
demonstrably distorts its factor-model math (Spearman R may not be PSD
in edge cases), then the two stay separate — but that argument must be
written where today there is silence between two adjacent modules.

---

## 6. `canonical_bucket_v1` is a superseded instrument still offered as current surface

**Severity: MERGE** (demote to replay-only)

**Receipt.** README Prompt surfaces table offers it: "Bucket-index
variant for runs that need to map output logprobs onto the fixed ratio
ladder." That purpose is exactly what `ratio_letter_v1` does, with the
live receipt (README: "≈4.0σ vs ≈1.4σ … roughly 3× the resolving power
per dollar"; FIRST_PRINCIPLES instrument grid assigns the
pairwise·ratio·PMF cell to `ratio_letter_v1` and never mentions bucket).
The grid — the repo's own honest occupancy map — has no cell for bucket;
it exists only in the template registry, examples, and the offered docs
table.

**Cheaper form.** Freeze the template for replay (cached receipts from
`live_openrouter_benchmark.py` packs must stay parseable) and remove it
from the offered surface: README table, `experiment-expand` guidance,
examples that suggest it for new runs. One less instrument to explain,
zero receipts lost.

**Refutation.** A receipted case where a provider rejects the 52-letter
alphabet path but yields logprobs on the bucket JSON path — then bucket
is the documented fallback and earns its table row with that stated
purpose (which is not its current stated purpose).

---

## Below headline (one line each, no remedy expansion)

- 21 CLI verbs, several (weigh/distinguish/explain/canonize) already
  compositions of sort/multi in code — the sprawl is surface (per-verb
  report structs), not implementation; WATCH with the existing §9 rule.
- `research/notes/BENCHMARK.md` once competed with a near-colliding scaling
  benchmark doc; the latter has since been folded away. MERGE closed.
- Issue queue imports frameworks (#46 sheaves, #48 Cooke + Rank
  Centrality + CUmulative..., #50 bounty underwriting): gate each on
  "deletes ≥1 existing mechanism" — #46 in particular risks being a
  third branding of the already-proved monoid (after "CRDT", finding 3).

## Where parsimony holds (negative results)

- The packet is the composition claim honored: entity hashes, attribute
  string, judge, (log-ratio, precision) observations, posterior via the
  one solver — no parallel machinery.
- The scope fences are real and enforced by deletion, not prose: commit
  `91af6a8` actually narrowed the repo, and differentiation.md §4's
  "do NOT build" list (no SaaS, no ingestion, no multiplayer) has been
  respected since 2026-07-05.
- `repeat_pooling` σ_b² vs Hodge frustration is NOT a duplicate despite
  measuring related energy: different data regimes (repeat draws per
  pair vs single-pass graph), explicitly cross-pinned in the module
  header ("the two views must agree in order of magnitude") — two views
  with an agreement test is the correct form, not slop.
- `discrete.rs`, `repeat_pooling.rs`, `gain_calibration.rs` are small,
  single-purpose, and each names one estimator — boring primitives as
  advertised.
