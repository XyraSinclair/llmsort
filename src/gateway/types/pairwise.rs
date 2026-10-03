use super::{ConfidenceSource, TokenLogprob};
use crate::discrete::{DiscreteDistribution, WeightedValue};
use serde::{Deserialize, Serialize};
use std::ops::{Add, Mul, Neg, Sub};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PairwisePreferredSide {
    A,
    B,
}

impl PairwisePreferredSide {
    fn index(self) -> usize {
        match self {
            PairwisePreferredSide::A => 0,
            PairwisePreferredSide::B => 1,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum RatioBucket {
    R00,
    R01,
    R02,
    R03,
    R04,
    R05,
    R06,
    R07,
    R08,
    R09,
    R10,
    R11,
    R12,
    R13,
    R14,
    R15,
    R16,
}

impl RatioBucket {
    pub const ALL: [Self; 17] = [
        Self::R00,
        Self::R01,
        Self::R02,
        Self::R03,
        Self::R04,
        Self::R05,
        Self::R06,
        Self::R07,
        Self::R08,
        Self::R09,
        Self::R10,
        Self::R11,
        Self::R12,
        Self::R13,
        Self::R14,
        Self::R15,
        Self::R16,
    ];

    pub fn all() -> &'static [Self] {
        &Self::ALL
    }

    pub fn index(self) -> usize {
        match self {
            Self::R00 => 0,
            Self::R01 => 1,
            Self::R02 => 2,
            Self::R03 => 3,
            Self::R04 => 4,
            Self::R05 => 5,
            Self::R06 => 6,
            Self::R07 => 7,
            Self::R08 => 8,
            Self::R09 => 9,
            Self::R10 => 10,
            Self::R11 => 11,
            Self::R12 => 12,
            Self::R13 => 13,
            Self::R14 => 14,
            Self::R15 => 15,
            Self::R16 => 16,
        }
    }

    pub fn ratio(self) -> f64 {
        match self {
            Self::R00 => 1.0,
            Self::R01 => 1.05,
            Self::R02 => 1.1,
            Self::R03 => 1.2,
            Self::R04 => 1.3,
            Self::R05 => 1.5,
            Self::R06 => 1.75,
            Self::R07 => 2.1,
            Self::R08 => 2.5,
            Self::R09 => 3.1,
            Self::R10 => 3.9,
            Self::R11 => 5.1,
            Self::R12 => 6.8,
            Self::R13 => 9.2,
            Self::R14 => 12.7,
            Self::R15 => 18.0,
            Self::R16 => 26.0,
        }
    }

    pub fn ln_ratio(self) -> f64 {
        self.ratio().ln()
    }

    pub fn from_ratio(ratio: f64) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|bucket| (bucket.ratio() - ratio).abs() < 1e-9)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PairwiseAnswer {
    A(RatioBucket),
    B(RatioBucket),
    Refuse,
}

impl PairwiseAnswer {
    pub fn observation(side: PairwisePreferredSide, ratio_bucket: RatioBucket) -> Self {
        match side {
            PairwisePreferredSide::A => Self::A(ratio_bucket),
            PairwisePreferredSide::B => Self::B(ratio_bucket),
        }
    }

    pub fn preferred_side(self) -> Option<PairwisePreferredSide> {
        match self {
            Self::A(_) => Some(PairwisePreferredSide::A),
            Self::B(_) => Some(PairwisePreferredSide::B),
            Self::Refuse => None,
        }
    }

    pub fn ratio_bucket(self) -> Option<RatioBucket> {
        match self {
            Self::A(bucket) | Self::B(bucket) => Some(bucket),
            Self::Refuse => None,
        }
    }

    pub fn ratio(self) -> Option<f64> {
        self.ratio_bucket().map(RatioBucket::ratio)
    }

    pub fn signed_ln_ratio(self) -> Option<f64> {
        match self {
            Self::A(bucket) => Some(bucket.ln_ratio()),
            Self::B(bucket) => Some(-bucket.ln_ratio()),
            Self::Refuse => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SignedLogRatioDistribution {
    pub distribution: DiscreteDistribution<f64>,
    pub abstain_probability: f64,
}

impl SignedLogRatioDistribution {
    const DEFAULT_MAX_SUPPORT: usize = 128;
    const MERGE_TOLERANCE: f64 = 1e-12;

    pub fn new(distribution: DiscreteDistribution<f64>, abstain_probability: f64) -> Self {
        Self {
            distribution,
            abstain_probability: abstain_probability.clamp(0.0, 1.0),
        }
    }

    pub fn from_answer_distribution(
        answer_distribution: &DiscreteDistribution<PairwiseAnswer>,
    ) -> Self {
        let mut support = Vec::with_capacity(answer_distribution.support.len());
        let mut abstain_probability = 0.0;

        for entry in &answer_distribution.support {
            if let Some(value) = entry.value.signed_ln_ratio() {
                push_merged_float_probability(
                    &mut support,
                    value,
                    entry.probability,
                    Self::MERGE_TOLERANCE,
                );
            } else {
                abstain_probability += entry.probability;
            }
        }

        Self::new(
            DiscreteDistribution::new(support, answer_distribution.residual_probability),
            abstain_probability,
        )
    }

    pub fn total_probability(&self) -> f64 {
        self.distribution.total_probability() + self.abstain_probability
    }

    pub fn mean(&self) -> Option<f64> {
        self.distribution.expectation_by(|value| *value)
    }

    pub fn variance(&self) -> Option<f64> {
        self.distribution.variance_by(|value| *value)
    }

    pub fn probability_positive(&self) -> f64 {
        self.distribution.probability_of(|value| *value > 0.0)
    }

    pub fn probability_negative(&self) -> f64 {
        self.distribution.probability_of(|value| *value < 0.0)
    }

    pub fn scale(&self, factor: f64) -> Self {
        Self::new(
            DiscreteDistribution::new(
                self.distribution
                    .support
                    .iter()
                    .map(|entry| WeightedValue {
                        value: entry.value * factor,
                        probability: entry.probability,
                    })
                    .collect(),
                self.distribution.residual_probability,
            ),
            self.abstain_probability,
        )
    }

    pub fn compress(&self, max_support: usize) -> Self {
        if self.distribution.support.len() <= max_support || max_support == 0 {
            return self.clone();
        }

        let mut sorted_support = self.distribution.support.clone();
        sorted_support.sort_by(|left, right| {
            left.value
                .partial_cmp(&right.value)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let chunk_size = sorted_support.len().div_ceil(max_support);
        let mut compressed = Vec::with_capacity(max_support);

        for chunk in sorted_support.chunks(chunk_size) {
            let probability: f64 = chunk.iter().map(|entry| entry.probability).sum();
            if probability <= 0.0 {
                continue;
            }

            let weighted_value = chunk
                .iter()
                .map(|entry| entry.value * entry.probability)
                .sum::<f64>()
                / probability;

            compressed.push(WeightedValue {
                value: weighted_value,
                probability,
            });
        }

        Self::new(
            DiscreteDistribution::new(compressed, self.distribution.residual_probability),
            self.abstain_probability,
        )
    }

    pub fn convolve(&self, other: &Self) -> Self {
        let mut support =
            Vec::with_capacity(self.distribution.support.len() * other.distribution.support.len());

        for left in &self.distribution.support {
            for right in &other.distribution.support {
                push_merged_float_probability(
                    &mut support,
                    left.value + right.value,
                    left.probability * right.probability,
                    Self::MERGE_TOLERANCE,
                );
            }
        }

        let abstain_probability = (self.abstain_probability + other.abstain_probability
            - self.abstain_probability * other.abstain_probability)
            .clamp(0.0, 1.0);
        let support_probability =
            self.distribution.support_probability() * other.distribution.support_probability();
        let residual_probability =
            (1.0 - support_probability - abstain_probability).clamp(0.0, 1.0);

        Self::new(
            DiscreteDistribution::new(support, residual_probability),
            abstain_probability,
        )
        .compress(Self::DEFAULT_MAX_SUPPORT)
    }
}

impl Neg for SignedLogRatioDistribution {
    type Output = Self;

    fn neg(self) -> Self::Output {
        self.scale(-1.0)
    }
}

impl Add for SignedLogRatioDistribution {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        self.convolve(&rhs)
    }
}

impl Sub for SignedLogRatioDistribution {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        self + (-rhs)
    }
}

impl Mul<f64> for SignedLogRatioDistribution {
    type Output = Self;

    fn mul(self, rhs: f64) -> Self::Output {
        self.scale(rhs)
    }
}

fn push_merged_float_probability(
    support: &mut Vec<WeightedValue<f64>>,
    value: f64,
    probability: f64,
    tolerance: f64,
) {
    if !value.is_finite() || !probability.is_finite() || probability <= 0.0 {
        return;
    }

    if let Some(existing) = support
        .iter_mut()
        .find(|entry| (entry.value - value).abs() <= tolerance)
    {
        let combined_probability = existing.probability + probability;
        existing.value =
            (existing.value * existing.probability + value * probability) / combined_probability;
        existing.probability = combined_probability;
        return;
    }

    support.push(WeightedValue { value, probability });
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PairwiseLogprobPosterior {
    /// Selected semantic answer in the structured output.
    pub selected_answer: PairwiseAnswer,
    /// Preferred side chosen by the model in the parsed structured output.
    pub selected_higher_ranked: PairwisePreferredSide,
    /// Ratio chosen by the model in the parsed structured output.
    pub selected_ratio: f64,
    /// Ratio bucket chosen by the model in the parsed structured output.
    pub selected_ratio_bucket: RatioBucket,
    /// Discrete posterior over the preferred side token.
    pub higher_ranked_distribution: DiscreteDistribution<PairwisePreferredSide>,
    /// Discrete posterior over ratio ladder buckets at the selected token position.
    pub ratio_distribution: DiscreteDistribution<RatioBucket>,
    /// Approximate semantic posterior over pairwise answer states.
    pub answer_distribution: DiscreteDistribution<PairwiseAnswer>,
    /// Latent posterior over signed log-ratio values, suitable for solver-side algebra.
    pub signed_ln_ratio_distribution: SignedLogRatioDistribution,
    /// Derived confidence metrics from the ratio distribution.
    pub confidence: ConfidenceSource,
}

impl PairwiseLogprobPosterior {
    pub fn mean_signed_ln_ratio(&self) -> Option<f64> {
        self.signed_ln_ratio_distribution.mean()
    }

    pub fn variance_signed_ln_ratio(&self) -> Option<f64> {
        self.signed_ln_ratio_distribution.variance()
    }

    pub fn probability_positive(&self) -> f64 {
        self.signed_ln_ratio_distribution.probability_positive()
    }

    pub fn probability_negative(&self) -> f64 {
        self.signed_ln_ratio_distribution.probability_negative()
    }
}

fn token_numeric_value(token: &str) -> Option<f64> {
    let mut start = None;
    let mut end = 0usize;
    let mut dot_count = 0usize;

    for (idx, ch) in token.char_indices() {
        if start.is_none() {
            let next_is_digit = token[idx + ch.len_utf8()..]
                .chars()
                .next()
                .is_some_and(|next| next.is_ascii_digit());
            if ch.is_ascii_digit() || (ch == '.' && next_is_digit) {
                start = Some(idx);
                end = idx + ch.len_utf8();
                if ch == '.' {
                    dot_count = 1;
                }
            }
            continue;
        }

        if ch.is_ascii_digit() {
            end = idx + ch.len_utf8();
            continue;
        }

        if ch == '.' {
            dot_count += 1;
            end = idx + ch.len_utf8();
            continue;
        }

        break;
    }

    let start = start?;
    let numeric = &token[start..end];
    if numeric == "." || dot_count > 1 {
        return None;
    }

    numeric.parse::<f64>().ok()
}

fn ratio_ladder_index_for_token(token: &str, ratio_ladder: &[f64]) -> Option<usize> {
    let parsed = token_numeric_value(token)?;
    ratio_ladder
        .iter()
        .position(|ratio| (ratio - parsed).abs() < 1e-9)
}

fn pairwise_preferred_side_for_token(token: &str) -> Option<PairwisePreferredSide> {
    let letters: String = token
        .chars()
        .filter(|ch| ch.is_ascii_alphabetic())
        .collect();
    match letters.to_ascii_uppercase().as_str() {
        "A" => Some(PairwisePreferredSide::A),
        "B" => Some(PairwisePreferredSide::B),
        _ => None,
    }
}

fn collect_token_probabilities(
    position: &TokenLogprob,
    support_len: usize,
    mut token_index: impl FnMut(&str) -> Option<usize>,
) -> (Vec<f64>, f64) {
    let mut probabilities = vec![0.0; support_len];

    if let Some(idx) = token_index(&position.token) {
        probabilities[idx] = position.logprob.exp();
    }

    for alternative in &position.top_alternatives {
        if let Some(idx) = token_index(&alternative.token) {
            probabilities[idx] = probabilities[idx].max(alternative.logprob.exp());
        }
    }

    let covered_mass: f64 = probabilities.iter().sum();
    let residual_probability = (1.0 - covered_mass).max(0.0);
    (probabilities, residual_probability)
}

pub fn truncate_output_logprobs(
    logprobs: &[TokenLogprob],
    max_alternatives: usize,
) -> Vec<TokenLogprob> {
    logprobs
        .iter()
        .map(|entry| TokenLogprob {
            token: entry.token.clone(),
            logprob: entry.logprob,
            top_alternatives: entry
                .top_alternatives
                .iter()
                .take(max_alternatives)
                .cloned()
                .collect(),
        })
        .collect()
}

/// Build a pairwise posterior from winner and ratio token alternatives.
///
/// This helper assumes the preferred side and ratio each correspond to a single
/// token position whose alternatives enumerate the relevant support. That is a
/// useful synthetic model and can be valid for genuinely atomic vocabularies,
/// but it is not sufficient for decimal ratio ladders without continuation
/// rescoring.
pub fn pairwise_logprob_posterior(
    logprobs: &[TokenLogprob],
    selected_higher_ranked: PairwisePreferredSide,
    selected_ratio: f64,
    ratio_ladder: &[f64],
) -> Option<PairwiseLogprobPosterior> {
    let selected_ratio_bucket = RatioBucket::from_ratio(selected_ratio)?;
    let selected_answer =
        PairwiseAnswer::observation(selected_higher_ranked, selected_ratio_bucket);

    let higher_ranked_position = logprobs.iter().find(|lp| {
        pairwise_preferred_side_for_token(&lp.token)
            .is_some_and(|side| side == selected_higher_ranked)
    })?;

    // Find the token position corresponding to the ratio output.
    let ratio_position = logprobs.iter().find(|lp| {
        ratio_ladder_index_for_token(&lp.token, ratio_ladder)
            .is_some_and(|idx| (ratio_ladder[idx] - selected_ratio).abs() < 1e-9)
    })?;

    let selected_idx = ratio_ladder
        .iter()
        .position(|&r| (r - selected_ratio).abs() < 1e-9)?;
    let (winner_probs, winner_residual_probability) =
        collect_token_probabilities(higher_ranked_position, 2, |token| {
            pairwise_preferred_side_for_token(token).map(PairwisePreferredSide::index)
        });
    let higher_ranked_distribution = DiscreteDistribution::new(
        [PairwisePreferredSide::A, PairwisePreferredSide::B]
            .into_iter()
            .enumerate()
            .map(|(idx, side)| WeightedValue {
                value: side,
                probability: winner_probs[idx],
            })
            .collect(),
        winner_residual_probability,
    );

    let (ratio_probs, ratio_residual_probability) =
        collect_token_probabilities(ratio_position, ratio_ladder.len(), |token| {
            ratio_ladder_index_for_token(token, ratio_ladder)
        });

    let neighbor_indices: Vec<usize> = (0..ratio_ladder.len())
        .filter(|&i| i.abs_diff(selected_idx) <= 1)
        .collect();
    let ratio_distribution = DiscreteDistribution::new(
        RatioBucket::all()
            .iter()
            .copied()
            .zip(ratio_probs)
            .map(|(ratio_bucket, probability)| WeightedValue {
                value: ratio_bucket,
                probability,
            })
            .collect(),
        ratio_residual_probability,
    );
    let answer_distribution = higher_ranked_distribution
        .product(&ratio_distribution, |higher_ranked, ratio| {
            PairwiseAnswer::observation(*higher_ranked, *ratio)
        });
    let signed_ln_ratio_distribution =
        SignedLogRatioDistribution::from_answer_distribution(&answer_distribution);
    let top_prob = answer_distribution.probability_of(|answer| *answer == selected_answer);
    let neighborhood_prob = answer_distribution.probability_of(|answer| {
        answer.preferred_side() == Some(selected_higher_ranked)
            && answer
                .ratio_bucket()
                .map(|bucket| neighbor_indices.contains(&bucket.index()))
                .unwrap_or(false)
    });
    let neighborhood_prob = neighborhood_prob.clamp(0.0, 1.0);
    let confidence = ConfidenceSource::Logprob {
        entropy: answer_distribution.entropy(),
        top_prob,
        neighborhood_prob,
    };

    Some(PairwiseLogprobPosterior {
        selected_answer,
        selected_higher_ranked,
        selected_ratio,
        selected_ratio_bucket,
        higher_ranked_distribution,
        ratio_distribution,
        answer_distribution,
        signed_ln_ratio_distribution,
        confidence,
    })
}

/// Extract confidence from logprob distribution over a ratio ladder.
///
/// Given the output logprobs and the set of valid ratio tokens, compute
/// confidence metrics from the token probability distribution.
pub fn confidence_from_logprobs(
    logprobs: &[TokenLogprob],
    selected_higher_ranked: PairwisePreferredSide,
    selected_ratio: f64,
    ratio_ladder: &[f64],
) -> Option<ConfidenceSource> {
    pairwise_logprob_posterior(
        logprobs,
        selected_higher_ranked,
        selected_ratio,
        ratio_ladder,
    )
    .map(|posterior| posterior.confidence)
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
