use std::collections::HashSet;

use crate::gateway::pricing as provider_pricing;
use crate::prompts::prompt_by_slug;
use crate::text_chunking::count_tokens;

use super::super::comparison::{
    estimate_pairwise_input_tokens, ComparisonError, PAIRWISE_MAX_OUTPUT_TOKENS_DEFAULT,
    PAIRWISE_TYPICAL_OUTPUT_TOKENS,
};
use super::super::gates::validate_gate_specs;
use super::super::trace::TraceError;
use super::super::types::MultiRerankRequest;
// =============================================================================
// Constants
// =============================================================================

/// Variance floor applied to PMF-derived evidence at solver ingestion: a
/// delta-certain PMF must not claim infinite precision against the rest of
/// the graph. Public so estimator replay (`examples/replay_trace.rs`) applies
/// the exact ingestion transform when reconstructing solver observations.
pub const EVIDENCE_VAR_FLOOR: f64 = 1e-3;

/// Default batch size for proposed comparisons.
pub(super) const DEFAULT_BATCH_SIZE: usize = 32;

/// Default judge when the caller names none: the current family's
/// mid class (terra sits at gpt-5.4's price and dominates gpt-5.4-mini).
/// The setwise path keeps its own deliberately cheaper authority
/// (`setwise.rs` `DEFAULT_MODEL`, luna): an adequate order needs less
/// judge than cardinal magnitudes.
pub const DEFAULT_MODEL: &str = "openai/gpt-5.6-terra";

/// Default maximum number of comparisons to run concurrently.
pub(super) const DEFAULT_COMPARISON_CONCURRENCY: usize = 8;
pub(super) const MAX_COMPARISON_CONCURRENCY: usize = 64;

/// Hard caps to prevent resource exhaustion / DoS.
///
/// Note: the per-attribute RatingEngine uses a dense solver and currently rejects n > 5,000.
pub(super) const MAX_ENTITIES: usize = 5_000;
pub(super) const MAX_ATTRIBUTES: usize = 256;
const MAX_GATES: usize = 256;

/// Rerank billing multiplier: 20% markup on top of provider cost.
const RERANK_MARKUP_NUM: i64 = 6;
const RERANK_MARKUP_DEN: i64 = 5;

// =============================================================================
// Error type
// =============================================================================

#[derive(Debug, thiserror::Error)]
pub enum MultiRerankError {
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
    #[error("Trait search error: {0}")]
    TraitSearch(#[from] crate::trait_search::TraitSearchError),
    #[error("Rating engine error: {0}")]
    RatingEngine(String),
    #[error("Comparison error: {0}")]
    Comparison(#[from] ComparisonError),
    #[error("Trace error: {0}")]
    Trace(#[from] TraceError),
}

// =============================================================================
// Billing helpers
// =============================================================================

/// Apply 20% markup to provider cost, rounding up.
pub fn apply_rerank_markup(provider_cost_nanodollars: i64) -> i64 {
    if provider_cost_nanodollars <= 0 {
        return 0;
    }
    // ceil(cost * 6/5)
    (provider_cost_nanodollars.saturating_mul(RERANK_MARKUP_NUM) + (RERANK_MARKUP_DEN - 1))
        / RERANK_MARKUP_DEN
}

/// Conservative reservation estimate for a rerank request.
///
/// Reserves enough credits to cover the worst case (comparison_budget comparisons),
/// then refunds unused credits at completion.
#[derive(Debug, Clone, Copy)]
pub struct RerankChargeEstimate {
    pub comparison_budget: usize,
    pub input_tokens_per_comparison: u32,
    pub output_tokens_per_comparison: u32,
    pub typical_output_tokens_per_comparison: u32,
    pub provider_cost_typical_nanodollars: i64,
    pub provider_cost_max_nanodollars: i64,
    pub user_charge_max_nanodollars: i64,
}

/// The template a `None` `prompt_template_slug` resolves to for `model`:
/// the single-token PMF rail wherever the measured logprob matrix serves it
/// (docs/LOGPROBS.md; E9 head-to-head at identical cost: stat error ±0.020
/// vs ±0.464, rank agreement ρ 0.886), the canonical JSON rail elsewhere.
/// `None` model means the library default model.
pub fn default_template_slug(model: Option<&str>) -> &'static str {
    let model = model.unwrap_or(DEFAULT_MODEL);
    match crate::rerank::comparison::seriate_logprob_route(model) {
        // Two-phase for reasoning-native judges; rationale and measured
        // numbers on `SeriateLogprobRoute::requires_effort_none`.
        Some(route) if route.requires_effort_none => {
            crate::rerank::comparison::RATIO_LETTER_2P_SLUG
        }
        Some(_) => crate::rerank::comparison::RATIO_LETTER_SLUG,
        None => "canonical_v2",
    }
}

/// Materialize the per-model template default into every attribute that
/// didn't choose one, so dispatch, cache keys, and trace rows all carry the
/// concrete instrument that actually ran.
pub(super) fn materialize_template_defaults(req: &mut MultiRerankRequest) {
    let slug = default_template_slug(req.model.as_deref());
    for attr in &mut req.attributes {
        if attr.prompt_template_slug.is_none() {
            attr.prompt_template_slug = Some(slug.to_string());
        }
    }
}

pub fn estimate_max_rerank_charge(req: &MultiRerankRequest) -> RerankChargeEstimate {
    let n_entities = req.entities.len();
    let n_attributes = req.attributes.len();
    let comparison_budget = req
        .comparison_budget
        .unwrap_or_else(|| default_comparison_budget(n_entities, n_attributes));

    if n_entities < 2 || n_attributes == 0 || comparison_budget == 0 {
        return RerankChargeEstimate {
            comparison_budget,
            input_tokens_per_comparison: 0,
            output_tokens_per_comparison: PAIRWISE_MAX_OUTPUT_TOKENS_DEFAULT,
            typical_output_tokens_per_comparison: PAIRWISE_TYPICAL_OUTPUT_TOKENS,
            provider_cost_typical_nanodollars: 0,
            provider_cost_max_nanodollars: 0,
            user_charge_max_nanodollars: 0,
        };
    }

    let model = req.model.as_deref().unwrap_or(DEFAULT_MODEL);
    // Evidence-path templates answer with a single token; their call cap is
    // 16 output tokens, not the JSON path's reasoning-sized ceiling. A `None`
    // slug prices what it will actually run: the per-model default.
    let default_slug = default_template_slug(req.model.as_deref());
    let all_evidence = req.attributes.iter().all(|a| {
        crate::rerank::comparison::is_evidence_slug(
            a.prompt_template_slug.as_deref().unwrap_or(default_slug),
        )
    });
    let any_two_phase = req.attributes.iter().any(|a| {
        a.prompt_template_slug.as_deref().unwrap_or(default_slug)
            == crate::rerank::comparison::RATIO_LETTER_2P_SLUG
    });
    let output_tokens_per_comparison = if all_evidence {
        if any_two_phase {
            // Analysis turn cap plus the one-token verdict call's floor.
            crate::rerank::comparison::TWO_PHASE_ANALYSIS_MAX_TOKENS + 16
        } else {
            16
        }
    } else {
        // A mixed pool prices the JSON ceiling; the 2-call multiplier below
        // then strictly dominates the two-phase shape as well.
        PAIRWISE_MAX_OUTPUT_TOKENS_DEFAULT
    };
    // Instruments that make several gateway calls per comparison (each
    // resending the full prompt): decimal ledger K draws, two-phase 2.
    let calls_per_comparison = if req.attributes.iter().any(|a| {
        a.prompt_template_slug.as_deref() == Some(crate::rerank::comparison::DECIMAL_LEDGER_SLUG)
    }) {
        crate::rerank::comparison::DECIMAL_LEDGER_DRAWS as u32
    } else if any_two_phase {
        2
    } else {
        1
    };

    // Worst-case attribute prompt: choose the largest prompt by token count (bounded, cheap).
    let (attr_id, attr_prompt, attr_template_slug) = req
        .attributes
        .iter()
        .map(|a| {
            (
                a.id.as_str(),
                a.prompt.as_str(),
                a.prompt_template_slug.as_deref(),
            )
        })
        .max_by_key(|(_, p, _)| count_tokens(p))
        .expect("n_attributes > 0 checked above, so attributes is non-empty");

    // Worst-case entity texts: choose 2 longest by byte length (fast upper bound).
    let mut idxs: Vec<usize> = (0..n_entities).collect();
    idxs.sort_by_key(|&i| req.entities[i].text.len());
    idxs.reverse();
    let a_text = &req.entities[idxs[0]].text;
    let b_text = &req.entities[idxs[1]].text;

    let input_tokens_per_comparison =
        estimate_pairwise_input_tokens(attr_id, attr_prompt, attr_template_slug, a_text, b_text);

    // Provider cost per comparison at the capped max output tokens, times
    // the calls one comparison actually makes (decimal ledger: K draws).
    let provider_cost_per_comparison = provider_pricing::chat_cost(
        model,
        input_tokens_per_comparison,
        output_tokens_per_comparison,
    )
    .saturating_mul(i64::from(calls_per_comparison));
    let typical_output_tokens_per_call =
        PAIRWISE_TYPICAL_OUTPUT_TOKENS.min(output_tokens_per_comparison);
    let provider_cost_typical_per_comparison = provider_pricing::chat_cost(
        model,
        input_tokens_per_comparison,
        typical_output_tokens_per_call,
    )
    .saturating_mul(i64::from(calls_per_comparison));

    let provider_cost_max_nanodollars =
        provider_cost_per_comparison.saturating_mul(comparison_budget as i64);
    let provider_cost_typical_nanodollars =
        provider_cost_typical_per_comparison.saturating_mul(comparison_budget as i64);
    let user_charge_max_nanodollars = apply_rerank_markup(provider_cost_max_nanodollars);

    RerankChargeEstimate {
        comparison_budget,
        input_tokens_per_comparison: input_tokens_per_comparison
            .saturating_mul(calls_per_comparison),
        output_tokens_per_comparison: output_tokens_per_comparison
            .saturating_mul(calls_per_comparison),
        typical_output_tokens_per_comparison: typical_output_tokens_per_call
            .saturating_mul(calls_per_comparison),
        provider_cost_typical_nanodollars,
        provider_cost_max_nanodollars,
        user_charge_max_nanodollars,
    }
}

// =============================================================================
// Orchestrator
// =============================================================================

/// Default comparison budget: 4 * n * num_attributes.
pub(super) fn default_comparison_budget(n_entities: usize, n_attributes: usize) -> usize {
    4usize
        .saturating_mul(n_entities.max(1))
        .saturating_mul(n_attributes.max(1))
}

pub fn validate_multi_rerank_request(req: &MultiRerankRequest) -> Result<(), MultiRerankError> {
    if req.entities.len() < 2 {
        return Err(MultiRerankError::InvalidRequest(
            "entities must contain at least 2 items".into(),
        ));
    }
    if req.entities.len() > MAX_ENTITIES {
        return Err(MultiRerankError::InvalidRequest(format!(
            "entities must contain at most {MAX_ENTITIES} items (n={})",
            req.entities.len()
        )));
    }
    if req.attributes.is_empty() {
        return Err(MultiRerankError::InvalidRequest(
            "attributes must not be empty".into(),
        ));
    }
    if req.attributes.len() > MAX_ATTRIBUTES {
        return Err(MultiRerankError::InvalidRequest(format!(
            "attributes must contain at most {MAX_ATTRIBUTES} items (n={})",
            req.attributes.len()
        )));
    }

    if req.gates.len() > MAX_GATES {
        return Err(MultiRerankError::InvalidRequest(format!(
            "gates must contain at most {MAX_GATES} items (n={})",
            req.gates.len()
        )));
    }

    if req.topk.k == 0 {
        return Err(MultiRerankError::InvalidRequest(
            "topk.k must be >= 1".into(),
        ));
    }
    if req.topk.k > req.entities.len() {
        return Err(MultiRerankError::InvalidRequest(format!(
            "topk.k must be <= number of entities (k={}, n={})",
            req.topk.k,
            req.entities.len()
        )));
    }
    if req.topk.band_size == 0 {
        return Err(MultiRerankError::InvalidRequest(
            "topk.band_size must be >= 1".into(),
        ));
    }
    if !req.topk.tolerated_error.is_finite() || req.topk.tolerated_error < 0.0 {
        return Err(MultiRerankError::InvalidRequest(
            "topk.tolerated_error must be finite and >= 0".into(),
        ));
    }
    if !req.topk.weight_exponent.is_finite() || req.topk.weight_exponent < 0.0 {
        return Err(MultiRerankError::InvalidRequest(
            "topk.weight_exponent must be finite and >= 0".into(),
        ));
    }
    if !req.topk.stop_sigma_inflate.is_finite() || req.topk.stop_sigma_inflate <= 0.0 {
        return Err(MultiRerankError::InvalidRequest(
            "topk.stop_sigma_inflate must be finite and > 0".into(),
        ));
    }

    if matches!(req.comparison_budget, Some(0)) {
        return Err(MultiRerankError::InvalidRequest(
            "comparison_budget must be >= 1".into(),
        ));
    }
    if req.max_cost_nanodollars.is_some_and(|cap| cap <= 0) {
        return Err(MultiRerankError::InvalidRequest(
            "max_cost_nanodollars must be >= 1".into(),
        ));
    }

    if let Some(concurrency) = req.comparison_concurrency {
        if concurrency == 0 {
            return Err(MultiRerankError::InvalidRequest(
                "comparison_concurrency must be >= 1".into(),
            ));
        }
        if concurrency > MAX_COMPARISON_CONCURRENCY {
            return Err(MultiRerankError::InvalidRequest(format!(
                "comparison_concurrency must be <= {MAX_COMPARISON_CONCURRENCY}"
            )));
        }
    }

    if let Some(max) = req.max_pair_repeats {
        if max == 0 {
            return Err(MultiRerankError::InvalidRequest(
                "max_pair_repeats must be >= 1".into(),
            ));
        }
    }

    let mut entity_ids: HashSet<&str> = HashSet::new();
    for e in &req.entities {
        if !entity_ids.insert(e.id.as_str()) {
            return Err(MultiRerankError::InvalidRequest(format!(
                "duplicate entity id: {}",
                e.id
            )));
        }
    }

    let mut attribute_ids: HashSet<&str> = HashSet::new();
    let mut attribute_definitions: HashSet<(&str, Option<&str>)> = HashSet::new();
    for a in &req.attributes {
        if !a.weight.is_finite() {
            return Err(MultiRerankError::InvalidRequest(format!(
                "attribute weight must be finite (attribute_id={})",
                a.id
            )));
        }
        if let Some(slug) = a.prompt_template_slug.as_deref() {
            if !crate::rerank::comparison::is_evidence_slug(slug) && prompt_by_slug(slug).is_none()
            {
                return Err(MultiRerankError::InvalidRequest(format!(
                    "unknown prompt_template_slug: {slug}"
                )));
            }
        }
        if !attribute_ids.insert(a.id.as_str()) {
            return Err(MultiRerankError::InvalidRequest(format!(
                "duplicate attribute id: {}",
                a.id
            )));
        }
        if !attribute_definitions.insert((a.prompt.as_str(), a.prompt_template_slug.as_deref())) {
            return Err(MultiRerankError::InvalidRequest(format!(
                "duplicate attribute definition: prompt and template match attribute {}",
                a.id
            )));
        }
    }

    validate_gate_specs(&req.gates, &attribute_ids)?;

    Ok(())
}

/// Batch size permitted by the cost cap: `None` once accrued spend has
/// reached the cap, otherwise `DEFAULT_BATCH_SIZE.min(remaining_budget)`
/// shrunk so the batch's projected spend stays inside the cap — measured
/// mean cost per attempted comparison once data exists, the typical-cost
/// estimate before any. Floor 2 (one counterbalanced pair), so overshoot
/// is bounded by ~2 comparisons, never a whole 32-comparison batch.
/// Unknown pricing estimates as 0 and degrades to the uncapped size.
pub(super) fn cost_capped_batch_size(
    req: &MultiRerankRequest,
    accrued: i64,
    attempted: usize,
    remaining_budget: usize,
) -> Option<usize> {
    // A planner wave must never be smaller than the requested comparison
    // concurrency, or the extra parallelism can never leave the station
    // (measured 2026-08-31: --concurrency 48 pinned at 32 in-flight, engine
    // draining to ~5 at every wave tail).
    let uncapped = DEFAULT_BATCH_SIZE
        .max(req.comparison_concurrency.unwrap_or(0))
        .min(remaining_budget);
    let Some(cap) = req.max_cost_nanodollars else {
        return Some(uncapped);
    };
    if accrued >= cap {
        return None;
    }
    let per_comparison = if attempted > 0 && accrued > 0 {
        accrued as f64 / attempted as f64
    } else {
        let est = estimate_max_rerank_charge(req);
        est.provider_cost_typical_nanodollars as f64 / est.comparison_budget.max(1) as f64
    };
    if per_comparison <= 0.0 {
        return Some(uncapped);
    }
    let affordable = ((cap - accrued) as f64 / per_comparison).floor() as usize;
    Some(affordable.clamp(2, uncapped))
}

pub(super) fn finite_or_zero(x: f64) -> f64 {
    if x.is_finite() {
        x
    } else {
        0.0
    }
}
