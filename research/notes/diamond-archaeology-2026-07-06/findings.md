# Diamond2 / openpriors archaeology — deltas against cardinal-harness

Read-only excavation of `~/Documents/shelf/warm/diamond2` (25GB, ~303k files,
predecessor repo containing crates `cardinal-harness-v2` and
`canonical-pairwise-ratio-harness`, plus a real executed $100 live campaign
under `runs/`) and `~/Documents/shelf/warm/openpriors` (the live Postgres
custody service). Baseline for "already absorbed" is
`research/notes/FIRST_PRINCIPLES.md` and `research/notes/MATH_FRONTIER.md` in the current
cardinal-harness. Three sub-agents did the reading in parallel (docs/design
language, crate code, runs+openpriors); this document synthesizes their
reports with file paths and verbatim quotes preserved. Nothing in either
archive repo was modified.

Diamond2's own root docs are self-aware about the risk this kind of archive
poses: `docs/judgment-infrastructure-consolidation-plan.md` (2026-05-29)
diagnoses its own repo — *"The danger is now obvious: the project can keep
producing tranches, gates, authority handoffs, verifier layers, and
philosophical refinements without shipping a small surface that a serious
operator can actually use... The roadmap has become partly a proof-of-proof
machine."* Working motto: **"Do not build a cathedral of judgment
provenance without a door."** Most of `scripts/` (221 files, 194 of them
`verify-*.sh`) and a large fraction of `canonical-pairwise-ratio-harness`'s
`main.rs` (34.7k lines) and `reference_panel.rs` (27k lines) — OAuth/PKCE
auth flows, "authority certificates," HTML dashboard renderers — is exactly
that bureaucracy and is *not* recommended for porting. The signal is
concentrated in a handful of type definitions, one prompt-cache mechanism,
one SQL schema, and the docs quoted below.

## 1. Representations of elicited priors over (entity, entity, attribute)

**No named conjugate family anywhere** (grepped explicitly for Beta,
Dirichlet, log-normal — zero hits in diamond2). Both diamond2 and openpriors
converge on the same idea cardinal-harness already has — a **discrete PMF
over a bucketed log-ratio ladder** — but diamond2's version carries a richer
*completeness* tag and openpriors names the exact **sufficient statistic
kept at rest**.

`crates/cardinal-harness-v2/src/lib.rs` (diamond2), core types:

```rust
pub enum AnswerAtom { Parity, Abstain, A(u8), B(u8), OffAlphabet }   // :70

pub enum PmfCompleteness {                                            // :156
    Complete,
    Truncated { shown_mass: F, unresolved_mass: F },
    Empirical { samples: u32 },
    Bounded { mean_lo: F, mean_hi: F },
}

pub struct AnswerEvidence { support: Vec<AtomProb>, pub completeness: PmfCompleteness }  // :170
```

`PmfCompleteness` tags *how* a PMF is known-incomplete (truncated top-k mass
vs. empirical resample count vs. only bounded between two means) and that
tag propagates through composition (`composed_projected_completeness`,
`lib.rs:657`) so downstream solvers discount incomplete evidence
differently by cause. Cardinal-harness's `PmfCompleteness` (per
`FIRST_PRINCIPLES.md` section 3) tracks escape mass but the archive's
truncated/empirical/bounded taxonomy is more granular than what's described
as shipped.

The distribution over one (entity, entity, attribute) triple, with
logprob-vs-resample fusion:

```rust
pub struct SemanticTarget { pub left: EntityId, pub right: EntityId, pub attribute: AttributeId, pub scope: EvidenceScopeId }
pub struct PairwiseRatioDistribution {                                 // lib.rs:947
    pub target: SemanticTarget,
    pub observations: Vec<PhysicalPromptObservation>,
    pub logprob_evidence: AnswerEvidence,
    pub resampled_evidence: Option<AnswerEvidence>,
    pub fused_evidence: AnswerEvidence,
    pub logprob_resample_jsd: Option<F>,   // JSD between logprob-PMF and resample-PMF
    pub resample_count: u32,
    pub total_cost: CostLedger,
}
```

`logprob_resample_jsd` is a cheap, concrete **calibration diagnostic**
(`jsd_nats`, `lib.rs:319`) flagging exactly when the token-logprob PMF and
the repeat-sampled empirical PMF disagree — a direct answer to "does the
PMF instrument agree with the model's revealed behavior," computable from
data already collected, apparently not currently reported.

`ContrastKind` (`lib.rs:1172`) taxonomizes *why* a measurement was taken,
richer than order/repeat:

```rust
pub enum ContrastKind { Baseline, ExactRepeat, OrderReversal, KeyRemap, EntityBodySwap, Padding(String), PromptDoubling, InstrumentSwitch }
```

`KeyRemap` and `EntityBodySwap` isolate whether the model is tracking the
JSON key name vs. the entity body text — confound probes cardinal-harness's
invariance table (section 5 of FIRST_PRINCIPLES.md) doesn't list.

**Openpriors names the actual minimal sufficient statistic kept at rest**,
`db/schema.sql`, table `comparisons` (the aggregated table feeding the
solver, distinct from the raw `judgements` log):

```sql
CREATE TABLE comparisons (
    entity_a_id, entity_b_id, attribute_id, attribute_version_id, rater_id, instrument_id, evidence_scope_id,
    ln_ratio        DOUBLE PRECISION NOT NULL CHECK (abs(ln_ratio) <= 50.0),
    confidence      DOUBLE PRECISION NOT NULL DEFAULT 0.5 CHECK (confidence BETWEEN 0.0 AND 1.0),
    repeats         DOUBLE PRECISION NOT NULL DEFAULT 1.0 CHECK (repeats > 0.0),
    privacy_mode    TEXT NOT NULL DEFAULT 'private_raw' CHECK (...),
    CHECK (entity_a_id < entity_b_id)
);
```

with the header comment stating the pooling rule as policy, not just code:
*"Multiple judgements for the same (entity_a, entity_b, attribute version,
rater, instrument, evidence scope, privacy mode) are aggregated here via
repeats-weighted ln_ratio averaging and max-pooled confidence. Public
publication-event recomputation collapses each complete pairwise audit
group to one effective repeat."* — i.e. `(ln_ratio, confidence, repeats)`
**is** the sufficient statistic openpriors is willing to store durably; the
full `judgements` table (raw prompt/output/logprobs/reasoning) is kept
separately as the audit trail, not the query surface. The DB *enforces* the
downstream re-derivation: `score_solve_input_sources` has a deferred
trigger (`enforce_score_solve_input_source_judgement_snapshot`) that
recomputes the repeats-weighted average from source rows and checks it
matches the snapshot within `1e-9` — a numerically-checked invariant, not a
documented one.

Openpriors' `src/posterior.rs` confirms cardinal-harness's own gateway
already produces a full discrete distribution over ladder buckets from raw
token logprobs (`PairwiseLogprobPosterior` with `ratio_distribution`,
`answer_distribution`, `signed_ln_ratio_distribution.distribution.support`)
— openpriors just truncates to `STORED_LOGPROB_ALTERNATIVES: usize = 50`
before persisting. This confirms the PMF-over-ladder design is shared
lineage, already present in cardinal-harness's own code, not a delta.

## 2. Prompt-cache-aware elicitation — nonces, prefix structure, repeat sampling

This is fully designed and was run live, and the mechanism is small and
portable. `crates/cardinal-harness-v2/src/prompt.rs`:

```rust
pub struct CachePaddingPlan {                                          // prompt.rs:84
    pub min_stable_prefix_chars: usize,
    pub nonce: Option<String>,
    pub pad_unit: Option<String>,
}
impl CachePaddingPlan {
    pub fn cache_floor(min_stable_prefix_chars: usize, nonce: impl Into<String>) -> Self { .. }
    fn render_for_prefix(&self, stable_prefix_chars: usize) -> String { .. }
}
```

`render_for_prefix` pads the system prompt to a target character floor with
a repeated neutral filler (`DEFAULT_PAD_UNIT`, `" cache-pad: ignore this
neutral prefix-padding token run for measurement caching. "`, prompt.rs:70-71)
inside a `<cache_padding purpose="prefix-cache-floor">` block, then appends
`nonce: <value>` *after* the padded region — padding pushes the provider's
prompt-cache prefix-match boundary past a target length; the nonce sits
downstream of that boundary so repeat/resample draws still cache-hit on the
stable prefix while remaining distinguishable per-draw (deterministic
resampling without defeating the cache).

The cache key is **content-derived and independent of padding/nonce**, so
tuning the cache floor or adding more resamples never invalidates cache
slots:

```rust
fn stable_prompt_cache_key(input: &RatioPromptInput) -> String {       // prompt.rs:728
    let mut state = 0xcbf2_9ce4_8422_2325_u64;  // FNV-1a
    state = stable_hash_str(state, DEFAULT_RATIO_TEMPLATE_ID);
    state = stable_hash_str(state, &input.attribute_name);
    state = stable_hash_str(state, &input.attribute_text);
    format!("cardinal:{DEFAULT_RATIO_TEMPLATE_ID}:{state:016x}")
}
```

Pinned by a test (`prompt.rs:1012-1027`,
`cache_nonce_changes_prompt_without_changing_cache_key`): two renders with
different nonces produce different `system` text but **identical**
`prompt_cache_key`, which is what's sent as OpenAI/OpenRouter's
`prompt_cache_key` field. `CostLedger.cache_hit_fraction`
(`usage.prompt_tokens_details.cached_tokens / prompt_tokens`,
prompt.rs:614-638) closes the loop with the provider's own reported hit
rate.

At the plan level (`canonical-pairwise-ratio-harness/src/lib.rs:39-64,
677-694, 2095-2106`), every unordered pair gets both a canonical and an
order-reversed `PromptWorkItem` sharing one `prompt_cache_key` but with
distinct `nonce` values derived deterministically from `(nonce_prefix,
low_id, high_id, order-suffix)`.

**This ran live**, not just as a design doc. From
`runs/structured-judgment-2026-06-live-20260603T065502/` (18 studies x 10
entities x 90 items, `anthropic/claude-opus-4.7-fast`, $100.006 spent
against a $100 cap), a verbatim completed work item
(`queue_work_items.work_item_json`):

```json
{
  "work_id": "structured-judgment-live-counterfactual-operator-leverage__pair_1_2__canonical",
  "nonce": "prompt-bank-counterfactual-operator-leverage:1:2:canonical",
  "prompt_cache_key": "cardinal:cardinal-ratio-json-letters-v1:f06659b0740c4c35",
  "rendered": { "template_id": "cardinal-ratio-json-letters-v1", "cache_padding_chars": 2831 }
}
```

and the study spec itself carries `min_cache_prefix_chars: 4096` and
`nonce_prefix` as first-class study parameters. The theory behind *why*
this matters (not just cost, but evidence independence) is stated plainly
in `crates/cardinal-harness-v2/docs/vibe-list-solver-roadmap.md`:

> "prompt cache is a discount on repeated measurement structure, not a
> separate philosophical lane" (line 85)
>
> "track effective sample size when observations share prompt prefixes,
> templates, providers, or parser versions" (line 128)
>
> sharp-edge warning in `buildout-plan.md` Phase 3: "Do not count
> cache-warmed repetitions as independent evidence."

This is operationalized as an `effective_independence: F` field on
`AcquisitionVariantProfile` (`lib.rs:3742-3866`, keyed by `AcquisitionKind {
Fresh, CacheWarm, Resample, LogprobOnly, ConsensusEscalation }`) — a
cache-warm resample is explicitly modeled as giving *less independent*
information than a fresh cold call, feeding into a VoI planner's
`information_factor`.

## 3. The type-signature landscape: 'statically cacheable', 'agent continuations', 'algebraic effects'

**Negative result, checked thoroughly.** These exact phrases (and close
paraphrases) do not appear anywhere in diamond2 (root docs, AGENTS.md,
README.md, `.impeccable.md`, both crate docs/READMEs, all source under
`crates/`) or anywhere in openpriors (`docs/`, `src/`, `SPEC.md`,
`AGENTS.md`, `THE_BOOK.md`). No `pub trait` exists in either
`cardinal-harness-v2` or `canonical-pairwise-ratio-harness` at all — grepped
explicitly; "continuation"/"effect"/"monad"/"cacheable" appear only as UI
copy strings or unrelated identifier substrings (`effective_weight`,
`store_effect`/`provider_effect` fields in an unrelated governance struct).

I also checked the obvious next place: `MATH_FRONTIER.md`'s "Compass"
section attributes the quote to **"corpus entity 'LLM Prior Elicitation
Framework'"** — i.e. this is not a repo at all, it's an entity in Xyra's
scored/annotated corpus (`corpus_v3`, reachable via the `corpus-search`
skill), most likely a ChatGPT conversation about elicitation type theory
that predates both diamond2 and openpriors. `~/Documents/diamond/` (a
plausible "legacy Diamond" filesystem location diamond2's own consolidation
plan names as a source, e.g. `one_plan_optimal_pairwise_ratio_elicitation_v3.md`,
`xyra_hand_artifacts/pairwise_prompt_templates.md`) **does not exist on
disk** — confirmed by `find`. So the "statically cacheable / agent
continuations / algebraic effects" framing is real (it's quoted verbatim in
cardinal-harness's own docs) but its *source document* is not recoverable
from either archive repo; it lives only in the corpus, and would need the
`corpus-search` skill (not a repo grep) to chase further. This is the one
lead I could not close within this task's scope — flagging it rather than
guessing.

The closest diamond2 gets to a type-signature landscape is a phase
taxonomy, not an effect system — `cardinal-harness-v2/README.md:23`: *"The
goal of this tranche is not to exhaustively ship the full plan. The goal is
to lock the denominator: **the objects, type barriers, audit gates, and
build order** that keep future implementation from drifting back into
prompt scripts plus rankings."* — `AnswerEvidence` -> `ProviderOutputCapture`
/ `PhysicalPromptObservation` / `PairwiseRatioDistribution` ->
`MeasurementReading` -> `TargetTrace` -> `WorkingGraph`/`AdmittedGraph` ->
robust solver -> active planner -> governance registry. An event-sourcing
enum treats the pipeline as typed events (`lib.rs:5320-5329`):

```rust
pub enum KernelEvent {
    MeasurementObserved(MeasurementReading), SameTargetAudited(SameTargetAudit),
    EdgeAdmitted(AuditedEdge), SolverUpdated(SolverReport),
    AttributeSolverReported(AttributeSolverReport), ActionFrontierPlanned(Vec<ActionCandidate>),
}
```

openpriors' `SPEC.md` gets closest to an algebraic type-signature framing,
in the operator's own words, though not phrased as "algebraic effects":

> `judgement = rater + instrument + attribute_version + evidence_scope +
> targets + judgement_kind + typed_payload + uncertainty +
> provenance_packet + privacy_rights + audit_state`

## 4. Judgment stability/inconsistency measurement not yet absorbed

Cardinal-harness already ships Hodge decomposition, the orbit/character
transform, judge-portfolio precision weighting, and repeat-pooling variance
components (per FIRST_PRINCIPLES.md sections 5-5 7/8). Diamond2 has several
**additional, independent** mechanisms:

**(a) An 18-variant `DegradedMode` enum used as an admission gate, not just
a report field** (`cardinal-harness-v2/src/lib.rs:2351-2372`):

```rust
pub enum DegradedMode {
    NoReadings, IncompletePmf, LowMeasurableMass, RepeatInstability, OrderResidual,
    MissingReverseProbe, DisconnectedGraph, SolverDidNotConverge, IllConditionedLaplacian,
    RidgeEscalated, CovarianceUnavailable, HighPosteriorVariance, HighCycleStress,
    HighEntropyPmf, HighOffAlphabetMass, ProjectionMismatch, InsufficientCalibration,
    PlannerNoUsableCandidates,
}
```

Each is triggered by an explicit threshold (e.g. `repeat_instability > 0.15`
-> `RepeatInstability`, `lib.rs:2476-2477`).

**(b) `SameTargetAudit.measurement_trust` gates evidence BEFORE the solver
sees it** (`lib.rs:2384-2503`):

```rust
pub struct SameTargetAudit {
    pub repeat_instability: F,   // stddev of canonical_mean() across repeat readings of the SAME target
    pub order_residual: Option<F>,  // |forward.mean + reverse.mean|, should be ~0 under antisymmetry
    pub measurement_trust: F,    // 1/(1+penalty), penalty = repeat_instability + order_residual + dropped_mass + (1-measurable_mass)
    pub model_order_biases: Vec<ModelOrderBias>,
    pub degraded_modes: Vec<DegradedMode>,
}
```

`AdmittedGraph::admit_trace` (`lib.rs:2582-2615`) refuses to admit a trace
unless it has both slot orders and clears a caller-supplied `min_trust` —
stability is a **precondition for entering the solver**, versus
cardinal-harness's current invariance-table posture of measuring and
reporting violations after the fact (FIRST_PRINCIPLES.md section 5 3/4 item 2
already moves this direction — "Estimators are group-averaged by
construction" — but doesn't gate admission on a trust scalar).

**(c) A robust Huber-IRLS solver with probabilistic rank-reversal risk**,
distinct from plain weighted log-ratio least squares (`lib.rs:2668-2930`):

```rust
pub struct RobustSolverReport {
    pub covariance: Vec<Vec<F>>, pub expected_rank_reversals: F,
    pub max_pair_reversal_probability: F, pub rank_risk: F,
    pub risk_quality: DecisionRiskQuality, pub degraded_modes: Vec<DegradedMode>,
}
```

`expected_rank_reversals`/`max_pair_reversal_probability` are propagated
from the posterior covariance matrix — rank instability quantified
probabilistically, not just via residual size. This composes cleanly with
cardinal-harness's existing Hodge/spectral machinery (same solve, an extra
propagation step) and directly answers "how likely is my top-k to flip,"
which the live run's own final output reports as `topk_flip_probability`
(see section 6 below) — i.e. this was validated as a load-bearing statistic
in a real $100 campaign, not a speculative addition.

**(d) A separate AHP (Saaty) eigenvector consistency-ratio module for
weighting attributes themselves**, orthogonal to Hodge
(`canonical-pairwise-ratio-harness/src/ahp.rs:174-280`):

```rust
let weights_raw = geometric_mean_weights(&matrix);
let lambda = principal_eigenvalue_estimate(&matrix, &weights_raw);
let consistency_index = (lambda - n) / (n - 1);
let consistency_ratio = consistency_index.max(0.0) / random_index(n);  // Saaty's RI table
let passed = consistency_ratio <= spec.consistency_ratio_threshold;
```

Cardinal-harness already has AHP (`cardinal weigh`, FIRST_PRINCIPLES.md
section 7) built on its own log-latent solver rather than Saaty's classical
eigenvector+CR test — this is a second, textbook-standard consistency
check worth having as a cross-validation lens on `cardinal weigh`'s output,
not a replacement. On failure it emits a synthesized repair prompt: *"identify
a triad where A>B, B>C, but A is not proportionally stronger than C"*
(ahp.rs:256-259).

**(e) A rater/route reliability router** tiering candidates
`Strong/Usable/Probation/Reject` by `cost_adjusted_score = reliability_score
/ routing_effective_cost` across `RaterKind::{Human, Model, Provider, Agent,
Policy}` (`reliability.rs`) — discrete tiering + routing, a different
framing from the Markowitz-style continuous precision-weighting
cardinal-harness's judge-portfolio theory already does; could compose as a
pre-filter before portfolio weighting.

**(f) Ranking-level stability distinct from single-pair repeat sampling** —
openpriors' `docs/subjective_reranking.md` describes a full pipeline:
pointwise coverage backbone (z-normalized per-judge) -> listwise refinement
(Plackett-Luce over randomized overlapping windows guaranteeing every item
is seen "early/middle/late") -> frontier-only pairwise refinement ("pairwise
comparison as a scalpel for the thin ambiguous boundary, not as the
discovery mechanism... treat disagreement as uncertainty, not a winner") ->
seeded-bootstrap Plackett-Luce aggregation reporting `p_rank1`, median rank,
and **80% rank bands**, with an explicit decision rule: clean winner only
if `p_rank1 >= 0.75` AND the 80% band's upper bound is rank 1, else emit a
tie band. Closing line worth carrying forward verbatim: *"The old audit's
durable finding was not 'never use pairwise'. It was: pairwise without a
coverage-guaranteed front door can leave low-seeded candidates unexamined
while reporting a crisp-looking ranking."* This is a coverage-guarantee
concern cardinal-harness's current planner/budget machinery
(FIRST_PRINCIPLES.md section 4) doesn't explicitly name — its budget
defaults are O(n) but nothing guarantees every entity gets an early/middle/
late viewing slot in a listwise pre-pass.

**(g) `topk_flip_probability`** — the live run's `final-operator-packet.json`
reports this per top-ranked entity directly: a bootstrap-style probability
that the top-k ranking would flip under resampling, tied to a declared
`decision_utility: {"kind":"top_k","k":2}`. This is exactly the kind of
decision-facing (not just diagnostic) stability number `cardinal sort`'s
error budget (FIRST_PRINCIPLES.md section 5 3/4 item 3) could report
alongside `stat +/- syst order . syst cyclic`.

## 5. Performant harness/backend storage designs

**Diamond2's SQLite design** (`canonical-pairwise-ratio-harness/src/store.rs`):
WAL mode, `wal_autocheckpoint 1000`, foreign keys on. A **single
polymorphic content-addressed `artifacts` table**:

```sql
CREATE TABLE artifacts (
    kind TEXT NOT NULL, artifact_id TEXT NOT NULL, study_id TEXT, tranche_id TEXT,
    capture_mode TEXT, created_at_unix INTEGER NOT NULL,
    content_hash TEXT NOT NULL, content_json TEXT NOT NULL,
    PRIMARY KEY (kind, artifact_id)
);
```

any report/certificate/plan is `(kind, artifact_id) -> content_json` with an
integrity hash — one table shape for arbitrarily many artifact kinds,
versus a growing set of bespoke tables. Content hash is FNV-1a-64 for the
hot dedup path (`stable_content_hash_bytes`, cheap) with a separate SHA-256
path (`sha256_content_hash`) reserved for cryptographic needs like approval
signatures — a deliberate two-tier hashing choice, not an oversight.

Paired with a **lease-based, budget-capped work queue as a table in the
same store** (not a separate scheduler process):

```sql
CREATE TABLE queue_work_items (
    work_id TEXT PRIMARY KEY, status TEXT, attempt_count INTEGER,
    available_at_unix INTEGER, leased_by TEXT, lease_expires_at_unix INTEGER,
    completion_budget_id TEXT, completion_cost_usd REAL, response_json TEXT,
    CHECK (status IN ('queued', 'leased', 'completed', 'failed'))
);
CREATE TABLE queue_budgets (budget_id TEXT PRIMARY KEY, max_cost_usd REAL, spent_cost_usd REAL);
```

Budget spend is debited atomically in the same transaction as work
completion (`complete_queue_response_with_budget_spend`). Migrations use an
idempotent `ensure_column` helper checking `PRAGMA table_info` before
`ALTER TABLE ADD COLUMN`. Cross-process write safety is a separate
`StoreSyncLock` built on `fs4` advisory file locks, layered on top of
SQLite's own locking — `docs/store-sync-strategy.md`: *"Primary: SQLite.
Diamond2 needs ACID writes, indexed artifact lookup, and durable run
provenance... JSONL is an operator-readable export and recovery format, not
the live write authority."*

**This was operated, not just designed**: the live run directory has both
`structured-judgment-live.sqlite` (with `-wal`/`-shm`/`.sync.lock`) *and*
`structured-judgment-live-store.jsonl` (1644 lines), verified against each
other via `store-verify-rebuilt-live.json` — rebuild a fresh SQLite from
the JSONL, hash-compare table-by-table (`"stable_hash": "fnv64:..."`)
against the original. This dual-format rebuild-and-diff discipline is a
complete, cheap integrity check cardinal-harness's current
`.cardinal_pairwise_cache.sqlite` doesn't appear to have an equivalent of.

**Openpriors' Postgres design** goes one level further: append-only
enforcement is a **DB trigger**, not an app convention — nearly every table
has a `BEFORE UPDATE OR DELETE` trigger raising an exception. Content
addressing is used for *identity dedup* of the measurement apparatus, not
just artifacts: `instruments.config_hash` and `evidence_scopes.scope_hash`
are both `UNIQUE NOT NULL`, built via `INSERT ... ON CONFLICT DO NOTHING ...
RETURNING id` — repeated identical (judge config, evidence context) pairs
collapse to one row regardless of how many judgements reference them.
`attribute_versions` are frozen by trigger
(`prevent_attribute_version_mutation`) — attributes are versioned prompt
contracts, immutable once content-hashed, exactly matching
cardinal-harness's "content-addressed: reworded = different attribute"
design (FIRST_PRINCIPLES.md section 1) but with the immutability enforced
at the database layer rather than only by convention.

`judgement_receipts` are Ed25519-signed and **self-verify on every read**
(`openpriors/src/receipts.rs`: `fetch_receipt` recomputes canonical bytes,
checks the stored hash, verifies the signature, and reconstructs+diffs the
expected redacted JSON) — receipts are a continuously-checked artifact, not
write-once-trust-forever.

## 6. openpriors-specific design language worth carrying forward

`SPEC.md` (verbatim, the clearest "why this shape" statement in either
archive):

> "The ledger is the custody layer. The judgement graph is the measurement
> layer. Scores, leaderboards, feeds, and public pages are derived views."
>
> "Any product surface that hides provenance, privacy state, evidence
> scope, disagreement, or solver lineage is incomplete."
>
> "Global scores are convenient cache rows, not source-of-truth claims."

`docs/structured_judgements.md` states the double-order-run-as-one-unit
rule plainly: the `/v1/pairwise/judge` route runs the model **twice** (A/B
then B/A) explicitly for order-bias detection, and *"complete
non-order-sensitive pairwise LLM audit groups count as one effective
repeat"* — i.e. a validated-consistent double-order pair is one unit of
evidence for repeat-counting, not two; order-sensitive runs "keep their
ledger and judgement provenance but do not write comparison input" (get
excluded from the solver rather than silently averaged in).

`src/attribute_versions.rs` bakes an adversarial self-check directly into
attribute authoring as code constants:

```
AGENT_CONTEXT: "...If changing entity order or changing a positive wording
into a negative wording would change the result, the attribute is not
crisp enough yet."
```

plus `POSITIVE_WORDING_PROBE` / `NEGATIVE_WORDING_PROBE` constants — attribute
quality is checked by literally rewriting through its own negation before
acceptance, a stronger and more automatic version of cardinal-harness's
`--two-sided`/`--wordings` probes (FIRST_PRINCIPLES.md section 5), applied
at authoring time rather than only at measurement time.

`docs/score_feeds.md` documents byte-exact canonical JSON (recursive
key-sort, compact separators, a worked test vector) + blake3 hashing
applied uniformly to every externally-facing derived artifact, with
solve-pinned (never "latest") URLs: *"an old signed feed never points at a
mutable latest lineage document."*

## Ranked "unabsorbed depth" list

Ordered by (mathematical/architectural depth x how cheaply it composes with
what cardinal-harness already has). Each entry names a buildable receipt.

1. **Rank-reversal risk propagation from the posterior covariance**
   (`expected_rank_reversals`, `max_pair_reversal_probability`,
   `rank_risk` in diamond2's `RobustSolverReport`; validated live as
   `topk_flip_probability` in the $100 campaign's final packet). Buildable
   receipt: extend cardinal-harness's existing covariance output (already
   computed for the error budget) with a Monte-Carlo-from-covariance
   estimate of P(rank swap) for the top-k boundary pair, surfaced next to
   `cardinal sort`'s `stat +/- syst` line — no new solver needed, just a
   propagation step on data already in hand.

2. **`logprob_resample_jsd` as a per-target calibration diagnostic**
   (diamond2 `PairwiseRatioDistribution.logprob_resample_jsd`). Buildable
   receipt: whenever both a PMF (logprob) reading and a repeat-sampled
   reading exist for the same pair, compute Jensen-Shannon divergence
   between them and report it — flags exactly when "the model's revealed
   preference under repeat sampling disagrees with its own stated PMF,"
   using only data cardinal-harness's repeat-pooling instrument already
   collects.

3. **`SameTargetAudit.measurement_trust` as an admission gate, not just a
   report field** (diamond2 `lib.rs:2384-2615`). Buildable receipt: a
   `min_trust` threshold on `1/(1+penalty)` where penalty sums
   repeat-instability + order-residual + dropped-mass, refusing solver
   admission below threshold — turns the existing invariance-table
   violations (already measured) into a hard gate rather than an
   after-the-fact report.

4. **Coverage-guaranteed listwise pre-pass before frontier-only pairwise
   refinement** (openpriors `docs/subjective_reranking.md`'s
   pointwise-backbone -> windowed-Plackett-Luce -> frontier-pairwise
   pipeline, with the explicit finding that pairwise-only sampling can
   leave low-seeded candidates unexamined). Buildable receipt: for large
   entity sets, guarantee every entity appears in at least one early/
   middle/late randomized window before the active planner starts spending
   budget on frontier pairs — a coverage invariant checkable independent of
   the planner's utility function.

5. **`ContrastKind::{KeyRemap, EntityBodySwap, PromptDoubling}` as
   additional invariance-group generators** (diamond2 `lib.rs:1172`).
   Buildable receipt: three new rows for FIRST_PRINCIPLES.md's stability
   table, each a cheap probe (swap the JSON key labels vs. swap the entity
   text vs. double the prompt) reusing the existing orbit-transform
   machinery (section 5 5/8) as new generators to adjoin, exactly the
   "growth path" MATH_FRONTIER.md section 3.5 already invites.

6. **Cache-padding + downstream nonce as a small, complete, tested
   mechanism** (diamond2 `prompt.rs` `CachePaddingPlan` +
   `stable_prompt_cache_key`), already run live at $100 scale. Buildable
   receipt: port `CachePaddingPlan` nearly verbatim (it's ~40 lines plus
   one pinned test) to make cardinal-harness's repeat-sampling instrument
   explicitly prompt-cache-aware — pads to a floor, keeps the cache key
   content-derived and nonce-independent, reports `cache_hit_fraction` from
   provider usage.

7. **AHP eigenvector consistency-ratio (Saaty CR test) as a second lens on
   `cardinal weigh`** (diamond2 `ahp.rs:174-280`, live in the $100
   campaign's `ahp-weights-report.json`). Buildable receipt: run both the
   existing log-latent AHP solve and a classical principal-eigenvalue CR
   check on the same criteria matrix; report disagreement as a diagnostic
   — orthogonal statistical technique, cheap to compute from data already
   collected.

8. **Dual SQLite+JSONL storage with rebuild-and-hash-diff verification**
   (diamond2 `store-verify-rebuilt-live.json` pattern). Buildable receipt:
   for `.cardinal_pairwise_cache.sqlite`, add a JSONL export path plus a
   `store-verify --rebuild` command that reconstructs a fresh DB from the
   export and table-by-table hash-diffs it against the live one — closes
   the "is my cache actually recoverable" question with a receipt instead
   of an assumption.

9. **Content-addressed dedup of judging apparatus and evidence context**
   (openpriors `instruments.config_hash`, `evidence_scopes.scope_hash`).
   Buildable receipt: if/when cardinal-harness persists judge configs or
   evidence scopes as first-class rows (not just embedded in each
   judgement), hash-dedup them the same way — cheap, and prevents the
   judgement table from re-storing identical apparatus metadata per row.

10. **Attribute-authoring self-check via forced negation rewrite**
    (openpriors `POSITIVE_WORDING_PROBE`/`NEGATIVE_WORDING_PROBE`
    constants, baked into `AGENT_CONTEXT`). Buildable receipt: when
    proposing a new attribute (`explain --propose`, `canonize`), have the
    proposer also emit the negated wording and immediately run
    `--two-sided` against it as an acceptance gate, rather than waiting for
    a human to invoke `--two-sided` after the fact.

**Not recommended for porting** (bureaucracy, explicitly diagnosed as such
by diamond2's own docs): the OAuth/PKCE/device-code/JWKS auth layer in
`main.rs`/`reference_panel.rs`, the "authority certificate" and "year-one
readiness report" generators, and the ~194-script `verify-*.sh` operational
scaffold. These exist and are load-bearing *for diamond2's own governance
process*, but porting them would be re-importing exactly the "proof-of-proof
machine" diamond2's consolidation plan warned against.

**Open lead, not closed**: the operator's own compass quote — "exhaustively
map out the space of type signatures [of elicitation primitives], especially
statically cacheable ones... then agent continuations and algebraic
effects" — is attributed by `MATH_FRONTIER.md` to a **corpus entity**
("LLM Prior Elicitation Framework"), not a repo. It does not appear in
diamond2, openpriors, or any `~/Documents/diamond/` path (that path doesn't
exist on disk). Recovering that document would require the `corpus-search`
skill against `corpus_v3`, not further repo archaeology.

## Notable file paths referenced

- `/Users/xyra/Documents/shelf/warm/diamond2/docs/architecture.md`
- `/Users/xyra/Documents/shelf/warm/diamond2/docs/program.md`
- `/Users/xyra/Documents/shelf/warm/diamond2/docs/judgment-infrastructure-consolidation-plan.md`
- `/Users/xyra/Documents/shelf/warm/diamond2/docs/store-sync-strategy.md`
- `/Users/xyra/Documents/shelf/warm/diamond2/docs/openpriors-judgment-packet-import-spec.md`
- `/Users/xyra/Documents/shelf/warm/diamond2/crates/cardinal-harness-v2/src/lib.rs`
- `/Users/xyra/Documents/shelf/warm/diamond2/crates/cardinal-harness-v2/src/prompt.rs`
- `/Users/xyra/Documents/shelf/warm/diamond2/crates/cardinal-harness-v2/docs/vibe-list-solver-roadmap.md`
- `/Users/xyra/Documents/shelf/warm/diamond2/crates/cardinal-harness-v2/docs/buildout-plan.md`
- `/Users/xyra/Documents/shelf/warm/diamond2/crates/canonical-pairwise-ratio-harness/src/store.rs`
- `/Users/xyra/Documents/shelf/warm/diamond2/crates/canonical-pairwise-ratio-harness/src/ahp.rs`
- `/Users/xyra/Documents/shelf/warm/diamond2/crates/canonical-pairwise-ratio-harness/src/reliability.rs`
- `/Users/xyra/Documents/shelf/warm/diamond2/crates/canonical-pairwise-ratio-harness/src/corpus.rs`
- `/Users/xyra/Documents/shelf/warm/diamond2/runs/structured-judgment-2026-06-live-20260603T065502/`
- `/Users/xyra/Documents/shelf/warm/openpriors/db/schema.sql`
- `/Users/xyra/Documents/shelf/warm/openpriors/src/posterior.rs`
- `/Users/xyra/Documents/shelf/warm/openpriors/src/measurement_events.rs`
- `/Users/xyra/Documents/shelf/warm/openpriors/src/attribute_versions.rs`
- `/Users/xyra/Documents/shelf/warm/openpriors/src/receipts.rs`
- `/Users/xyra/Documents/shelf/warm/openpriors/docs/structured_judgements.md`
- `/Users/xyra/Documents/shelf/warm/openpriors/docs/subjective_reranking.md`
- `/Users/xyra/Documents/shelf/warm/openpriors/docs/score_feeds.md`
- `/Users/xyra/Documents/shelf/warm/openpriors/SPEC.md`
- `/Users/xyra/Documents/shelf/warm/openpriors/THE_BOOK.md`
