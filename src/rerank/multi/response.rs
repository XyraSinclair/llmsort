use std::collections::{HashMap, HashSet};
use std::time::Instant;

use crate::rating_engine::EngineSpec;
use crate::trait_search::{TopKConfig, TraitSearchManager};

use super::super::types::{
    AttributeScoreSummary, MultiRerankEntityResult, MultiRerankMeta, MultiRerankRequest,
    MultiRerankResponse, RerankStopReason,
};
use super::request::{finite_or_zero, MultiRerankError};

pub(super) struct ResponseContext<'a> {
    pub(super) topk_cfg: &'a TopKConfig,
    pub(super) comparisons_attempted: usize,
    pub(super) comparisons_failed: usize,
    pub(super) first_error: Option<String>,
    pub(super) comparisons_used: usize,
    pub(super) comparisons_refused: usize,
    pub(super) comparisons_cached: usize,
    pub(super) comparison_budget: usize,
    pub(super) start_time: Instant,
    pub(super) base_model: &'a str,
    pub(super) rater_id: &'a str,
    pub(super) engine_spec: EngineSpec,
    pub(super) warm_start_observations: usize,
    pub(super) provider_input_tokens: u32,
    pub(super) provider_output_tokens: u32,
    pub(super) provider_cost_nanodollars: i64,
    pub(super) provider_cost_is_estimate: bool,
    pub(super) models_used: HashSet<String>,
    pub(super) pairs_counterbalanced: usize,
    pub(super) position_flips: usize,
    pub(super) evidence_judgements: usize,
    pub(super) logprob_mode_judgements: usize,
    pub(super) visible_mass_sum: f64,
    pub(super) evidence_order_residual_sum_abs: f64,
    pub(super) evidence_order_residual_pairs: usize,
    pub(super) evidence_sigma_w: Option<f64>,
    pub(super) evidence_obs_sigma_rms: Option<f64>,
    pub(super) stop_reason: RerankStopReason,
}

pub(crate) struct BuiltResponse {
    pub(crate) response: MultiRerankResponse,
    pub(crate) comparisons_failed: usize,
    pub(crate) first_error: Option<String>,
}

pub(super) fn build_response(
    req: &MultiRerankRequest,
    manager: &mut TraitSearchManager,
    context: ResponseContext<'_>,
) -> Result<BuiltResponse, MultiRerankError> {
    let ResponseContext {
        topk_cfg,
        comparisons_attempted,
        comparisons_failed,
        first_error,
        comparisons_used,
        comparisons_refused,
        comparisons_cached,
        comparison_budget,
        start_time,
        base_model,
        rater_id,
        engine_spec,
        warm_start_observations,
        provider_input_tokens,
        provider_output_tokens,
        provider_cost_nanodollars,
        provider_cost_is_estimate,
        models_used,
        pairs_counterbalanced,
        position_flips,
        evidence_judgements,
        logprob_mode_judgements,
        visible_mass_sum,
        evidence_order_residual_sum_abs,
        evidence_order_residual_pairs,
        evidence_sigma_w,
        evidence_obs_sigma_rms,
        stop_reason,
    } = context;
    // Final recompute and response assembly
    manager.recompute_global_state()?;
    manager.ensure_all_attribute_units()?;
    let global_topk_error = manager.estimate_topk_error();
    let latency_ms = start_time.elapsed().as_millis();
    // Per-attribute scores and derived units
    let n = req.entities.len();
    let mut attr_scores: HashMap<String, Vec<f64>> = HashMap::new();
    let mut attr_stds: HashMap<String, Vec<f64>> = HashMap::new();
    let mut attr_z: HashMap<String, Vec<f64>> = HashMap::new();
    let mut attr_min_norm: HashMap<String, Vec<f64>> = HashMap::new();
    let mut attr_pct: HashMap<String, Vec<f64>> = HashMap::new();

    for attr in &req.attributes {
        let id = &attr.id;
        if let Some(scores) = manager.attribute_scores(id) {
            let scores_vec = scores.to_vec();
            let stds = manager.attribute_std(id).unwrap_or_else(|| vec![0.0; n]);
            let z = manager
                .attribute_z_scores(id)
                .map(|v| v.to_vec())
                .unwrap_or_else(|| vec![0.0; n]);
            let min_norm = manager
                .attribute_min_norm(id)
                .map(|v| v.to_vec())
                .unwrap_or_else(|| vec![0.0; n]);
            let pct = manager
                .attribute_percentiles(id)
                .map(|v| v.to_vec())
                .unwrap_or_else(|| vec![0.0; n]);

            attr_scores.insert(id.clone(), scores_vec);
            attr_stds.insert(id.clone(), stds);
            attr_z.insert(id.clone(), z);
            attr_min_norm.insert(id.clone(), min_norm);
            attr_pct.insert(id.clone(), pct);
        }
    }

    // Build entity results
    let sorted_indices = manager.ranked_indices();
    let mut seen = vec![false; n];
    let mut entities_out: Vec<MultiRerankEntityResult> = Vec::with_capacity(n);

    // Feasible entities in rank order
    for idx in sorted_indices.iter().copied() {
        let state = manager.entity_state(idx);
        let feasible = state.feasible;
        let rank = state.rank;

        let u_mean = if feasible && state.u_mean.is_finite() {
            state.u_mean
        } else {
            0.0
        };
        let u_std = if feasible && state.u_var.is_finite() && state.u_var >= 0.0 {
            state.u_var.sqrt()
        } else {
            0.0
        };
        let p_flip = if state.p_flip.is_finite() {
            state.p_flip.clamp(0.0, 1.0)
        } else {
            0.0
        };

        let mut attr_map = HashMap::with_capacity(req.attributes.len());
        for attr in &req.attributes {
            let id = &attr.id;
            if let (Some(scores), Some(stds), Some(zs), Some(mns), Some(pcts)) = (
                attr_scores.get(id),
                attr_stds.get(id),
                attr_z.get(id),
                attr_min_norm.get(id),
                attr_pct.get(id),
            ) {
                if idx < scores.len() {
                    attr_map.insert(
                        id.clone(),
                        AttributeScoreSummary {
                            latent_mean: finite_or_zero(scores[idx]),
                            latent_std: finite_or_zero(stds[idx]),
                            z_score: finite_or_zero(zs[idx]),
                            min_normalized: finite_or_zero(mns[idx]),
                            percentile: finite_or_zero(pcts[idx]).clamp(0.0, 1.0),
                        },
                    );
                }
            }
        }

        entities_out.push(MultiRerankEntityResult {
            id: req.entities[idx].id.clone(),
            rank,
            feasible,
            u_mean,
            u_std,
            p_flip,
            attribute_scores: attr_map,
        });

        seen[idx] = true;
    }

    // Add remaining (infeasible) entities
    for idx in 0..n {
        if seen[idx] {
            continue;
        }

        let state = manager.entity_state(idx);

        let mut attr_map = HashMap::with_capacity(req.attributes.len());
        for attr in &req.attributes {
            let id = &attr.id;
            if let (Some(scores), Some(stds), Some(zs), Some(mns), Some(pcts)) = (
                attr_scores.get(id),
                attr_stds.get(id),
                attr_z.get(id),
                attr_min_norm.get(id),
                attr_pct.get(id),
            ) {
                if idx < scores.len() {
                    attr_map.insert(
                        id.clone(),
                        AttributeScoreSummary {
                            latent_mean: finite_or_zero(scores[idx]),
                            latent_std: finite_or_zero(stds[idx]),
                            z_score: finite_or_zero(zs[idx]),
                            min_normalized: finite_or_zero(mns[idx]),
                            percentile: finite_or_zero(pcts[idx]).clamp(0.0, 1.0),
                        },
                    );
                }
            }
        }

        entities_out.push(MultiRerankEntityResult {
            id: req.entities[idx].id.clone(),
            rank: state.rank,
            feasible: state.feasible,
            u_mean: if state.feasible && state.u_mean.is_finite() {
                state.u_mean
            } else {
                0.0
            },
            u_std: if state.feasible && state.u_var.is_finite() && state.u_var >= 0.0 {
                state.u_var.sqrt()
            } else {
                0.0
            },
            p_flip: if state.p_flip.is_finite() {
                state.p_flip.clamp(0.0, 1.0)
            } else {
                0.0
            },
            attribute_scores: attr_map,
        });
    }

    let meta = MultiRerankMeta {
        global_topk_error,
        tolerated_error: topk_cfg.tolerated_error,
        k: topk_cfg.k,
        band_size: topk_cfg.band_size,
        comparisons_attempted,
        comparisons_used,
        comparisons_refused,
        comparisons_cached,
        comparison_budget,
        latency_ms,
        model_used: if models_used.len() <= 1 {
            models_used
                .iter()
                .next()
                .cloned()
                .unwrap_or_else(|| base_model.to_string())
        } else {
            let mut models: Vec<String> = models_used.into_iter().collect();
            models.sort();
            format!("mixed: {}", models.join(", "))
        },
        rater_id_used: rater_id.to_string(),
        engine_spec: Some(engine_spec),
        warm_start_observations,
        provider_input_tokens,
        provider_output_tokens,
        provider_cost_nanodollars,
        provider_cost_is_estimate,
        entities_pruned: manager.explore_pruned_count(),
        pairs_counterbalanced,
        position_flips,
        evidence_judgements,
        logprob_mode_judgements,
        evidence_visible_mass_mean: if evidence_judgements > 0 {
            Some(visible_mass_sum / evidence_judgements as f64)
        } else {
            None
        },
        evidence_order_residual_mean_abs: if evidence_order_residual_pairs > 0 {
            Some(evidence_order_residual_sum_abs / evidence_order_residual_pairs as f64)
        } else {
            None
        },
        evidence_sigma_w,
        evidence_obs_sigma_rms,
        judgement_frustration_mean: {
            let values: Vec<f64> = req
                .attributes
                .iter()
                .filter_map(|attribute| manager.attribute_frustration(&attribute.id))
                .collect();
            if values.is_empty() {
                None
            } else {
                Some(values.iter().sum::<f64>() / values.len() as f64)
            }
        },
        stop_reason,
    };

    // Multi-attribute summary: Pareto front (non-dominated on per-attribute
    // posterior means, weight-sign oriented) and the attribute correlation
    // matrix (do the attributes measure different things?).
    let (pareto_front, attribute_correlations) = multi_objective_summary(req, &entities_out);

    Ok(BuiltResponse {
        response: MultiRerankResponse {
            entities: entities_out,
            meta,
            pareto_front,
            attribute_correlations,
        },
        comparisons_failed,
        first_error,
    })
}

/// Compute the Pareto front and attribute-correlation matrix over the
/// finished entity results. Orientation: each attribute's latent means are
/// multiplied by the sign of its weight, so "higher is better" holds
/// uniformly (a negative-weight attribute like "lack of X" counts inverted).
fn multi_objective_summary(
    req: &MultiRerankRequest,
    entities: &[MultiRerankEntityResult],
) -> (Vec<usize>, Vec<Vec<f64>>) {
    let m = req.attributes.len();
    if m < 2 {
        return (Vec::new(), Vec::new());
    }
    // Oriented score matrix: rows = entities, cols = attributes.
    let oriented: Vec<Option<Vec<f64>>> = entities
        .iter()
        .map(|entity| {
            if !entity.feasible {
                return None;
            }
            let mut row = Vec::with_capacity(m);
            for attribute in &req.attributes {
                let sign = if attribute.weight < 0.0 { -1.0 } else { 1.0 };
                match entity.attribute_scores.get(&attribute.id) {
                    Some(scores) => row.push(sign * scores.latent_mean),
                    None => return None,
                }
            }
            Some(row)
        })
        .collect();

    // Pareto: entity i is dominated if some feasible j is >= on every
    // oriented attribute and > on at least one.
    let mut front = Vec::new();
    for (i, row_i) in oriented.iter().enumerate() {
        let Some(row_i) = row_i else { continue };
        let dominated = oriented.iter().enumerate().any(|(j, row_j)| {
            if i == j {
                return false;
            }
            let Some(row_j) = row_j else { return false };
            let mut strictly_better_somewhere = false;
            for (a, b) in row_j.iter().zip(row_i.iter()) {
                if a < b {
                    return false;
                }
                if a > b {
                    strictly_better_somewhere = true;
                }
            }
            strictly_better_somewhere
        });
        if !dominated {
            front.push(i);
        }
    }

    // Correlation matrix over feasible entities' oriented columns.
    let rows: Vec<&Vec<f64>> = oriented.iter().flatten().collect();
    let mut correlations = vec![vec![0.0; m]; m];
    if rows.len() >= 3 {
        let n = rows.len() as f64;
        let means: Vec<f64> = (0..m)
            .map(|a| rows.iter().map(|r| r[a]).sum::<f64>() / n)
            .collect();
        for a in 0..m {
            for b in 0..m {
                let mut cov = 0.0;
                let mut var_a = 0.0;
                let mut var_b = 0.0;
                for row in &rows {
                    let da = row[a] - means[a];
                    let db = row[b] - means[b];
                    cov += da * db;
                    var_a += da * da;
                    var_b += db * db;
                }
                correlations[a][b] = if var_a > 1e-12 && var_b > 1e-12 {
                    cov / (var_a.sqrt() * var_b.sqrt())
                } else if a == b {
                    1.0
                } else {
                    0.0
                };
            }
        }
    } else {
        correlations = Vec::new();
    }

    (front, correlations)
}
