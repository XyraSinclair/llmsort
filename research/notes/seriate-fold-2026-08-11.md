# Seriate fold-back — decided cull, execution spec

**EXECUTED 2026-08-11** (operator "ok do it"): vendored per this spec,
full suite green, released as 0.10.0. One invariant the spec missed,
found by the vendored tests: seriate required serde_json's
`float_roundtrip` feature — content-addressed record ids depend on exact
float parse-roundtrip; cardinal's manifest now pins it. Also culled
beyond spec: the dead `InvalidPresentationCount` error variant and the
`OrdinalKWise`/`ScalarControl` enum variants (kwise/scalar-only).

**Decision (operator, 2026-08-11):** cull the separate seriate repo/crate;
fold the slice cardinal-harness actually uses back in as `src/seriate/`.
The split (agent-created 2026-07-04) has one consumer, produced three
coordination commits and one errata in its first week as a published dep
(`f4ef9c4`, `84aff7b`, `16d4025`), and its standalone scope (own CLI,
gateway, sqlite evidence log, posterior compiler — ~4.4k of 6.7k lines)
has zero users. Re-extraction later is cheap (crate name owned, seam
clean); fold-back only gets more expensive. Cull now, at lifetime-minimum
public-API cost (published to crates.io 2026-08-10; ~zero external users).

**Execution was deliberately NOT started 2026-08-11 02:50:** a live agent
was mid-flight on the decimal-ledger stream (commits `9d33215`..`2538624`,
untracked `examples/decimal_ledger_groundtruth.rs` touched 02:44), and the
fold rewrites imports across the same files (comparison.rs, cache.rs,
rating_engine.rs). One writer per live branch. Execute when the tree is
quiet, rebased on whatever that stream lands.

## Vendor closure (measured 2026-08-11, seriate @ ba32ca0)

Copy into `src/seriate/` (serde is the only external dep; all files
verified to import nothing outside this closure):

- `ontology.rs` (217 ln) — Entity, Attribute, ContentId, ids
- `atom.rs` (306) — AnswerAtom, RATIO_LADDER
- `evidence.rs` (625) — PMF machinery, evidence_from_logprobs, PmfCompleteness
- `record.rs` (287) — AcquisitionMode, JudgementRecord, EvidenceHealth
- `instrument/mod.rs` (280) — Instrument trait; DROP `pub mod kwise;` /
  `pub mod scalar;` decls + their doc lines
- `instrument/ratio_letter.rs` (326), `instrument/ordinal.rs` (287)
- NEW `gateway.rs` shim: just `TokenLogprob {token, logprob, top}` (3-field
  struct from seriate gateway.rs:129) so vendored `crate::gateway::
  TokenLogprob` paths keep shape
- `mod.rs`: seriate lib.rs trimmed to the vendored modules + re-exports
  (drop compile/log/probe/capture/gateway re-exports; keep the three
  invariants doc)

Culled (dies with the tombstoned repo, preserved in its git history):
gateway.rs 766, compile.rs 841 (parallel posterior compiler), log.rs 986,
main.rs 490, probe.rs 252, capture.rs 186, instrument/kwise.rs 502,
instrument/scalar.rs 291.

## Steps

1. Worktree off quiet origin/main.
2. Copy closure; sed vendored files `use crate::` → `use crate::seriate::`
   (`super::` untouched).
3. cardinal lib.rs: `pub mod seriate;` (public — ContentId et al. appear in
   cardinal's public API). Rewrite consumers: src `use seriate::` →
   `use crate::seriate::`; tests/bins → `use cardinal_harness::seriate::`.
   8 symbols used: Entity, Attribute, ContentId, TokenLogprob,
   AcquisitionMode, Instrument, RatioLetterInstrument, OrdinalInstrument.
4. Cargo.toml: remove `seriate` dep (serde already present).
5. Full suite green via `~/.cargo/bin/cargo test`.
6. Docs scrub: README sibling-project link;
   changelog entry. Move seriate's
   `artifacts/live/logprob-reality-2026-07-04/` (DeepSeek JSD 0.81 reality
   map, cited by WHAT_WHY_HOW) into `notes/`.
7. Version **0.10.0** (semver: public types change identity from
   `seriate::X` to `cardinal_harness::seriate::X`). Publish to crates.io.
8. Tombstone seriate repo README: "folded into cardinal-harness at
   <commit>; crate name parked for possible future re-extraction; 0.1.2
   stays published." **Do NOT yank 0.1.2** — fresh (non `--locked`)
   resolution of cardinal-harness 0.9.0 needs it.
9. Note in changelog that two TokenLogprob types now coexist
   (cardinal `gateway::TokenLogprob` vs vendored `seriate::gateway::
   TokenLogprob`) — unification is a follow-up seam cleanup, not part of
   the fold.

Scratch clone with measurements: /tmp/seriate-fold (disposable).
