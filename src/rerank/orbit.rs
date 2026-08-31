//! The judgment orbit transform: harmonic analysis on the elicitation group.
//!
//! A pairwise judgment is elicited under a group G = Z₂³ of prompt
//! transformations with KNOWN equivariance:
//!
//!   s — presentation order swap   (answer reflects between slots)
//!   p — polarity negation of the criterion ("lack of X"; answer negates)
//!   w — wording inversion ("which has LESS"; parser already inverts)
//!
//! Pull each measured answer back to canonical coordinates through the
//! generator's known action. For an ideal judge the pulled-back function
//! m: G → ℝ is CONSTANT — the belief. For a real judge, decompose m by
//! the characters of Z₂³: for S ⊆ {s,p,w},
//!
//!   m̂(S) = (1/8) Σ_g (−1)^{Σ_{i∈S} g_i} · m(g)
//!
//! m̂(∅) is the belief — the unique G-invariant projection (the orbit
//! mean). Each nontrivial coefficient is a named bias, and they are
//! ORTHOGONAL: Parseval gives the exact energy accounting
//!
//!   (1/8) Σ_g m(g)² = Σ_S m̂(S)²
//!
//! so `coherence = m̂(∅)² / mean-square` is the fraction of judgment
//! energy that is actually belief. The one-axis-at-a-time probes
//! (counterbalance, two-sided, wordings) are restrictions of this
//! transform to subgroups; the interaction coefficients (S with |S| ≥ 2)
//! are invisible to all of them.
//!
//! Worked algebra with teeth (corrected by its own test — the first
//! derivation conflated two pathologies): "position bias" splits into two
//! distinct characters. A judge that always FAVORS the slot-A entity,
//! answering each wording coherently, has m(g) = μ + (−1)^{s+p}·ln r —
//! the order·polarity coefficient. A judge that always NAMES the token
//! "A", whatever the question, flips again under wording inversion:
//! m(g) = μ + (−1)^{s+p+w}·ln r — the triple character. Counterbalancing
//! cannot tell these apart; the transform separates them exactly. Both
//! pinned in tests with every other coefficient vanishing.

use serde::Serialize;

use super::comparison::{
    compare_pair, ComparisonError, PairwiseComparisonAttribute, PairwiseComparisonEntity,
    PairwiseComparisonRequest, PairwiseComparisonSpec,
};
use super::types::signed_log_ratio_toward_first;
use crate::cache::PairwiseCache;
use crate::gateway::{Attribution, ChatGateway};

/// Character labels for Z₂³, indexed by the subset bitmask s|p<<1|w<<2.
pub const CHARACTERS: [&str; 8] = [
    "belief",
    "order",
    "polarity",
    "order·polarity",
    "wording",
    "order·wording",
    "polarity·wording",
    "order·polarity·wording",
];

/// Result of [`orbit_transform`].
#[derive(Debug, Serialize)]
pub struct OrbitReport {
    /// Fourier coefficients m̂(S), indexed like [`CHARACTERS`].
    /// `coefficients[0]` is the belief (nats, toward the first item).
    pub coefficients: [f64; 8],
    /// Squared coefficients — the orthogonal energy budget.
    pub energies: [f64; 8],
    /// m̂(∅)² / mean-square: the fraction of judgment energy that is
    /// invariant under the whole group. 1.0 = pure belief.
    pub coherence: Option<f64>,
    /// |mean-square − Σ energies|: Parseval is an identity; a nonzero
    /// residual means arithmetic, not judge, is broken.
    pub parseval_residual: f64,
    /// The raw pulled-back orbit m(g), indexed by g = s|p<<1|w<<2.
    pub orbit: [Option<f64>; 8],
    pub refusals: usize,
    pub comparisons: usize,
    pub comparisons_cached: usize,
    pub cost_nanodollars: i64,
}

fn negate_criterion(criterion: &str) -> String {
    format!("lack of {criterion}")
}

/// Measure the full Z₂³ orbit of one pairwise judgment and decompose it.
///
/// Eight comparisons: {order} × {polarity} × {wording}. Every answer is
/// pulled back to canonical coordinates through the generator's known
/// equivariance before the transform. Refusals leave orbit holes; the
/// transform is only computed when the orbit is complete.
#[expect(clippy::too_many_arguments)]
pub async fn orbit_transform(
    gateway: &dyn ChatGateway,
    cache: Option<&dyn PairwiseCache>,
    model: &str,
    criterion: &str,
    first: (&str, &str),
    second: (&str, &str),
    ratio_template: &str,
    attribution: Attribution,
) -> Result<OrbitReport, ComparisonError> {
    let negated = negate_criterion(criterion);
    let mut orbit: [Option<f64>; 8] = [None; 8];
    let mut refusals = 0usize;
    let mut comparisons = 0usize;
    let mut comparisons_cached = 0usize;
    let mut cost = 0i64;

    for (g, slot) in orbit.iter_mut().enumerate() {
        let s = g & 1 != 0;
        let p = g & 2 != 0;
        let w = g & 4 != 0;
        let prompt: &str = if p { &negated } else { criterion };
        let template = if w { "less_v1" } else { ratio_template };
        let (slot_a, slot_b) = if s { (second, first) } else { (first, second) };
        let spec = PairwiseComparisonSpec {
            model,
            attribute: PairwiseComparisonAttribute {
                id: "orbit",
                prompt,
                prompt_template_slug: Some(template),
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
        // Pullback: order reflection is handled by
        // signed_log_ratio_toward_first (slot bookkeeping); wording
        // inversion is handled by the less_v1 parser; polarity negation
        // flips the sign of the canonical answer.
        match signed_log_ratio_toward_first(&judgement, !s) {
            Some(m_raw) => *slot = Some(if p { -m_raw } else { m_raw }),
            None => refusals += 1,
        }
    }

    let mut coefficients = [0.0f64; 8];
    let mut energies = [0.0f64; 8];
    let mut coherence = None;
    let mut parseval_residual = 0.0;
    if refusals == 0 {
        let m: Vec<f64> = orbit.iter().map(|x| x.unwrap()).collect();
        for (chi, coefficient) in coefficients.iter_mut().enumerate() {
            let mut acc = 0.0;
            for (g, mg) in m.iter().enumerate() {
                let parity = (chi & g).count_ones() & 1;
                acc += if parity == 1 { -mg } else { *mg };
            }
            *coefficient = acc / 8.0;
        }
        for (e, c) in energies.iter_mut().zip(coefficients.iter()) {
            *e = c * c;
        }
        let mean_square: f64 = m.iter().map(|x| x * x).sum::<f64>() / 8.0;
        let total: f64 = energies.iter().sum();
        parseval_residual = (mean_square - total).abs();
        if mean_square > 0.0 {
            coherence = Some(energies[0] / mean_square);
        }
    }

    Ok(OrbitReport {
        coefficients,
        energies,
        coherence,
        parseval_residual,
        orbit,
        refusals,
        comparisons,
        comparisons_cached,
        cost_nanodollars: cost,
    })
}
