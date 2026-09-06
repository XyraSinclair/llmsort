use llmsort::rerank::{
    validate_multi_rerank_request, MultiRerankAttributeSpec, MultiRerankEntity, MultiRerankRequest,
};

fn base_request() -> MultiRerankRequest {
    MultiRerankRequest {
        nonce_draws: None,
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
            prompt: "quality".to_string(),
            prompt_template_slug: Some("ordinal_v1".to_string()),
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
fn ordinal_prompt_template_slug_validates() {
    validate_multi_rerank_request(&base_request()).expect("ordinal_v1 should validate");
}

#[test]
fn unknown_prompt_template_slug_still_rejected() {
    let mut req = base_request();
    req.attributes[0].prompt_template_slug = Some("ordinal_v9".to_string());
    assert!(validate_multi_rerank_request(&req).is_err());
}
