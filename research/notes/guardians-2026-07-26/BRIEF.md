# Guardian Panel Brief — 2026-07-26

## POSTURE

Exploration, not decision-attack. The operator (Xyra) reports discomfort:
"a lot of autonomous agent work, but I'm looking for really more profoundly
meaningful things — profound noticing of interesting structured-judgment
results, good work toward canonical things, and canonical tools other repos
can actually use."

Gentleness contract: at most 3 moves per lens. "This part is sound" and
"this must never be churned" are first-class answers. Do not re-litigate
adjudicated red-team findings (see notes/red-team-2026-07-09/) — cite them.

## THE QUESTION

Where is the profound meaning and the canonical leverage in cardinal-harness
and the surrounding OpenPriors stack, and what should the next tranche of
effort be? Distinguish (a) results already produced that deserve deeper
noticing/amplification, (b) tool surfaces that deserve canonicalization for
cross-repo use, (c) work streams that are churn and should stop.

## WHAT THE REPO IS

Pairwise-ratio elicitation engine: LLM judges emit pairwise ratio judgments;
robust IRLS solves them into globally consistent cardinal scores with
uncertainty. Rust core (src/rating_engine.rs, src/rerank/, src/gateway/,
src/packet.rs). 3,877 tracked files, 32G working tree (mostly data/ and
caches). Public face: pairwiseratio.org (JCB leaderboard, committed HTML in
site/).

## MAJOR WORK STREAMS (dated, with denominators)

1. **Judgment packets / fusion** (07-07): content-addressed judgment bundles
   (blake3, f64 bit patterns) that fuse byte-identically for any partition
   order; BTreeMap fix made the monoid pin bitwise. CRDT-shaped.
2. **The map** (07-06→08): 470 corpus entities × 2 attributes × 2 judges,
   11,200 judgments, $2.42. Transmissibility 0.87/0.81; fused judges beat
   both singles (rho up to 0.934 vs operator's own annotations); rigor ×
   ambition measured orthogonal (+0.072, CI [-0.02,+0.16]).
3. **Repeat elicitation / nonce draws / DL floor** (07-06): cache-priced
   repeats, sigma_w (contextual) vs sigma_b (structural) decomposition;
   judge portfolio theory (GLS weights, effective error channels: six
   models carry 2.89 channels; deepseek 10× sonnet info/$).
4. **Red-team drives** (07-09): 30 findings adjudicated; doctrine itself
   red-teamed. KEY STANDING FINDING: "distribution is the unlisted binding
   constraint — 1 star, 0 watchers, all five stakes supply-side; JCB is the
   only asset with a living external consumer; the protocol has one
   speaker." Also: the validation loop closes through one person.
5. **JCB / pairwiseratio.org** (07-18→24): public judge-calibration board.
   Kimi K3 debut #1 (0.626), then kimi-k2.6 and fable-5 tie at #1.
6. **Manifund campaign P0→P2** (07-13→19): 83 ACX 2024 grants × 4 canonized
   attributes, predictions registered before unblinding. ACX AUC 0.635,
   Spearman vs dollars +0.288; EA CC replication STRONGER (AUC 0.682,
   Spearman +0.420) and the attribute profile FLIPS: impact_per_dollar
   strongest for the crowd mechanism, weakest (0.000 revealed weight) for
   the juried order. Headline: professed vs revealed values invert, and the
   gap is a property of the funding mechanism. Total spend $1.43 of $15.
7. **Logprob instrument layer** (07-19): effort=none serves top-5 logprobs
   on gpt-5.5/5.6 via OpenRouter (40/40 census, $0.021); LOGPROBS.md.
8. **judgement-run.v1** (07-19): the portable judgment atom — execute,
   persist, reload, reproduce.
9. **cardinald** (07-21): localhost judgment daemon, provenance landing,
   exactly-once, bounded admission.
10. **Axis research waves 1–2** (07-24→25): which judgment axes show
    tier-divergence (big models judge differently from small)?
    69-candidate catalog, decoy-planted probes, 6 axes admitted.
    scar_tissue_density inverts 10th/9th→1st between tiers
    (per-frontier ranks corrected 2026-07-27, Fable audit);
    eschatological_seriousness vindicates a reworded end_of_time latent.
    Notes in notes/axis-research-2026-07-24/.

## THE DOCTRINE

research/notes/PRINCIPLES.md: refutability is the product; validate instruments on
scripted pathologies; no claim without denominator and noise class;
mathematics is the register, stories are contamination; errata on top.
AGENTS.md precommits a coverage denominator. Red-team 07-09
found the doctrine's own gaps (validation loop closes through one person;
nothing can retire an instrument).

## THE SURROUNDING STACK

~/projects: exopriors-core (canonical core, 256-file mandate), pivotality
(hosting), scry-* (research surface), priorsio, forecasting, corpus tooling.
cardinal-harness is described as "the algorithmic core behind the rest of
the OpenPriors stack." The operator wants tools OTHER repos can consume:
current candidates are `sort_texts`/`sort_documents` (src/rerank/sort.rs),
the `cardinal` CLI sort verb, judgement-run.v1 packets, cardinald.

## EVIDENCE LANES (read-only; do not mutate, do not run paid API calls)

- Feynman: notes/axis-research-2026-07-24/ (RESULTS.md, RESULTS-WAVE2.md,
  WAVE2_SPEC.md, sort-*.json), notes/manifund-campaign-2026-07-13/,
  research/notes/EVALUATION.md, docs/LOGPROBS.md.
- Dijkstra: src/rerank/sort.rs, src/packet.rs, src/bin/cardinal.rs,
  src/rating_engine.rs (skim), docs/ALGORITHM.md, docs/MODEL.md, README.md.
- Leveson: research/notes/PRINCIPLES.md, notes/red-team-2026-07-09/, research/notes/PUBLIC_BENCH.md,
  site/index.html, CHANGELOG.md.
- Hamming: README.md, research/notes/PUBLIC_BENCH.md, AGENTS.md,
  research/notes/FIRST_PRINCIPLES.md, notes/red-team-2026-07-09/, notes/ideation-2026-07-05/,
  git log, plus a look across ~/projects for consumers.

## OUTPUT CONTRACT (each seat)

1. CORE JUDGMENT (what is profound here / what is churn)
2. STRONGEST OBJECTION to the current trajectory
3. ≤3 MOVES (concrete, sized, each with what would make it a loser)
4. WHAT IS SOUND / MUST NOT BE CHURNED
5. CONFLICT you foresee with another guardian's likely answer
