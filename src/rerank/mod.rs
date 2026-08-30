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
pub mod hooks;
pub mod model_policy;
pub mod multi;
pub mod options;
pub mod orbit;
pub mod policy_registry;
#[doc(hidden)]
pub mod proposal_json;
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
    apply_rerank_markup, default_template_slug, estimate_max_rerank_charge, multi_rerank,
    validate_multi_rerank_request, MultiRerankError, RerankChargeEstimate, RerankExecution,
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
