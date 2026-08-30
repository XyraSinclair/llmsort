# Changelog

All notable changes to this project will be documented in this file.

The format is based on Keep a Changelog, and this project adheres to Semantic
Versioning once it reaches `1.0.0`.

## [Unreleased]

- **Default instrument flip (NORTH spine 3)**: when no prompt template is
  chosen, `sort`/rerank now resolve it per model (`default_template_slug`)
  — `ratio_letter_v1`, the single-token PMF rail, wherever the measured
  logprob matrix (docs/LOGPROBS.md) serves answer alternatives
  (gpt-4.1/gpt-4o: 20; gpt-5.1/5.2/5.4/5.5/5.6 families: 5 at reasoning
  off), `canonical_v2` JSON elsewhere. The default is materialized into
  the request before validation, so cache keys, trace rows, dispatch, and
  the charge estimate all carry the instrument that actually runs, and
  the CLI summary names it. The seriate call now clamps `top_logprobs`
  to the route's cap — over-cap on OpenRouter returned 200 with
  `logprobs: null`, silently discarding the whole PMF on 5.x models —
  and pins reasoning off where required (5.5/5.6 400 otherwise; a
  16-token single-letter budget burns as hidden reasoning). Consistency
  probes (`--two-sided`, `--also-by`) inherit the criterion's instrument
  instead of always running canonical JSON. Live A/B on the default
  model (gpt-5.4-mini, 32 comparisons, identical cost): stat error
  ±0.019 vs ±0.521, order residual 0.031 vs 0.192 nats, frustration
  0.029 vs 0.107.
- The family-sweep rail (NORTH E10): `ratio_letter_attrlast_v1` — the
  attribute-LAST twin of the ratio-letter instrument (entities first, so
  the pair prefix is byte-stable across attribute variants and provider
  prefix caches serve {A, A′, ¬A} sweeps at cached-input prices; same
  alphabet, parser, and evidence currency; pair-keyed cache routing).
  `ComparisonUsage` gains `cache_read_tokens` so cache economics are
  measurable per call. Measured (E10, 960 calls): 38.5% cached input and
  −29% cost on long entities — at a real accuracy price on this judge
  (truth ρ roughly halves); the trade is documented in docs/NORTH.md and
  the default template is unchanged.
- Hard spend and wall caps in the run loop: `RerankRequest` /
  `MultiRerankRequest` gain `max_cost_nanodollars` (serde-defaulted,
  validated ≥ 1) and the CLI gains `--max-dollars` / `--max-seconds`
  (pairwise path; `SortOptions` also exposes `latency_budget_ms`). The
  orchestrator sizes each batch to the remaining cost cap using the
  measured mean cost per comparison (typical estimate before any data),
  so overshoot is bounded by one counterbalanced pair — not a full
  32-comparison batch — and stops with `stop_reason:
  cost_budget_exhausted` (verified live: $0.004 cap → 12 of 32
  comparisons, $0.0047). `--estimate` names the cap it will honor.
  `--max-seconds` still checks between batches only; a long in-flight
  batch can overshoot the wall cap.
- 429 backpressure is shared and `retry_after` is honored: a rate-limited
  worker extends one gateway-wide cooldown (never shortens it) that every
  dispatch waits out, instead of each worker retrying independently;
  retry delay is `max(backoff, retry_after)` capped at 30s.
- Setwise error parity: `SetwiseSorted` gains `first_error` and up to
  three truncated `malformed_samples`; the CLI summary appends them when
  nonzero, and an all-failed run says why ("no usable judge calls …;
  first error: openrouter error: User not found.") instead of bare
  counts.
- First-run honesty (measured 2026-08-29 on the 8-item demo): `sort`
  reports the first comparison error in its failure message and stops after
  five consecutive non-retryable failures (`stop_reason:
  consecutive_failures`; `RerankMeta` gains `comparisons_failed` and
  `first_error`, both serde-defaulted); `--estimate` prints a typical cost
  (48 output tokens/comparison, measured 27 mean) beside the hard max it
  used to print alone (the cap-based figure overquoted the demo 94x); the
  run summary gains a `resolution:` line counting adjacent ranks inside
  joint 1σ, so an order that is statistically noise says so.
- One repo, all names, all history (operator decisions 2026-08-19):
  the GitHub repo is `llmsort`; the legacy names `cardinal-harness`,
  `ratiometer`, and `llmsorting` all redirect here (each was walked
  through this repo to capture its redirect — downstream pointers land
  on the maintained state of the art, never a tombstone). The full
  pre-extraction history and seriate's history are grafted into this
  repo's ancestry via ours-merges; the llmsort-lab and seriate
  satellite repos are deleted (colo2 bare mirrors retained).

- The repo is now a workspace: `experiments/` (`llmsort-experiments`,
  never published) carries the research side folded in from llmsort-lab —
  the `cardinal` research CLI, the `cardinald` daemon, live batteries, and
  the research test suites. The published crate is unchanged apart from a
  handful of `#[doc(hidden)] pub` seams (`rerank::gates` application frame,
  judgement-run instrumentation hooks, config builders) that the
  experiments crate consumes; these are not public API.
- Everything is llmsort: the measured record folded in as `research/` —
  replayable evidence packs (`research/artifacts/live/`), dated notes,
  campaign definitions, and python analysis — with `PROGRAM.md` at the
  root and the program docs (FIRST_PRINCIPLES, MATH_FRONTIER, PRINCIPLES,
  …) merged into `docs/`. None of this ships in the published package
  (the crates.io include-list is unchanged). The llmsort-lab repo's
  history is grafted into this repo's ancestry and the satellite repo
  is deleted.

## [0.14.0] - 2026-08-18

### Changed

- **Crate, binary, and repository renamed `llmsorting` -> `llmsort`; the
  engine extracted into its own repo.** The research program that produced
  it (evidence packs, notes, benchmark sites, research verbs, the
  `cardinald` daemon) lives on with full history at
  [llmsort-lab](https://github.com/XyraSinclair/llmsort-lab) (the renamed
  original repo; GitHub redirects remain live). This repo is the seeded
  engine: solver, evidence/packet core, elicitation instruments, gateway
  adapters, and the `llmsort` CLI (`sort`, `judge`, `explain`, `rerank`,
  `report`, `validate`, cache and policy utilities). The `llmsorting`
  crates.io name is parked (like `ratiometer` and `cardinal-harness`
  before it) and its releases keep resolving. Lineage: cardinal-harness ->
  ratiometer -> llmsorting -> llmsort.
- Research verbs (`weigh`, `distinguish`, `slate`, `canonize`, `anp`,
  `bench`, `calibrate`, `elaborate`, the eval verbs, `experiment-expand`,
  `load`) moved to the lab. Packet format, prompt slugs, cache schema, and
  the default local cache filename are unchanged; existing caches keep
  hitting.
- Gate specs on multi-rerank requests are still validated identically;
  the gate-application research frame lives in the lab.
- Declared MSRV: rust-version = 1.88.

## [0.13.0] - 2026-08-15

### Changed

- **Crate and repository renamed `ratiometer` -> `llmsorting`** (operator
  decision 2026-08-15, closing OPERATOR-QUEUE Q6). Sorting is the
  application the whole instrument serves; the program hub at
  llmsorting.com, the experiments ladder, and the engine now share one
  name. Library paths change (`use llmsorting::...`); the CLI binaries
  (`cardinal`, `cardinald`), the packet/judgement-run atom names, prompt
  slugs, and all frozen contracts are unchanged. GitHub redirects from
  `XyraSinclair/ratiometer` remain live; the `ratiometer` crates.io name
  is parked (like `cardinal-harness` before it) and its releases keep
  resolving. Lineage: cardinal-harness (until 2026-08-12) -> ratiometer
  (until 2026-08-15) -> llmsorting.
- The llmsorting program repo (PROGRAM.md, experiments/, the
  llmsorting.com static site) folds in: `PROGRAM.md`, `experiments/`,
  `www/` (site + deploy). pairwiseratio.org keeps `site/`.

## [0.12.0] - 2026-08-12

### Changed

- **Renamed the crate and repository: `cardinal-harness` → `ratiometer`**
  (operator decision 2026-08-12; north-star ontology and naming map in
  `notes/north-star-ontology-2026-08-11.md`). A ratiometer measures the
  ratio of two signals; ratiometric measurement — no absolute anchor,
  every reading taken against a paired reference — is this engine's
  epistemics. Public type paths move from `cardinal_harness::X` to
  `ratiometer::X` (semver-honest minor bump pre-1.0). The old crate name
  is parked: `cardinal-harness` 0.11.1 is a pointer release and earlier
  versions stay published so existing lockfiles keep resolving.
- Deliberately UNCHANGED, as frozen contracts: the binary names
  (`cardinal`, `cardinald`), the judgement-run atom (`cardinal.judgement-run.v1`),
  and the content-address domain string
  `cardinal-harness/rendered-prompt/v1` (now commented as frozen in
  `src/prompts.rs`) — changing the last would invalidate every existing
  cache key and packet id.
- Data-plane label: the default landing provenance (`landing.rs` HARNESS)
  now writes `ratiometer`; rows landed earlier carry `cardinal-harness`.
- Docs: `docs/WHAT_WHY_HOW.md` gains the locked "What we are building"
  section (attribute → magnitude → instrument → evidence → scaling of
  readings; "readings, not rankings").

## [0.11.0] - 2026-08-11

### Changed

- **TokenLogprob unified** (the follow-up flagged in 0.10.0): the vendored
  instruments now consume `gateway::TokenLogprob` directly; the vendored
  transport shim (`seriate::gateway`) and both hand-written adapters in
  `comparison.rs` and the CLI are deleted. BREAKING for the vendored
  module's public API: `seriate::TokenLogprob` is gone —
  `Instrument::parse` takes `&[cardinal_harness::gateway::TokenLogprob]`.

## [0.10.0] - 2026-08-11

### Changed

- **Seriate folded back in** (`src/seriate/`): the external `seriate`
  dependency is gone; the slice cardinal actually uses is vendored —
  ontology, atoms, evidence PMFs, judgement records, the `Instrument`
  trait with `ratio_letter`/`ordinal`, and the `TokenLogprob` transport
  shape (seriate @ `ba32ca0`, decision record in
  `notes/seriate-fold-2026-08-11.md`). The standalone crate's CLI,
  gateway, sqlite evidence log, posterior compiler, and unused
  `kwise`/`scalar` instruments were culled (~4.4k lines; history stays in
  the tombstoned repo). BREAKING for type identity: what was
  `seriate::X` in public signatures is now `cardinal_harness::seriate::X`.
  seriate 0.1.2 stays published un-yanked so 0.9.0 keeps resolving.
- `serde_json` now pins the `float_roundtrip` feature: vendored judgement
  records are content-addressed over their JSON serialization, and exact
  float parse-roundtrip is load-bearing for id stability (caught by the
  vendored `json_round_trip_preserves_id` test under cardinal's default
  serde_json).
- The logprob reality map (DeepSeek logprobs vs own sampling at JSD 0.81)
  moved from the seriate repo to `notes/logprob-reality-2026-07-04/`.
- Two `TokenLogprob` types now coexist (cardinal's `gateway::TokenLogprob`
  and the vendored `seriate::gateway::TokenLogprob`); unification is a
  known follow-up seam cleanup, deliberately out of the fold's scope.

## [0.9.0] - 2026-08-10

First release published to crates.io (`cargo install cardinal-harness`).
The 0.9.0 version number was assigned internally on 2026-07-05 (the
`[0.9.0-dev]` section below); the published crate contains both sections.
The pre-existing git tag `v0.9.0` marks the 07-05 internal bump, not this
release — the published crate was built from commit `84aff7b` (clean
tree), recorded in the package's `.cargo_vcs_info.json` and verified
against the crates.io CDN copy. Tag-to-crate correspondence realigns at
the next release.

### Added
- The judgment packet (`src/packet.rs`, issue #46): content-addressed
  evidence bundles (blake3 over canonical bytes, f64 bit patterns) that fuse
  byte-identically for any partition of the same evidence in any order,
  pinned with `to_bits` equality. The pin forced a real solver fix (HashMap
  fuse buckets randomized edge order; now BTreeMap).
- `cardinal.judgement-run.v1` (`src/judgement_run.rs`): the portable
  judgment atom for finite-candidate single-axis runs — execute, persist,
  reload, reproduce.
- `cardinald` (`src/bin/cardinald.rs`): localhost judgement-run daemon with
  ClickHouse provenance landing. Endpoints: `/healthz`, `POST /v1/estimate`
  (worst-case spend bound), `POST /v1/runs` (adaptive), `GET
  /v1/runs/{ref}`. Contract in `docs/CARDINALD.md`.
- cardinald external-harness lane: `POST /v1/schedule` returns a stateless
  counterbalanced comparison plan (prompts rendered by the same
  `canonical_v2` code as the adaptive path); `mode=external` on `POST
  /v1/runs` accepts one-shot pushed comparison results from an allowlisted
  harness (`claude-code`) with zero provider calls. Hardened per the
  2026-08-10 independent review: `schedule_digest` binds results to the
  issued rendering, coverage floors reject partial result sets, and
  `GET /v1/runs/{ref}` carries `entity_ids` + `entity_text_hashes`.
- The public JCB board site (`site/index.html`) at pairwiseratio.org —
  one static committed HTML file, every row recomputable from committed
  evidence packs.
- Codex gateway adapter (`gateway::codex`): `codex/<model>` slugs route
  through the subscription-billed Codex exec CLI (pooled shim, scratch-cwd
  isolation, zero marginal cost). Smoke-verified; no rail-fitness study
  yet — the claude-code rail has one (21/21 decisive-pair agreement,
  notes/claudecode-vs-api-2026-08-06).
- Native Claude Code gateway adapter (`gateway::claude_code`): chat
  completions through local `claude -p` print mode, billed to the operator's
  subscription at zero marginal API cost. `ChatModel::ClaudeCode` routes
  through the same `ProviderGateway::chat` entry point as OpenRouter;
  subscription quota errors are a non-retryable rate-limit class
  (`RateLimitSource::Subscription`) so callers control rescheduling around
  the CLI-named reset. `ClaudeCodeConfig::config_dir` points calls at a
  scratch `CLAUDE_CONFIG_DIR` (prepared by `scripts/claude_code_judge.py
  --pure`) for isolated judging context. Live smoke in
  `examples/claude_code_chat.rs`: fable served, cost 0 nanodollars,
  ~7s latency.
- `ChatResponse::served_model`: the model the provider reports it actually
  served (OpenRouter response `model`; Claude Code `modelUsage`), so
  measurement runs can assert served-vs-requested instead of trusting the
  request.
- `scripts/claude_code_judge.py`: subscription-billed structured-judgment
  elicitation through Claude Code print mode (`--json-schema` →
  server-validated `structured_output`, zero marginal API cost). `--pure`
  runs each judgment in a scratch `CLAUDE_CONFIG_DIR` (Keychain mirror
  keyed by config-dir hash) so no user memory/rules/hooks contaminate the
  judge — probe-verified context reduction ~40k → ~18k tokens. Quota-aware
  exit codes for battery pause/resume; served-model provenance on stderr.
- `cardinal judge --consortium m1,m2,...`: the consortium verdict primitive.
  Each judge measures the full Z₂³ orbit; complete orbits become judgment
  packets (`--packets-out`) and the belief is computed by fusing them —
  composition of the orbit transform, the judgment packet, and the robust
  solver into one operation with an explicit error budget (within-judge
  orbit-bias rms, cross-judge spread, direction unanimity, shared-bias
  residual correlation). Live smoke on a Manifund ACX pair: 3 judges,
  24 comparisons, $0.021, unanimous direction with per-judge coherence
  0.049–0.572.
- An experimental ordered-probit module for ladder-valued judgements, with
  symmetric cut construction, interval-censored likelihood fitting, a declared
  weak prior, gauge-projected covariance, and zero-spend synthetic comparison
  against the former point-center model. It remains off the production path
  until contaminated-channel and calibration gates pass.

### Changed
- `cardinal canonize --budget` is now the TOTAL comparison budget across
  every sort the protocol runs (accepted + candidates × judges), divided
  evenly, with the projected sort count printed before any spend and a loud
  error when the budget cannot cover the sorts. The old per-(candidate,
  judge) reading was a measured footgun: the Manifund P1 run turned
  `--budget 240` into ~1,900 comparisons and a 20-minute silent run.
- Proposal-JSON parsing (`slate`, `weigh --propose`, `canonize --propose`,
  `explain --propose`, `distinguish --propose`) is now lenient — whole
  completion parsed first, then the first balanced JSON span — and an
  empty or unparseable completion earns exactly one retry. Both failure
  modes were measured on the Manifund P1 run (deepseek intermittent empty
  completions; gpt-5.4-mini's valid-but-decorated `{"[]": [...]}` envelope,
  which the old first-bracket slice turned into a parse error).
- Point observations now use explicit measured `precision` when present and
  unit precision otherwise. Removed the anti-calibrated
  `eps_confidence`/`gamma_confidence` transform and planner
  `default_confidence`; model-stated confidence remains trace metadata.
  The deterministic method suite moves from ratio 0.648 versus ordinal 0.726
  under the old transform to ratio 0.808 versus ordinal 0.726, and three named
  cases now match full-budget Likert tau at half the comparison budget.
- Renamed spectral, leave-one-out, and multi-attribute diagnostic APIs to say
  what they contain rather than using a generic audit-artifact label.
- Corrected install and release documentation: source installs track `main`,
  tagged binaries come from GitHub Releases, and the crate is not currently
  published to crates.io. (True when written; superseded by this release —
  the blocker, seriate being git-only, dissolved when seriate 0.1.2 was
  published to crates.io the same day.)

## [0.9.0-dev] - 2026-07-05

Internal version bump, never separately released — included in the 0.9.0
crates.io release above.

### Added
- `cardinal weigh` (AHP priority vector over attributes-as-entities) and
  `weigh --propose`: automated AHP — the goal decomposed into judgeable
  considerations, then measured pairwise on importance for that goal.
- `cardinal distinguish`: the propagation primitive — propose-then-MEASURE
  the attributes under which a focal item stands out
  (`differentiation_profile`: percentile and z-score per attribute).
- Hodge curl fraction of the judgement edge field surfaced per attribute
  and in run meta; transitive-vs-cyclic judge test pins the
  quantization-curl floor and planted-cycle detection.
- `docs/FIRST_PRINCIPLES.md`: the instrument type grid, invariance group,
  and efficiency theory matched cell-by-cell against the repo.

## [0.8.1] - 2026-07-05

### Fixed
- Build: seriate v0.1.1 with default features off; the `cardinal` binary
  requires `sqlite-store` — pure-library consumers avoid the
  `libsqlite3-sys` links conflict. (Cargo version drift from the v0.8.1
  tag repaired in v0.9.0.)

## [0.8.0] - 2026-07-04

### Added
- `cardinal calibrate`: null-pair artifact measurement — identical text in
  both slots; directional mass = pure position+letter prior. Live study:
  four models measured clean (parity 1.000, bias 0.0000 nats) at the null
  point.
- Multi-attribute diagnostics on every multi-attribute response: the Pareto
  front (non-dominated on weight-oriented posterior means) and the
  attribute correlation matrix (planted trade-off test pins a negative
  off-diagonal). Cross-attribute information SHARING remains open (#44).
- Fixed-budget planner accuracy benchmark alongside first-hit-time, after
  catching the flicker artifact in exact-set first-hit metrics.

### Changed
- Exploration anchor diversity (issue #43): quantile-rotating anchors
  (chain fallback) replace the hub-and-spoke single-anchor geometry.
  Measured: global-tau regret flipped to a planner WIN (ratio 0.92);
  scarce-budget accuracy now favors the planner (budget 60: tau 0.894 vs
  0.871, top-5 12/16 vs 10/16).
- The synthetic ratio-vs-ordinal suite relationship FLIPPED under the new
  geometry (ordinal 0.726 vs ratio 0.648) — re-pinned with measurement
  history preserved; live logprob-PMF evidence is unaffected.

## [0.7.0] - 2026-07-04

### Added
- `ordinal_letter_v1`: the seriate three-token direction instrument
  (A / B / =) as a second evidence template — the cheapest logprob-native
  path; direction PMFs enter the solver at fixed modest magnitude with
  measured uncertainty.
- Order-residual diagnostic: for pairs asked in both orders in evidence mode,
  the mean |sum of presented-coordinate log-ratio means| — position bias in
  nats, per run (`evidence_order_residual_mean_abs`; ~0 for an unbiased
  judge, large under pure position bias; strictly richer than binary flip
  counts).
- `cardinal sort --estimate`: worst-case comparisons, per-call tokens, and
  provider dollars before any network or cache touch — with per-template
  honesty (single-letter evidence calls cap at 16 output tokens, ~100x
  cheaper worst case than the JSON path).
- Planner regret benchmark (`tests/planner_regret.rs`): comparisons-to-
  answer for the active planner vs uniform random pair selection.

### Findings (measured, pinned two-sided)
- HONEST NEGATIVE: the current planner LOSES to uniform random pair
  selection at n=20 under a noisy simulated judge — on top-5
  identification (~134.7 vs ~86.7 comparisons) and global tau (~51.3 vs
  ~47.3); the gap widens with noise. README claims tempered; fix cycle
  tracked in #43 with the benchmark as the instrument.

## [0.6.0] - 2026-07-04

### Added
- The seriate evidence path (`--template ratio_letter_v1`): single-token
  ratio-letter elicitation whose answer-position top-k logprobs form the
  judgement PMF; rendering/parsing delegated to the `seriate` crate (no
  prompt duplication, cache identity derived from seriate's content-
  addressed template hash).
- Explicit-precision observations: `Observation::from_log_ratio_moments`
  feeds PMF mean/variance into the IRLS solver directly, replacing the
  `g(c)` stated-confidence mapping for evidence-mode judgements.
- Evidence health diagnostics in response meta and the sort summary line:
  `evidence_judgements`, `logprob_mode_judgements`,
  `evidence_visible_mass_mean`.
- Loud degradation: providers that reject the logprobs parameter
  (reasoning-class models) or silently omit logprobs fall back to sampled
  mode, visibly in run metadata.
- Cache schema: nullable `log_ratio_mean` / `log_ratio_var` /
  `visible_mass` columns; evidence moments survive cache replay.
- Live study: at equal budget and cost on gpt-5.4-mini the PMF path
  yields ~3x the top-to-bottom separation per dollar (4.0 sigma vs 1.4
  sigma); instruments agree at Spearman 0.74 — documented honestly.

## [0.5.0] - 2026-07-02

### Added
- Adversarial test battery: six new suites, 74 tests (266 total across 27
  suites) attacking solver recovery (planted truth, Huber influence bounds,
  gauge invariance, confidence weighting, ladder monotonicity), metamorphic
  invariances of the sort path, uncertainty calibration coverage, a
  pathological-judge taxonomy (position-biased, intransitive, compressed,
  refusing, gaslighting, format-vandal), method head-to-heads vs Likert and
  ordinal baselines, and planner/pruning/stopping efficiency. Authored and
  adversarially reviewed by independent agents; see docs/TESTING.md.

### Fixed
- `solve_irls_huber`: MAD outlier-scale estimate collapsed when residuals
  were tied up to floating-point noise (absolute 1e-18 zero-guard), clipping
  every edge and crushing the fit by 3–4 orders of magnitude. Now falls back
  to the max-abs scale when MAD is below 1e-8 of the max-abs residual.
  Found by the battery's adversarial review; regression test pinned to the
  hand-solved normal equations.
- Synthetic evaluation gate-prewarm loop could overrun `comparison_budget`
  before the main loop's budget check ever ran; prewarm now spends from and
  stops at the same budget.

## [0.4.0] - 2026-07-02

### Added
- `cardinal judge`: single fully-transparent pairwise judgement (`--show-prompt`
  prints the rendered system+user prompt; `--json` for structured output;
  ratio, ordinal, and bucket templates).
- `cardinal elaborate` and `sort --elaborate`: one LLM call expands a terse
  criterion into a precise judging rubric (definition, what counts, what must
  not be rewarded), printed and used verbatim as the attribute prompt.
- `cardinal explain`: reverse-engineer an existing ranking — measure candidate
  attributes (user-supplied and/or `--propose`d by an LLM) against a believed
  order, report per-attribute Spearman and fitted non-negative weights
  (`explain_ranking` / `propose_candidates` in the library).
- Top-k exploration pruning: `prune_p_topk_below` on top-k specs (and
  `--prune-below` on `sort`) stops spending forced-exploration comparisons on
  items whose posterior chance of reaching the top-k is negligible;
  `entities_pruned` count in response meta.
- Live taste-tooling study pack under
  `artifacts/live/taste-tools-demo-2026-07-02/` showing attribute recovery:
  explain identifies the criterion that actually generated a ranking (ρ=+0.98,
  weight 0.85) against three LLM-proposed decoys.

## [0.3.0] - 2026-07-02

### Added
- Counterbalanced comparisons: `counterbalance_pairs` on rerank requests asks
  every planned pair in both presentation orders, cancelling position bias
  per-pair; `pairs_counterbalanced` / `position_flips` diagnostics in response
  meta. Default ON for the `sort` surface (`--no-counterbalance` to opt out).
- Attribute health probes on `sort`: `--two-sided` judges the opposite of the
  criterion ("lack of X", weight −1) and `--also-by` judges paraphrases; both
  report sign-adjusted Spearman rank-consistency diagnostics (`probes` in JSON
  output, verdict lines on stderr).
- Natural ordinal prompt template `ordinal_v1` (direction + confidence only),
  entering the solver as a fixed modest log-ratio shared with the synthetic
  ordinal mode (`ORDINAL_OBSERVATION_RATIO`).
- Live healthy-elicitation study pack under
  `artifacts/live/healthy-sort-demo-2026-07-02/`: a real Sonnet 4.6 run
  measuring 11/51 order flips, +0.81 opposite-side consistency, and a +0.35
  (shaky) paraphrase.

## [0.2.0] - 2026-07-02

### Added
- `cardinal sort`: sort newline-delimited items (or a JSON array) from a file
  or stdin by a natural-language criterion, with `--scores`, `--reverse`,
  `--format text|json|jsonl|csv`, `--top-k`, `--budget`, `--trace`,
  `--cache-only` (keyless offline replay), and one-line cost/stop accounting
  on stderr. Refuses to print output when every comparison failed.
- Library conveniences `sort_texts` / `sort_documents` (`rerank::sort`) over
  the single-attribute rerank path, including a middle-boundary default for
  whole-list sorts (a `top_k = n` degenerate case would stop before the first
  comparison).
- Tag-triggered release workflow building `cardinal` binaries for six targets
  with sha256 checksums.
- Tight crates.io packaging (explicit `include`, ~50 files), docs.rs metadata,
  and `CITATION.cff`.
- Live `cardinal sort` demo study pack under
  `artifacts/live/sort-demo-2026-07-02/`.
- Fixed CI checks under current stable toolchain (rustfmt/clippy/rustdoc).
- Updated transitive dependency `bytes` to address RUSTSEC-2026-0007.
- Added a Likert baseline synthetic eval runner (`cardinal eval-likert`) for comparisons.

### Removed
- Retired prompt template `canonical_v2_attr_first`. Empirically tested in a
  comprehensive prompt layout sweep (4 variants × 7 models × 8 attributes) and
  found to offer no advantage over `canonical_v2`. The slug still resolves to
  `canonical_v2` for backward compatibility but is no longer a distinct template.

## [0.1.0] - 2026-01-31

- Initial public release.
