//! Extension hooks for integrating the rerank core into production systems.
//!
//! Cardinal-harness stays DB-agnostic. Production callers can inject:
//! - Warm-start observations (e.g., from Postgres)
//! - Per-comparison side effects (e.g., persistence, progress reporting)

use std::collections::HashMap;

use crate::rating_engine::Observation;

use super::comparison::ComparisonUsage;
use super::types::{MultiRerankRequest, PairwiseJudgement};

#[derive(Debug, Default)]
pub struct WarmStartData {
    /// Map attribute_id -> observations to seed the solver.
    pub observations_by_attribute: HashMap<String, Vec<Observation>>,
}

#[derive(Debug, thiserror::Error)]
pub enum WarmStartError {
    #[error("{0}")]
    Message(String),
}

/// Suppliers must provide RAW observations (per-comparison variance as
/// measured, e.g. reconstructed from a trace) — never post-refit state.
/// The honest-σ refit widens warm-started evidence observations with the
/// consuming run's own σ_w; pre-inflated input would be widened twice.
#[async_trait::async_trait]
pub trait WarmStartProvider: Send + Sync {
    async fn warm_start(
        &self,
        req: &MultiRerankRequest,
        rater_id: &str,
    ) -> Result<WarmStartData, WarmStartError>;
}

#[derive(Debug, Clone)]
pub struct ComparisonEvent {
    pub attribute_id: String,
    pub attribute_index: usize,
    pub entity_a_id: String,
    pub entity_b_id: String,
    pub entity_a_index: usize,
    pub entity_b_index: usize,
    pub model: String,
    pub judgement: PairwiseJudgement,
    pub usage: ComparisonUsage,
}

#[derive(Debug, thiserror::Error)]
pub enum ObserverError {
    #[error("{0}")]
    Message(String),
}

#[async_trait::async_trait]
pub trait ComparisonObserver: Send + Sync {
    async fn on_comparison(&self, event: ComparisonEvent) -> Result<(), ObserverError>;
}
