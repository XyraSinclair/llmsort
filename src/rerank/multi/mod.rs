//! Multi-attribute reranking / trait search orchestrator.
//!
//! Wires together:
//! - TraitSearchManager (multi-attribute top-k uncertainty logic)
//! - RatingEngine (per-attribute IRLS solver)
//! - Pairwise LLM comparisons on a ratio ladder with confidence
//!
//! Core loop:
//! 1. Solve per-attribute rating engines and build global utility + uncertainty.
//! 2. Estimate top-k error via TraitSearchManager::estimate_topk_error().
//! 3. If error > tolerated_error and budgets remain, call propose_batch()
//!    to select highest-value comparisons.
//! 4. For each proposed (attribute_id, i, j):
//!    - Call LLM with evaluator prompt.
//!    - Parse JSON `{higher_ranked, ratio, confidence}` or `{refused:true}`.
//!    - Map to (ln_ratio, variance) and feed into the corresponding engine.
//! 5. Repeat until top-k error ≤ tolerated_error or budget/latency hit.

mod execution;
mod orchestrator;
mod request;
mod response;
mod task;

pub use execution::{
    build_engine_config, build_trait_search_config, JudgementRunInstrumentation, RerankExecution,
};
pub(crate) use orchestrator::multi_rerank_with_failures;
pub use request::{
    apply_rerank_markup, default_template_slug, estimate_max_rerank_charge,
    validate_multi_rerank_request, MultiRerankError, RerankChargeEstimate, DEFAULT_MODEL,
    EVIDENCE_VAR_FLOOR,
};

/// Run a multi-attribute reranking session.
pub async fn multi_rerank(
    request: super::types::MultiRerankRequest,
    execution: RerankExecution<'_>,
) -> Result<super::types::MultiRerankResponse, MultiRerankError> {
    Ok(multi_rerank_with_failures(request, execution)
        .await?
        .response)
}

#[cfg(test)]
mod tests;
