use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use llmsort::gateway::openrouter::OpenRouterAdapter;
use llmsort::gateway::{Attribution, GatewayConfig, NoopUsageSink, ProviderGateway};
use llmsort::rerank::{
    multi_rerank, MultiRerankAttributeSpec, MultiRerankEntity, MultiRerankRequest,
    MultiRerankTopKSpec, RerankStopReason,
};

#[tokio::test]
async fn multi_rerank_honors_cancel_flag_before_any_comparisons() {
    let adapter = OpenRouterAdapter::with_config(
        "sk-test",
        "http://127.0.0.1:9",
        Duration::from_secs(1),
        None,
        None,
    )
    .unwrap();
    let gateway = Arc::new(ProviderGateway::with_config(
        adapter,
        Arc::new(NoopUsageSink),
        GatewayConfig {
            max_retries: 0,
            retry_base_delay: Duration::from_millis(0),
        },
    ));

    let req = MultiRerankRequest {
        nonce_draws: None,
        entities: vec![
            MultiRerankEntity {
                id: "a".into(),
                text: "A".into(),
            },
            MultiRerankEntity {
                id: "b".into(),
                text: "B".into(),
            },
        ],
        attributes: vec![MultiRerankAttributeSpec {
            id: "attr".into(),
            prompt: "prompt".into(),
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
        gates: vec![],
        comparison_budget: Some(10),
        latency_budget_ms: None,
        max_cost_nanodollars: None,
        model: Some("openai/gpt-5-mini".into()),
        rater_id: None,
        comparison_concurrency: Some(1),
        max_pair_repeats: Some(1),
        randomize_presentation_order: true,
        counterbalance_pairs: false,
    };

    let cancel_flag = AtomicBool::new(true);
    let resp = multi_rerank(
        req,
        llmsort::rerank::RerankExecution::new(gateway, Attribution::new("test"))
            .cancel_flag(&cancel_flag),
    )
    .await
    .unwrap();

    assert_eq!(resp.meta.stop_reason, RerankStopReason::Cancelled);
    assert_eq!(resp.meta.comparisons_attempted, 0);
}
