use super::request::{MAX_ATTRIBUTES, MAX_COMPARISON_CONCURRENCY, MAX_ENTITIES};
use super::*;
use crate::rating_engine::Config as EngineConfig;
use crate::rerank::options::RerankRunOptions;
use crate::rerank::types::{
    MultiRerankAttributeSpec, MultiRerankEntity, MultiRerankGateSpec, MultiRerankRequest,
    MultiRerankTopKSpec,
};

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
            id: "attr".to_string(),
            prompt: "prompt".to_string(),
            prompt_template_slug: None,
            weight: 1.0,
        }],
        topk: MultiRerankTopKSpec {
            k: 1,
            weight_exponent: 1.3,
            tolerated_error: 0.1,
            band_size: 5,
            effective_resistance_max_active: 64,
            stop_sigma_inflate: 1.25,
            stop_min_consecutive: 2,
            min_explore_degree: 2,
            prune_p_topk_below: None,
        },
        gates: Vec::new(),
        comparison_budget: Some(1),
        latency_budget_ms: None,
        max_cost_nanodollars: None,
        model: None,
        rater_id: None,
        comparison_concurrency: None,
        max_pair_repeats: None,
        randomize_presentation_order: true,
        counterbalance_pairs: false,
    }
}

#[test]
fn validate_rejects_unknown_gate_attribute() {
    let mut req = base_request();
    req.gates.push(MultiRerankGateSpec {
        attribute_id: "missing".to_string(),
        unit: "latent".to_string(),
        op: ">=".to_string(),
        threshold: 0.0,
    });
    let err = validate_multi_rerank_request(&req).unwrap_err();
    assert!(matches!(err, MultiRerankError::InvalidRequest(_)));
}

#[test]
fn validate_rejects_unknown_gate_unit() {
    let mut req = base_request();
    req.gates.push(MultiRerankGateSpec {
        attribute_id: "attr".to_string(),
        unit: "wat".to_string(),
        op: ">=".to_string(),
        threshold: 0.0,
    });
    let err = validate_multi_rerank_request(&req).unwrap_err();
    assert!(matches!(err, MultiRerankError::InvalidRequest(_)));
}

#[test]
fn validate_accepts_case_insensitive_gate_unit() {
    let mut req = base_request();
    req.gates.push(MultiRerankGateSpec {
        attribute_id: "attr".to_string(),
        unit: "Percentile".to_string(),
        op: ">=".to_string(),
        threshold: 0.5,
    });
    validate_multi_rerank_request(&req).unwrap();
}

#[test]
fn validate_rejects_percentile_threshold_out_of_range() {
    let mut req = base_request();
    req.gates.push(MultiRerankGateSpec {
        attribute_id: "attr".to_string(),
        unit: "percentile".to_string(),
        op: ">=".to_string(),
        threshold: 1.1,
    });
    let err = validate_multi_rerank_request(&req).unwrap_err();
    assert!(matches!(err, MultiRerankError::InvalidRequest(_)));
}

#[test]
fn validate_rejects_duplicate_attribute_ids() {
    let mut req = base_request();
    req.attributes.push(MultiRerankAttributeSpec {
        id: "attr".to_string(),
        prompt: "prompt2".to_string(),
        prompt_template_slug: None,
        weight: 1.0,
    });
    let err = validate_multi_rerank_request(&req).unwrap_err();
    assert!(matches!(err, MultiRerankError::InvalidRequest(_)));
}

#[test]
fn validate_rejects_duplicate_attribute_definitions() {
    let mut req = base_request();
    req.attributes.push(MultiRerankAttributeSpec {
        id: "attr-copy".to_string(),
        prompt: req.attributes[0].prompt.clone(),
        prompt_template_slug: Some("canonical_v2".to_string()),
        weight: 1.0,
    });
    let err = validate_multi_rerank_request(&req).unwrap_err();
    assert!(
        matches!(err, MultiRerankError::InvalidRequest(message) if message.contains("duplicate attribute definition"))
    );
}

#[test]
fn validate_rejects_negative_attribute_weight_exponent() {
    let mut req = base_request();
    req.topk.weight_exponent = -1.0;
    let err = validate_multi_rerank_request(&req).unwrap_err();
    assert!(
        matches!(err, MultiRerankError::InvalidRequest(message) if message.contains("weight_exponent"))
    );
}

#[test]
fn attribute_weight_exponent_does_not_change_rank_weighting() {
    let mut req = base_request();
    req.topk.k = 2;
    req.topk.weight_exponent = 2.5;
    let (_, topk) = build_trait_search_config(&req);
    let config = build_engine_config(&RerankRunOptions::default(), &topk);
    assert_eq!(
        config.rank_weight_exponent,
        EngineConfig::default().rank_weight_exponent
    );
    assert!((config.tail_weight - 0.5).abs() < 1e-12);
}

#[test]
fn validate_rejects_topk_k_gt_n() {
    let mut req = base_request();
    req.topk.k = 3;
    let err = validate_multi_rerank_request(&req).unwrap_err();
    assert!(matches!(err, MultiRerankError::InvalidRequest(_)));
}

#[test]
fn validate_rejects_concurrency_zero() {
    let mut req = base_request();
    req.comparison_concurrency = Some(0);
    let err = validate_multi_rerank_request(&req).unwrap_err();
    assert!(matches!(err, MultiRerankError::InvalidRequest(_)));
}

#[test]
fn validate_rejects_concurrency_too_high() {
    let mut req = base_request();
    req.comparison_concurrency = Some(MAX_COMPARISON_CONCURRENCY + 1);
    let err = validate_multi_rerank_request(&req).unwrap_err();
    assert!(matches!(err, MultiRerankError::InvalidRequest(_)));
}

#[test]
fn validate_rejects_max_pair_repeats_zero() {
    let mut req = base_request();
    req.max_pair_repeats = Some(0);
    let err = validate_multi_rerank_request(&req).unwrap_err();
    assert!(matches!(err, MultiRerankError::InvalidRequest(_)));
}

#[test]
fn validate_rejects_duplicate_entity_ids() {
    let mut req = base_request();
    req.entities.push(MultiRerankEntity {
        id: "a".to_string(),
        text: "A2".to_string(),
    });
    let err = validate_multi_rerank_request(&req).unwrap_err();
    assert!(matches!(err, MultiRerankError::InvalidRequest(_)));
}

#[test]
fn validate_accepts_canonical_prompt_template_slug() {
    let mut req = base_request();
    req.attributes[0].prompt_template_slug = Some("canonical_v2".to_string());
    validate_multi_rerank_request(&req).expect("canonical_v2 should validate");
}

#[test]
fn validate_rejects_empty_prompt_template_slug() {
    let mut req = base_request();
    req.attributes[0].prompt_template_slug = Some("".to_string());
    let err = validate_multi_rerank_request(&req).unwrap_err();
    assert!(matches!(err, MultiRerankError::InvalidRequest(_)));
}

#[test]
fn validate_rejects_unknown_prompt_template_slug() {
    let mut req = base_request();
    req.attributes[0].prompt_template_slug = Some("canonical_v1".to_string());
    let err = validate_multi_rerank_request(&req).unwrap_err();
    assert!(matches!(err, MultiRerankError::InvalidRequest(_)));
}

#[test]
fn validate_rejects_unsupported_gate_op() {
    let mut req = base_request();
    req.gates.push(MultiRerankGateSpec {
        attribute_id: "attr".to_string(),
        unit: "latent".to_string(),
        op: "=".to_string(),
        threshold: 0.0,
    });
    let err = validate_multi_rerank_request(&req).unwrap_err();
    assert!(matches!(err, MultiRerankError::InvalidRequest(_)));
}

#[test]
fn validate_rejects_entities_len_lt_2() {
    let mut req = base_request();
    req.entities.truncate(1);
    let err = validate_multi_rerank_request(&req).unwrap_err();
    assert!(matches!(err, MultiRerankError::InvalidRequest(_)));
}

#[test]
fn validate_rejects_empty_attributes() {
    let mut req = base_request();
    req.attributes.clear();
    let err = validate_multi_rerank_request(&req).unwrap_err();
    assert!(matches!(err, MultiRerankError::InvalidRequest(_)));
}

#[test]
fn validate_rejects_too_many_entities() {
    let mut req = base_request();
    req.entities = (0..(MAX_ENTITIES + 1))
        .map(|i| MultiRerankEntity {
            id: format!("e{i}"),
            text: "x".to_string(),
        })
        .collect();
    let err = validate_multi_rerank_request(&req).unwrap_err();
    assert!(matches!(err, MultiRerankError::InvalidRequest(_)));
}

#[test]
fn validate_rejects_too_many_attributes() {
    let mut req = base_request();
    req.attributes = (0..(MAX_ATTRIBUTES + 1))
        .map(|i| MultiRerankAttributeSpec {
            id: format!("a{i}"),
            prompt: "p".to_string(),
            prompt_template_slug: None,
            weight: 1.0,
        })
        .collect();
    let err = validate_multi_rerank_request(&req).unwrap_err();
    assert!(matches!(err, MultiRerankError::InvalidRequest(_)));
}

#[test]
fn validate_rejects_zero_band_size() {
    let mut req = base_request();
    req.topk.band_size = 0;
    let err = validate_multi_rerank_request(&req).unwrap_err();
    assert!(matches!(err, MultiRerankError::InvalidRequest(_)));
}

#[test]
fn validate_rejects_nan_tolerated_error() {
    let mut req = base_request();
    req.topk.tolerated_error = f64::NAN;
    let err = validate_multi_rerank_request(&req).unwrap_err();
    assert!(matches!(err, MultiRerankError::InvalidRequest(_)));
}
