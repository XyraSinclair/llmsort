//! Model selection policies for dynamic LLM routing during rerank.

use std::fmt;

/// Context available to model policies when selecting a model for a pair.
#[derive(Debug, Clone)]
pub struct ModelPolicyContext<'a> {
    /// Current global top-k error estimate.
    pub global_topk_error: f64,
    /// Number of comparisons attempted so far.
    pub comparisons_attempted: usize,
    /// Number of comparisons that yielded observations.
    pub comparisons_used: usize,
    /// Number of comparisons attempted for this attribute.
    pub attribute_comparisons_attempted: usize,
    /// Number of comparisons used (observations) for this attribute.
    pub attribute_comparisons_used: usize,
    /// Attribute being compared.
    pub attribute_id: &'a str,
    /// Index of entity A.
    pub i: usize,
    /// Index of entity B.
    pub j: usize,
    /// Current attribute score means (latent space), if available.
    pub attribute_scores: Option<&'a [f64]>,
    /// Current attribute standard deviations, if available.
    pub attribute_stds: Option<&'a [f64]>,
}

/// Policy that chooses a model for each comparison.
pub trait ModelPolicy: Send + Sync {
    fn select_model(&self, ctx: &ModelPolicyContext<'_>) -> String;

    fn describe(&self) -> Option<String> {
        None
    }
}

/// A simple 3-tier ladder policy (high → mid → low) based on uncertainty.
#[derive(Debug, Clone)]
pub struct ModelLadderPolicy {
    pub high_model: String,
    pub mid_model: Option<String>,
    pub low_model: String,
    /// Switch away from high quality once global error is below this.
    pub global_error_switch: f64,
    /// Treat pairs as "similar" if |delta| <= this (latent units).
    pub similarity_ln_ratio: f64,
    /// Only downgrade if pair std is below this threshold.
    pub max_pair_std: f64,
    /// Require at least this many observations before downgrading.
    pub min_comparisons: usize,
}

impl Default for ModelLadderPolicy {
    fn default() -> Self {
        Self {
            high_model: "anthropic/claude-fable-5".to_string(),
            mid_model: Some("anthropic/claude-opus-4.6".to_string()),
            low_model: "openai/gpt-5.6-luna".to_string(),
            global_error_switch: 0.10,
            similarity_ln_ratio: 0.12,
            max_pair_std: 0.60,
            min_comparisons: 12,
        }
    }
}

impl ModelLadderPolicy {
    fn pair_delta_and_std(ctx: &ModelPolicyContext<'_>) -> Option<(f64, f64)> {
        let scores = ctx.attribute_scores?;
        let stds = ctx.attribute_stds?;
        if ctx.i >= scores.len() || ctx.j >= scores.len() {
            return None;
        }
        let std_i = stds.get(ctx.i).copied().unwrap_or(0.0);
        let std_j = stds.get(ctx.j).copied().unwrap_or(0.0);
        let delta = scores[ctx.i] - scores[ctx.j];
        let pair_std = (std_i * std_i + std_j * std_j).sqrt();
        Some((delta, pair_std))
    }
}

impl ModelPolicy for ModelLadderPolicy {
    fn select_model(&self, ctx: &ModelPolicyContext<'_>) -> String {
        if ctx.attribute_comparisons_used < self.min_comparisons {
            return self.high_model.clone();
        }

        if ctx.global_topk_error > self.global_error_switch {
            return self.high_model.clone();
        }

        let Some((delta, pair_std)) = Self::pair_delta_and_std(ctx) else {
            return self.high_model.clone();
        };
        if !delta.is_finite() || !pair_std.is_finite() {
            return self.high_model.clone();
        }
        if delta.abs() <= self.similarity_ln_ratio && pair_std <= self.max_pair_std {
            return self.low_model.clone();
        }

        if let Some(mid) = &self.mid_model {
            return mid.clone();
        }

        self.low_model.clone()
    }

    fn describe(&self) -> Option<String> {
        Some(self.to_string())
    }
}

impl fmt::Display for ModelLadderPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ModelLadderPolicy(high={}, mid={}, low={}, switch_error={}, similarity_ln_ratio={}, max_pair_std={}, min_comparisons={})",
            self.high_model,
            self.mid_model.as_deref().unwrap_or("none"),
            self.low_model,
            self.global_error_switch,
            self.similarity_ln_ratio,
            self.max_pair_std,
            self.min_comparisons
        )
    }
}
