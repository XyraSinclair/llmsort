//! Explain an existing ranking: which attributes reconstruct it?
//!
//! You hand this module a list in the order you *believe* is right (best
//! first) plus candidate attribute prompts. It measures each candidate with
//! the normal pairwise-ratio machinery, reports how well each one alone
//! agrees with your order, fits non-negative weights whose combination best
//! reconstructs it, and returns the full cost accounting. Use it to turn a gut
//! ranking into named, weighted, reusable criteria — or to discover that
//! none of your candidate attributes explains your own taste.

use super::multi::{multi_rerank_with_failures, MultiRerankError, RerankExecution};
use super::simple;
use super::sort::{average_ranks, spearman};
use super::types::{
    MultiRerankAttributeSpec, MultiRerankEntity, MultiRerankRequest, MultiRerankTopKSpec,
    RerankDocument, RerankMeta,
};
use crate::gateway::{Attribution, ChatGateway};
use serde::{Deserialize, Serialize};

/// Options for [`explain_ranking`].
#[derive(Debug, Clone)]
pub struct ExplainOptions {
    /// Model slug for the pairwise judgements.
    pub model: Option<String>,
    /// Total comparison budget across all candidate attributes.
    /// Default: the engine default (4 · n · n_attributes).
    pub comparison_budget: Option<usize>,
    /// Ask each planned pair in both presentation orders (default: true).
    pub counterbalance: bool,
    /// Maximum concurrent comparisons.
    pub comparison_concurrency: Option<usize>,
}

impl Default for ExplainOptions {
    fn default() -> Self {
        Self {
            model: None,
            comparison_budget: None,
            counterbalance: true,
            comparison_concurrency: None,
        }
    }
}

/// How well one candidate attribute explains the reference ranking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributeExplanation {
    /// Attribute id used in traces and cache keys (`explain_<n>`).
    pub attribute_id: String,
    /// The candidate attribute prompt.
    pub prompt: String,
    /// Spearman rank correlation between this attribute's measured latent
    /// scores and the reference order, alone. `None` when fewer than 3 items
    /// had scores.
    pub spearman_alone: Option<f64>,
    /// Non-negative weight of this attribute in the best joint
    /// reconstruction of the reference order (weights sum to 1 unless all
    /// are zero).
    pub fitted_weight: f64,
}

/// Result of [`explain_ranking`].
#[derive(Debug, Serialize)]
pub struct Explanation {
    /// Per-candidate evidence, in the order the candidates were given.
    pub attributes: Vec<AttributeExplanation>,
    /// Spearman correlation of the fitted weighted combination with the
    /// reference order — how much of your ranking these attributes jointly
    /// reconstruct.
    pub combined_spearman: Option<f64>,
    /// Run metadata (comparisons, tokens, cost, counterbalancing flips).
    pub meta: RerankMeta,
}

/// Errors from [`explain_ranking`] / [`propose_candidates`].
#[derive(Debug, thiserror::Error)]
pub enum ExplainError {
    /// Need at least 3 documents to correlate against.
    #[error("explaining a ranking requires at least 3 items, got {0}")]
    TooFewItems(usize),
    /// Need at least one candidate attribute.
    #[error("no candidate attributes given")]
    NoCandidates,
    /// The underlying rerank failed.
    #[error(transparent)]
    Rerank(#[from] MultiRerankError),
    /// A provider call failed.
    #[error("provider error: {0}")]
    Provider(#[from] crate::gateway::error::ProviderError),
    /// The proposal call returned unusable output.
    #[error("could not parse proposed candidates: {0}")]
    ProposalParse(String),
}

/// Measure candidate attributes against a reference ranking.
///
/// `documents` must be in the believed order, best first. Each candidate in
/// `candidates` becomes one attribute of a single multi-rerank (equal
/// weights, so the planner treats them as peers); the per-attribute latent
/// scores are then correlated against the reference order, and a
/// non-negative weighted combination is fitted to it.
pub async fn explain_ranking(
    documents: Vec<RerankDocument>,
    candidates: Vec<String>,
    execution: RerankExecution<'_>,
    opts: ExplainOptions,
) -> Result<Explanation, ExplainError> {
    let n = documents.len();
    if n < 3 {
        return Err(ExplainError::TooFewItems(n));
    }
    if candidates.is_empty() {
        return Err(ExplainError::NoCandidates);
    }

    let attributes: Vec<MultiRerankAttributeSpec> = candidates
        .iter()
        .enumerate()
        .map(|(idx, prompt)| MultiRerankAttributeSpec {
            id: format!("explain_{idx}"),
            prompt: prompt.clone(),
            prompt_template_slug: None,
            weight: 1.0,
        })
        .collect();

    let request = MultiRerankRequest {
        entities: documents
            .iter()
            .map(|doc| MultiRerankEntity {
                id: doc.id.clone(),
                text: doc.text.clone(),
            })
            .collect(),
        attributes,
        topk: MultiRerankTopKSpec {
            // The reference order matters everywhere, not only at one
            // boundary; target the middle like whole-list sorts do.
            k: n.div_ceil(2),
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
        comparison_budget: opts.comparison_budget,
        latency_budget_ms: None,
        max_cost_nanodollars: None,
        model: opts.model.clone(),
        rater_id: None,
        comparison_concurrency: opts.comparison_concurrency,
        max_pair_repeats: None,
        randomize_presentation_order: true,
        counterbalance_pairs: opts.counterbalance,
    };

    let outcome = multi_rerank_with_failures(request, execution).await?;
    let response = outcome.response;

    // Reference scores: input order, best first -> descending scores.
    // Entity order in the response matches ranking, so look up by id.
    let mut ref_score_by_id = std::collections::HashMap::new();
    for (pos, doc) in documents.iter().enumerate() {
        ref_score_by_id.insert(doc.id.clone(), (n - pos) as f64);
    }

    // Per-attribute score vectors aligned to input document order.
    let mut score_by_id: Vec<std::collections::HashMap<&str, f64>> =
        vec![std::collections::HashMap::new(); candidates.len()];
    for entity in &response.entities {
        for (idx, _) in candidates.iter().enumerate() {
            if let Some(s) = entity.attribute_scores.get(&format!("explain_{idx}")) {
                score_by_id[idx].insert(entity.id.as_str(), s.latent_mean);
            }
        }
    }

    let reference: Vec<f64> = documents.iter().map(|d| ref_score_by_id[&d.id]).collect();
    let columns: Vec<Vec<f64>> = (0..candidates.len())
        .map(|idx| {
            documents
                .iter()
                .map(|d| score_by_id[idx].get(d.id.as_str()).copied().unwrap_or(0.0))
                .collect()
        })
        .collect();

    let alone: Vec<Option<f64>> = columns
        .iter()
        .map(|col| spearman(col, &reference))
        .collect();

    // Fit non-negative weights over rank-transformed columns to reconstruct
    // the reference ranks (projected gradient descent; m is tiny).
    let rank_columns: Vec<Vec<f64>> = columns.iter().map(|c| average_ranks(c)).collect();
    let target = average_ranks(&reference);
    let weights = fit_nonnegative_weights(&rank_columns, &target);

    let combined: Vec<f64> = (0..n)
        .map(|i| {
            rank_columns
                .iter()
                .zip(weights.iter())
                .map(|(col, w)| col[i] * w)
                .sum()
        })
        .collect();
    let combined_spearman = if weights.iter().any(|&w| w > 0.0) {
        spearman(&combined, &reference)
    } else {
        None
    };

    let attributes = candidates
        .into_iter()
        .enumerate()
        .map(|(idx, prompt)| AttributeExplanation {
            attribute_id: format!("explain_{idx}"),
            prompt,
            spearman_alone: alone[idx],
            fitted_weight: weights[idx],
        })
        .collect();

    Ok(Explanation {
        attributes,
        combined_spearman,
        meta: simple::meta_from_multi(
            response.meta,
            outcome.comparisons_failed,
            outcome.first_error,
        ),
    })
}

/// Ask an LLM to propose candidate attributes that might explain a given
/// ranking (best first). Returns short criterion strings, ready for
/// [`explain_ranking`]. One chat call; usage in the returned tuple.
/// Token and cost accounting for a single proposal call.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct ProposalUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cost_nanodollars: i64,
}

pub async fn propose_candidates(
    gateway: &dyn ChatGateway,
    model: &str,
    documents: &[RerankDocument],
    how_many: usize,
    attribution: Attribution,
) -> Result<(Vec<String>, ProposalUsage), ExplainError> {
    let list = documents
        .iter()
        .enumerate()
        .map(|(i, d)| format!("{}. {}", i + 1, d.text))
        .collect::<Vec<_>>()
        .join("\n");
    let system = "You analyze rankings. Given a list someone has ordered from \
        best to worst by gut feeling, propose the distinct underlying attributes \
        that most plausibly explain the ordering. Each attribute must be a short, \
        judgeable criterion phrase (2-8 words) usable in the question 'which of \
        these two items has more of X?'. Attributes must be genuinely different \
        from each other, not paraphrases. Output only a JSON array of strings.";
    let user = format!(
        "This list is ranked best-first. Propose exactly {how_many} candidate \
         attributes that might explain the ordering.\n\n<ranked_list>\n{list}\n</ranked_list>\n\njson:"
    );
    propose_via_chat(gateway, model, system, user, attribution).await
}

/// Ask an LLM to decompose a goal into judgeable considerations — the
/// automated front half of AHP. Returns short criterion strings ready to be
/// weighed pairwise for importance (`cardinal weigh --propose`). Proposals
/// are hypotheses; the weighing measures them.
pub async fn propose_for_goal(
    gateway: &dyn ChatGateway,
    model: &str,
    goal: &str,
    how_many: usize,
    attribution: Attribution,
) -> Result<(Vec<String>, ProposalUsage), ExplainError> {
    let system = "You decompose goals into judgeable considerations. Given a \
        goal, propose the distinct considerations that most plausibly determine \
        success at it. Each consideration must be a short, judgeable criterion \
        phrase (2-8 words) usable in the question 'which of these two matters \
        more for the goal?'. Considerations must be genuinely different from \
        each other, not paraphrases, and must span the goal — include the \
        unglamorous ones. Output only a JSON array of strings.";
    let user = format!(
        "Goal: {goal}\n\nPropose exactly {how_many} considerations that \
         determine success at this goal.\n\njson:"
    );
    propose_via_chat(gateway, model, system, user, attribution).await
}

/// Ask an LLM to propose attributes on which one focal item stands out from
/// its peers — candidate directions along which the item is worth
/// propagating. Proposals are hypotheses; [`differentiation_profile`]
/// measures them.
pub async fn propose_distinguishing(
    gateway: &dyn ChatGateway,
    model: &str,
    documents: &[RerankDocument],
    focal_id: &str,
    how_many: usize,
    attribution: Attribution,
) -> Result<(Vec<String>, ProposalUsage), ExplainError> {
    let focal = documents
        .iter()
        .find(|d| d.id == focal_id)
        .ok_or_else(|| ExplainError::ProposalParse(format!("no document with id {focal_id}")))?;
    let peers = documents
        .iter()
        .filter(|d| d.id != focal_id)
        .enumerate()
        .map(|(i, d)| format!("{}. {}", i + 1, d.text))
        .collect::<Vec<_>>()
        .join("\n");
    let system = "You distinguish one item from its peers. Given a focal item \
        and a peer set, propose the distinct attributes on which the focal item \
        most plausibly stands out — attributes under which judging the whole \
        set would place the focal item at an extreme. Each attribute must be a \
        short, judgeable criterion phrase (2-8 words) usable in the question \
        'which of these two items has more of X?'. Attributes must be genuinely \
        different from each other, not paraphrases. Output only a JSON array \
        of strings.";
    let user = format!(
        "<focal_item>\n{}\n</focal_item>\n\n<peer_items>\n{peers}\n</peer_items>\n\n\
         Propose exactly {how_many} attributes on which the focal item stands \
         out from its peers.\n\njson:",
        focal.text
    );
    propose_via_chat(gateway, model, system, user, attribution).await
}

/// Ask an LLM for alternative precise wordings/refinements of an
/// attribute — candidates for the canonize protocol. Proposals are
/// hypotheses; transmissibility across judges decides.
pub async fn propose_rewordings(
    gateway: &dyn ChatGateway,
    model: &str,
    attribute: &str,
    how_many: usize,
    attribution: Attribution,
) -> Result<(Vec<String>, ProposalUsage), ExplainError> {
    let system = "You reword attributes for judgement. Given an attribute \
        someone rates items by, propose alternative precise wordings or sharper \
        refinements of the SAME underlying dimension — versions that might \
        elicit a more consistent, more meaningful ordering. Each must be a \
        short, judgeable criterion phrase (2-10 words) usable in the question \
        'which of these two items has more of X?'. Do not drift to different \
        dimensions; reword and sharpen this one. Output only a JSON array of \
        strings.";
    let user = format!(
        "Attribute: {attribute}\n\nPropose exactly {how_many} alternative \
         wordings or refinements.\n\njson:"
    );
    propose_via_chat(gateway, model, system, user, attribution).await
}

async fn propose_via_chat(
    gateway: &dyn ChatGateway,
    model: &str,
    system: &str,
    user: String,
    attribution: Attribution,
) -> Result<(Vec<String>, ProposalUsage), ExplainError> {
    use crate::gateway::{ChatModel, ChatRequest, Message};

    // Some model families intermittently return an empty completion or a
    // decorated envelope (measured on the Manifund P1 run: deepseek empty,
    // gpt-5.4-mini `{"[]": [...]}`). Extraction is lenient; a completion
    // with no extractable strings earns exactly one retry before failing.
    let mut usage = ProposalUsage {
        input_tokens: 0,
        output_tokens: 0,
        cost_nanodollars: 0,
    };
    let mut last_content = String::new();
    for _attempt in 0..2 {
        let response = gateway
            .chat(ChatRequest {
                model: ChatModel::parse(model),
                messages: vec![Message::system(system), Message::user(user.clone())],
                temperature: 0.4,
                max_tokens: Some(600),
                json_mode: true,
                attribution: attribution.clone(),
                logprobs: false,
                top_logprobs: None,
                reasoning: None,
                prompt_cache_key: None,
            })
            .await?;
        usage.input_tokens += response.input_tokens;
        usage.output_tokens += response.output_tokens;
        usage.cost_nanodollars += response.cost_nanodollars;
        if let Some(parsed) = super::proposal_json::lenient_string_array(&response.content) {
            return Ok((parsed, usage));
        }
        last_content = response.content;
    }
    Err(ExplainError::ProposalParse(format!(
        "no string array in completion after retry: {last_content:?}"
    )))
}

/// One attribute's measured differentiation result for a focal item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributeDifferentiation {
    /// Attribute id used in traces and cache keys (`distinguish_<n>`).
    pub attribute_id: String,
    /// The attribute prompt that was measured.
    pub prompt: String,
    /// Focal item's percentile among the set on this attribute (0..1).
    pub percentile: f64,
    /// Focal item's robust z-score on this attribute.
    pub z_score: f64,
    /// Focal item's posterior latent mean.
    pub latent_mean: f64,
    /// Focal item's posterior latent std.
    pub latent_std: f64,
}

/// Result of [`differentiation_profile`]: where does the focal item actually
/// land, per candidate attribute, measured — not asserted.
#[derive(Debug, Serialize)]
pub struct DifferentiationProfile {
    /// The focal entity id.
    pub focal_id: String,
    /// Per-attribute results, sorted by focal z-score descending: the top
    /// entries are the measured directions along which the item stands out.
    pub attributes: Vec<AttributeDifferentiation>,
    /// Run metadata (comparisons, tokens, cost, counterbalancing flips,
    /// frustration).
    pub meta: RerankMeta,
}

/// Measure candidate attributes over the whole entity set and profile one
/// focal item: its percentile and z-score per attribute, best direction
/// first. The propagation primitive — "under which judgeable attribute does
/// this item deserve to travel far?" — with the answer measured by the same
/// counterbalanced pairwise machinery as everything else, never taken from
/// the proposer's say-so.
pub async fn differentiation_profile(
    documents: Vec<RerankDocument>,
    focal_id: &str,
    candidates: Vec<String>,
    execution: RerankExecution<'_>,
    opts: ExplainOptions,
) -> Result<DifferentiationProfile, ExplainError> {
    let n = documents.len();
    if n < 3 {
        return Err(ExplainError::TooFewItems(n));
    }
    if candidates.is_empty() {
        return Err(ExplainError::NoCandidates);
    }
    if !documents.iter().any(|d| d.id == focal_id) {
        return Err(ExplainError::ProposalParse(format!(
            "no document with id {focal_id}"
        )));
    }

    let attributes: Vec<MultiRerankAttributeSpec> = candidates
        .iter()
        .enumerate()
        .map(|(idx, prompt)| MultiRerankAttributeSpec {
            id: format!("distinguish_{idx}"),
            prompt: prompt.clone(),
            prompt_template_slug: None,
            weight: 1.0,
        })
        .collect();

    let request = MultiRerankRequest {
        entities: documents
            .iter()
            .map(|doc| MultiRerankEntity {
                id: doc.id.clone(),
                text: doc.text.clone(),
            })
            .collect(),
        attributes,
        topk: MultiRerankTopKSpec {
            // Whole-set resolution matters (percentiles, not one boundary);
            // target the middle like whole-list sorts do.
            k: n.div_ceil(2),
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
        comparison_budget: opts.comparison_budget,
        latency_budget_ms: None,
        max_cost_nanodollars: None,
        model: opts.model.clone(),
        rater_id: None,
        comparison_concurrency: opts.comparison_concurrency,
        max_pair_repeats: None,
        randomize_presentation_order: true,
        counterbalance_pairs: opts.counterbalance,
    };

    let outcome = multi_rerank_with_failures(request, execution).await?;
    let response = outcome.response;

    let focal = response
        .entities
        .iter()
        .find(|entity| entity.id == focal_id)
        .ok_or_else(|| {
            ExplainError::ProposalParse(format!("focal id {focal_id} missing from result"))
        })?;

    let mut profile: Vec<AttributeDifferentiation> = candidates
        .iter()
        .enumerate()
        .filter_map(|(idx, prompt)| {
            let attribute_id = format!("distinguish_{idx}");
            focal
                .attribute_scores
                .get(&attribute_id)
                .map(|score| AttributeDifferentiation {
                    attribute_id,
                    prompt: prompt.clone(),
                    percentile: score.percentile,
                    z_score: score.z_score,
                    latent_mean: score.latent_mean,
                    latent_std: score.latent_std,
                })
        })
        .collect();
    profile.sort_by(|a, b| {
        b.z_score
            .partial_cmp(&a.z_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(DifferentiationProfile {
        focal_id: focal_id.to_string(),
        attributes: profile,
        meta: simple::meta_from_multi(
            response.meta,
            outcome.comparisons_failed,
            outcome.first_error,
        ),
    })
}

/// Projected gradient descent for `min ||Xw - y||²  s.t.  w >= 0`,
/// normalized to sum 1 when any weight is positive. Deterministic; intended
/// for a handful of attributes.
fn fit_nonnegative_weights(columns: &[Vec<f64>], target: &[f64]) -> Vec<f64> {
    let m = columns.len();
    let n = target.len();
    if m == 0 || n == 0 {
        return vec![0.0; m];
    }
    // Center columns and target so the intercept drops out.
    let center = |v: &[f64]| {
        let mean = v.iter().sum::<f64>() / v.len() as f64;
        v.iter().map(|x| x - mean).collect::<Vec<f64>>()
    };
    let x: Vec<Vec<f64>> = columns.iter().map(|c| center(c)).collect();
    let y = center(target);

    // Lipschitz-ish step from the largest column norm.
    let max_norm_sq = x
        .iter()
        .map(|c| c.iter().map(|v| v * v).sum::<f64>())
        .fold(0.0_f64, f64::max)
        .max(1e-12);
    let step = 1.0 / (max_norm_sq * m as f64);

    let mut w = vec![1.0 / m as f64; m];
    for _ in 0..2000 {
        // residual r = Xw - y
        let mut r = vec![0.0; n];
        for (col, &wj) in x.iter().zip(w.iter()) {
            for (ri, &cv) in r.iter_mut().zip(col.iter()) {
                *ri += wj * cv;
            }
        }
        for (ri, &yv) in r.iter_mut().zip(y.iter()) {
            *ri -= yv;
        }
        // gradient = Xᵀ r ; project onto w >= 0
        let mut moved = 0.0_f64;
        for (j, col) in x.iter().enumerate() {
            let g: f64 = col.iter().zip(r.iter()).map(|(c, ri)| c * ri).sum();
            let next = (w[j] - step * g).max(0.0);
            moved += (next - w[j]).abs();
            w[j] = next;
        }
        if moved < 1e-12 {
            break;
        }
    }
    let total: f64 = w.iter().sum();
    if total > 0.0 {
        for wj in w.iter_mut() {
            *wj /= total;
        }
    }
    w
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nonnegative_fit_recovers_dominant_column() {
        // Target is exactly column 0; column 1 is anti-correlated noise.
        let col0 = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let col1 = vec![5.0, 4.0, 3.0, 2.0, 1.0];
        let target = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let w = fit_nonnegative_weights(&[col0, col1], &target);
        assert!(w[0] > 0.95, "weights: {w:?}");
        assert!(w[1] < 0.05, "weights: {w:?}");
    }

    #[test]
    fn nonnegative_fit_splits_between_complementary_columns() {
        // Target = col0 + col1; both should get meaningful weight.
        let col0 = vec![1.0, 3.0, 2.0, 5.0, 4.0];
        let col1 = vec![2.0, 1.0, 4.0, 3.0, 5.0];
        let target: Vec<f64> = col0.iter().zip(col1.iter()).map(|(a, b)| a + b).collect();
        let w = fit_nonnegative_weights(&[col0, col1], &target);
        assert!(w[0] > 0.2 && w[1] > 0.2, "weights: {w:?}");
    }
}
