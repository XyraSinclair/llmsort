use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use llmsort::gateway::{
    Attribution, ChatGateway, ChatRequest, ChatResponse, FinishReason, ProviderError,
};
use llmsort::rerank::RerankExecution;
use llmsort_experiments::judgement_run::edge::RerankJudgementResponse;
use llmsort_experiments::judgement_run::{
    execute_judgement_run, JudgementCandidate, JudgementPrivacy, JudgementProviderCallOutcome,
    JudgementRunRequest, JudgementRunStore, JudgementRunTerminal, JUDGEMENT_RUN_SCHEMA,
};

#[derive(Default)]
struct CountingGateway {
    calls: AtomicUsize,
}

#[async_trait::async_trait]
impl ChatGateway for CountingGateway {
    async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, ProviderError> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(ChatResponse {
            provider_call_id: Some(format!("mock-completion-{call}")),
            provider_request_id: Some(format!("mock-request-{call}")),
            served_model: None,
            content: r#"{"higher_ranked":"A","ratio":1.3,"confidence":0.9}"#.to_string(),
            reasoning: None,
            reasoning_tokens: None,
            input_tokens: 11,
            output_tokens: 3,
            cost_nanodollars: 17,
            cost_is_estimate: false,
            upstream_cost_nanodollars: Some(17),
            latency: Duration::from_millis(1),
            finish_reason: FinishReason::Stop,
            output_logprobs: None,
            cache_read_tokens: None,
            cache_write_tokens: None,
        })
    }
}

#[tokio::test]
async fn persisted_run_reloads_to_identical_edge_response_without_provider_call() {
    let temporary = tempfile::tempdir().unwrap();
    let store = JudgementRunStore::new(temporary.path());
    let gateway = Arc::new(CountingGateway::default());
    let request = JudgementRunRequest {
        entities: vec![
            JudgementCandidate {
                id: " alpha ".to_string(),
                text: "Alpha candidate".to_string(),
            },
            JudgementCandidate {
                id: "beta".to_string(),
                text: "Beta candidate".to_string(),
            },
            JudgementCandidate {
                id: "gamma".to_string(),
                text: "Gamma candidate".to_string(),
            },
        ],
        axis_key: " quality ".to_string(),
        axis_prompt: " overall quality ".to_string(),
        requested_k: 1,
        model: " mock/judge ".to_string(),
        privacy: JudgementPrivacy::Private,
        comparison_concurrency: None,
        min_request_interval_ms: None,
    };

    let record = execute_judgement_run(
        request,
        RerankExecution::new(gateway.clone(), Attribution::new("judgement_run_test")),
        &store,
    )
    .await
    .unwrap();

    let calls_after_execution = gateway.calls.load(Ordering::SeqCst);
    assert!(calls_after_execution > 0);
    assert_eq!(record.schema, JUDGEMENT_RUN_SCHEMA);
    assert!(record.run_ref.starts_with("jrun_"));
    assert_eq!(record.request.entities[0].id, "alpha");
    assert_eq!(record.request.axis_key, "quality");
    assert_eq!(record.request.axis_prompt, "overall quality");
    assert_eq!(record.request.model, "mock/judge");
    assert!(record.instrument.engine_spec.is_some());
    assert!(!record.comparison_trace.is_empty());
    assert_eq!(record.provider_calls.len(), calls_after_execution);
    assert_eq!(
        record.usage.provider_input_tokens,
        11 * calls_after_execution as u32
    );
    assert_eq!(
        record.usage.provider_output_tokens,
        3 * calls_after_execution as u32
    );
    assert_eq!(
        record.usage.provider_cost_nanodollars,
        17 * calls_after_execution as i64
    );
    assert!(record.provider_calls.iter().all(|call| {
        call.gateway_request_digest.len() == 64
            && matches!(
                &call.outcome,
                JudgementProviderCallOutcome::Succeeded {
                    provider_call_id: Some(provider_call_id),
                    provider_request_id: Some(provider_request_id),
                    ..
                } if provider_call_id.starts_with("mock-completion-")
                    && provider_request_id.starts_with("mock-request-")
            )
    }));
    assert!(matches!(
        record.terminal,
        JudgementRunTerminal::Completed { .. }
    ));

    let original_edge = RerankJudgementResponse::try_from(&record).unwrap();
    let original_json = serde_json::to_value(&original_edge).unwrap();
    let persisted_path = temporary.path().join(format!("{}.json", record.run_ref));
    assert!(persisted_path.is_file());

    let loaded = store.load(&record.run_ref).unwrap();
    let loaded_edge = RerankJudgementResponse::try_from(&loaded).unwrap();
    let loaded_json = serde_json::to_value(&loaded_edge).unwrap();

    assert_eq!(loaded_json, original_json);
    assert_eq!(gateway.calls.load(Ordering::SeqCst), calls_after_execution);
    assert_eq!(
        loaded_edge.judgement_run.run_ref.as_deref(),
        Some(record.run_ref.as_str())
    );
    assert_eq!(loaded_edge.judgement_run.privacy, JudgementPrivacy::Private);
}
