//! Reranking API module.
//!
//! Provides LLM-powered pairwise comparison-based reranking with:
//! - Calibrated uncertainty (top-k error semantics)
//! - Multi-attribute composition with weighted traits
//! - Adaptive stopping when error tolerance is met
//!
//! Two API tiers:
//! - Simple: Single-attribute, query-document relevance
//! - Multi: Full trait search with gates and weights

pub mod comparison;
pub mod consortium;
pub mod decimal_ledger;
pub mod elaborate;
pub mod explain;
pub mod gates;
pub mod hooks {
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
}
pub mod model_policy;
pub mod multi;
pub mod options {
    //! Optional execution settings for reproducible runs.

    #[derive(Debug, Clone, Default)]
    pub struct RerankRunOptions {
        /// Override the internal RNG seed used by the planner.
        pub rng_seed: Option<u64>,
        /// Require all comparisons to be served from cache (no network calls).
        pub cache_only: bool,
    }
}
pub mod orbit;
pub mod policy_registry;
#[doc(hidden)]
pub mod proposal_json {
    //! Tolerant extraction of proposal JSON from model completions.
    //!
    //! json_mode output varies by model family: a bare array, an object
    //! wrapping one, an object whose VALUES are the strings, or a valid
    //! object under a pathological key (gpt-5.4-mini shipped `{"[]": [...]}`
    //! during the Manifund P1 run — the old first-`[`-to-last-`]` slice
    //! turned that VALID document into a parse error). The rule here:
    //! parse the whole completion first; only when that fails, scan for the
    //! first balanced JSON span (string- and escape-aware) that parses.

    use serde_json::Value;

    /// Parse a completion into JSON, whole-string first, then the first
    /// balanced `[...]`/`{...}` span that parses. `None` only when no
    /// parseable JSON exists anywhere in the content.
    pub fn lenient_value(content: &str) -> Option<Value> {
        let trimmed = content.trim();
        if trimmed.is_empty() {
            return None;
        }
        if let Ok(v) = serde_json::from_str::<Value>(trimmed) {
            return Some(v);
        }
        let bytes = trimmed.as_bytes();
        for start in 0..bytes.len() {
            let open = bytes[start];
            if open != b'[' && open != b'{' {
                continue;
            }
            if let Some(end) = balanced_end(bytes, start) {
                if let Ok(v) = serde_json::from_str::<Value>(&trimmed[start..=end]) {
                    return Some(v);
                }
            }
        }
        None
    }

    /// Index of the byte closing the bracket opened at `start`, respecting
    /// strings and escapes. `None` when the span never closes.
    fn balanced_end(bytes: &[u8], start: usize) -> Option<usize> {
        let mut depth = 0usize;
        let mut in_string = false;
        let mut escaped = false;
        for (idx, &b) in bytes.iter().enumerate().skip(start) {
            if in_string {
                if escaped {
                    escaped = false;
                } else if b == b'\\' {
                    escaped = true;
                } else if b == b'"' {
                    in_string = false;
                }
                continue;
            }
            match b {
                b'"' => in_string = true,
                b'[' | b'{' => depth += 1,
                b']' | b'}' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        return Some(idx);
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// Pull a list of strings out of whatever JSON shape the model chose:
    /// a bare array, an object wrapping an array (any key), an object whose
    /// values are the strings, or array elements that are single-string
    /// objects.
    pub(crate) fn lenient_string_array(content: &str) -> Option<Vec<String>> {
        let value = lenient_value(content)?;
        let strings = value_string_array(&value)?;
        (!strings.is_empty()).then_some(strings)
    }

    pub fn value_string_array(value: &Value) -> Option<Vec<String>> {
        let element = |v: &Value| -> Option<String> {
            v.as_str().map(str::to_string).or_else(|| {
                v.as_object()
                    .and_then(|o| o.values().find_map(Value::as_str))
                    .map(str::to_string)
            })
        };
        if let Some(array) = value.as_array().or_else(|| {
            value
                .as_object()
                .and_then(|o| o.values().find_map(Value::as_array))
        }) {
            return array.iter().map(element).collect();
        }
        if let Some(object) = value.as_object() {
            let strings: Vec<String> = object.values().filter_map(element).collect();
            if !strings.is_empty() {
                return Some(strings);
            }
        }
        None
    }
}
pub mod report;
pub mod sampling;
pub mod setwise;
pub mod simple;
pub mod sort;
pub mod spin;
pub mod trace;
pub mod types;
pub mod wordings;

/// Cache-routing key (OpenAI `prompt_cache_key`) derived from stable prompt
/// content. Key on the shared PREFIX of the calls that should co-route:
/// requests with equal keys land on the same provider cache shard, so their
/// longest common prompt prefix can hit. Routing hint only — never changes
/// prompt bytes, packet identity, or the judgment cache. Benefit realizes
/// once the shared prefix crosses the provider cache floor (~1024 tokens on
/// OpenAI); short prefixes cost nothing.
fn prompt_cache_key_from_parts(template_slug: &str, parts: &[&str]) -> String {
    let mut state = 0xcbf2_9ce4_8422_2325_u64;
    for part in std::iter::once(template_slug).chain(parts.iter().copied()) {
        for byte in part.bytes() {
            state ^= u64::from(byte);
            state = state.wrapping_mul(0x0000_0100_0000_01B3);
        }
    }
    format!("cardinal:{template_slug}:{state:016x}")
}

// Re-export main entry points
pub use comparison::{
    compare_pair, ComparisonError, PairwiseComparisonAttribute, PairwiseComparisonEntity,
    PairwiseComparisonRequest, PairwiseComparisonSpec,
};
pub use consortium::{consortium_verdict, ConsortiumError, ConsortiumJudge, ConsortiumReport};
pub use elaborate::{elaborate_criterion, ElaborateError, ElaboratedCriterion};
pub use explain::{
    differentiation_profile, explain_ranking, propose_candidates, propose_distinguishing,
    propose_for_goal, propose_rewordings, AttributeDifferentiation, AttributeExplanation,
    DifferentiationProfile, ExplainError, ExplainOptions, Explanation, ProposalUsage,
};
pub use hooks::{
    ComparisonEvent, ComparisonObserver, ObserverError, WarmStartData, WarmStartError,
    WarmStartProvider,
};
pub use model_policy::ModelLadderPolicy;
pub use multi::{
    default_template_slug, estimate_max_rerank_charge, multi_rerank, validate_multi_rerank_request,
    MultiRerankError, RerankChargeEstimate, RerankExecution, DEFAULT_MODEL,
};
pub use options::RerankRunOptions;
pub use orbit::{orbit_transform, OrbitReport, CHARACTERS};
pub use policy_registry::{load_policy_from_path, PolicyConfig, PolicyRegistry, PolicySpec};
pub use report::{build_report, render_report_markdown, RerankReport, RerankReportOptions};
pub use sampling::{nonce_draws, NonceDrawReport};
pub use setwise::{
    sort_documents_setwise, sort_texts_setwise, OrderSensitivity, SetwiseDesign, SetwiseOptions,
    SetwiseSortError, SetwiseSorted,
};
pub use simple::rerank;
pub use sort::{
    sort_documents, sort_texts, SortError, SortOptions, SortProbe, SortProbeKind, SortedItem,
    SortedTexts,
};
pub use spin::{
    spin_probe, spin_sweep, SpinFraming, SpinProbeReport, SpinReading, SpinSweepReport,
    SweepReading,
};
pub use trace::{ComparisonTrace, JsonlTraceSink, TraceError, TraceSink, TraceWorker};
pub use types::*;
pub use wordings::{wording_invariance, WordingInvarianceReport, WordingReading, WORDING_SLUGS};
