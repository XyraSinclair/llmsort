//! Report generation for rerank runs.

use std::collections::{HashMap, HashSet};

use blake3;
use serde::Serialize;

use super::types::{
    AttributeScoreSummary, MultiRerankAttributeSpec, MultiRerankEntityResult, MultiRerankGateSpec,
    MultiRerankMeta, MultiRerankRequest, MultiRerankResponse, RerankStopReason,
};

#[derive(Debug, Clone, Serialize)]
pub struct RerankReportOptions {
    pub top_n: usize,
    pub include_infeasible: bool,
    pub include_attribute_scores: bool,
    pub rng_seed: Option<u64>,
    pub model_policy: Option<String>,
    pub cache_only: bool,
}

impl Default for RerankReportOptions {
    fn default() -> Self {
        Self {
            top_n: 10,
            include_infeasible: false,
            include_attribute_scores: true,
            rng_seed: None,
            model_policy: None,
            cache_only: false,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RerankReport {
    pub request_hash: String,
    pub summary: ReportSummary,
    pub attributes: Vec<ReportAttribute>,
    pub gates: Vec<MultiRerankGateSpec>,
    pub top_entities: Vec<ReportEntity>,
    pub run_stamp: ReportStamp,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReportSummary {
    pub stop_reason: RerankStopReason,
    pub k: usize,
    pub global_topk_error: f64,
    pub tolerated_error: f64,
    pub comparisons_attempted: usize,
    pub comparisons_used: usize,
    pub comparisons_refused: usize,
    pub comparisons_cached: usize,
    pub comparison_budget: usize,
    pub latency_ms: u128,
    pub model_used: String,
    pub rater_id_used: String,
    pub provider_input_tokens: u32,
    pub provider_output_tokens: u32,
    pub provider_cost_nanodollars: i64,
    pub provider_cost_is_estimate: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReportAttribute {
    pub id: String,
    pub prompt: String,
    pub weight: f64,
    pub prompt_template_slug: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReportEntity {
    pub id: String,
    pub rank: Option<usize>,
    pub feasible: bool,
    pub u_mean: f64,
    pub u_std: f64,
    pub p_flip: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attribute_scores: Option<HashMap<String, AttributeScoreSummary>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReportStamp {
    pub rng_seed: Option<u64>,
    pub model_policy: Option<String>,
    pub cache_only: bool,
}

pub fn validate_report_inputs(
    req: &MultiRerankRequest,
    resp: &MultiRerankResponse,
) -> Result<(), String> {
    let request_entity_ids: HashSet<&str> = req
        .entities
        .iter()
        .map(|entity| entity.id.as_str())
        .collect();
    let request_attribute_ids: HashSet<&str> = req
        .attributes
        .iter()
        .map(|attribute| attribute.id.as_str())
        .collect();
    let mut response_entity_ids = HashSet::new();

    for entity in &resp.entities {
        if !request_entity_ids.contains(entity.id.as_str()) {
            return Err(format!(
                "response entity '{}' is not present in the request; refusing to generate a potentially stale report",
                entity.id
            ));
        }
        if !response_entity_ids.insert(entity.id.as_str()) {
            return Err(format!(
                "response contains duplicate entity result '{}'",
                entity.id
            ));
        }

        for attribute_id in entity.attribute_scores.keys() {
            if !request_attribute_ids.contains(attribute_id.as_str()) {
                return Err(format!(
                    "response entity '{}' contains score for unknown attribute '{}'",
                    entity.id, attribute_id
                ));
            }
        }

        for attribute in &req.attributes {
            if !entity.attribute_scores.contains_key(&attribute.id) {
                return Err(format!(
                    "response entity '{}' is missing score for request attribute '{}'",
                    entity.id, attribute.id
                ));
            }
        }
    }

    for entity in &req.entities {
        if !response_entity_ids.contains(entity.id.as_str()) {
            return Err(format!(
                "response is missing request entity '{}'; refusing to generate a partial report",
                entity.id
            ));
        }
    }

    Ok(())
}

pub fn build_report(
    req: &MultiRerankRequest,
    resp: &MultiRerankResponse,
    opts: &RerankReportOptions,
) -> RerankReport {
    let request_hash = hash_request(req);
    let summary = ReportSummary::from_meta(&resp.meta);
    let attributes = req
        .attributes
        .iter()
        .map(ReportAttribute::from_attr)
        .collect::<Vec<_>>();

    let mut top_entities: Vec<ReportEntity> = Vec::new();
    for entity in &resp.entities {
        if !opts.include_infeasible && !entity.feasible {
            continue;
        }
        if top_entities.len() >= opts.top_n {
            break;
        }
        top_entities.push(ReportEntity::from_entity(
            entity,
            opts.include_attribute_scores,
        ));
    }

    RerankReport {
        request_hash,
        summary,
        attributes,
        gates: req.gates.clone(),
        top_entities,
        run_stamp: ReportStamp {
            rng_seed: opts.rng_seed,
            model_policy: opts.model_policy.clone(),
            cache_only: opts.cache_only,
        },
    }
}

impl ReportSummary {
    fn from_meta(meta: &MultiRerankMeta) -> Self {
        Self {
            stop_reason: meta.stop_reason,
            k: meta.k,
            global_topk_error: meta.global_topk_error,
            tolerated_error: meta.tolerated_error,
            comparisons_attempted: meta.comparisons_attempted,
            comparisons_used: meta.comparisons_used,
            comparisons_refused: meta.comparisons_refused,
            comparisons_cached: meta.comparisons_cached,
            comparison_budget: meta.comparison_budget,
            latency_ms: meta.latency_ms,
            model_used: meta.model_used.clone(),
            rater_id_used: meta.rater_id_used.clone(),
            provider_input_tokens: meta.provider_input_tokens,
            provider_output_tokens: meta.provider_output_tokens,
            provider_cost_nanodollars: meta.provider_cost_nanodollars,
            provider_cost_is_estimate: meta.provider_cost_is_estimate,
        }
    }
}

impl ReportAttribute {
    fn from_attr(attr: &MultiRerankAttributeSpec) -> Self {
        Self {
            id: attr.id.clone(),
            prompt: attr.prompt.clone(),
            weight: attr.weight,
            prompt_template_slug: attr.prompt_template_slug.clone(),
        }
    }
}

impl ReportEntity {
    fn from_entity(entity: &MultiRerankEntityResult, include_scores: bool) -> Self {
        Self {
            id: entity.id.clone(),
            rank: entity.rank,
            feasible: entity.feasible,
            u_mean: entity.u_mean,
            u_std: entity.u_std,
            p_flip: entity.p_flip,
            attribute_scores: if include_scores {
                Some(entity.attribute_scores.clone())
            } else {
                None
            },
        }
    }
}

pub fn render_report_markdown(report: &RerankReport) -> String {
    let mut out = String::new();
    out.push_str("# Rerank Report\n\n");
    out.push_str(&format!("- Request hash: `{}`\n", report.request_hash));
    out.push_str(&format!(
        "- Stop reason: `{}`\n",
        stop_reason_label(report.summary.stop_reason)
    ));
    out.push_str(&format!("- k: {}\n", report.summary.k));
    out.push_str(&format!(
        "- Global top-k error: {:.4}\n",
        report.summary.global_topk_error
    ));
    out.push_str(&format!(
        "- Tolerated error: {:.4}\n",
        report.summary.tolerated_error
    ));
    out.push_str(&format!(
        "- Comparisons used/attempted/refused/cached: {}/{}/{}/{}\n",
        report.summary.comparisons_used,
        report.summary.comparisons_attempted,
        report.summary.comparisons_refused,
        report.summary.comparisons_cached
    ));
    out.push_str(&format!(
        "- Comparison budget: {}\n",
        report.summary.comparison_budget
    ));
    out.push_str(&format!("- Model used: {}\n", report.summary.model_used));
    out.push_str(&format!("- Rater ID: {}\n", report.summary.rater_id_used));
    out.push_str(&format!("- Latency: {} ms\n", report.summary.latency_ms));
    out.push_str(&format!(
        "- Provider tokens input/output/total: {}/{}/{}\n",
        report.summary.provider_input_tokens,
        report.summary.provider_output_tokens,
        report
            .summary
            .provider_input_tokens
            .saturating_add(report.summary.provider_output_tokens)
    ));
    let cost_label = if report.summary.provider_cost_is_estimate {
        "Provider cost estimate"
    } else {
        "Provider cost"
    };
    out.push_str(&format!(
        "- {cost_label}: {}\n",
        format_nanodollars(report.summary.provider_cost_nanodollars)
    ));
    if let Some(seed) = report.run_stamp.rng_seed {
        out.push_str(&format!("- RNG seed: {seed}\n"));
    }
    if let Some(policy) = &report.run_stamp.model_policy {
        out.push_str(&format!("- Model policy: {policy}\n"));
    }
    if report.run_stamp.cache_only {
        out.push_str("- Cache-only mode: true\n");
    }

    out.push_str("\n## Warnings / Degraded State\n\n");
    let warnings = report_warnings(report);
    if warnings.is_empty() {
        out.push_str("- None.\n");
    } else {
        for warning in warnings {
            out.push_str(&format!("- {warning}\n"));
        }
    }

    out.push_str("\n## Run Status\n\n");
    out.push_str(stop_reason_interpretation(report.summary.stop_reason));
    out.push('\n');
    out.push_str("\n## Attributes\n\n");
    for attr in &report.attributes {
        out.push_str(&format!(
            "- `{}` (weight {:.3}) — {}\n",
            attr.id, attr.weight, attr.prompt
        ));
    }

    if !report.gates.is_empty() {
        out.push_str("\n## Gates\n\n");
        for gate in &report.gates {
            out.push_str(&format!(
                "- {} {} {} {}\n",
                gate.attribute_id, gate.op, gate.threshold, gate.unit
            ));
        }
    }

    out.push_str("\n## Top Entities\n\n");
    for entity in &report.top_entities {
        out.push_str(&format!(
            "- {} (rank {:?}, feasible {}, u_mean {:.3}, u_std {:.3}, p_flip {:.3})\n",
            entity.id, entity.rank, entity.feasible, entity.u_mean, entity.u_std, entity.p_flip
        ));

        if let Some(attribute_scores) = &entity.attribute_scores {
            let mut attribute_ids: Vec<&String> = attribute_scores.keys().collect();
            attribute_ids.sort();

            for attribute_id in attribute_ids {
                let score = &attribute_scores[attribute_id];
                out.push_str(&format!(
                    "  - `{}`: latent {:.3} ± {:.3}, z {:.3}, min_norm {:.3}, percentile {:.3}\n",
                    attribute_id,
                    score.latent_mean,
                    score.latent_std,
                    score.z_score,
                    score.min_normalized,
                    score.percentile
                ));
            }
        }
    }
    out
}

fn report_warnings(report: &RerankReport) -> Vec<String> {
    let summary = &report.summary;
    let mut warnings = Vec::new();

    if !is_converged_stop_reason(summary.stop_reason) {
        warnings.push(format!(
            "Run stopped with non-converged stop reason `{}`; inspect uncertainty before sharing this as a settled ordering.",
            stop_reason_label(summary.stop_reason)
        ));
    }
    if summary.global_topk_error > summary.tolerated_error {
        warnings.push(format!(
            "Global top-k error {:.4} exceeds tolerated error {:.4}.",
            summary.global_topk_error, summary.tolerated_error
        ));
    }

    if summary.comparisons_refused > 0 {
        warnings.push(format!(
            "{} comparison(s) were refused and could not contribute to the ranking.",
            summary.comparisons_refused
        ));
    }

    if summary.comparisons_cached > 0 {
        warnings.push(format!(
            "{} comparison(s) came from cache rather than a fresh provider call.",
            summary.comparisons_cached
        ));
    }

    let provider_tokens = summary
        .provider_input_tokens
        .saturating_add(summary.provider_output_tokens);
    if provider_tokens > 0 && summary.provider_cost_nanodollars <= 0 {
        warnings.push(
            "Provider token usage is non-zero but provider cost is unavailable or zero; do not read this as a free run."
                .to_string(),
        );
    }
    if summary.provider_cost_is_estimate {
        warnings.push(
            "Provider cost is an estimate because at least one model lacked an exact local pricing entry and did not report upstream cost metadata."
                .to_string(),
        );
    }

    warnings
}

fn is_converged_stop_reason(reason: RerankStopReason) -> bool {
    matches!(
        reason,
        RerankStopReason::ToleratedErrorMet | RerankStopReason::CertifiedStop
    )
}

fn format_nanodollars(nanodollars: i64) -> String {
    if nanodollars <= 0 {
        return "$0.000000000".to_string();
    }
    format!("${:.9}", nanodollars as f64 / 1_000_000_000.0)
}

fn stop_reason_label(reason: RerankStopReason) -> &'static str {
    match reason {
        RerankStopReason::ToleratedErrorMet => "tolerated_error_met",
        RerankStopReason::CertifiedStop => "certified_stop",
        RerankStopReason::BudgetExhausted => "budget_exhausted",
        RerankStopReason::LatencyBudgetExceeded => "latency_budget_exceeded",
        RerankStopReason::CostBudgetExhausted => "cost_budget_exhausted",
        RerankStopReason::Cancelled => "cancelled",
        RerankStopReason::NoProposals => "no_proposals",
        RerankStopReason::NoNewPairs => "no_new_pairs",
        RerankStopReason::ConsecutiveFailures => "consecutive_failures",
    }
}

fn stop_reason_interpretation(reason: RerankStopReason) -> &'static str {
    match reason {
        RerankStopReason::ToleratedErrorMet => {
            "The run stopped because the estimated top-k error is within the requested tolerance."
        }
        RerankStopReason::CertifiedStop => {
            "The run stopped because the certified separation check found a stable top-k boundary."
        }
        RerankStopReason::BudgetExhausted => {
            "The run used the configured comparison budget before meeting the stopping tolerance; inspect the top-k error before treating the frontier as settled."
        }
        RerankStopReason::LatencyBudgetExceeded => {
            "The run stopped because it hit the configured latency budget; inspect the top-k error before treating the frontier as settled."
        }
        RerankStopReason::CostBudgetExhausted => {
            "The run stopped because accrued provider spend reached the configured cost cap; inspect the top-k error before treating the frontier as settled."
        }
        RerankStopReason::Cancelled => "The run was cancelled before normal convergence.",
        RerankStopReason::NoProposals => {
            "The planner found no comparison that could improve the ranking under the current constraints."
        }
        RerankStopReason::NoNewPairs => {
            "The planner found candidate comparisons, but all eligible pairs were already known or blocked."
        }
        RerankStopReason::ConsecutiveFailures => {
            "The run stopped after repeated non-retryable comparison failures."
        }
    }
}

fn hash_request(req: &MultiRerankRequest) -> String {
    let bytes = serde_json::to_vec(req).unwrap_or_default();
    blake3::hash(&bytes).to_hex().to_string()
}
