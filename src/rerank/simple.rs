//! Simple single-attribute reranking.
//!
//! This is syntactic sugar over multi-attribute reranking with a single attribute.
//! Guarantees identical semantics while providing a simpler request shape.

use super::multi::{multi_rerank_with_failures, MultiRerankError, RerankExecution};
use super::types::{
    MultiRerankAttributeSpec, MultiRerankEntity, MultiRerankRequest, MultiRerankTopKSpec,
    RerankMeta, RerankRequest, RerankResponse, RerankResult,
};

// =============================================================================
// Simple rerank
// =============================================================================

/// Convert a simple rerank request into a multi-rerank request.
pub fn to_multi_request(req: &RerankRequest) -> MultiRerankRequest {
    // Convert documents to entities
    let entities: Vec<MultiRerankEntity> = req
        .documents
        .iter()
        .map(|doc| MultiRerankEntity {
            id: doc.id.clone(),
            text: doc.text.clone(),
        })
        .collect();

    // Build attribute prompt, optionally incorporating query
    let attribute_prompt = if let Some(ref query) = req.query {
        format!("{} for query: '{}'", req.attribute_prompt, query)
    } else {
        req.attribute_prompt.clone()
    };

    // Single attribute with weight 1.0
    let attributes = vec![MultiRerankAttributeSpec {
        id: req.attribute_id.clone(),
        prompt: attribute_prompt,
        prompt_template_slug: req.prompt_template_slug.clone(),
        weight: 1.0,
    }];

    // Top-k config
    let n = entities.len();
    let k = req.top_k.unwrap_or(n);
    let topk = MultiRerankTopKSpec {
        k,
        weight_exponent: 1.0, // No exponent effect with single attribute
        tolerated_error: req.tolerated_error,
        band_size: 5,
        effective_resistance_max_active: 64,
        stop_sigma_inflate: 1.25,
        stop_min_consecutive: 2,
        min_explore_degree: 2,
        prune_p_topk_below: req.prune_p_topk_below,
    };

    MultiRerankRequest {
        entities,
        attributes,
        topk,
        gates: Vec::new(), // No gates in simple mode
        comparison_budget: req.comparison_budget,
        latency_budget_ms: req.latency_budget_ms,
        max_cost_nanodollars: req.max_cost_nanodollars,
        model: req.model.clone(),
        rater_id: req.rater_id.clone(),
        comparison_concurrency: req.comparison_concurrency,
        max_pair_repeats: req.max_pair_repeats,
        randomize_presentation_order: true,
        counterbalance_pairs: req.counterbalance_pairs,
    }
}

/// Run a single-attribute reranking session.
///
/// Internally converts to a multi-attribute request with one attribute.
pub async fn rerank(
    req: RerankRequest,
    execution: RerankExecution<'_>,
) -> Result<RerankResponse, MultiRerankError> {
    let multi_req = to_multi_request(&req);

    let outcome = multi_rerank_with_failures(multi_req, execution).await?;
    let multi_resp = outcome.response;

    // Map response to simple format
    let results: Vec<RerankResult> = multi_resp
        .entities
        .iter()
        .filter(|e| e.feasible)
        .map(|e| {
            // Get scores for our single attribute
            let attr_scores = e.attribute_scores.get(&req.attribute_id);

            RerankResult {
                id: e.id.clone(),
                rank: e.rank.unwrap_or(0),
                latent_mean: attr_scores.map(|s| s.latent_mean).unwrap_or(0.0),
                latent_std: attr_scores.map(|s| s.latent_std).unwrap_or(0.0),
                z_score: attr_scores.map(|s| s.z_score).unwrap_or(0.0),
                min_normalized: attr_scores.map(|s| s.min_normalized).unwrap_or(1.0),
                percentile: attr_scores.map(|s| s.percentile).unwrap_or(0.0),
            }
        })
        .collect();

    let meta = meta_from_multi(
        multi_resp.meta,
        outcome.comparisons_failed,
        outcome.first_error,
    );

    Ok(RerankResponse { results, meta })
}

/// Flatten multi-rerank run metadata into the single-attribute meta shape.
pub(crate) fn meta_from_multi(
    meta: super::types::MultiRerankMeta,
    comparisons_failed: usize,
    first_error: Option<String>,
) -> RerankMeta {
    RerankMeta {
        topk_error: meta.global_topk_error,
        tolerated_error: meta.tolerated_error,
        comparisons_attempted: meta.comparisons_attempted,
        comparisons_failed,
        first_error,
        comparisons_used: meta.comparisons_used,
        comparisons_refused: meta.comparisons_refused,
        comparisons_cached: meta.comparisons_cached,
        comparison_budget: meta.comparison_budget,
        latency_ms: meta.latency_ms,
        model_used: meta.model_used,
        rater_id_used: meta.rater_id_used,
        provider_input_tokens: meta.provider_input_tokens,
        provider_output_tokens: meta.provider_output_tokens,
        provider_cost_nanodollars: meta.provider_cost_nanodollars,
        provider_cost_is_estimate: meta.provider_cost_is_estimate,
        entities_pruned: meta.entities_pruned,
        pairs_counterbalanced: meta.pairs_counterbalanced,
        position_flips: meta.position_flips,
        evidence_judgements: meta.evidence_judgements,
        logprob_mode_judgements: meta.logprob_mode_judgements,
        evidence_visible_mass_mean: meta.evidence_visible_mass_mean,
        evidence_order_residual_mean_abs: meta.evidence_order_residual_mean_abs,
        evidence_sigma_w: meta.evidence_sigma_w,
        evidence_obs_sigma_rms: meta.evidence_obs_sigma_rms,
        judgement_frustration_mean: meta.judgement_frustration_mean,
        stop_reason: meta.stop_reason,
    }
}
