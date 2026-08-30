//! Multi-attribute summary: Pareto front and attribute correlations.

use std::sync::Arc;
use std::time::Duration;

use llmsort::gateway::openrouter::OpenRouterAdapter;
use llmsort::gateway::{Attribution, GatewayConfig, NoopUsageSink, ProviderGateway};
use llmsort::rerank::{
    multi_rerank, MultiRerankAttributeSpec, MultiRerankEntity, MultiRerankRequest,
    MultiRerankTopKSpec, RerankExecution,
};
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

fn extract_between<'a>(s: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let i = s.find(start)? + start.len();
    let rest = &s[i..];
    let j = rest.find(end)?;
    Some(&rest[..j])
}

/// Two attributes with a PLANTED TRADE-OFF: "stars" counts '*', "moons"
/// counts 'o'. Entities are constructed so no single entity maximizes both.
#[derive(Clone, Copy)]
struct TwoAxisJudge;

fn count(ctx: &str, needle: char) -> i64 {
    ctx.chars().filter(|&c| c == needle).count() as i64
}

impl Respond for TwoAxisJudge {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap_or_default();
        let user = body["messages"]
            .as_array()
            .and_then(|m| {
                m.iter()
                    .find(|x| x["role"] == "user")
                    .and_then(|x| x["content"].as_str())
            })
            .unwrap_or("")
            .to_string();
        let a = extract_between(&user, "<entity_A_context>", "</entity_A_context>").unwrap_or("");
        let b = extract_between(&user, "<entity_B_context>", "</entity_B_context>").unwrap_or("");
        let needle = if user.contains("count of moons") {
            'o'
        } else {
            '*'
        };
        let d = count(a, needle) - count(b, needle);
        let (higher, ratio) = if d >= 0 {
            (
                "A",
                if d.abs() >= 2 {
                    3.9
                } else if d == 0 {
                    1.0
                } else {
                    1.5
                },
            )
        } else {
            ("B", if d.abs() >= 2 { 3.9 } else { 1.5 })
        };
        let content = format!(r#"{{"higher_ranked":"{higher}","ratio":{ratio},"confidence":0.9}}"#);
        ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{ "message": { "content": content }, "finish_reason": "stop" }],
            "usage": { "prompt_tokens": 10, "completion_tokens": 10 }
        }))
    }
}

#[tokio::test]
async fn pareto_front_and_correlations_reflect_a_planted_trade_off() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(TwoAxisJudge)
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

    // Trade-off roster: high-star/low-moon, balanced, low-star/high-moon,
    // and one strictly dominated straggler.
    let entities = [
        ("s", "*****"),   // stars champion
        ("m", "ooooo"),   // moons champion
        ("b", "*** ooo"), // balanced — on the front
        ("d", "* o"),     // dominated by everyone
    ];
    let req = MultiRerankRequest {
        entities: entities
            .iter()
            .map(|(id, text)| MultiRerankEntity {
                id: (*id).into(),
                text: (*text).into(),
            })
            .collect(),
        attributes: vec![
            MultiRerankAttributeSpec {
                id: "stars".into(),
                prompt: "count of stars".into(),
                prompt_template_slug: Some("canonical_v2".into()),
                weight: 1.0,
            },
            MultiRerankAttributeSpec {
                id: "moons".into(),
                prompt: "count of moons".into(),
                prompt_template_slug: Some("canonical_v2".into()),
                weight: 1.0,
            },
        ],
        topk: MultiRerankTopKSpec {
            k: 2,
            weight_exponent: 1.0,
            tolerated_error: 0.1,
            band_size: 5,
            effective_resistance_max_active: 64,
            stop_sigma_inflate: 1.25,
            stop_min_consecutive: 2,
            min_explore_degree: 2,
            prune_p_topk_below: None,
        },
        gates: vec![],
        comparison_budget: Some(60),
        latency_budget_ms: None,
        max_cost_nanodollars: None,
        model: Some("test/judge".into()),
        rater_id: None,
        comparison_concurrency: Some(1),
        max_pair_repeats: None,
        randomize_presentation_order: false,
        counterbalance_pairs: false,
    };
    let resp = multi_rerank(
        req,
        RerankExecution::new(gateway, Attribution::new("test::mo")),
    )
    .await
    .unwrap();

    let id_of = |idx: usize| resp.entities[idx].id.as_str();
    let front: Vec<&str> = resp.pareto_front.iter().map(|&i| id_of(i)).collect();
    // The two champions and the balanced entity are non-dominated; the
    // straggler is dominated by all three.
    assert!(front.contains(&"s"), "stars champion on front: {front:?}");
    assert!(front.contains(&"m"), "moons champion on front: {front:?}");
    assert!(front.contains(&"b"), "balanced entity on front: {front:?}");
    assert!(
        !front.contains(&"d"),
        "dominated straggler excluded: {front:?}"
    );

    // Correlations: 2x2, symmetric, diagonal 1, off-diagonal NEGATIVE
    // (the planted trade-off).
    let c = &resp.attribute_correlations;
    assert_eq!(c.len(), 2);
    assert!((c[0][0] - 1.0).abs() < 1e-9 && (c[1][1] - 1.0).abs() < 1e-9);
    assert!((c[0][1] - c[1][0]).abs() < 1e-12, "symmetric");
    assert!(
        c[0][1] < -0.2,
        "planted trade-off must show negative attribute correlation: {}",
        c[0][1]
    );
}
