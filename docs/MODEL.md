# Model

This is the compact mathematical contract behind `llmsort`. `docs/ALGORITHM.md` gives the engineering rationale; this page states the objects, assumptions, and limits.

## Objects

For one attribute, each entity $i \in \{1, \dots, n\}$ has an unobserved latent score $s_i \in \mathbb{R}$. The model only cares about score differences. Adding the same constant to every $s_i$ changes nothing, so the fitted system is gauge-fixed internally.

A pairwise judgement on entities $(i, j)$ returns:

- `higher_ranked`: which side has more of the attribute,
- `ratio`: how many times more, chosen from the finite ladder in `docs/PROMPTS.md`,
- `confidence`: optional self-report in $[0,1]$, retained as trace metadata rather than assumed precision,
- optional refusal.

A non-refused judgement becomes a noisy edge:

$$
y_{ij} = \operatorname{sign}(i,j) \log(r_{ij}) \approx s_i - s_j.
$$

Repeated judgements on the same pair are fused as observations of the same latent difference. Point observations have unit precision; evidence paths may supply positive measured precision derived from a response distribution.

## Robust solve

The solver fits scores by robust iteratively reweighted least squares over the comparison graph:

$$
\min_s \sum_{(i,j) \in E} w_{ij}\,\rho\!\left((s_i - s_j) - y_{ij}\right) + \lambda \lVert s \rVert_2^2.
$$

Implementation notes:

- ratio values are clamped onto the finite ladder before becoming log edges,
- explicit measured precision, repeats, rater efficacy, and attribute temperature determine edge weights; self-reported confidence does not,
- Huber-style residual weighting reduces the damage from inconsistent or adversarial edges,
- ridge regularization and gauge pinning keep disconnected or weakly connected systems numerically sane,
- the current implementation uses dense linear algebra and caps item counts accordingly.

## Uncertainty

After solving, the engine keeps per-entity diagonal covariance estimates and rank-risk diagnostics. For planner work it uses effective resistance of a proposed edge:

$$
\operatorname{var}(s_i - s_j) \approx \Sigma_{ii} + \Sigma_{jj} - 2\Sigma_{ij}.
$$

Exact covariance is available while the active set is small. For larger solves the implementation uses diagonal approximations, including Hutchinson-style trace estimation where appropriate. These are operational uncertainty estimates, not a full Bayesian posterior over all possible rankings.

## Multi-attribute utility

For attributes $a \in A$, each attribute has its own fitted latent state $s_{i,a}$. The multi-rerank layer normalizes per-attribute state and combines it with configured weights:

$$
U_i = \sum_{a \in A} \alpha_a\,z_{i,a}.
$$

Gates are separate constraints. They decide feasibility from an attribute value expressed as one of the supported units:

- latent score,
- z-score,
- percentile,
- min-normalized score.

An entity can rank well by utility and still be infeasible if it fails a gate.

## Planner objective

The active planner proposes comparisons near the current top-k frontier. A candidate pair is valuable when it is likely to reduce uncertainty where a rank flip would change the selected set. The planner biases toward:

- entities close to the top-k boundary,
- pairs with high effective resistance,
- under-measured attributes,
- feasible or near-feasible entities when gates matter,
- exploration anchors when the comparison graph is sparse.

The objective is heuristic. It is meant to spend comparisons better than uniform random pairs, not to prove global optimality.

## Stopping

A run stops when one of the explicit stop reasons is reached:

- `tolerated_error_met`: estimated top-k error is within the requested tolerance,
- `certified_stop`: the separation check sees a stable top-k boundary,
- `budget_exhausted`: comparison budget is spent,
- `latency_budget_exceeded`: latency budget is spent,
- `cancelled`: caller requested cancellation,
- `no_proposals`: the planner has no useful comparison,
- `no_new_pairs`: all useful pairs are already known or blocked.

Only the first two reasons are convergence-like stops. Budget and latency stops require reading the reported top-k error before treating the result as settled.

## Model routing and cost accounting

The solver is model-agnostic: a rater can be a fixed model, a ladder policy, a cache hit, or a human-compatible implementation behind the same pairwise judgement contract. The research examples under `research/examples/model-policy-*.json` currently cover:

- quality-only routing with `anthropic/claude-opus-4.6`,
- cost-aware/fast routing with `deepseek/deepseek-v4-flash`,
- a frontier ladder that starts on `anthropic/claude-opus-4.6`, falls back through `google/gemini-3.1-pro-preview`, and uses `openai/gpt-5.4-mini` once uncertainty is low enough.

Model slugs prefixed `claude-code/` route through a subscription-billed Claude Code adapter instead of OpenRouter (`src/gateway/types/mod.rs`, `src/gateway/claude_code.rs`) — same pairwise judgement contract, $0 marginal provider cost, measured 21/21 decisive-pair agreement with the API rail (`research/notes/claudecode-vs-api-2026-08-06/RESULTS.md`).

Pricing is separate from ranking. Reports distinguish exact local/provider cost from fallback cost estimates; an estimated cost is operational telemetry, not evidence that the provider actually charged that amount.

## Assumptions

The model works best when:

- the attribute is comparative and the ratio wording is meaningful,
- raters interpret the prompt consistently enough that repeats are informative,
- the comparison graph connects the entities that may enter top-k,
- refusals are explicit rather than hidden in malformed JSON,
- gates are monotone in the attribute units they use,
- users inspect uncertainty instead of treating rank order as deterministic truth.

## Non-goals

This crate does not claim:

- absolute, calibration-free scores,
- a universal win over Likert ratings,
- a full Bayesian posterior over rankings,
- immunity to prompt injection or malicious raters,
- optimal active-learning policy,
- useful rankings when the prompt asks for an incoherent attribute.

## Known failure modes

- Sparse graphs can look overconfident for disconnected clusters.
- Severe outlier edges can still move the top-k frontier despite robust weighting.
- Budget stops can return a good-looking ordering with an unsettled error bound.
- Per-attribute normalization can hide a bad attribute if the weight configuration is wrong.
- Distribution-derived precision depends on provider logprob behavior and tokenization details.
- Dense covariance and planner work bound the current practical scale.

The evaluation study record in `research/notes/EVALUATION.md` shows current strengths and embarrassment cases from the synthetic harness.
