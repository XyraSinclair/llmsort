//! Request/response types for the reranking API.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::rating_engine::EngineSpec;

// =============================================================================
// Tier 1: Simple Rerank (/v1/rerank)
// =============================================================================

/// Input document for reranking.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RerankDocument {
    /// Stable identifier for the document.
    pub id: String,
    /// Text content shown to the rater.
    pub text: String,
}

/// Request for single-attribute reranking.
#[derive(Debug, Deserialize)]
pub struct RerankRequest {
    /// Optional query context (folded into attribute_prompt).
    #[serde(default)]
    pub query: Option<String>,

    /// Documents to rerank.
    pub documents: Vec<RerankDocument>,

    /// Attribute identifier (for caching).
    #[serde(default = "default_attribute_id")]
    pub attribute_id: String,

    /// Natural language description of the attribute.
    #[serde(default = "default_attribute_prompt")]
    pub attribute_prompt: String,

    /// Focus region: return/optimize for top k.
    #[serde(default)]
    pub top_k: Option<usize>,

    /// Maximum pairwise comparisons to make.
    #[serde(default)]
    pub comparison_budget: Option<usize>,

    /// Maximum time budget in milliseconds.
    #[serde(default)]
    pub latency_budget_ms: Option<u64>,

    /// Provider-reported spend cap. Batches shrink to fit the remaining
    /// cap (measured mean cost per comparison), so overshoot is bounded by
    /// one counterbalanced pair, not a full batch.
    #[serde(default)]
    pub max_cost_nanodollars: Option<i64>,

    /// Stop when top-k error falls below this threshold.
    #[serde(default = "default_tolerated_error")]
    pub tolerated_error: f64,

    /// Model to use for comparisons.
    #[serde(default)]
    pub model: Option<String>,

    /// Logical rater ID for planner.
    #[serde(default)]
    pub rater_id: Option<String>,

    /// Maximum number of pairwise comparisons to run concurrently.
    /// Defaults to a conservative internal value when omitted.
    #[serde(default)]
    pub comparison_concurrency: Option<usize>,

    /// Maximum total repeats per (attribute, pair) during this rerank run.
    ///
    /// Each successful pairwise comparison increments repeats by 1.
    #[serde(default)]
    pub max_pair_repeats: Option<usize>,

    /// Ask every planned pair in both presentation orders. See
    /// [`MultiRerankRequest::counterbalance_pairs`].
    #[serde(default)]
    pub counterbalance_pairs: bool,

    /// Prune hopeless entities from forced exploration. See
    /// [`MultiRerankTopKSpec::prune_p_topk_below`].
    #[serde(default)]
    pub prune_p_topk_below: Option<f64>,

    /// Prompt template slug for the single attribute (e.g. `canonical_v2`,
    /// `ratio_letter_v1` for the PMF evidence path).
    #[serde(default)]
    pub prompt_template_slug: Option<String>,
}

fn default_attribute_id() -> String {
    "relevance".to_string()
}

fn default_attribute_prompt() -> String {
    "relevance to the query".to_string()
}

fn default_tolerated_error() -> f64 {
    0.1
}

/// Per-document result in the rerank response.
#[derive(Debug, Serialize, Deserialize)]
pub struct RerankResult {
    /// Document identifier.
    pub id: String,
    /// 1-based rank among results.
    pub rank: usize,
    /// Posterior mean in latent space.
    pub latent_mean: f64,
    /// Posterior std in latent space.
    pub latent_std: f64,
    /// Robust z-score: (x - median) / (MAD * 1.4826).
    pub z_score: f64,
    /// Shifted so min = 1.0.
    pub min_normalized: f64,
    /// Percentile among documents (0..1).
    pub percentile: f64,
}

/// Metadata for a rerank response.
#[derive(Debug, Serialize, Deserialize)]
pub struct RerankMeta {
    /// Estimated top-k error (sum of p_flip in band).
    pub topk_error: f64,
    /// User-specified threshold.
    pub tolerated_error: f64,
    /// Total comparisons attempted (including refusals).
    pub comparisons_attempted: usize,
    /// Comparisons that failed before producing a judgement.
    #[serde(default)]
    pub comparisons_failed: usize,
    /// First comparison error observed during the run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_error: Option<String>,
    /// Comparisons that produced observations.
    pub comparisons_used: usize,
    /// Comparisons where model refused.
    pub comparisons_refused: usize,
    /// Comparisons served from cache.
    #[serde(default)]
    pub comparisons_cached: usize,
    /// Budget that was set.
    pub comparison_budget: usize,
    /// Elapsed time.
    pub latency_ms: u128,
    /// Model that was used.
    pub model_used: String,
    /// Rater ID that was used.
    pub rater_id_used: String,
    /// Provider input tokens consumed across all comparisons.
    pub provider_input_tokens: u32,
    /// Provider output tokens generated across all comparisons.
    pub provider_output_tokens: u32,
    /// Provider cost (nanodollars) across all comparisons.
    pub provider_cost_nanodollars: i64,
    /// True when at least one comparison used fallback pricing instead of exact known/provider cost.
    #[serde(default)]
    pub provider_cost_is_estimate: bool,

    /// Entities excluded from further forced exploration by top-k pruning.
    #[serde(default)]
    pub entities_pruned: usize,
    /// Pairs judged in both presentation orders with a decisive direction.
    #[serde(default)]
    pub pairs_counterbalanced: usize,
    /// Counterbalanced pairs whose two orders disagreed on direction.
    #[serde(default)]
    pub position_flips: usize,
    /// Judgements that carried PMF-derived evidence moments (ratio-letter
    /// path) into the solver.
    #[serde(default)]
    pub evidence_judgements: usize,
    /// Evidence judgements whose PMF came from answer-token logprobs
    /// (the rest degraded to sampled mode, loudly).
    #[serde(default)]
    pub logprob_mode_judgements: usize,
    /// Mean probability mass visible at the answer position across
    /// evidence judgements.
    #[serde(default)]
    pub evidence_visible_mass_mean: Option<f64>,
    /// Mean |sum of presented-coordinate log-ratios| over pairs asked in
    /// both orders — ANY instrument (PMF means or point answers): 0 for an
    /// unbiased judge; the magnitude of position bias in nats. This is the
    /// order-axis SYSTEMATIC uncertainty of the run, to be read alongside
    /// the statistical posterior stds.
    #[serde(default)]
    pub evidence_order_residual_mean_abs: Option<f64>,
    /// Per-call context-noise sigma (nats), estimated from the run's own
    /// counterbalance residual (sigma_w = mean|m_fwd + m_rev| * sqrt(pi)/2 —
    /// exact when slot bias is ~0, conservative where real bias exists) and
    /// folded into every evidence observation's variance by an end-of-run
    /// refit, so posterior stds are honest about within-call stochasticity
    /// the PMF cannot see. None when nothing was folded: no counterbalanced
    /// pairs (no estimator) or no evidence observations (nothing to widen).
    #[serde(default)]
    pub evidence_sigma_w: Option<f64>,
    /// Mean curl fraction across attributes: the share of judgement energy
    /// that is cyclically inconsistent (A>B>C>A structure) and cannot be
    /// explained by ANY scores. 0 = transitive judge; the Hodge residual
    /// of the log-ratio edge field.
    #[serde(default)]
    pub judgement_frustration_mean: Option<f64>,

    /// Why the rerank loop stopped. Stop decisions are made on the
    /// in-run (pre-refit) fit; after the honest-σ refit the reported
    /// error budget can exceed the tolerance that triggered the stop —
    /// read `topk_error`, not `stop_reason`, as the certification.
    pub stop_reason: RerankStopReason,
}

/// Response for single-attribute reranking.
#[derive(Debug, Serialize, Deserialize)]
pub struct RerankResponse {
    /// Ranked results, sorted by descending latent_mean.
    pub results: Vec<RerankResult>,
    /// Metadata about the reranking run.
    pub meta: RerankMeta,
}

// =============================================================================
// Tier 2: Multi-Attribute Rerank (/v1/rerank/multi)
// =============================================================================

/// Why the rerank loop stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RerankStopReason {
    /// Current top-k error is <= tolerated_error.
    ToleratedErrorMet,
    /// Certified separation bound implies stable top-k (consecutive checks).
    CertifiedStop,
    /// comparison_budget exhausted.
    BudgetExhausted,
    /// latency_budget_ms exceeded.
    LatencyBudgetExceeded,
    /// max_cost_nanodollars reached.
    CostBudgetExhausted,
    /// Cancellation requested (async worker).
    Cancelled,
    /// Planner produced no proposals.
    NoProposals,
    /// Proposals existed but none were eligible to run.
    NoNewPairs,
    /// Too many consecutive non-retryable comparison failures.
    ConsecutiveFailures,
}

/// Input entity for multi-attribute reranking.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MultiRerankEntity {
    /// Stable identifier.
    pub id: String,
    /// Text content.
    pub text: String,
}

/// Attribute specification in multi-rerank request.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MultiRerankAttributeSpec {
    /// Attribute identifier.
    pub id: String,
    /// Natural language description.
    pub prompt: String,
    /// Optional prompt template slug (e.g., canonical_v2).
    #[serde(default)]
    pub prompt_template_slug: Option<String>,
    /// Weight in global utility.
    pub weight: f64,
}

/// Top-k configuration for multi-rerank.
///
/// Controls how the system decides which items to focus on, when to stop
/// asking questions, and how conservative to be about the answer.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MultiRerankTopKSpec {
    /// How many top items you care about identifying correctly.
    /// The system focuses its comparison budget on separating the top-k from the rest.
    pub k: usize,

    /// How much to prioritize high-weight attributes in planning (default: 1.3).
    /// Values > 1.0 make the planner spend more comparisons on heavily-weighted
    /// attributes; 1.0 = proportional to weight; 2.0 = quadratic emphasis.
    #[serde(default = "default_weight_exponent")]
    pub weight_exponent: f64,

    /// Maximum acceptable probability that the top-k ranking is wrong (default: 0.1).
    /// This is the sum of frontier inversion probabilities — the chance that any
    /// item near the k-boundary is on the wrong side. Lower = more comparisons
    /// but higher confidence. 0.1 means ~90% confidence the top-k is correct.
    #[serde(default = "default_tolerated_error")]
    pub tolerated_error: f64,

    /// How many items around the k-boundary to monitor for uncertainty (default: 5).
    /// A band_size of 5 means tracking items ranked k-5 through k+5. Larger values
    /// catch more edge cases but increase planner work.
    #[serde(default = "default_band_size")]
    pub band_size: usize,

    /// Use exact effective-resistance planning when the active set is this small
    /// or smaller (default: 64). Effective resistance measures how much a new
    /// comparison would reduce uncertainty — exact computation is O(n³) so we
    /// only do it for small sets.
    #[serde(default = "default_effective_resistance_max_active")]
    pub effective_resistance_max_active: usize,

    /// Safety margin for the certified stopping check (default: 1.25).
    /// Inflates uncertainty estimates by this factor before checking if the top-k
    /// is settled. Values > 1.0 make the system ask extra questions rather than
    /// risk stopping with an incorrect ranking. 1.0 = no margin.
    #[serde(default = "default_stop_sigma_inflate")]
    pub stop_sigma_inflate: f64,

    /// How many consecutive rounds the certified stop check must pass before
    /// actually stopping (default: 2). Prevents premature stops from lucky
    /// fluctuations in the uncertainty estimate.
    #[serde(default = "default_stop_min_consecutive")]
    pub stop_min_consecutive: usize,

    /// Minimum total edge count (summed across all attributes) each entity must
    /// have before the planner switches to pure exploitation.  Until every entity
    /// reaches this threshold, a fraction of each batch is reserved for comparing
    /// under-observed entities against well-measured anchors.  Default: 2.
    /// Set to 0 to disable forced exploration.
    #[serde(default = "default_min_explore_degree")]
    pub min_explore_degree: usize,

    /// When set, stop spending forced-exploration comparisons on entities
    /// that already have at least one observation, sit below the top-k
    /// boundary, and whose probability of crossing it is under this
    /// threshold. Saves queries when only the top-k matters; pruned entities
    /// keep their scores and can re-enter if evidence moves them back into
    /// the band. Off by default. The count of pruned entities is reported in
    /// `entities_pruned`.
    #[serde(default)]
    pub prune_p_topk_below: Option<f64>,
}

fn default_weight_exponent() -> f64 {
    1.3
}

fn default_band_size() -> usize {
    5
}

fn default_effective_resistance_max_active() -> usize {
    64
}

fn default_stop_sigma_inflate() -> f64 {
    1.25
}

fn default_stop_min_consecutive() -> usize {
    2
}

fn default_min_explore_degree() -> usize {
    2
}

/// Gate specification for filtering entities.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MultiRerankGateSpec {
    /// Attribute to gate on.
    pub attribute_id: String,
    /// Unit for threshold: "latent", "z", "percentile", "min_norm".
    #[serde(default = "default_gate_unit")]
    pub unit: String,
    /// Comparison operator: ">=" or "<=".
    pub op: String,
    /// Threshold value.
    pub threshold: f64,
}

fn default_gate_unit() -> String {
    "latent".to_string()
}

/// Request for multi-attribute reranking.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MultiRerankRequest {
    /// Entities to rerank.
    pub entities: Vec<MultiRerankEntity>,

    /// Attributes with weights.
    pub attributes: Vec<MultiRerankAttributeSpec>,

    /// Top-k configuration.
    pub topk: MultiRerankTopKSpec,

    /// Optional gates for filtering.
    #[serde(default)]
    pub gates: Vec<MultiRerankGateSpec>,

    /// Maximum pairwise comparisons.
    #[serde(default)]
    pub comparison_budget: Option<usize>,

    /// Maximum time budget in milliseconds.
    #[serde(default)]
    pub latency_budget_ms: Option<u64>,

    /// Provider-reported spend cap. Batches shrink to fit the remaining
    /// cap (measured mean cost per comparison), so overshoot is bounded by
    /// one counterbalanced pair, not a full batch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_cost_nanodollars: Option<i64>,

    /// Model to use.
    #[serde(default)]
    pub model: Option<String>,

    /// Logical rater ID.
    #[serde(default)]
    pub rater_id: Option<String>,

    /// Maximum number of pairwise comparisons to run concurrently.
    /// Defaults to a conservative internal value when omitted.
    #[serde(default)]
    pub comparison_concurrency: Option<usize>,

    /// Maximum total repeats per (attribute, pair) during this rerank run.
    ///
    /// Each successful pairwise comparison increments repeats by 1.
    #[serde(default)]
    pub max_pair_repeats: Option<usize>,

    /// Randomly swap which entity is presented as "A" vs "B" in each
    /// comparison to eliminate position bias.  Default: true.
    ///
    /// Ignored when `counterbalance_pairs` is set: counterbalancing asks both
    /// orders deterministically, which subsumes randomization.
    #[serde(default = "default_randomize_presentation_order")]
    pub randomize_presentation_order: bool,

    /// Ask every planned pair in BOTH presentation orders (A-then-B and
    /// B-then-A), spending two comparisons per pair. This cancels position
    /// bias per-pair instead of merely averaging it across the run, and
    /// turns order disagreement into a measurable diagnostic
    /// (`pairs_counterbalanced` / `position_flips` in the response meta).
    /// Default: false (preserves existing request semantics; the `sort`
    /// surface enables it by default).
    #[serde(default)]
    pub counterbalance_pairs: bool,
}

fn default_randomize_presentation_order() -> bool {
    true
}

/// Signed log-ratio of a pairwise judgement toward the FIRST item of a
/// canonical pair, given which slot that item occupied. The one reflection
/// rule, written once: `None` = refused.
#[must_use]
pub fn signed_log_ratio_toward_first(
    judgement: &PairwiseJudgement,
    first_in_slot_a: bool,
) -> Option<f64> {
    match judgement {
        PairwiseJudgement::Observation {
            higher_ranked,
            ratio,
            ..
        } => {
            let toward_slot_a = match higher_ranked {
                HigherRanked::A => 1.0,
                HigherRanked::B => -1.0,
            };
            let toward_first = if first_in_slot_a {
                toward_slot_a
            } else {
                -toward_slot_a
            };
            Some(toward_first * ratio.max(1.0).ln())
        }
        PairwiseJudgement::Refused => None,
    }
}

/// Per-attribute score summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributeScoreSummary {
    /// Posterior mean in latent space.
    pub latent_mean: f64,
    /// Posterior std.
    pub latent_std: f64,
    /// Robust z-score.
    pub z_score: f64,
    /// Min-normalized (min -> 1.0).
    pub min_normalized: f64,
    /// Percentile among feasible entities.
    pub percentile: f64,
}

/// Per-entity result in multi-rerank response.
#[derive(Debug, Serialize, Deserialize)]
pub struct MultiRerankEntityResult {
    /// Entity identifier.
    pub id: String,
    /// 1-based rank among feasible entities, None if infeasible.
    pub rank: Option<usize>,
    /// Whether entity passes all gates.
    pub feasible: bool,
    /// Combined utility mean.
    pub u_mean: f64,
    /// Combined utility std.
    pub u_std: f64,
    /// Probability of crossing the k-boundary (Gaussian approximation).
    pub p_flip: f64,
    /// Per-attribute scores.
    pub attribute_scores: HashMap<String, AttributeScoreSummary>,
}

/// Metadata for multi-rerank response.
#[derive(Debug, Serialize, Deserialize)]
pub struct MultiRerankMeta {
    /// Global top-k error (frontier inversion bound).
    pub global_topk_error: f64,
    /// User-specified threshold.
    pub tolerated_error: f64,
    /// k value used.
    pub k: usize,
    /// Frontier width used.
    pub band_size: usize,
    /// Total comparisons attempted.
    pub comparisons_attempted: usize,
    /// Comparisons that produced observations.
    pub comparisons_used: usize,
    /// Comparisons where model refused.
    pub comparisons_refused: usize,
    /// Comparisons served from cache.
    #[serde(default)]
    pub comparisons_cached: usize,
    /// Budget that was set.
    pub comparison_budget: usize,
    /// Elapsed time.
    pub latency_ms: u128,
    /// Model that was used.
    pub model_used: String,
    /// Rater ID that was used.
    pub rater_id_used: String,
    /// Complete solver configuration preimage for replay.
    /// Trace rows bind accepted observations to this spec's content ID.
    #[serde(default)]
    pub engine_spec: Option<EngineSpec>,
    /// Warm-start observations entered the solver without trace rows.
    /// Replay must fail closed when this is nonzero.
    #[serde(default)]
    pub warm_start_observations: usize,
    /// Provider input tokens consumed across all comparisons.
    pub provider_input_tokens: u32,
    /// Provider output tokens generated across all comparisons.
    pub provider_output_tokens: u32,
    /// Provider cost (nanodollars) across all comparisons.
    pub provider_cost_nanodollars: i64,
    /// True when at least one comparison used fallback pricing instead of exact known/provider cost.
    #[serde(default)]
    pub provider_cost_is_estimate: bool,

    /// Entities excluded from further forced exploration because their
    /// probability of reaching the top-k fell below
    /// `topk.prune_p_topk_below`.
    #[serde(default)]
    pub entities_pruned: usize,
    /// Pairs judged in both presentation orders with a decisive (ratio > 1)
    /// direction in each. Only populated when `counterbalance_pairs` was set.
    #[serde(default)]
    pub pairs_counterbalanced: usize,
    /// Counterbalanced pairs whose two presentation orders disagreed on
    /// direction — a direct, per-run measurement of position bias.
    #[serde(default)]
    pub position_flips: usize,
    /// Judgements that carried PMF-derived evidence moments (ratio-letter
    /// path) into the solver.
    #[serde(default)]
    pub evidence_judgements: usize,
    /// Evidence judgements whose PMF came from answer-token logprobs
    /// (the rest degraded to sampled mode, loudly).
    #[serde(default)]
    pub logprob_mode_judgements: usize,
    /// Mean probability mass visible at the answer position across
    /// evidence judgements.
    #[serde(default)]
    pub evidence_visible_mass_mean: Option<f64>,
    /// Mean |sum of presented-coordinate log-ratios| over pairs asked in
    /// both orders — ANY instrument (PMF means or point answers): 0 for an
    /// unbiased judge; the magnitude of position bias in nats. This is the
    /// order-axis SYSTEMATIC uncertainty of the run, to be read alongside
    /// the statistical posterior stds.
    #[serde(default)]
    pub evidence_order_residual_mean_abs: Option<f64>,
    /// Per-call context-noise sigma (nats), estimated from the run's own
    /// counterbalance residual (sigma_w = mean|m_fwd + m_rev| * sqrt(pi)/2 —
    /// exact when slot bias is ~0, conservative where real bias exists) and
    /// folded into every evidence observation's variance by an end-of-run
    /// refit, so posterior stds are honest about within-call stochasticity
    /// the PMF cannot see. None when nothing was folded: no counterbalanced
    /// pairs (no estimator) or no evidence observations (nothing to widen).
    #[serde(default)]
    pub evidence_sigma_w: Option<f64>,
    /// Mean curl fraction across attributes: the share of judgement energy
    /// that is cyclically inconsistent (A>B>C>A structure) and cannot be
    /// explained by ANY scores. 0 = transitive judge; the Hodge residual
    /// of the log-ratio edge field.
    #[serde(default)]
    pub judgement_frustration_mean: Option<f64>,

    /// Why the rerank loop stopped. Stop decisions are made on the
    /// in-run (pre-refit) fit; after the honest-σ refit the reported
    /// error budget can exceed the tolerance that triggered the stop —
    /// read `topk_error`, not `stop_reason`, as the certification.
    pub stop_reason: RerankStopReason,
}

/// Response for multi-attribute reranking.
#[derive(Debug, Serialize, Deserialize)]
pub struct MultiRerankResponse {
    /// Ranked entities.
    pub entities: Vec<MultiRerankEntityResult>,
    /// Metadata about the run.
    pub meta: MultiRerankMeta,
    /// Multi-objective view: indices (into `entities`) of the Pareto front —
    /// feasible entities not dominated on posterior means across ALL
    /// attributes (with positive-weight orientation; negative-weight
    /// attributes contribute inverted). Computed on means; treat membership
    /// near ties with the per-attribute stds in mind. Empty when fewer than
    /// two attributes.
    #[serde(default)]
    pub pareto_front: Vec<usize>,
    /// Pearson correlation matrix between attribute latent-mean vectors
    /// (attribute order matches the request). High off-diagonal values mean
    /// the attributes are measuring nearly the same thing — evidence for
    /// deciding whether the extra attribute earns its comparison budget.
    /// Empty when fewer than two attributes.
    #[serde(default)]
    pub attribute_correlations: Vec<Vec<f64>>,
}

// =============================================================================
// Internal types
// =============================================================================

/// Direction of preference in a pairwise comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HigherRanked {
    A,
    B,
}

/// Result of a pairwise LLM comparison.
#[derive(Debug, Clone)]
pub enum PairwiseJudgement {
    /// Valid comparison result.
    Observation {
        higher_ranked: HigherRanked,
        ratio: f64,
        confidence: f64,
    },
    /// Model refused to judge.
    Refused,
}
