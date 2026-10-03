# Changelog

All notable changes to this project will be documented in this file.

The format is based on Keep a Changelog, and this project adheres to Semantic
Versioning once it reaches `1.0.0`.

## [Unreleased]

## [0.15.0] - 2026-09-27

### Added

- **Rows in, rows out.** `llmsort sort` now reads JSONL and RFC 4180 CSV,
  preserves every field, and writes the same shape best-first with rank,
  latent mean/std, z-score, and percentile attached. Repeatable `--field`
  selects what the judge sees; `--input` overrides shape detection; `--format`
  converts among text, JSON, JSONL, and CSV. Re-sorting replaces prior llmsort
  scores before judging. A text file beginning with `{` is now detected as
  JSONL; use `--input lines` to override it.
- Pairwise runs accept `--max-dollars` and `--max-seconds`.
  `RerankRequest`/`MultiRerankRequest` expose `max_cost_nanodollars`, and
  `SortOptions` exposes `latency_budget_ms`. Cost-cap overshoot is bounded by
  one counterbalanced pair and stops with `cost_budget_exhausted`; wall time is
  checked between batches.
- `judge --draws` now measures JSON, single-token PMF, attribute-last, and
  two-phase instruments through their real execution paths. Unknown evidence
  slugs fail instead of silently measuring `canonical_v2`.
- `install.sh` installs checksum-verified macOS and Linux binaries; release
  assets use stable version-less names and `cargo binstall llmsort` resolves
  them through package metadata.

### Changed

- The default judge is `openai/gpt-5.6-terra`. Models with measured logprob
  support default to a single-token PMF instrument; reasoning-native 5.5/5.6
  families use the two-phase `ratio_letter_2p_v1` path. The analysis turn now
  disables hidden reasoning, charge estimates include both calls, and
  `--two-sided`/`--also-by` inherit the selected instrument. Policy ladders now
  use Fable 5, Opus 4.6, and gpt-5.6-luna; `fast_only` uses luna.
- Posterior error bars now include the counterbalance-derived per-call context
  noise σ_w. Run metadata adds `evidence_sigma_w` and
  `evidence_obs_sigma_rms`; `--no-counterbalance` cannot estimate this term and
  leaves it unset.
- The summary reports independent-rerun reproducibility and distinguishes
  budget-limited resolution from converged near-ties. `rank risk` is correctly
  labelled as expected top-k boundary inversions, not a probability.
- CLI failures print a readable cause chain. Pairwise runs stop after five
  consecutive non-retryable failures (`consecutive_failures`) and expose
  `comparisons_failed` plus `first_error`; `SetwiseSorted` adds `first_error`
  and `malformed_samples`.
- Provider 429 cooldown is shared across workers and honors `retry_after`.
  `--estimate` reports a typical cost alongside the hard maximum, and
  `ComparisonUsage` now records `cache_read_tokens`.

### Research (docs only)

- PROGRAM.md E16 records the hosted-Jev result: ten-level ratings in 24-item
  windows led 29 tested designs across four cohorts and every budget. No typed
  judge gateway ships in this release.
- The attribute-last family sweep measured 38.5% cached input and 29% lower
  cost on long entities, but roughly halved truth correlation; the default
  prompt remains attribute-first.
- Research code and records were consolidated under `experiments/` and
  `research/`, with predecessor histories grafted here. The published package
  changed only through hidden seams consumed by the experiments crate.

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
  `research/notes/north-star-ontology-2026-08-11.md`). A ratiometer measures the
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

### Research (docs only)

- `README.md` recorded the attribute → magnitude → instrument → evidence →
  scaling ontology used by the research program at this release.

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
  `research/notes/seriate-fold-2026-08-11.md`). The standalone crate's CLI,
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
- Two `TokenLogprob` types now coexist (cardinal's `gateway::TokenLogprob`
  and the vendored `seriate::gateway::TokenLogprob`); unification is a
  known follow-up seam cleanup, deliberately out of the fold's scope.

### Research (docs only)

- The logprob reality map (DeepSeek logprobs vs own sampling at JSD 0.81)
  moved from the seriate repo to `research/notes/logprob-reality-2026-07-04/`.

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
- `cardinal.judgement-run.v1` (`experiments/src/judgement_run.rs`): the portable
  judgment atom for finite-candidate single-axis runs — execute, persist,
  reload, reproduce.
- `cardinald` (`experiments/src/bin/cardinald.rs`): localhost judgement-run daemon with
  ClickHouse provenance landing. Endpoints: `/healthz`, `POST /v1/estimate`
  (worst-case spend bound), `POST /v1/runs` (adaptive), `GET
  /v1/runs/{ref}`. Contract in `research/notes/CARDINALD.md`.
- cardinald external-harness lane: `POST /v1/schedule` returns a stateless
  counterbalanced comparison plan (prompts rendered by the same
  `canonical_v2` code as the adaptive path); `mode=external` on `POST
  /v1/runs` accepts one-shot pushed comparison results from an allowlisted
  harness (`claude-code`) with zero provider calls. Hardened per the
  2026-08-10 independent review: `schedule_digest` binds results to the
  issued rendering, coverage floors reject partial result sets, and
  `GET /v1/runs/{ref}` carries `entity_ids` + `entity_text_hashes`.
- Codex gateway adapter (`gateway::codex`): `codex/<model>` slugs route
  through the subscription-billed Codex exec CLI (pooled shim, scratch-cwd
  isolation, zero marginal cost). Smoke-verified; no rail-fitness study
  yet — the claude-code rail has one (21/21 decisive-pair agreement,
  `research/notes/claudecode-vs-api-2026-08-06/`).
- Native Claude Code gateway adapter (`gateway::claude_code`): chat
  completions through local `claude -p` print mode, billed to the operator's
  subscription at zero marginal API cost. `ChatModel::ClaudeCode` routes
  through the same `ProviderGateway::chat` entry point as OpenRouter;
  subscription quota errors are a non-retryable rate-limit class
  (`RateLimitSource::Subscription`) so callers control rescheduling around
  the CLI-named reset. `ClaudeCodeConfig::config_dir` points calls at a
  scratch `CLAUDE_CONFIG_DIR` (prepared by `research/scripts/claude_code_judge.py
  --pure`) for isolated judging context. Live smoke in
  `experiments/examples/claude_code_chat.rs`: fable served, cost 0 nanodollars,
  ~7s latency.
- `ChatResponse::served_model`: the model the provider reports it actually
  served (OpenRouter response `model`; Claude Code `modelUsage`), so
  measurement runs can assert served-vs-requested instead of trusting the
  request.
- `research/scripts/claude_code_judge.py`: subscription-billed structured-judgment
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

### Research (docs only)

- Published the pairwiseratio.org JCB board with every row recomputable from
  committed evidence packs.

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

### Research (docs only)

- `research/notes/FIRST_PRINCIPLES.md`: the instrument type grid, invariance group,
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
- Planner regret benchmark (`experiments/tests/planner_regret.rs`): comparisons-to-
  answer for the active planner vs uniform random pair selection.

### Research (docs only)
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
  adversarially reviewed by independent agents; see research/notes/TESTING.md.

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

### Research (docs only)

- Live taste-tooling study pack under
  `research/artifacts/live/taste-tools-demo-2026-07-02/` showing attribute recovery:
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

### Research (docs only)

- Live healthy-elicitation study pack under
  `research/artifacts/live/healthy-sort-demo-2026-07-02/`: a real Sonnet 4.6 run
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
- Fixed CI checks under current stable toolchain (rustfmt/clippy/rustdoc).
- Updated transitive dependency `bytes` to address RUSTSEC-2026-0007.
- Added a Likert baseline synthetic eval runner (`cardinal eval-likert`) for comparisons.

### Removed
- Retired prompt template `canonical_v2_attr_first`. Empirically tested in a
  comprehensive prompt layout sweep (4 variants × 7 models × 8 attributes) and
  found to offer no advantage over `canonical_v2`. The slug still resolves to
  `canonical_v2` for backward compatibility but is no longer a distinct template.

### Research (docs only)

- Live `cardinal sort` demo study pack under
  `research/artifacts/live/sort-demo-2026-07-02/`.

## [0.1.0] - 2026-01-31

- Initial public release.
