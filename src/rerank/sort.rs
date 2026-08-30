//! Sort plain text items by a natural-language criterion.
//!
//! This is the list-in, list-out convenience surface over the single-attribute
//! rerank path in [`super::simple`]. Same planner, same solver, same stopping
//! semantics, same run accounting — with none of the request-shape ceremony.
//!
//! ```no_run
//! use std::sync::Arc;
//! use llmsort::gateway::NoopUsageSink;
//! use llmsort::rerank::sort::{sort_texts, SortOptions};
//! use llmsort::rerank::RerankExecution;
//! use llmsort::{Attribution, ProviderGateway};
//!
//! # async fn demo() -> Result<(), Box<dyn std::error::Error>> {
//! let gateway = ProviderGateway::from_env(Arc::new(NoopUsageSink))?;
//! let execution = RerankExecution::new(Arc::new(gateway), Attribution::new("docs::sort"));
//!
//! let sorted = sort_texts(
//!     vec![
//!         "a bird in the hand is worth two in the bush".into(),
//!         "premature optimization is the root of all evil".into(),
//!         "measure twice, cut once".into(),
//!     ],
//!     "usefulness as advice for a software engineer",
//!     execution,
//!     SortOptions::default(),
//! )
//! .await?;
//!
//! for item in &sorted.items {
//!     println!("{:>2}. {:.3} ± {:.3}  {}", item.rank, item.latent_mean, item.latent_std, item.text);
//! }
//! # Ok(())
//! # }
//! ```

use super::multi::{MultiRerankError, RerankExecution};
use super::simple;
use super::types::{RerankDocument, RerankMeta, RerankRequest};
use serde::{Deserialize, Serialize};

/// Attribute id used for cache keys by [`sort_texts`].
///
/// The cache key also hashes the criterion text, so different criteria never
/// collide even though they share this id.
pub const SORT_ATTRIBUTE_ID: &str = "sort";

/// Options for [`sort_texts`]. Defaults match the single-attribute rerank
/// defaults, except `counterbalance`, which is on: healthy elicitation asks
/// every planned pair in both presentation orders.
#[derive(Debug, Clone)]
pub struct SortOptions {
    /// Model slug (OpenRouter), e.g. `anthropic/claude-sonnet-4.6`.
    pub model: Option<String>,
    /// Maximum pairwise comparisons to spend. Note: with `counterbalance`
    /// (the default) each planned pair costs two comparisons.
    pub comparison_budget: Option<usize>,
    /// Maximum provider-reported spend in nanodollars.
    pub max_cost_nanodollars: Option<i64>,
    /// Maximum wall-clock time in milliseconds.
    pub latency_budget_ms: Option<u64>,
    /// Certify only the top k of the list; the tail is still returned,
    /// ordered by posterior mean, but the stopping rule targets the top-k
    /// boundary. Default: the whole list.
    pub top_k: Option<usize>,
    /// Stop when the estimated top-k error falls below this (default 0.1).
    pub tolerated_error: Option<f64>,
    /// Maximum concurrent comparisons.
    pub comparison_concurrency: Option<usize>,
    /// Maximum repeats per pair.
    pub max_pair_repeats: Option<usize>,
    /// Ask every planned pair in both presentation orders (default: true).
    /// Cancels position bias per-pair and reports order disagreement in
    /// `meta.pairs_counterbalanced` / `meta.position_flips`.
    pub counterbalance: bool,
    /// Also judge the OPPOSITE of the criterion (`lack of <criterion>`,
    /// weight −1) and fold it into the ranking, reporting cross-side rank
    /// consistency as a probe diagnostic. An attribute whose two sides disagree
    /// is incoherent for this judge — better to learn that than to ship it.
    pub two_sided: bool,
    /// Alternate phrasings of the criterion, each judged as an additional
    /// weight-1 attribute and reported as a paraphrase-consistency probe.
    pub also_by: Vec<String>,
    /// Prune hopeless items from forced exploration once their probability
    /// of reaching the top-k falls below this threshold. Saves queries when
    /// only the top of the list matters. Off by default.
    pub prune_p_topk_below: Option<f64>,
    /// Prompt template slug. `None` resolves per model — see
    /// [`super::default_template_slug`]. Explicit values also accept
    /// `canonical_bucket_v1`.
    pub prompt_template_slug: Option<String>,
}

impl Default for SortOptions {
    fn default() -> Self {
        Self {
            model: None,
            comparison_budget: None,
            max_cost_nanodollars: None,
            latency_budget_ms: None,
            top_k: None,
            tolerated_error: None,
            comparison_concurrency: None,
            max_pair_repeats: None,
            counterbalance: true,
            two_sided: false,
            also_by: Vec::new(),
            prune_p_topk_below: None,
            prompt_template_slug: None,
        }
    }
}

/// Kind of attribute probe run alongside the primary criterion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortProbeKind {
    /// The polarity-flipped criterion ("lack of X", weight −1).
    Opposite,
    /// An alternate phrasing of the criterion (weight +1).
    Paraphrase,
}

/// Consistency diagnostics for one probe attribute.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SortProbe {
    /// Attribute id used in traces and cache keys.
    pub attribute_id: String,
    /// The probe's prompt text.
    pub prompt: String,
    /// Opposite or paraphrase.
    pub kind: SortProbeKind,
    /// Sign-adjusted Spearman rank correlation between the probe's latent
    /// scores and the primary criterion's (opposite probes are negated
    /// first). 1.0 = the judge treats both phrasings/sides identically;
    /// near or below 0 = the attribute is incoherent for this judge.
    /// `None` when fewer than 3 items had scores on both attributes.
    pub consistency: Option<f64>,
}

/// One sorted item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SortedItem {
    /// Stable id (synthesized `item-<n>` for plain text input).
    pub id: String,
    /// The original item text, byte-identical to the input.
    pub text: String,
    /// 1-based rank, best first.
    pub rank: usize,
    /// Posterior mean in latent (log-ratio) space.
    pub latent_mean: f64,
    /// Posterior standard deviation.
    pub latent_std: f64,
    /// Robust z-score across the list.
    pub z_score: f64,
    /// Percentile among items (0..1).
    pub percentile: f64,
}

/// Result of [`sort_texts`]: items sorted best-first, plus the full run
/// accounting (comparisons, tokens, cost, stop reason, probe consistency).
#[derive(Debug, Serialize)]
pub struct SortedTexts {
    /// Items in rank order (best first). When probes are active, the ranking
    /// and `latent_mean`/`latent_std` reflect the combined utility across
    /// the criterion and its probes (opposite sides enter with weight −1).
    pub items: Vec<SortedItem>,
    /// Run metadata: comparisons attempted/used/cached/refused, provider
    /// tokens and cost, counterbalancing flips, stop reason.
    pub meta: RerankMeta,
    /// Consistency diagnostics for `two_sided` / `also_by` probes; empty when
    /// no probes were requested.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub probes: Vec<SortProbe>,
}

/// Errors from [`sort_texts`] / [`sort_documents`].
#[derive(Debug, thiserror::Error)]
pub enum SortError {
    /// The input list was empty.
    #[error("cannot sort an empty list")]
    EmptyInput,
    /// Two documents shared the same id.
    #[error("duplicate document id: {0}")]
    DuplicateId(String),
    /// The underlying rerank failed.
    #[error(transparent)]
    Rerank(#[from] MultiRerankError),
}

/// Sort `texts` by `criterion`, best first.
///
/// Thin wrapper over the single-attribute rerank path: each pairwise ratio
/// judgement ("how many times more of the criterion does A have than B?")
/// becomes a noisy log-ratio observation; a robust solver fits globally
/// consistent scores with uncertainty; the planner spends each comparison
/// where it buys the most information about the (top-k) order.
pub async fn sort_texts(
    texts: Vec<String>,
    criterion: &str,
    execution: RerankExecution<'_>,
    opts: SortOptions,
) -> Result<SortedTexts, SortError> {
    let documents: Vec<RerankDocument> = texts
        .into_iter()
        .enumerate()
        .map(|(idx, text)| RerankDocument {
            id: format!("item-{idx:04}"),
            text,
        })
        .collect();
    sort_documents(documents, criterion, execution, opts).await
}

/// Sort caller-identified documents by `criterion`, best first.
///
/// Like [`sort_texts`], but the caller owns the document ids (which appear in
/// results, traces, and cache keys as entity ids).
pub async fn sort_documents(
    documents: Vec<RerankDocument>,
    criterion: &str,
    execution: RerankExecution<'_>,
    opts: SortOptions,
) -> Result<SortedTexts, SortError> {
    if documents.is_empty() {
        return Err(SortError::EmptyInput);
    }
    if documents.len() == 1 {
        // A single item is already sorted; the rerank path requires >= 2
        // entities, so return the trivial answer with all-zero run accounting.
        let doc = documents.into_iter().next().expect("len checked above");
        return Ok(SortedTexts {
            items: vec![SortedItem {
                id: doc.id,
                text: doc.text,
                rank: 1,
                latent_mean: 0.0,
                latent_std: 0.0,
                z_score: 0.0,
                percentile: 1.0,
            }],
            meta: RerankMeta {
                topk_error: 0.0,
                tolerated_error: opts.tolerated_error.unwrap_or(0.1),
                comparisons_attempted: 0,
                comparisons_failed: 0,
                first_error: None,
                comparisons_used: 0,
                comparisons_refused: 0,
                comparisons_cached: 0,
                comparison_budget: 0,
                latency_ms: 0,
                model_used: String::new(),
                rater_id_used: String::new(),
                provider_input_tokens: 0,
                provider_output_tokens: 0,
                provider_cost_nanodollars: 0,
                provider_cost_is_estimate: false,
                entities_pruned: 0,
                pairs_counterbalanced: 0,
                position_flips: 0,
                evidence_judgements: 0,
                logprob_mode_judgements: 0,
                evidence_visible_mass_mean: None,
                evidence_order_residual_mean_abs: None,
                judgement_frustration_mean: None,
                stop_reason: super::types::RerankStopReason::ToleratedErrorMet,
            },
            probes: Vec::new(),
        });
    }
    {
        let mut seen = std::collections::HashSet::with_capacity(documents.len());
        for doc in &documents {
            if !seen.insert(doc.id.as_str()) {
                return Err(SortError::DuplicateId(doc.id.clone()));
            }
        }
    }
    let texts: std::collections::HashMap<String, String> = documents
        .iter()
        .map(|doc| (doc.id.clone(), doc.text.clone()))
        .collect();

    if opts.two_sided || !opts.also_by.is_empty() {
        return sort_with_probes(documents, texts, criterion, execution, opts).await;
    }

    let request = sort_request(documents, criterion, &opts);
    let response = simple::rerank(request, execution).await?;

    let mut items: Vec<SortedItem> = response
        .results
        .iter()
        .map(|result| SortedItem {
            id: result.id.clone(),
            text: texts
                .get(&result.id)
                .expect("rerank results only contain input document ids")
                .clone(),
            rank: result.rank,
            latent_mean: result.latent_mean,
            latent_std: result.latent_std,
            z_score: result.z_score,
            percentile: result.percentile,
        })
        .collect();
    items.sort_by_key(|item| item.rank);

    Ok(SortedTexts {
        items,
        meta: response.meta,
        probes: Vec::new(),
    })
}

/// Probe-mode sort: the criterion plus its opposite side and/or alternate
/// phrasings run as parallel attributes of one multi-rerank; the ranking is
/// the signed-weight combined utility, and each probe yields a consistency
/// diagnostic against the primary criterion.
async fn sort_with_probes(
    documents: Vec<RerankDocument>,
    texts: std::collections::HashMap<String, String>,
    criterion: &str,
    execution: RerankExecution<'_>,
    opts: SortOptions,
) -> Result<SortedTexts, SortError> {
    use super::types::{MultiRerankAttributeSpec, MultiRerankEntity, MultiRerankRequest};

    let n = documents.len();
    let mut attributes = vec![MultiRerankAttributeSpec {
        id: SORT_ATTRIBUTE_ID.to_string(),
        prompt: criterion.to_string(),
        prompt_template_slug: opts.prompt_template_slug.clone(),
        weight: 1.0,
    }];
    let mut probe_specs: Vec<(String, String, SortProbeKind)> = Vec::new();
    if opts.two_sided {
        let id = format!("{SORT_ATTRIBUTE_ID}_opposite");
        let prompt = format!("lack of {criterion}");
        attributes.push(MultiRerankAttributeSpec {
            id: id.clone(),
            prompt: prompt.clone(),
            // Probes run on the same instrument as the criterion: a
            // consistency probe on a different template measures the
            // template, not the judge.
            prompt_template_slug: opts.prompt_template_slug.clone(),
            weight: -1.0,
        });
        probe_specs.push((id, prompt, SortProbeKind::Opposite));
    }
    for (idx, alt) in opts.also_by.iter().enumerate() {
        let id = format!("{SORT_ATTRIBUTE_ID}_alt{}", idx + 1);
        attributes.push(MultiRerankAttributeSpec {
            id: id.clone(),
            prompt: alt.clone(),
            prompt_template_slug: opts.prompt_template_slug.clone(),
            weight: 1.0,
        });
        probe_specs.push((id, alt.clone(), SortProbeKind::Paraphrase));
    }

    let top_k = opts.top_k.filter(|&k| k < n).unwrap_or(n.div_ceil(2));
    let request = MultiRerankRequest {
        entities: documents
            .into_iter()
            .map(|doc| MultiRerankEntity {
                id: doc.id,
                text: doc.text,
            })
            .collect(),
        attributes,
        topk: super::types::MultiRerankTopKSpec {
            k: top_k,
            // Probes are peers of the criterion; no super-linear emphasis.
            weight_exponent: 1.0,
            tolerated_error: opts.tolerated_error.unwrap_or(0.1),
            band_size: 5,
            effective_resistance_max_active: 64,
            stop_sigma_inflate: 1.25,
            stop_min_consecutive: 2,
            min_explore_degree: 2,
            prune_p_topk_below: opts.prune_p_topk_below,
        },
        gates: Vec::new(),
        comparison_budget: opts.comparison_budget,
        latency_budget_ms: opts.latency_budget_ms,
        max_cost_nanodollars: opts.max_cost_nanodollars,
        model: opts.model.clone(),
        rater_id: None,
        comparison_concurrency: opts.comparison_concurrency,
        max_pair_repeats: opts.max_pair_repeats,
        randomize_presentation_order: true,
        counterbalance_pairs: opts.counterbalance,
    };

    let outcome = super::multi::multi_rerank_with_failures(request, execution).await?;
    let response = outcome.response;

    // Primary latent vectors (entity order) for probe consistency.
    let primary: Vec<Option<f64>> = response
        .entities
        .iter()
        .map(|e| {
            e.attribute_scores
                .get(SORT_ATTRIBUTE_ID)
                .map(|s| s.latent_mean)
        })
        .collect();

    let probes: Vec<SortProbe> = probe_specs
        .into_iter()
        .map(|(attribute_id, prompt, kind)| {
            let sign = match kind {
                SortProbeKind::Opposite => -1.0,
                SortProbeKind::Paraphrase => 1.0,
            };
            let mut xs = Vec::new();
            let mut ys = Vec::new();
            for (entity, p) in response.entities.iter().zip(primary.iter()) {
                if let (Some(primary_score), Some(score)) =
                    (p, entity.attribute_scores.get(&attribute_id))
                {
                    xs.push(*primary_score);
                    ys.push(sign * score.latent_mean);
                }
            }
            SortProbe {
                attribute_id,
                prompt,
                kind,
                consistency: spearman(&xs, &ys),
            }
        })
        .collect();

    let mut items: Vec<SortedItem> = response
        .entities
        .iter()
        .filter(|e| e.feasible)
        .map(|e| {
            let rank = e.rank.unwrap_or(0);
            let primary_scores = e.attribute_scores.get(SORT_ATTRIBUTE_ID);
            SortedItem {
                id: e.id.clone(),
                text: texts
                    .get(&e.id)
                    .expect("rerank results only contain input document ids")
                    .clone(),
                rank,
                latent_mean: e.u_mean,
                latent_std: e.u_std,
                z_score: primary_scores.map(|s| s.z_score).unwrap_or(0.0),
                percentile: if n > 1 {
                    (n - rank) as f64 / (n - 1) as f64
                } else {
                    1.0
                },
            }
        })
        .collect();
    items.sort_by_key(|item| item.rank);

    Ok(SortedTexts {
        items,
        meta: simple::meta_from_multi(
            response.meta,
            outcome.comparisons_failed,
            outcome.first_error,
        ),
        probes,
    })
}

/// Spearman rank correlation with average ranks for ties.
/// Returns `None` for fewer than 3 points or zero variance.
#[doc(hidden)]
pub fn spearman(xs: &[f64], ys: &[f64]) -> Option<f64> {
    if xs.len() != ys.len() || xs.len() < 3 {
        return None;
    }
    let rx = average_ranks(xs);
    let ry = average_ranks(ys);
    let n = rx.len() as f64;
    let mx = rx.iter().sum::<f64>() / n;
    let my = ry.iter().sum::<f64>() / n;
    let mut cov = 0.0;
    let mut vx = 0.0;
    let mut vy = 0.0;
    for (a, b) in rx.iter().zip(ry.iter()) {
        cov += (a - mx) * (b - my);
        vx += (a - mx).powi(2);
        vy += (b - my).powi(2);
    }
    if vx <= f64::EPSILON || vy <= f64::EPSILON {
        return None;
    }
    Some(cov / (vx.sqrt() * vy.sqrt()))
}

pub(crate) fn average_ranks(values: &[f64]) -> Vec<f64> {
    let mut order: Vec<usize> = (0..values.len()).collect();
    order.sort_by(|&a, &b| {
        values[a]
            .partial_cmp(&values[b])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut ranks = vec![0.0; values.len()];
    let mut i = 0;
    while i < order.len() {
        let mut j = i;
        while j + 1 < order.len() && values[order[j + 1]] == values[order[i]] {
            j += 1;
        }
        let avg = (i + j) as f64 / 2.0 + 1.0;
        for &idx in &order[i..=j] {
            ranks[idx] = avg;
        }
        i = j + 1;
    }
    ranks
}

/// Build the underlying single-attribute request used by [`sort_texts`].
///
/// Exposed so callers (and the CLI) can construct the exact same request
/// while owning document ids themselves.
pub fn sort_request(
    documents: Vec<RerankDocument>,
    criterion: &str,
    opts: &SortOptions,
) -> RerankRequest {
    // A whole-list sort (top_k = None) must not degenerate to k = n: with
    // k = n there is no k/k+1 boundary, the top-k error is trivially zero,
    // and the loop would stop before the first comparison. Target the middle
    // boundary instead — the band around it covers the most crowded region,
    // and forced exploration (min_explore_degree) still touches every item.
    // Callers who care about a specific boundary should set top_k explicitly.
    let n = documents.len();
    let top_k = opts.top_k.filter(|&k| k < n).or(Some(n.div_ceil(2)));
    RerankRequest {
        query: None,
        documents,
        attribute_id: SORT_ATTRIBUTE_ID.to_string(),
        attribute_prompt: criterion.to_string(),
        top_k,
        comparison_budget: opts.comparison_budget,
        latency_budget_ms: opts.latency_budget_ms,
        max_cost_nanodollars: opts.max_cost_nanodollars,
        tolerated_error: opts.tolerated_error.unwrap_or(0.1),
        model: opts.model.clone(),
        rater_id: None,
        comparison_concurrency: opts.comparison_concurrency,
        max_pair_repeats: opts.max_pair_repeats,
        counterbalance_pairs: opts.counterbalance,
        prune_p_topk_below: opts.prune_p_topk_below,
        prompt_template_slug: opts.prompt_template_slug.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sort_request_maps_options() {
        let docs = vec![
            RerankDocument {
                id: "item-0000".into(),
                text: "a".into(),
            },
            RerankDocument {
                id: "item-0001".into(),
                text: "b".into(),
            },
        ];
        let opts = SortOptions {
            model: Some("test/model".into()),
            comparison_budget: Some(7),
            top_k: Some(1),
            tolerated_error: Some(0.05),
            comparison_concurrency: Some(2),
            max_pair_repeats: Some(1),
            ..Default::default()
        };
        let req = sort_request(docs, "clarity", &opts);
        assert!(req.counterbalance_pairs, "sort counterbalances by default");
        assert_eq!(req.attribute_id, SORT_ATTRIBUTE_ID);
        assert_eq!(req.attribute_prompt, "clarity");
        assert_eq!(req.top_k, Some(1));
        assert_eq!(req.comparison_budget, Some(7));
        assert_eq!(req.tolerated_error, 0.05);
        assert_eq!(req.model.as_deref(), Some("test/model"));
        assert_eq!(req.comparison_concurrency, Some(2));
        assert_eq!(req.max_pair_repeats, Some(1));
        assert_eq!(req.documents.len(), 2);
    }

    #[test]
    fn sort_request_defaults_top_k_to_middle_boundary() {
        let docs = |n: usize| {
            (0..n)
                .map(|idx| RerankDocument {
                    id: format!("item-{idx:04}"),
                    text: format!("t{idx}"),
                })
                .collect::<Vec<_>>()
        };
        // Whole-list sort: no top_k => middle boundary, never k = n.
        let req = sort_request(docs(4), "clarity", &SortOptions::default());
        assert_eq!(req.top_k, Some(2));
        let req = sort_request(docs(9), "clarity", &SortOptions::default());
        assert_eq!(req.top_k, Some(5));
        // Degenerate k >= n is remapped the same way.
        let opts = SortOptions {
            top_k: Some(4),
            ..Default::default()
        };
        let req = sort_request(docs(4), "clarity", &opts);
        assert_eq!(req.top_k, Some(2));
        // Real boundaries pass through untouched.
        let opts = SortOptions {
            top_k: Some(3),
            ..Default::default()
        };
        let req = sort_request(docs(4), "clarity", &opts);
        assert_eq!(req.top_k, Some(3));
    }

    #[tokio::test]
    async fn sort_texts_rejects_empty_input() {
        // Construct a gateway that will never be reached: empty input fails first.
        let adapter = crate::gateway::openrouter::OpenRouterAdapter::with_config(
            "sk-unused",
            "http://127.0.0.1:9",
            std::time::Duration::from_secs(1),
            None,
            None,
        )
        .unwrap();
        let gateway = std::sync::Arc::new(crate::gateway::ProviderGateway::with_config(
            adapter,
            std::sync::Arc::new(crate::gateway::NoopUsageSink),
            crate::gateway::GatewayConfig::default(),
        ));
        let execution = RerankExecution::new(gateway, crate::gateway::Attribution::new("test"));
        let err = sort_texts(vec![], "clarity", execution, SortOptions::default())
            .await
            .unwrap_err();
        assert!(matches!(err, SortError::EmptyInput));
    }
}
