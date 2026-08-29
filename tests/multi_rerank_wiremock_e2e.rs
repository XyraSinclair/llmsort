use std::sync::Arc;
use std::time::Duration;

use llmsort::gateway::openrouter::OpenRouterAdapter;
use llmsort::gateway::{Attribution, GatewayConfig, NoopUsageSink, ProviderGateway};
use llmsort::rerank::{
    multi_rerank, MultiRerankAttributeSpec, MultiRerankEntity, MultiRerankRequest,
    MultiRerankTopKSpec,
};
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

#[derive(Clone, Copy)]
struct DeterministicJudge;

#[derive(Clone, Copy)]
struct DeterministicOrdinalJudge;

fn extract_between<'a>(s: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let start_idx = s.find(start)? + start.len();
    let rest = &s[start_idx..];
    let end_idx = rest.find(end)?;
    Some(&rest[..end_idx])
}

fn score_for_context(ctx: &str) -> i32 {
    if ctx.contains("BEST") {
        3
    } else if ctx.contains("MID") {
        2
    } else if ctx.contains("WORST") {
        1
    } else {
        0
    }
}

impl Respond for DeterministicJudge {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let parsed: serde_json::Value = serde_json::from_slice(&request.body).unwrap_or_default();
        let messages = parsed
            .get("messages")
            .and_then(|m| m.as_array())
            .cloned()
            .unwrap_or_default();

        let user_content = messages
            .iter()
            .find(|m| m.get("role").and_then(|r| r.as_str()) == Some("user"))
            .and_then(|m| m.get("content").and_then(|c| c.as_str()))
            .unwrap_or("");

        let a_ctx = extract_between(user_content, "<entity_A_context>", "</entity_A_context>")
            .unwrap_or("")
            .trim();
        let b_ctx = extract_between(user_content, "<entity_B_context>", "</entity_B_context>")
            .unwrap_or("")
            .trim();

        let a_score = score_for_context(a_ctx);
        let b_score = score_for_context(b_ctx);

        let (higher, ratio) = if a_score >= b_score {
            let diff = (a_score - b_score).abs();
            ("A", if diff >= 2 { 3.0 } else { 1.3 })
        } else {
            let diff = (b_score - a_score).abs();
            ("B", if diff >= 2 { 3.0 } else { 1.3 })
        };

        let content = format!(r#"{{"higher_ranked":"{higher}","ratio":{ratio},"confidence":0.9}}"#);

        ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{
                "message": { "content": content },
                "finish_reason": "stop"
            }],
            "usage": { "prompt_tokens": 10, "completion_tokens": 10 }
        }))
    }
}

impl Respond for DeterministicOrdinalJudge {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let parsed: serde_json::Value = serde_json::from_slice(&request.body).unwrap_or_default();
        let messages = parsed
            .get("messages")
            .and_then(|m| m.as_array())
            .cloned()
            .unwrap_or_default();

        let user_content = messages
            .iter()
            .find(|m| m.get("role").and_then(|r| r.as_str()) == Some("user"))
            .and_then(|m| m.get("content").and_then(|c| c.as_str()))
            .unwrap_or("");

        let a_ctx = extract_between(user_content, "<entity_A_context>", "</entity_A_context>")
            .unwrap_or("")
            .trim();
        let b_ctx = extract_between(user_content, "<entity_B_context>", "</entity_B_context>")
            .unwrap_or("")
            .trim();

        let a_score = score_for_context(a_ctx);
        let b_score = score_for_context(b_ctx);
        let higher = if a_score >= b_score { "A" } else { "B" };
        let content = format!(r#"{{"higher_ranked":"{higher}","confidence":0.9}}"#);

        ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{
                "message": { "content": content },
                "finish_reason": "stop"
            }],
            "usage": { "prompt_tokens": 10, "completion_tokens": 10 }
        }))
    }
}

fn test_request(prompt_template_slug: &str) -> MultiRerankRequest {
    MultiRerankRequest {
        entities: vec![
            MultiRerankEntity {
                id: "best".into(),
                text: "BEST".into(),
            },
            MultiRerankEntity {
                id: "mid".into(),
                text: "MID".into(),
            },
            MultiRerankEntity {
                id: "worst".into(),
                text: "WORST".into(),
            },
        ],
        attributes: vec![MultiRerankAttributeSpec {
            id: "quality".into(),
            prompt: "quality".into(),
            prompt_template_slug: Some(prompt_template_slug.into()),
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
        randomize_presentation_order: false,
        counterbalance_pairs: false,
    }
}

#[tokio::test]
async fn multi_rerank_runs_end_to_end_against_wiremock_gateway() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(DeterministicJudge)
        .mount(&server)
        .await;

    let adapter =
        OpenRouterAdapter::with_config("sk-test", server.uri(), Duration::from_secs(5), None, None)
            .unwrap();
    let gateway = Arc::new(ProviderGateway::with_config(
        adapter,
        Arc::new(NoopUsageSink),
        GatewayConfig {
            max_retries: 0,
            retry_base_delay: Duration::from_millis(0),
        },
    ));

    let req = test_request("canonical_v2");

    let resp = multi_rerank(
        req,
        llmsort::rerank::RerankExecution::new(gateway, Attribution::new("test")),
    )
    .await
    .unwrap();

    assert_eq!(resp.entities[0].id, "best");
    assert_eq!(resp.entities[1].id, "mid");
    assert_eq!(resp.entities[2].id, "worst");
    assert_eq!(resp.meta.comparisons_refused, 0);
    assert!(resp.meta.comparisons_used > 0);

    let received = server.received_requests().await.unwrap();
    assert_eq!(received.len(), resp.meta.comparisons_attempted);
}

#[tokio::test]
async fn multi_rerank_runs_end_to_end_with_ordinal_prompt_template() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(DeterministicOrdinalJudge)
        .mount(&server)
        .await;

    let adapter =
        OpenRouterAdapter::with_config("sk-test", server.uri(), Duration::from_secs(5), None, None)
            .unwrap();
    let gateway = Arc::new(ProviderGateway::with_config(
        adapter,
        Arc::new(NoopUsageSink),
        GatewayConfig {
            max_retries: 0,
            retry_base_delay: Duration::from_millis(0),
        },
    ));

    let resp = multi_rerank(
        test_request("ordinal_v1"),
        llmsort::rerank::RerankExecution::new(gateway, Attribution::new("test")),
    )
    .await
    .unwrap();

    assert_eq!(resp.entities[0].id, "best");
    assert_eq!(resp.entities[1].id, "mid");
    assert_eq!(resp.entities[2].id, "worst");
    assert_eq!(resp.meta.comparisons_refused, 0);
    assert!(resp.meta.comparisons_used > 0);

    let received = server.received_requests().await.unwrap();
    assert_eq!(received.len(), resp.meta.comparisons_attempted);
}
