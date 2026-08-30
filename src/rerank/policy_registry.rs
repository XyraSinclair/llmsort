//! Policy registry and config loader for model routing.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::model_policy::{ModelLadderPolicy, ModelPolicy};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PolicySpec {
    Ladder {
        high_model: Option<String>,
        mid_model: Option<String>,
        low_model: Option<String>,
        global_error_switch: Option<f64>,
        similarity_ln_ratio: Option<f64>,
        max_pair_std: Option<f64>,
        min_comparisons: Option<usize>,
    },
    Fixed {
        model: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyConfig {
    pub name: Option<String>,
    pub policy: PolicySpec,
}

pub struct PolicyRegistry {
    policies: HashMap<String, Arc<dyn ModelPolicy>>,
}

impl Default for PolicyRegistry {
    fn default() -> Self {
        let mut policies: HashMap<String, Arc<dyn ModelPolicy>> = HashMap::new();
        policies.insert(
            "ladder_default".to_string(),
            Arc::new(ModelLadderPolicy::default()),
        );
        policies.insert(
            "fast_only".to_string(),
            Arc::new(FixedPolicy::new("openai/gpt-5.6-luna")),
        );
        policies.insert(
            "cost_aware_fast".to_string(),
            Arc::new(FixedPolicy::new("deepseek/deepseek-v4-flash")),
        );
        policies.insert(
            "quality_only".to_string(),
            Arc::new(FixedPolicy::new("anthropic/claude-fable-5")),
        );
        policies.insert(
            "frontier_ladder".to_string(),
            Arc::new(ModelLadderPolicy {
                high_model: "anthropic/claude-fable-5".to_string(),
                mid_model: Some("google/gemini-3.1-pro-preview".to_string()),
                low_model: "openai/gpt-5.6-luna".to_string(),
                ..ModelLadderPolicy::default()
            }),
        );
        Self { policies }
    }
}

impl PolicyRegistry {
    pub fn list(&self) -> Vec<String> {
        let mut keys: Vec<String> = self.policies.keys().cloned().collect();
        keys.sort();
        keys
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn ModelPolicy>> {
        self.policies.get(name).cloned()
    }

    pub fn insert(&mut self, name: impl Into<String>, policy: Arc<dyn ModelPolicy>) {
        self.policies.insert(name.into(), policy);
    }
}

pub fn policy_from_spec(spec: &PolicySpec) -> Result<Arc<dyn ModelPolicy>, String> {
    validate_policy_spec(spec)?;
    Ok(match spec {
        PolicySpec::Fixed { model } => Arc::new(FixedPolicy::new(model)),
        PolicySpec::Ladder {
            high_model,
            mid_model,
            low_model,
            global_error_switch,
            similarity_ln_ratio,
            max_pair_std,
            min_comparisons,
        } => {
            let mut policy = ModelLadderPolicy::default();
            if let Some(v) = high_model {
                policy.high_model = v.clone();
            }
            if let Some(v) = mid_model {
                policy.mid_model = Some(v.clone());
            }
            if let Some(v) = low_model {
                policy.low_model = v.clone();
            }
            if let Some(v) = global_error_switch {
                policy.global_error_switch = *v;
            }
            if let Some(v) = similarity_ln_ratio {
                policy.similarity_ln_ratio = *v;
            }
            if let Some(v) = max_pair_std {
                policy.max_pair_std = *v;
            }
            if let Some(v) = min_comparisons {
                policy.min_comparisons = *v;
            }
            Arc::new(policy)
        }
    })
}

pub fn load_policy_from_path(path: impl AsRef<Path>) -> Result<Arc<dyn ModelPolicy>, String> {
    let raw = std::fs::read_to_string(path.as_ref())
        .map_err(|e| format!("failed to read policy config: {e}"))?;
    let config: PolicyConfig =
        serde_json::from_str(&raw).map_err(|e| format!("failed to parse policy config: {e}"))?;
    policy_from_spec(&config.policy)
}

fn validate_policy_spec(spec: &PolicySpec) -> Result<(), String> {
    match spec {
        PolicySpec::Fixed { model } => {
            if model.trim().is_empty() {
                return Err("fixed policy model must be non-empty".to_string());
            }
        }
        PolicySpec::Ladder {
            high_model,
            mid_model,
            low_model,
            global_error_switch,
            similarity_ln_ratio,
            max_pair_std,
            min_comparisons,
        } => {
            if let Some(v) = high_model {
                if v.trim().is_empty() {
                    return Err("high_model must be non-empty".to_string());
                }
            }
            if let Some(v) = low_model {
                if v.trim().is_empty() {
                    return Err("low_model must be non-empty".to_string());
                }
            }
            if let Some(v) = mid_model {
                if v.trim().is_empty() {
                    return Err("mid_model must be non-empty when provided".to_string());
                }
            }
            if let Some(v) = global_error_switch {
                if !(0.0..=1.0).contains(v) {
                    return Err("global_error_switch must be in [0,1]".to_string());
                }
            }
            if let Some(v) = similarity_ln_ratio {
                if *v < 0.0 {
                    return Err("similarity_ln_ratio must be >= 0".to_string());
                }
            }
            if let Some(v) = max_pair_std {
                if *v < 0.0 {
                    return Err("max_pair_std must be >= 0".to_string());
                }
            }
            if let Some(v) = min_comparisons {
                if *v == 0 {
                    return Err("min_comparisons must be >= 1".to_string());
                }
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct FixedPolicy {
    model: String,
}

impl FixedPolicy {
    fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
        }
    }
}

impl super::model_policy::ModelPolicy for FixedPolicy {
    fn select_model(&self, _ctx: &super::model_policy::ModelPolicyContext<'_>) -> String {
        self.model.clone()
    }

    fn describe(&self) -> Option<String> {
        Some(format!("FixedPolicy(model={})", self.model))
    }
}
