//! Scratch reproduction: does the portable judgement run (cardinald/freelane
//! request shape) cover its cohort, or collapse onto a few boundary pairs?
//! Deterministic judge behind wiremock; no LLM.
//!
//! cargo run -p llmsort-experiments --example planner_coverage_repro -- [n] [k] [max_pair_repeats]

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use llmsort::gateway::openrouter::OpenRouterAdapter;
use llmsort::gateway::{Attribution, GatewayConfig, NoopUsageSink, ProviderGateway};
use llmsort::rerank::RerankExecution;
use llmsort_experiments::judgement_run::{
    execute_judgement_run, JudgementCandidate, JudgementPrivacy, JudgementRunRequest,
    JudgementRunStore, JudgementRunTerminal,
};
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

fn extract_between<'a>(s: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let start_idx = s.find(start)? + start.len();
    let rest = &s[start_idx..];
    let end_idx = rest.find(end)?;
    Some(&rest[..end_idx])
}

fn hidden_score(ctx: &str) -> i64 {
    extract_between(ctx, "score=", ";")
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0)
}

struct DeterministicJudge;

impl Respond for DeterministicJudge {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let parsed: serde_json::Value = serde_json::from_slice(&request.body).unwrap_or_default();
        let user = parsed["messages"]
            .as_array()
            .and_then(|m| m.iter().find(|m| m["role"] == "user"))
            .and_then(|m| m["content"].as_str())
            .unwrap_or("")
            .to_string();
        let a = extract_between(&user, "<entity_A_context>", "</entity_A_context>").unwrap_or("");
        let b = extract_between(&user, "<entity_B_context>", "</entity_B_context>").unwrap_or("");
        let (sa, sb) = (hidden_score(a), hidden_score(b));
        let (higher, ratio) = if sa >= sb {
            ("A", 1.0 + (sa - sb) as f64 * 0.3)
        } else {
            ("B", 1.0 + (sb - sa) as f64 * 0.3)
        };
        let content =
            format!(r#"{{"higher_ranked":"{higher}","ratio":{ratio:.2},"confidence":0.9}}"#);
        ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{"message": {"content": content}, "finish_reason": "stop"}],
            "usage": {"prompt_tokens": 10, "completion_tokens": 10}
        }))
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let n: usize = args.get(1).and_then(|v| v.parse().ok()).unwrap_or(20);
    let k: usize = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(10);
    let draws: Option<u32> = args.get(3).and_then(|v| v.parse().ok());

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(DeterministicJudge)
        .mount(&server)
        .await;
    let adapter = OpenRouterAdapter::with_config(
        "sk-test",
        server.uri(),
        Duration::from_secs(20),
        None,
        None,
    )
    .expect("adapter");
    let gateway = Arc::new(ProviderGateway::with_config(
        adapter,
        Arc::new(NoopUsageSink),
        GatewayConfig {
            max_retries: 0,
            retry_base_delay: Duration::from_millis(0),
        },
    ));

    let entities = (0..n)
        .map(|i| JudgementCandidate {
            id: format!("e{i:02}"),
            text: format!("Item {i}. score={};  filler text about the item.", (i * 7919) % 97),
        })
        .collect();
    let request = JudgementRunRequest {
        entities,
        axis_key: "repro-axis".into(),
        axis_prompt: "Which item scores higher?".into(),
        requested_k: k,
        model: "test/judge".into(),
        privacy: JudgementPrivacy::Private,
        comparison_concurrency: Some(8),
        min_request_interval_ms: None,
        provider_base_url: None,
        nonce_draws: draws,
    };
    let dir = tempfile::tempdir().expect("tempdir");
    let store = JudgementRunStore::new(dir.path());
    let execution = RerankExecution::new(gateway, Attribution::new("repro"));
    let record = execute_judgement_run(request, execution, &store)
        .await
        .expect("run");

    let trace = &record.comparison_trace;
    let mut touched: HashSet<&str> = HashSet::new();
    let mut pairs: HashSet<(String, String)> = HashSet::new();
    let mut cells: HashMap<(String, String, bool), usize> = HashMap::new();
    let mut errors = 0usize;
    for t in trace {
        touched.insert(&t.entity_a_id);
        touched.insert(&t.entity_b_id);
        let (lo, hi) = if t.entity_a_id <= t.entity_b_id {
            (t.entity_a_id.clone(), t.entity_b_id.clone())
        } else {
            (t.entity_b_id.clone(), t.entity_a_id.clone())
        };
        pairs.insert((lo.clone(), hi.clone()));
        *cells.entry((lo, hi, t.swapped)).or_insert(0) += 1;
        if t.error.is_some() {
            errors += 1;
        }
    }
    let max_rep = cells.values().copied().max().unwrap_or(0);
    let stop = match &record.terminal {
        JudgementRunTerminal::Completed { stop_reason, .. } => format!("{stop_reason:?}"),
        other => other.status().to_string(),
    };
    println!(
        "n={n} k={k} draws={draws:?} comparisons={} errors={errors} touched={}/{n} pairs={}/{} cells={} max_repeats_per_cell={max_rep} stop={stop}",
        trace.len(),
        touched.len(),
        pairs.len(),
        n * (n - 1) / 2,
        cells.len()
    );
    let mut per_entity: Vec<(String, usize)> = (0..n)
        .map(|i| {
            let id = format!("e{i:02}");
            let d = trace
                .iter()
                .filter(|t| t.entity_a_id == id || t.entity_b_id == id)
                .count();
            (id, d)
        })
        .collect();
    per_entity.sort_by_key(|(_, d)| *d);
    println!("degree by entity (ascending): {per_entity:?}");
    if let JudgementRunTerminal::Completed { response, .. } = &record.terminal {
        let mut rows: Vec<(f64, f64)> = response
            .entities
            .iter()
            .map(|e| {
                let idx: usize = e.id[1..].parse().unwrap();
                let truth = ((idx * 7919) % 97) as f64;
                (e.attribute_score.latent_mean, truth)
            })
            .collect();
        let rank = |v: Vec<f64>| -> Vec<f64> {
            let mut idx: Vec<usize> = (0..v.len()).collect();
            idx.sort_by(|&a, &b| v[a].partial_cmp(&v[b]).unwrap());
            let mut r = vec![0.0; v.len()];
            for (pos, &i) in idx.iter().enumerate() {
                r[i] = pos as f64;
            }
            r
        };
        let rx = rank(rows.iter().map(|r| r.0).collect());
        let ry = rank(rows.iter().map(|r| r.1).collect());
        let m = rx.len() as f64;
        let mx = rx.iter().sum::<f64>() / m;
        let my = ry.iter().sum::<f64>() / m;
        let cov: f64 = rx.iter().zip(&ry).map(|(a, b)| (a - mx) * (b - my)).sum();
        let vx: f64 = rx.iter().map(|a| (a - mx).powi(2)).sum();
        let vy: f64 = ry.iter().map(|b| (b - my).powi(2)).sum();
        rows.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        let topk_truth: Vec<f64> = rows.iter().take(k).map(|r| r.1).collect();
        println!("spearman(recovered, truth) = {:+.3}; top-{k} truth scores = {topk_truth:?}", cov / (vx * vy).sqrt());
    }
}
