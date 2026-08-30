# AGENTS.md

`llmsort` is the engine: pairwise LLM ratio judgements fitted into cardinal
scores with uncertainty, active pair selection, and explicit cost and
provenance. This is the ONE repo of the program — every former name
(`cardinal-harness`, `ratiometer`, `llmsorting`) redirects here, the
full pre-extraction and seriate histories are grafted into its
ancestry, and no satellite repos exist — crate, research code, and the
measured record in three parts of deliberately different polish:

- **the crate** (root package, published to crates.io) — small, promised,
  under the shape mandate below;
- **`experiments/`** (`llmsort-experiments`, never published) — the living
  research code: experimental verbs (`cardinal`), the `cardinald` daemon,
  live batteries, instruments whose evidence is not yet in. An instrument
  graduates into the crate only after its evidence pack earns it;
- **`research/`** — the record: replayable evidence packs
  (`research/artifacts/live/`, 38+ dated packs), dated investigation notes
  (`research/notes/`), campaign definitions (`research/campaigns/`,
  `research/batteries/`, `research/data/`), and python analysis
  (`research/scripts/`, `research/examples/`). `PROGRAM.md` at the root is
  the book of tricks — every method as a rung with its pack — and is
  served at <https://llmsorting.com/PROGRAM.md>.

Pre-extraction history is IN this repo (ours-merge grafts, 2026-08-19);
the former llmsort-lab and seriate repos are deleted, with colo2 bare
mirrors retained as belt-and-braces.

## Shape mandate (the crate — root package only)

- ≤ 120 tracked files under `src/`, `tests/`, `examples/`, `docs/`; no source file over 800 lines;
  ≤ 16 integration test suites in `tests/`.
- The five rooms (see `src/lib.rs` docs): solve, evidence, elicit,
  gateway, run. Dependencies point one way; nothing in solve/ evidence
  knows about gateways or I/O.
- The stability-promised surface: `sort_texts`/`sort_documents`, CLI
  `sort` + `judge`, and the packet format. Everything else may move —
  do not promise it to external consumers.
- `#[doc(hidden)] pub` items are seams for `experiments/`, not public
  API — they may change without notice.

`experiments/` and `research/` are exempt from the file and suite
ceilings but not from discipline: CI green at every commit (fmt + clippy
`-D warnings` + tests + docs run workspace-wide), python confined to
`research/`, and the crate must never depend on `experiments/` — the
dependency points one way. NOTE: a bare `cargo check`/`cargo test` at
the root does NOT build `experiments/`; any change to a `pub` item the
experiments binaries consume needs `cargo check --workspace` before
landing (2026-08-30: a parameter deletion broke `cardinald.rs`
invisibly through a green default test run).

## Research norms

- `docs/PRINCIPLES.md` is the anti-slop discipline: refutability,
  scripted-pathology validation, denominators, mathematical register,
  errata-on-top. Read it before substantial research work.
- `research/notes/OPERATOR-QUEUE.md` caps operator decisions at five
  open items; update item states in the same commit as the work.
- Never publish claude.ai Artifacts from this repo (operator ban,
  2026-07-08). Shareable pages are committed HTML served locally. The
  public sites (llmsorting.com, pairwiseratio.org) live in
  exopriors-core `sites/`.
- Evidence packs are replayable and content-addressed; a published
  number without its pack is slop.

## Core invariants (the embarrass-us list)

1. Solver math: IRLS/Huber fusion and the evidence currency
   (E[log-ratio], honest variance). Property-tested against planted truth.
2. Error-bar honesty: calibration coverage pinned; drift toward
   overconfidence must fail loudly.
3. Identity stability: packets and judgement records are
   content-addressed; serialization is load-bearing (`serde_json`
   float_roundtrip). A content address must never drift across versions.
4. Cost truth: comparisons, tokens, dollars reported per run.
5. The 30-second experience: `llmsort sort ideas.txt --by "..."` works on
   a cold clone.

## Collaboration

Fast direct-to-main: commit small coherent changes, push promptly, rebase
not merge, stage only intended paths, never force-push main. Publishing to
crates.io ships only the root package (`cargo publish -p llmsort`; the
include-list excludes `experiments/` and `research/` — verify with
`cargo package -p llmsort --list` when touching packaging). When changing
public request/response shapes or CLI behavior, update examples, tests,
and docs in the same change.
