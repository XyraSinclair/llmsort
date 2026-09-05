//! Analytic Network Process: the AHP hierarchy generalized to a network
//! with feedback, on our measured-pairwise primitives.
//!
//! Structure. Nodes are clusters: one goal, C criteria, A alternatives.
//! A column-stochastic supermatrix S distributes each node's influence:
//!
//! - goal column → criteria, by pairwise importance-for-goal (the AHP
//!   weigh: softmaxed log-latents of criteria-as-entities);
//! - criterion c's column → split α : (1−α) between the OTHER criteria
//!   (inner dependence: pairwise "contribution to strengthening c in
//!   pursuit of the goal") and the alternatives (per-criterion measured
//!   scores, softmaxed);
//! - alternative a's column → back to the criteria, proportional to
//!   softmax over criteria of a's per-criterion z-scores (which criterion
//!   does a stand out on — z is gauge-free within each criterion, so it is
//!   the comparable quantity across criteria). This closes the feedback
//!   loop: strong alternatives reinforce the criteria they exemplify.
//!
//! Limit. The limiting priorities are the Cesàro limit
//! lim_K (1/K) Σ_{k≤K} S^k e_goal — the average visit distribution of the
//! influence walk started at the goal. Cesàro (not plain powers) because
//! the criteria↔alternatives subchain is bipartite when α = 0 and plain
//! powers then oscillate with period 2; the average converges regardless.
//!
//! The headline diagnostic is mathematical: limiting criteria weights vs the
//! direct AHP weights — the network correction, per criterion, in
//! probability mass. Every edge of the supermatrix is a solved pairwise
//! measurement, never an assertion.

use std::sync::Arc;

use serde::Serialize;

use llmsort::cache::PairwiseCache;
use llmsort::gateway::{Attribution, ChatGateway};
use llmsort::rerank::multi::{multi_rerank, MultiRerankError, RerankExecution};
use llmsort::rerank::options::RerankRunOptions;
use llmsort::rerank::sort::{sort_documents, SortError, SortOptions};
use llmsort::rerank::types::{
    MultiRerankAttributeSpec, MultiRerankEntity, MultiRerankRequest, MultiRerankTopKSpec,
    RerankDocument,
};

/// Options for [`anp`].
#[derive(Debug, Clone)]
pub struct AnpOptions {
    /// Model slug (OpenRouter).
    pub model: Option<String>,
    /// Share of a criterion's influence flowing to other criteria (inner
    /// dependence) vs alternatives. 0 = pure hierarchy below the goal.
    pub alpha: f64,
    /// Comparison budget for the alternatives multi-rerank.
    pub comparison_budget: Option<usize>,
    /// RNG seed.
    pub seed: u64,
}

impl Default for AnpOptions {
    fn default() -> Self {
        Self {
            model: None,
            alpha: 0.4,
            comparison_budget: None,
            seed: 7,
        }
    }
}

/// One criterion's weights: direct (AHP) vs limiting (ANP).
#[derive(Debug, Clone, Serialize)]
pub struct AnpCriterion {
    pub name: String,
    /// Softmaxed importance-for-goal weight — the hierarchy answer.
    pub direct_weight: f64,
    /// Limiting weight under network feedback — the network answer.
    pub limiting_weight: f64,
    /// limiting − direct: the network correction, in probability mass.
    pub network_delta: f64,
}

/// One alternative's limiting priority.
#[derive(Debug, Clone, Serialize)]
pub struct AnpAlternative {
    pub id: String,
    pub limiting_priority: f64,
    /// Per-criterion z-scores (the feedback drivers), criteria order.
    pub z_scores: Vec<f64>,
}

/// Result of [`anp`].
#[derive(Debug, Serialize)]
pub struct AnpReport {
    pub goal: String,
    pub criteria: Vec<AnpCriterion>,
    /// Alternatives sorted by limiting priority, descending.
    pub alternatives: Vec<AnpAlternative>,
    /// The full weighted supermatrix (goal, criteria..., alternatives...),
    /// column-stochastic where a column has outflow. Row/col order:
    /// [goal, criteria in given order, alternatives in given order].
    pub supermatrix: Vec<Vec<f64>>,
    /// Cesàro iterations to convergence.
    pub iterations: usize,
    pub converged: bool,
    pub comparisons_used: usize,
    pub cost_nanodollars: i64,
}

/// Errors from [`anp`].
#[derive(Debug, thiserror::Error)]
pub enum AnpError {
    #[error("need at least 2 criteria, got {0}")]
    TooFewCriteria(usize),
    #[error("need at least 2 alternatives, got {0}")]
    TooFewAlternatives(usize),
    #[error("alpha must be in [0,1): {0}")]
    BadAlpha(f64),
    #[error(transparent)]
    Sort(#[from] SortError),
    #[error(transparent)]
    Rerank(#[from] MultiRerankError),
}

fn softmax(latents: &[f64]) -> Vec<f64> {
    let max = latents.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let exps: Vec<f64> = latents.iter().map(|l| (l - max).exp()).collect();
    let z: f64 = exps.iter().sum();
    exps.iter().map(|e| e / z).collect()
}

/// Cesàro limit of S^k e_source: the average visit distribution of the
/// influence walk, robust to periodic subchains where plain powers
/// oscillate.
///
/// Numerics: raw running-average deltas decay only as O(1/k), so a
/// per-step tolerance is unreachable; instead we burn in `burn`
/// iterations (transients decay geometrically), then compare the means of
/// two consecutive windows of length `window`. For geometrically mixing
/// chains this is exact to floating point; for a residual oscillation of
/// period p the window mean carries an O(p/window) error — with
/// window = 2¹⁶ and p bounded by the node count, that is far below the
/// three decimals we report. Returns (mean, iterations, converged).
fn cesaro_limit(s: &[Vec<f64>], source: usize, tol: f64) -> (Vec<f64>, usize, bool) {
    let n = s.len();
    let burn = 4096usize;
    let window = 65_536usize;
    let mut x = vec![0.0; n];
    x[source] = 1.0;
    let step = |x: &[f64]| -> Vec<f64> {
        let mut next = vec![0.0; n];
        for (i, row) in s.iter().enumerate() {
            let mut acc = 0.0;
            for (j, &sij) in row.iter().enumerate() {
                acc += sij * x[j];
            }
            next[i] = acc;
        }
        next
    };
    for _ in 0..burn {
        x = step(&x);
    }
    let window_mean = |x: &mut Vec<f64>| -> Vec<f64> {
        let mut sum = vec![0.0; n];
        for _ in 0..window {
            *x = step(x);
            for i in 0..n {
                sum[i] += x[i];
            }
        }
        sum.iter().map(|v| v / window as f64).collect()
    };
    let m1 = window_mean(&mut x);
    let m2 = window_mean(&mut x);
    let delta: f64 = m1.iter().zip(m2.iter()).map(|(a, b)| (a - b).abs()).sum();
    (m2, burn + 2 * window, delta < tol)
}

fn execution<'a>(
    gateway: Arc<dyn ChatGateway>,
    cache: Option<&'a dyn PairwiseCache>,
    seed: u64,
    tag: &'static str,
) -> RerankExecution<'a> {
    let mut execution =
        RerankExecution::new(gateway, Attribution::new(tag)).run_options(RerankRunOptions {
            rng_seed: Some(seed),
            cache_only: false,
        });
    if let Some(cache) = cache {
        execution = execution.cache(cache);
    }
    execution
}

/// Run the Analytic Network Process: goal, criteria, alternatives, with
/// measured inner dependence and alternative→criteria feedback.
pub async fn anp(
    gateway: Arc<dyn ChatGateway>,
    cache: Option<&dyn PairwiseCache>,
    goal: &str,
    criteria: &[(String, String)],
    alternatives: Vec<RerankDocument>,
    opts: AnpOptions,
) -> Result<AnpReport, AnpError> {
    let c = criteria.len();
    let a = alternatives.len();
    if c < 2 {
        return Err(AnpError::TooFewCriteria(c));
    }
    if a < 2 {
        return Err(AnpError::TooFewAlternatives(a));
    }
    if !(0.0..1.0).contains(&opts.alpha) {
        return Err(AnpError::BadAlpha(opts.alpha));
    }
    let mut comparisons = 0usize;
    let mut cost = 0i64;

    let criterion_docs = |exclude: Option<usize>| -> Vec<RerankDocument> {
        criteria
            .iter()
            .enumerate()
            .filter(|(idx, _)| Some(*idx) != exclude)
            .map(|(_, (name, text))| RerankDocument {
                id: name.clone(),
                text: if name == text {
                    name.clone()
                } else {
                    format!("{name}: {text}")
                },
            })
            .collect()
    };
    let sort_opts = || SortOptions {
        model: opts.model.clone(),
        comparison_budget: None,
        ..Default::default()
    };

    // ---- Goal column: direct importance weights (AHP weigh) ----
    let importance = format!(
        "importance for achieving this goal: {goal}. Judge how much more one \
         consideration matters than the other for that goal specifically."
    );
    let sorted = sort_documents(
        criterion_docs(None),
        &importance,
        execution(gateway.clone(), cache, opts.seed, "cardinal::anp::weigh"),
        sort_opts(),
    )
    .await?;
    comparisons += sorted.meta.comparisons_used;
    cost += sorted.meta.provider_cost_nanodollars;
    // Map back to the criteria order.
    let mut direct_latents = vec![0.0f64; c];
    for item in &sorted.items {
        if let Some(idx) = criteria.iter().position(|(name, _)| name == &item.id) {
            direct_latents[idx] = item.latent_mean;
        }
    }
    let direct_weights = softmax(&direct_latents);

    // ---- Inner dependence: for each target criterion, weigh the others'
    //      contribution to it ----
    let mut w_cc = vec![vec![0.0f64; c]; c]; // w_cc[row][col]: col criterion -> row criterion
    for (target, (target_name, target_text)) in criteria.iter().enumerate() {
        let others = criterion_docs(Some(target));
        let contribution = format!(
            "contribution to strengthening «{target_name}» ({target_text}) in \
             pursuit of the goal: {goal}. Judge how much more one consideration \
             feeds into that specific consideration."
        );
        let column: Vec<(usize, f64)> = if others.len() == 1 {
            let only = criteria
                .iter()
                .position(|(name, _)| name == &others[0].id)
                .expect("criterion present");
            vec![(only, 1.0)]
        } else {
            let sorted = sort_documents(
                others,
                &contribution,
                execution(gateway.clone(), cache, opts.seed, "cardinal::anp::inner"),
                sort_opts(),
            )
            .await?;
            comparisons += sorted.meta.comparisons_used;
            cost += sorted.meta.provider_cost_nanodollars;
            let latents: Vec<(usize, f64)> = sorted
                .items
                .iter()
                .filter_map(|item| {
                    criteria
                        .iter()
                        .position(|(name, _)| name == &item.id)
                        .map(|idx| (idx, item.latent_mean))
                })
                .collect();
            let weights = softmax(&latents.iter().map(|(_, l)| *l).collect::<Vec<f64>>());
            latents
                .iter()
                .zip(weights.iter())
                .map(|((idx, _), w)| (*idx, *w))
                .collect()
        };
        for (idx, w) in column {
            w_cc[idx][target] = w;
        }
    }

    // ---- Alternatives under each criterion: one multi-rerank ----
    let request = MultiRerankRequest {
        entities: alternatives
            .iter()
            .map(|d| MultiRerankEntity {
                id: d.id.clone(),
                text: d.text.clone(),
            })
            .collect(),
        attributes: criteria
            .iter()
            .map(|(name, text)| MultiRerankAttributeSpec {
                id: name.clone(),
                prompt: if name == text {
                    format!("{name} (in pursuit of the goal: {goal})")
                } else {
                    format!("{text} (in pursuit of the goal: {goal})")
                },
                prompt_template_slug: None,
                weight: 1.0,
            })
            .collect(),
        topk: MultiRerankTopKSpec {
            k: a.div_ceil(2),
            weight_exponent: 1.0,
            tolerated_error: 0.1,
            band_size: 5,
            effective_resistance_max_active: 64,
            stop_sigma_inflate: 1.25,
            stop_min_consecutive: 2,
            min_explore_degree: 2,
            prune_p_topk_below: None,
        },
        gates: Vec::new(),
        nonce_draws: None,
        comparison_budget: opts.comparison_budget,
        latency_budget_ms: None,
        max_cost_nanodollars: None,
        model: opts.model.clone(),
        rater_id: None,
        comparison_concurrency: None,
        max_pair_repeats: None,
        randomize_presentation_order: true,
        counterbalance_pairs: true,
    };
    let response = multi_rerank(
        request,
        execution(gateway.clone(), cache, opts.seed, "cardinal::anp::alts"),
    )
    .await?;
    comparisons += response.meta.comparisons_used;
    cost += response.meta.provider_cost_nanodollars;

    // Per-criterion alternative latents and z-scores, aligned to input order.
    let mut alt_latents = vec![vec![0.0f64; a]; c]; // [criterion][alternative]
    let mut alt_z = vec![vec![0.0f64; a]; c];
    for entity in &response.entities {
        let Some(ai) = alternatives.iter().position(|d| d.id == entity.id) else {
            continue;
        };
        for (ci, (name, _)) in criteria.iter().enumerate() {
            if let Some(score) = entity.attribute_scores.get(name) {
                alt_latents[ci][ai] = score.latent_mean;
                alt_z[ci][ai] = score.z_score;
            }
        }
    }
    // W_ac[:,c] = softmax over alternatives of criterion c's latents.
    let w_ac: Vec<Vec<f64>> = (0..c).map(|ci| softmax(&alt_latents[ci])).collect();
    // Feedback W_ca[:,a] = softmax over criteria of alternative a's z-scores.
    let w_ca: Vec<Vec<f64>> = (0..a)
        .map(|ai| softmax(&(0..c).map(|ci| alt_z[ci][ai]).collect::<Vec<f64>>()))
        .collect();

    // ---- Assemble the supermatrix: order [goal, criteria..., alternatives...] ----
    let n = 1 + c + a;
    let mut s = vec![vec![0.0f64; n]; n];
    for ci in 0..c {
        s[1 + ci][0] = direct_weights[ci];
    }
    for col in 0..c {
        let inner_total: f64 = (0..c).map(|row| w_cc[row][col]).sum();
        let alpha = if inner_total > 0.0 { opts.alpha } else { 0.0 };
        for row in 0..c {
            s[1 + row][1 + col] = alpha * w_cc[row][col];
        }
        for row in 0..a {
            s[1 + c + row][1 + col] = (1.0 - alpha) * w_ac[col][row];
        }
    }
    for col in 0..a {
        for row in 0..c {
            s[1 + row][1 + c + col] = w_ca[col][row];
        }
    }

    // ---- Limit ----
    let (limit, iterations, converged) = cesaro_limit(&s, 0, 1e-6);
    let crit_mass: f64 = (0..c).map(|ci| limit[1 + ci]).sum();
    let alt_mass: f64 = (0..a).map(|ai| limit[1 + c + ai]).sum();
    let limiting_criteria: Vec<f64> = (0..c)
        .map(|ci| {
            if crit_mass > 0.0 {
                limit[1 + ci] / crit_mass
            } else {
                0.0
            }
        })
        .collect();
    let limiting_alternatives: Vec<f64> = (0..a)
        .map(|ai| {
            if alt_mass > 0.0 {
                limit[1 + c + ai] / alt_mass
            } else {
                0.0
            }
        })
        .collect();

    let criteria_report: Vec<AnpCriterion> = criteria
        .iter()
        .enumerate()
        .map(|(ci, (name, _))| AnpCriterion {
            name: name.clone(),
            direct_weight: direct_weights[ci],
            limiting_weight: limiting_criteria[ci],
            network_delta: limiting_criteria[ci] - direct_weights[ci],
        })
        .collect();
    let mut alternatives_report: Vec<AnpAlternative> = alternatives
        .iter()
        .enumerate()
        .map(|(ai, d)| AnpAlternative {
            id: d.id.clone(),
            limiting_priority: limiting_alternatives[ai],
            z_scores: (0..c).map(|ci| alt_z[ci][ai]).collect(),
        })
        .collect();
    alternatives_report.sort_by(|x, y| {
        y.limiting_priority
            .partial_cmp(&x.limiting_priority)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(AnpReport {
        goal: goal.to_string(),
        criteria: criteria_report,
        alternatives: alternatives_report,
        supermatrix: s,
        iterations,
        converged,
        comparisons_used: comparisons,
        cost_nanodollars: cost,
    })
}

#[cfg(test)]
mod tests {
    use super::cesaro_limit;

    #[test]
    fn cesaro_limit_matches_known_stationary_distribution() {
        // Two-state recurrent chain fed from a source: columns are
        // outflow distributions. Source 0 -> state 1; then the classic
        // chain P(1->2)=1, P(2->1)=0.5, P(2->2)=0.5 whose stationary
        // distribution is (1/3, 2/3).
        let s = vec![
            vec![0.0, 0.0, 0.0],
            vec![1.0, 0.0, 0.5],
            vec![0.0, 1.0, 0.5],
        ];
        let (limit, _, converged) = cesaro_limit(&s, 0, 1e-6);
        assert!(converged);
        let mass = limit[1] + limit[2];
        assert!((limit[1] / mass - 1.0 / 3.0).abs() < 1e-4, "{limit:?}");
        assert!((limit[2] / mass - 2.0 / 3.0).abs() < 1e-4, "{limit:?}");
    }

    #[test]
    fn cesaro_limit_converges_on_a_periodic_chain() {
        // Pure 2-cycle (bipartite): plain powers oscillate forever; the
        // Cesàro average must converge to (1/2, 1/2).
        let s = vec![
            vec![0.0, 0.0, 0.0],
            vec![1.0, 0.0, 1.0],
            vec![0.0, 1.0, 0.0],
        ];
        let (limit, _, converged) = cesaro_limit(&s, 0, 1e-4);
        assert!(converged, "windowed average must tame periodicity");
        let mass = limit[1] + limit[2];
        assert!((limit[1] / mass - 0.5).abs() < 1e-3, "{limit:?}");
    }
}
