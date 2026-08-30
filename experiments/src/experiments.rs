//! Request expansion helpers for prompt and attribute experiments.
//!
//! These helpers do not call a model. They turn one validated rerank request into
//! an explicit prompt matrix so list-sorting experiments are reproducible instead
//! of living in ad hoc notebooks.

use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

use llmsort::prompts::prompt_by_slug;

use llmsort::rerank::multi::validate_multi_rerank_request;
use llmsort::rerank::types::{MultiRerankAttributeSpec, MultiRerankRequest};

/// Attribute direction for an experiment variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AttributePolarity {
    /// More of this attribute should move an entity in the same direction as the
    /// source attribute weight.
    Positive,
    /// More of this attribute should move an entity in the opposite direction
    /// from the source attribute weight.
    Negative,
}

fn default_positive_polarity() -> AttributePolarity {
    AttributePolarity::Positive
}

/// One extra positive or negative phrasing to try for a source attribute.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AttributeVariantSpec {
    /// Existing attribute whose weight and default template this variant inherits.
    pub source_attribute_id: String,
    /// New attribute id before template/polarity suffixes are applied.
    pub id: String,
    /// Natural-language attribute prompt shown to the model.
    pub prompt: String,
    /// Whether the variant points in the same or opposite utility direction.
    #[serde(default = "default_positive_polarity")]
    pub polarity: AttributePolarity,
    /// Final utility weight.  If omitted, positive variants inherit the source
    /// weight and negative variants invert it.
    #[serde(default)]
    pub weight: Option<f64>,
    /// Optional template override for this variant only.  If omitted, the matrix
    /// templates are used.
    #[serde(default)]
    pub prompt_template_slug: Option<String>,
}

/// Configuration for expanding a request into a prompt/attribute matrix.
#[derive(Debug, Clone, Default)]
pub struct PromptExperimentConfig {
    /// Prompt templates to apply to generated variants.  Empty preserves each
    /// source attribute's template, or `canonical_v2` when the source omits it.
    pub prompt_template_slugs: Vec<String>,
    /// Add an automatic negative version of every source attribute.
    pub include_negative: bool,
    /// Extra user-specified related attributes to include.
    pub variants: Vec<AttributeVariantSpec>,
}

#[derive(Debug)]
pub enum PromptExperimentError {
    UnknownPromptTemplate { slug: String },
    UnknownSourceAttribute { source_attribute_id: String },
    DuplicateAttributeId { id: String },
    InvalidExpandedRequest { message: String },
}

impl fmt::Display for PromptExperimentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownPromptTemplate { slug } => {
                write!(
                    f,
                    "unknown prompt_template_slug in experiment matrix: {slug}"
                )
            }
            Self::UnknownSourceAttribute {
                source_attribute_id,
            } => write!(
                f,
                "attribute variant references unknown source_attribute_id: {source_attribute_id}"
            ),
            Self::DuplicateAttributeId { id } => {
                write!(f, "experiment matrix produced duplicate attribute id: {id}")
            }
            Self::InvalidExpandedRequest { message } => {
                write!(f, "expanded experiment request is invalid: {message}")
            }
        }
    }
}

impl Error for PromptExperimentError {}

/// Expand one rerank request into an explicit prompt/attribute experiment matrix.
pub fn expand_prompt_experiment_request(
    req: &MultiRerankRequest,
    cfg: &PromptExperimentConfig,
) -> Result<MultiRerankRequest, PromptExperimentError> {
    let template_slugs = cfg.prompt_template_slugs.clone();
    for slug in &template_slugs {
        if prompt_by_slug(slug).is_none() {
            return Err(PromptExperimentError::UnknownPromptTemplate { slug: slug.clone() });
        }
    }

    let source_by_id: HashMap<&str, &MultiRerankAttributeSpec> = req
        .attributes
        .iter()
        .map(|attr| (attr.id.as_str(), attr))
        .collect();

    for variant in &cfg.variants {
        if !source_by_id.contains_key(variant.source_attribute_id.as_str()) {
            return Err(PromptExperimentError::UnknownSourceAttribute {
                source_attribute_id: variant.source_attribute_id.clone(),
            });
        }
        if let Some(slug) = variant.prompt_template_slug.as_deref() {
            if prompt_by_slug(slug).is_none() {
                return Err(PromptExperimentError::UnknownPromptTemplate {
                    slug: slug.to_string(),
                });
            }
        }
    }

    let mut attributes = Vec::new();
    let mut ids = HashSet::new();

    for source in &req.attributes {
        push_variant_for_templates(
            &mut attributes,
            &mut ids,
            ExpandedAttributeVariant {
                source,
                id_seed: &source.id,
                prompt: &source.prompt,
                polarity: AttributePolarity::Positive,
                weight: source.weight,
                template_slugs: &template_slugs,
            },
        )?;

        if cfg.include_negative {
            let id_seed = format!("{}_negative", source.id);
            let prompt = format!("lack of {}", source.prompt);
            push_variant_for_templates(
                &mut attributes,
                &mut ids,
                ExpandedAttributeVariant {
                    source,
                    id_seed: &id_seed,
                    prompt: &prompt,
                    polarity: AttributePolarity::Negative,
                    weight: -source.weight,
                    template_slugs: &template_slugs,
                },
            )?;
        }
    }

    for variant in &cfg.variants {
        let source = source_by_id[variant.source_attribute_id.as_str()];
        let weight = variant.weight.unwrap_or(match variant.polarity {
            AttributePolarity::Positive => source.weight,
            AttributePolarity::Negative => -source.weight,
        });
        let templates = match &variant.prompt_template_slug {
            Some(slug) => vec![slug.clone()],
            None => template_slugs.clone(),
        };
        push_variant_for_templates(
            &mut attributes,
            &mut ids,
            ExpandedAttributeVariant {
                source,
                id_seed: &variant.id,
                prompt: &variant.prompt,
                polarity: variant.polarity,
                weight,
                template_slugs: &templates,
            },
        )?;
    }

    let mut expanded = req.clone();
    expanded.attributes = attributes;
    validate_multi_rerank_request(&expanded).map_err(|err| {
        PromptExperimentError::InvalidExpandedRequest {
            message: err.to_string(),
        }
    })?;
    Ok(expanded)
}

struct ExpandedAttributeVariant<'a> {
    source: &'a MultiRerankAttributeSpec,
    id_seed: &'a str,
    prompt: &'a str,
    polarity: AttributePolarity,
    weight: f64,
    template_slugs: &'a [String],
}

fn push_variant_for_templates(
    attributes: &mut Vec<MultiRerankAttributeSpec>,
    ids: &mut HashSet<String>,
    variant: ExpandedAttributeVariant<'_>,
) -> Result<(), PromptExperimentError> {
    let ExpandedAttributeVariant {
        source,
        id_seed,
        prompt,
        polarity,
        weight,
        template_slugs,
    } = variant;
    for slug in template_slugs {
        let id = experiment_attribute_id(id_seed, polarity, slug);
        if !ids.insert(id.clone()) {
            return Err(PromptExperimentError::DuplicateAttributeId { id });
        }
        attributes.push(MultiRerankAttributeSpec {
            id,
            prompt: prompt.to_string(),
            prompt_template_slug: Some(slug.clone()),
            weight,
        });
    }

    if template_slugs.is_empty() {
        let slug = source
            .prompt_template_slug
            .clone()
            .unwrap_or_else(|| "canonical_v2".to_string());
        let id = experiment_attribute_id(id_seed, polarity, &slug);
        if !ids.insert(id.clone()) {
            return Err(PromptExperimentError::DuplicateAttributeId { id });
        }
        attributes.push(MultiRerankAttributeSpec {
            id,
            prompt: prompt.to_string(),
            prompt_template_slug: Some(slug),
            weight,
        });
    }

    Ok(())
}

fn experiment_attribute_id(seed: &str, polarity: AttributePolarity, template_slug: &str) -> String {
    let polarity = match polarity {
        AttributePolarity::Positive => "pos",
        AttributePolarity::Negative => "neg",
    };
    format!(
        "{}__{}__{}",
        sanitize_id_component(seed),
        polarity,
        sanitize_id_component(template_slug)
    )
}

fn sanitize_id_component(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut last_was_sep = false;
    for ch in raw.chars() {
        let next = if ch.is_ascii_alphanumeric() {
            last_was_sep = false;
            Some(ch.to_ascii_lowercase())
        } else if ch == '_' || ch == '-' {
            if last_was_sep {
                None
            } else {
                last_was_sep = true;
                Some('_')
            }
        } else if last_was_sep {
            None
        } else {
            last_was_sep = true;
            Some('_')
        };
        if let Some(ch) = next {
            out.push(ch);
        }
    }
    let trimmed = out.trim_matches('_').to_string();
    if trimmed.is_empty() {
        "variant".to_string()
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use llmsort::rerank::types::MultiRerankEntity;

    fn base_request() -> MultiRerankRequest {
        MultiRerankRequest {
            entities: vec![
                MultiRerankEntity {
                    id: "a".to_string(),
                    text: "A".to_string(),
                },
                MultiRerankEntity {
                    id: "b".to_string(),
                    text: "B".to_string(),
                },
            ],
            attributes: vec![MultiRerankAttributeSpec {
                id: "quality".to_string(),
                prompt: "quality of reasoning".to_string(),
                prompt_template_slug: None,
                weight: 1.0,
            }],
            topk: serde_json::from_str(r#"{"k":1}"#).unwrap(),
            gates: vec![],
            comparison_budget: Some(4),
            latency_budget_ms: None,
            max_cost_nanodollars: None,
            model: None,
            rater_id: None,
            comparison_concurrency: Some(1),
            max_pair_repeats: Some(1),
            randomize_presentation_order: true,
            counterbalance_pairs: false,
        }
    }

    #[test]
    fn expands_positive_negative_templates_and_aliases() {
        let cfg = PromptExperimentConfig {
            prompt_template_slugs: vec![
                "canonical_v2".to_string(),
                "canonical_bucket_v1".to_string(),
            ],
            include_negative: true,
            variants: vec![AttributeVariantSpec {
                source_attribute_id: "quality".to_string(),
                id: "clarity".to_string(),
                prompt: "clarity and readability".to_string(),
                polarity: AttributePolarity::Positive,
                weight: None,
                prompt_template_slug: Some("canonical_v2".to_string()),
            }],
        };

        let expanded = expand_prompt_experiment_request(&base_request(), &cfg).unwrap();
        let ids: Vec<&str> = expanded
            .attributes
            .iter()
            .map(|attr| attr.id.as_str())
            .collect();
        assert_eq!(expanded.attributes.len(), 5);
        assert!(ids.contains(&"quality__pos__canonical_v2"));
        assert!(ids.contains(&"quality__pos__canonical_bucket_v1"));
        assert!(ids.contains(&"quality_negative__neg__canonical_v2"));
        assert!(ids.contains(&"quality_negative__neg__canonical_bucket_v1"));
        assert!(ids.contains(&"clarity__pos__canonical_v2"));
        let negative = expanded
            .attributes
            .iter()
            .find(|attr| attr.id == "quality_negative__neg__canonical_v2")
            .unwrap();
        assert!(negative.weight < 0.0);
        assert_eq!(negative.prompt, "lack of quality of reasoning");
    }

    #[test]
    fn empty_template_matrix_preserves_source_template() {
        let mut req = base_request();
        req.attributes[0].prompt_template_slug = Some("canonical_bucket_v1".to_string());

        let expanded = expand_prompt_experiment_request(&req, &PromptExperimentConfig::default())
            .expect("default prompt expansion should preserve source templates");

        assert_eq!(expanded.attributes.len(), 1);
        assert_eq!(
            expanded.attributes[0].id,
            "quality__pos__canonical_bucket_v1"
        );
        assert_eq!(
            expanded.attributes[0].prompt_template_slug.as_deref(),
            Some("canonical_bucket_v1")
        );
    }

    #[test]
    fn expands_ordinal_template_slug() {
        let cfg = PromptExperimentConfig {
            prompt_template_slugs: vec!["ordinal_v1".to_string()],
            include_negative: false,
            variants: vec![],
        };

        let expanded = expand_prompt_experiment_request(&base_request(), &cfg).unwrap();

        assert_eq!(expanded.attributes.len(), 1);
        assert_eq!(expanded.attributes[0].id, "quality__pos__ordinal_v1");
        assert_eq!(
            expanded.attributes[0].prompt_template_slug.as_deref(),
            Some("ordinal_v1")
        );
    }

    #[test]
    fn rejects_unknown_template_slug() {
        let cfg = PromptExperimentConfig {
            prompt_template_slugs: vec!["canonical_v9".to_string()],
            include_negative: false,
            variants: vec![],
        };

        let err = expand_prompt_experiment_request(&base_request(), &cfg).unwrap_err();
        assert!(matches!(
            err,
            PromptExperimentError::UnknownPromptTemplate { slug } if slug == "canonical_v9"
        ));
    }
}
