//! Wording invariance: does the judge's ratio answer respect the group
//! structure it implicitly claims?
//!
//! A ratio judgement lives on (ℝ₊, ×): "A has r× more than B" COMMITS the
//! judge to "B has r× less than A" and "B reaches 1/r of A's level". Those
//! are the same proposition asked through the group inverse — a coherent
//! judge must return the same signed log-ratio through all three wordings.
//! Disagreement separates two failure modes that a polarity probe
//! conflates: **inversion failure** (the model cannot mirror its own scale
//! — the `less` wording comes back with the wrong sign) and **numerical
//! framing bias** (right sign, different magnitude between multiplicative
//! and fractional wording — a documented human bias worth measuring in
//! machines).
//!
//! Six comparisons: {times-more, fraction, times-less} × both presentation
//! orders, every answer lowered to the same (winner, ratio) shape by the
//! slug-aware parser.

use serde::Serialize;

use super::comparison::{
    compare_pair, ComparisonError, PairwiseComparisonAttribute, PairwiseComparisonEntity,
    PairwiseComparisonRequest, PairwiseComparisonSpec,
};
use super::types::signed_log_ratio_toward_first as signed_log_ratio;
use crate::cache::PairwiseCache;
use crate::gateway::{Attribution, ChatGateway};

/// The three wordings of one ratio question.
pub const WORDING_SLUGS: [&str; 3] = ["canonical_v2", "fraction_v1", "less_v1"];

/// One wording's fused measurement.
#[derive(Debug, Clone, Serialize)]
pub struct WordingReading {
    /// Template slug.
    pub template: String,
    /// Mean signed log-ratio toward the first item over both orders.
    pub mean_log_ratio: Option<f64>,
    /// Refusals among this wording's calls.
    pub refusals: usize,
}

/// Result of [`wording_invariance`].
#[derive(Debug, Serialize)]
pub struct WordingInvarianceReport {
    /// Per-wording readings.
    pub readings: Vec<WordingReading>,
    /// Max |difference| between any two recovered log-ratios (nats).
    pub max_disagreement_nats: Option<f64>,
    /// All wordings agree in sign (inversion works). `false` = the judge
    /// cannot mirror its own scale.
    pub sign_consistent: Option<bool>,
    /// Comparisons attempted (6 = 3 wordings × 2 orders).
    pub comparisons: usize,
    pub comparisons_cached: usize,
    pub cost_nanodollars: i64,
}

/// Ask the same ratio question through its three wordings and compare the
/// recovered log-ratios.
pub async fn wording_invariance(
    gateway: &dyn ChatGateway,
    cache: Option<&dyn PairwiseCache>,
    model: &str,
    criterion: &str,
    first: (&str, &str),
    second: (&str, &str),
    attribution: Attribution,
) -> Result<WordingInvarianceReport, ComparisonError> {
    let mut readings = Vec::with_capacity(WORDING_SLUGS.len());
    let mut comparisons = 0usize;
    let mut comparisons_cached = 0usize;
    let mut cost = 0i64;

    for slug in WORDING_SLUGS {
        let mut samples = Vec::with_capacity(2);
        let mut refusals = 0usize;
        for first_in_slot_a in [true, false] {
            let (slot_a, slot_b) = if first_in_slot_a {
                (first, second)
            } else {
                (second, first)
            };
            let spec = PairwiseComparisonSpec {
                model,
                attribute: PairwiseComparisonAttribute {
                    id: "wording",
                    prompt: criterion,
                    prompt_template_slug: Some(slug),
                },
                entity_a: PairwiseComparisonEntity {
                    id: slot_a.0,
                    text: slot_a.1,
                },
                entity_b: PairwiseComparisonEntity {
                    id: slot_b.0,
                    text: slot_b.1,
                },
            };
            let (judgement, usage) = compare_pair(
                gateway,
                cache,
                PairwiseComparisonRequest {
                    spec,
                    cache_only: false,
                    attribution: attribution.clone(),
                    nonce: None,
                },
            )
            .await?;
            comparisons += 1;
            if usage.cached {
                comparisons_cached += 1;
            }
            cost += usage.provider_cost_nanodollars;
            match signed_log_ratio(&judgement, first_in_slot_a) {
                Some(m) => samples.push(m),
                None => refusals += 1,
            }
        }
        let mean_log_ratio = if samples.is_empty() {
            None
        } else {
            Some(samples.iter().sum::<f64>() / samples.len() as f64)
        };
        readings.push(WordingReading {
            template: slug.to_string(),
            mean_log_ratio,
            refusals,
        });
    }

    let ms: Vec<f64> = readings.iter().filter_map(|r| r.mean_log_ratio).collect();
    let max_disagreement_nats = if ms.len() >= 2 {
        let mut max = 0.0f64;
        for i in 0..ms.len() {
            for j in (i + 1)..ms.len() {
                max = max.max((ms[i] - ms[j]).abs());
            }
        }
        Some(max)
    } else {
        None
    };
    let sign_consistent = if ms.len() >= 2 && ms.iter().any(|m| *m != 0.0) {
        let sign = ms
            .iter()
            .find(|m| **m != 0.0)
            .map(|m| m.signum())
            .unwrap_or(1.0);
        Some(ms.iter().all(|m| *m == 0.0 || m.signum() == sign))
    } else {
        None
    };

    Ok(WordingInvarianceReport {
        readings,
        max_disagreement_nats,
        sign_consistent,
        comparisons,
        comparisons_cached,
        cost_nanodollars: cost,
    })
}
