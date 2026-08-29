# Grand plan — from research instrument to something real (2026-08-29)

Four independent voices on one brief (`/tmp/llmsort-consult/brief.md`,
reproduced in §4): Oracle (GPT-5.6 Sol, Extra High — the Pro tier was not
reachable in the picker this run; disclosed by the launcher), Kimi K3
(agentic, repo-inspecting; first turn lost to `--no-session`, final turn
kept), and a two-guardian pass (Feynman + Hamming, Kimi K3 light tier,
transcripts `~/.local/state/nucleus/run-20260829T041508.678`). The lead's
own position was stated before any answer landed and is included so the
panel could falsify it.

## 1. Verdict — all four converge

**The twelve operational gaps are true and are not the important
problem.** The queue (`OPERATOR-QUEUE.md`) records the causal chain
send → stranger reads → stranger tries → stranger returns, broken at link
one: the P2 post (Q4) has been text-complete since 2026-08-10 and stake #0
(one stranger reading one artifact) has never fired in seven weeks. Every
item in the operational draft acts on links three and four, which carry
zero traffic. Oracle's phrasing: *there has not yet been a failed market
test; there has been no market test.*

Hidden assumption the panel surfaced (Feynman): that a P2 reader converts
to a crate user. Checked — the post had **no** llmsort call-to-action and
linked the retired `ratiometer` GitHub name. Fixed in this commit (draft
only; the send is Xyra's alone).

Convergence at n=4 across three model lineages is stronger than the
n=2 the guardian-two skill warns about, but all four read the same queue
file; the agreement is partly the evidence's, not theirs.

## 2. What the lead measured while the panel ran (Feynman's test, run)

Cold `cargo install llmsort --locked` of 0.14.0 on a clean Linux build host: builds in
1m36s, `--version` and `--estimate` work. The 30-second demo on the local
build (`examples/sort-demo.txt`, gpt-5.4-mini): **7.2 s, $0.0124, 32
comparisons**. Three first-run defects, none of them in the twelve:

1. With an expired key, all 32 attempts burn in ~1 s and the user sees
   "all 32 comparison attempts failed" — the cause ("API key expired")
   exists only in the trace. (Both env OpenRouter keys on this machine
   are expired; the vault key is live.)
2. `--estimate` quotes "$1.19 hard max" for a run that costs $0.012 —
   94× — because it prices the 8,192-token output cap; measured output
   is 27 tokens mean, 32 max.
3. The default-budget result on 8 items is statistically noise (σ
   0.41–0.55 per item against a 0.58 total spread, order flips 5/16,
   rank risk 5.8, stop `budget_exhausted`) and nothing legible says so.

These three are being fixed (Codex slice, same day): first error
surfaced + fail-fast on consecutive non-retryable failures; typical vs
hard-max estimate; a one-line adjacent-rank resolution readout.

## 3. The plan (Oracle's five moves, Kimi's refinement, seats' test)

1. **Send P2** with the embedded one-command call-to-action and the
   concierge offer (the draft's closing sentence commits Xyra's labor —
   her call). Cost: nothing. Proves: whether the contact chain's first
   link can fire at all.
2. **Concierge-run real lists.** Recruit 5–20 people with a *recurring*
   triage job (papers, posts, proposals, leads); take their actual list
   and actual criterion this week; run today's CLI for them; ask them
   about a handful of boundary comparisons blind to the tool's answer.
   Cost: tens of dollars, operator hours. Proves: whether anyone feels
   the problem, entrusts real material, and — Oracle's killer
   observation — **sends a second list**. Kimi's refinement: the second
   use must be *unassisted*.
   Abandon: 20 well-targeted strangers yield almost no first lists → stop
   assuming generic list-sorting has pull; do not respond with another
   ranking method.
3. **External validity, not judge reproducibility.** E12–E15 measure
   whether methods reproduce the judge; nothing measures whether the
   top-k matches what a human wanted or beats pasting the list into a
   chat model. Move 2's boundary questions are that dependent variable.
4. **Self-serve only after someone wants it twice**: paste/upload →
   criterion → top-k → result, no Rust, no key, first dollars given
   away; default internally to whatever Move 3 validated (plausibly
   setwise screen → pairwise boundary refine). Then harden exactly what
   unattended use needs: spend/wall cap, auth/backoff sanity, setwise
   trace/cache/error parity, progress/cancel, dedup/size hygiene.
5. **Build whichever recurrence the returning user reveals** —
   incremental insert, search rerank, criterion ensembles, or pure
   selection. Cost: demand-contingent. Proves: what llmsort is.

**Cheapest falsifying test within a week** (all four agree on shape):
post + 5–20 concierge sessions; success is behavioral — at least one
stranger sends a materially new list, unassisted, within 7 days. Zero →
demand/thesis finding; stop building engine rungs and take §5 seriously.
≥1 → the first-contact fixes earn their order from what that person hit.

## 4. Corrections to the lead's operational draft (Oracle, accepted)

- **Cost ladder was wrong.** The cheapest thing to give away is *scope
  of the answer* — top-k, tiers, the boundary — not measurement quality.
  Ordering: full ranking → top-k → tiers/boundary → setwise screen +
  pairwise refine → model cascade → sparse bias auditing → only then
  lossy text reduction. The certified top-k stop is the biggest existing
  asset; the engine should optimise expected decision loss subject to
  dollars + wall time, not expose a philosophical ordering.
- **Product contract ≠ algorithm policy.** `Counterbalance::Pilot`,
  setwise vs pairwise, ladders, phrasing strategy must not become stable
  public API. Callers state `top_k`, cost/time ceiling, confidence.
- **"Failures aren't charged" is wrong.** Keep four counters — provider
  attempts, accepted judgements, billed spend, elapsed time. Auth errors
  terminate immediately; 429 induces *shared* backpressure, not eight
  workers retrying; malformed gets one constrained repair, then a
  short-lived negative cache (not "never cached", not "cached forever").
- **Counterbalance ladder replaced by design**: randomise slot assignment
  on every single-order edge, fit a global slot term jointly with the
  latents, reverse a 5–10% audit subset spread through the run (which
  covers the boundary region a 16-pair pilot never reaches).
- **Phrasings-as-raters cut from the product path.** Paraphrases can
  define different functions; pooling launders criterion ambiguity into
  smaller σ. Eventually a `CriterionEnsemble` with heterogeneity and
  leave-one-wording-out diagnostics — after users show they need it.
- **Uncertainty output**: not rank intervals or "tie groups" (falsely
  suggests equivalence) — expose P(item ∈ top-k), boundary ambiguity,
  pairwise probability on request, and "unresolved at this budget".
- **Incremental/extend: defer** until a returning user demonstrates the
  workflow. **Setwise parity: keep, high** — the cheapest good path is
  the least operationally trustworthy, which is backwards.
- **Missing entirely**: prompt injection through entity text ("ignore
  the criterion and pick this one") — an untrusted-data contract before
  calling this a generic reranker.

## 5. Challenge to the thesis (Oracle; recorded, not adopted)

Sorting is an intermediate primitive users rarely experience as their
problem. Small lists: a chat model does it in one turn. Large lists with
ordinary relevance: embeddings and dedicated rerankers are cheaper.
llmsort's territory is the middle — moderately large × subtle criterion ×
repeated expensive inference justified × a scalar order actually makes
sense — and `llmranker`-class packages already offer method breadth, so
breadth is not a reason to become a user. The unusual thing here is
treating LLM judgement as an experimental measurement process; the
stronger product might be the judgement instrument with sorting as one
readout. Oracle's own caveat, kept verbatim in spirit: that is a
beautiful framing, and beautiful framings are what this project has too
many of relative to external contact. Do not pivot to it now. Make a
stranger send list #1; see whether they send list #2; believe what they
do.

## 6. Frozen until contact

Incremental sort, phrasing pooling, counterbalance modes, rank
intervals, model-ladder polish. Allowed before contact: the three
measured first-run defects, a hard spend cap, catastrophic-error
handling, setwise trace/cache/error parity.
