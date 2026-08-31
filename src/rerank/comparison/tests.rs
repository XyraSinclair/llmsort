use super::*;

use crate::gateway::{ChatResponse, FinishReason};
use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Duration;

struct VecGateway {
    responses: Mutex<VecDeque<ChatResponse>>,
}

#[async_trait::async_trait]
impl ChatGateway for VecGateway {
    async fn chat(&self, _req: ChatRequest) -> Result<ChatResponse, crate::gateway::ProviderError> {
        Ok(self
            .responses
            .lock()
            .unwrap()
            .pop_front()
            .expect("test response"))
    }
}

fn response(content: &str, output_logprobs: Option<Vec<TokenLogprob>>) -> ChatResponse {
    ChatResponse {
        provider_call_id: None,
        provider_request_id: None,
        served_model: None,
        content: content.to_string(),
        reasoning: None,
        reasoning_tokens: None,
        input_tokens: 10,
        output_tokens: 5,
        cost_nanodollars: 100,
        cost_is_estimate: false,
        upstream_cost_nanodollars: None,
        latency: Duration::from_millis(1),
        finish_reason: FinishReason::Stop,
        output_logprobs,
        cache_read_tokens: None,
        cache_write_tokens: None,
    }
}

fn bucket_output_logprobs() -> Vec<TokenLogprob> {
    vec![
        TokenLogprob {
            token: "higher".to_string(),
            logprob: -0.01,
            top_alternatives: vec![],
        },
        TokenLogprob {
            token: "_".to_string(),
            logprob: -0.01,
            top_alternatives: vec![],
        },
        TokenLogprob {
            token: "ranked".to_string(),
            logprob: -0.01,
            top_alternatives: vec![],
        },
        TokenLogprob {
            token: "\":\"".to_string(),
            logprob: -0.01,
            top_alternatives: vec![],
        },
        TokenLogprob {
            token: "A".to_string(),
            logprob: -0.2,
            top_alternatives: vec![crate::gateway::TokenAlternative {
                token: "B".to_string(),
                logprob: -1.8,
            }],
        },
        TokenLogprob {
            token: "ratio".to_string(),
            logprob: -0.01,
            top_alternatives: vec![],
        },
        TokenLogprob {
            token: "_".to_string(),
            logprob: -0.01,
            top_alternatives: vec![],
        },
        TokenLogprob {
            token: "bucket".to_string(),
            logprob: -0.01,
            top_alternatives: vec![],
        },
        TokenLogprob {
            token: "\":".to_string(),
            logprob: -0.01,
            top_alternatives: vec![],
        },
        TokenLogprob {
            token: "5".to_string(),
            logprob: -0.3,
            top_alternatives: vec![crate::gateway::TokenAlternative {
                token: "4".to_string(),
                logprob: -1.4,
            }],
        },
    ]
}

#[test]
fn test_parse_valid_json() {
    let raw = r#"{"higher_ranked": "A", "ratio": 1.3, "confidence": 0.74}"#;
    let result = parse_pairwise_response(raw, "canonical_v2", None).unwrap();
    match result {
        PairwiseJudgement::Observation {
            higher_ranked,
            ratio,
            confidence,
        } => {
            assert_eq!(higher_ranked, HigherRanked::A);
            assert!((ratio - 1.3).abs() < 0.001);
            assert!((confidence - 0.74).abs() < 0.001);
        }
        _ => panic!("Expected Observation"),
    }
}

#[test]
fn test_parse_ratio_bucket_json() {
    let raw = r#"{"higher_ranked": "B", "ratio_bucket": 9, "confidence": 0.82}"#;
    let result = parse_pairwise_response(raw, "canonical_bucket_v1", None).unwrap();
    match result {
        PairwiseJudgement::Observation {
            higher_ranked,
            ratio,
            confidence,
        } => {
            assert_eq!(higher_ranked, HigherRanked::B);
            assert!((ratio - 3.1).abs() < 0.001);
            assert!((confidence - 0.82).abs() < 0.001);
        }
        _ => panic!("Expected Observation"),
    }
}

#[test]
fn test_parse_refused() {
    let raw = r#"{"refused": true}"#;
    let result = parse_pairwise_response(raw, "canonical_v2", None).unwrap();
    assert!(matches!(result, PairwiseJudgement::Refused));
}

#[test]
fn test_parse_with_surrounding_text() {
    let raw = r#"Here's my evaluation:
{"higher_ranked": "B", "ratio": 2.5, "confidence": 0.9}
That's my assessment."#;
    let result = parse_pairwise_response(raw, "canonical_v2", None).unwrap();
    match result {
        PairwiseJudgement::Observation {
            higher_ranked,
            ratio,
            ..
        } => {
            assert_eq!(higher_ranked, HigherRanked::B);
            assert!((ratio - 2.5).abs() < 0.001);
        }
        _ => panic!("Expected Observation"),
    }
}

#[test]
fn test_parse_rejects_missing_confidence_even_with_logprobs() {
    let raw = r#"{"higher_ranked":"A","ratio":2.5}"#;
    let logprobs = vec![
        TokenLogprob {
            token: "\"A\"".to_string(),
            logprob: -0.1,
            top_alternatives: vec![crate::gateway::TokenAlternative {
                token: "\"B\"".to_string(),
                logprob: -2.3,
            }],
        },
        TokenLogprob {
            token: "2.5".to_string(),
            logprob: -0.22,
            top_alternatives: vec![crate::gateway::TokenAlternative {
                token: "2.1".to_string(),
                logprob: -1.61,
            }],
        },
    ];

    let err = parse_pairwise_response(raw, "canonical_v2", Some(&logprobs)).unwrap_err();
    assert!(
        matches!(err, ComparisonError::Parse(message) if message.contains("missing 'confidence'"))
    );
}

#[test]
fn test_parse_ordinal_json_a() {
    let raw = r#"{"higher_ranked":"A","confidence":0.61}"#;
    let result = parse_pairwise_response(raw, "ordinal_v1", None).unwrap();
    match result {
        PairwiseJudgement::Observation {
            higher_ranked,
            ratio,
            confidence,
        } => {
            assert_eq!(higher_ranked, HigherRanked::A);
            assert!((ratio - ORDINAL_OBSERVATION_RATIO).abs() < 0.001);
            assert!((confidence - 0.61).abs() < 0.001);
        }
        _ => panic!("Expected Observation"),
    }
}

#[test]
fn test_parse_ordinal_json_b() {
    let raw = r#"{"higher_ranked":"B","confidence":0.22}"#;
    let result = parse_pairwise_response(raw, "ordinal_v1", None).unwrap();
    match result {
        PairwiseJudgement::Observation {
            higher_ranked,
            ratio,
            confidence,
        } => {
            assert_eq!(higher_ranked, HigherRanked::B);
            assert!((ratio - ORDINAL_OBSERVATION_RATIO).abs() < 0.001);
            assert!((confidence - 0.22).abs() < 0.001);
        }
        _ => panic!("Expected Observation"),
    }
}

#[test]
fn test_parse_ordinal_refused() {
    let raw = r#"{"refused":true}"#;
    let result = parse_pairwise_response(raw, "ordinal_v1", None).unwrap();
    assert!(matches!(result, PairwiseJudgement::Refused));
}

#[test]
fn test_parse_ordinal_rejects_malformed_response() {
    let raw = r#"{"higher_ranked":"C","confidence":0.5}"#;
    let err = parse_pairwise_response(raw, "ordinal_v1", None).unwrap_err();
    assert!(
        matches!(err, ComparisonError::Parse(message) if message.contains("invalid higher_ranked"))
    );
}

#[test]
fn test_bucket_logprob_posterior_uses_ratio_bucket_field() {
    let logprobs = vec![
        TokenLogprob {
            token: "{\"".to_string(),
            logprob: -0.01,
            top_alternatives: vec![],
        },
        TokenLogprob {
            token: "higher".to_string(),
            logprob: -0.01,
            top_alternatives: vec![],
        },
        TokenLogprob {
            token: "_".to_string(),
            logprob: -0.01,
            top_alternatives: vec![],
        },
        TokenLogprob {
            token: "ranked".to_string(),
            logprob: -0.01,
            top_alternatives: vec![],
        },
        TokenLogprob {
            token: "\":\"".to_string(),
            logprob: -0.01,
            top_alternatives: vec![],
        },
        TokenLogprob {
            token: "B".to_string(),
            logprob: -0.2,
            top_alternatives: vec![crate::gateway::TokenAlternative {
                token: "A".to_string(),
                logprob: -1.8,
            }],
        },
        TokenLogprob {
            token: "\",\"".to_string(),
            logprob: -0.01,
            top_alternatives: vec![],
        },
        TokenLogprob {
            token: "ratio".to_string(),
            logprob: -0.01,
            top_alternatives: vec![],
        },
        TokenLogprob {
            token: "_".to_string(),
            logprob: -0.01,
            top_alternatives: vec![],
        },
        TokenLogprob {
            token: "bucket".to_string(),
            logprob: -0.01,
            top_alternatives: vec![],
        },
        TokenLogprob {
            token: "\":".to_string(),
            logprob: -0.01,
            top_alternatives: vec![],
        },
        TokenLogprob {
            token: "9".to_string(),
            logprob: -0.3,
            top_alternatives: vec![
                crate::gateway::TokenAlternative {
                    token: "8".to_string(),
                    logprob: -1.4,
                },
                crate::gateway::TokenAlternative {
                    token: "10".to_string(),
                    logprob: -2.0,
                },
            ],
        },
    ];

    let posterior = pairwise_bucket_logprob_posterior(&logprobs, PairwisePreferredSide::B, 3.1)
        .expect("posterior");
    assert_eq!(posterior.selected_ratio_bucket, RatioBucket::R09);
    assert!(posterior.answer_distribution.support_probability() > 0.0);
    assert!(matches!(
        posterior.confidence,
        ConfidenceSource::Logprob { .. }
    ));
    assert!(posterior.probability_negative() > posterior.probability_positive());
}

#[test]
fn test_bucket_logprob_posterior_handles_split_two_digit_bucket() {
    let mut logprobs = bucket_output_logprobs();
    let bucket = logprobs.last_mut().expect("bucket token");
    bucket.token = "1".to_string();
    bucket.logprob = -0.02;
    bucket.top_alternatives = vec![];
    logprobs.push(TokenLogprob {
        token: "2".to_string(),
        logprob: -0.3,
        top_alternatives: vec![
            crate::gateway::TokenAlternative {
                token: "3".to_string(),
                logprob: -0.9,
            },
            crate::gateway::TokenAlternative {
                token: "1".to_string(),
                logprob: -1.4,
            },
            crate::gateway::TokenAlternative {
                token: "6".to_string(),
                logprob: -2.0,
            },
        ],
    });

    let posterior = pairwise_bucket_logprob_posterior(&logprobs, PairwisePreferredSide::A, 6.8)
        .expect("posterior");
    let compact = compact_bucket_output_logprobs(&logprobs, PairwisePreferredSide::A, 6.8)
        .expect("compact logprobs");

    assert_eq!(posterior.selected_ratio_bucket, RatioBucket::R12);
    assert!(
        posterior
            .ratio_distribution
            .probability_of(|bucket| *bucket == RatioBucket::R12)
            > 0.0
    );
    assert!(
        posterior
            .ratio_distribution
            .probability_of(|bucket| *bucket == RatioBucket::R13)
            > 0.0
    );
    assert_eq!(compact.len(), 2);
    assert_eq!(compact[1].token, "2");
}

#[test]
fn test_compact_bucket_output_logprobs_keeps_decisive_positions_only() {
    let logprobs = bucket_output_logprobs();
    let compact = compact_bucket_output_logprobs(&logprobs, PairwisePreferredSide::A, 1.5)
        .expect("compact logprobs");

    assert_eq!(compact.len(), 2);
    assert_eq!(compact[0].token, "A");
    assert_eq!(compact[1].token, "5");
}

#[tokio::test]
async fn compare_pair_retries_bucket_prompt_until_pmf_available() {
    let content = r#"{"higher_ranked":"A","ratio_bucket":5,"confidence":0.85}"#;
    let gateway = VecGateway {
        responses: Mutex::new(VecDeque::from([
            response(content, None),
            response(content, Some(bucket_output_logprobs())),
        ])),
    };
    let request = PairwiseComparisonRequest {
        spec: PairwiseComparisonSpec {
            model: "google/gemma-4-26b-a4b-it",
            attribute: PairwiseComparisonAttribute {
                id: "pmf",
                prompt: "PMF test",
                prompt_template_slug: Some("canonical_bucket_v1"),
            },
            entity_a: PairwiseComparisonEntity { id: "a", text: "A" },
            entity_b: PairwiseComparisonEntity { id: "b", text: "B" },
        },
        cache_only: false,
        attribution: Attribution::new("test::bucket_retry"),
        nonce: None,
    };

    let (judgement, usage) = compare_pair(&gateway, None, request).await.unwrap();
    assert!(matches!(judgement, PairwiseJudgement::Observation { .. }));
    assert_eq!(usage.input_tokens, 20);
    assert_eq!(usage.output_tokens, 10);
    assert_eq!(usage.provider_cost_nanodollars, 200);
    assert!(usage.output_logprobs.is_some());
    assert_eq!(usage.output_logprobs.as_ref().unwrap().len(), 2);
    assert!(usage.pairwise_logprob_posterior.is_some());
    assert_eq!(gateway.responses.lock().unwrap().len(), 0);
}

#[test]
fn test_model_supports_logprobs() {
    // Anthropic: no logprobs via OpenRouter
    assert!(!model_supports_logprobs("anthropic/claude-opus-4-6"));
    assert!(!model_supports_logprobs("anthropic/claude-sonnet-4.6"));
    assert!(!model_supports_logprobs("anthropic/claude-sonnet-4"));
    assert!(!model_supports_logprobs("anthropic/claude-haiku-4.5"));

    // Reasoning models: logprobs don't reflect deliberation
    assert!(!model_supports_logprobs("openai/o3"));
    assert!(!model_supports_logprobs("openai/o3-pro"));
    assert!(!model_supports_logprobs("openai/o4-mini"));
    assert!(!model_supports_logprobs("openai/o4-mini-high"));
    assert!(!model_supports_logprobs("openai/o1"));
    assert!(!model_supports_logprobs("openai/o1-pro"));
    assert!(!model_supports_logprobs("deepseek/deepseek-r1"));
    assert!(!model_supports_logprobs("deepseek/deepseek-r1-0528"));
    assert!(!model_supports_logprobs("qwen/qwq-32b"));

    // :thinking variants
    assert!(!model_supports_logprobs(
        "anthropic/claude-3.7-sonnet:thinking"
    ));
    assert!(!model_supports_logprobs("moonshotai/kimi-k2-thinking"));
    assert!(!model_supports_logprobs(
        "qwen/qwen3-235b-a22b-thinking-2507"
    ));
    assert!(!model_supports_logprobs("baidu/ernie-4.5-21b-a3b-thinking"));

    // GPT-5.4 family: logprobs crash OpenAI backend via OpenRouter
    assert!(!model_supports_logprobs("openai/gpt-5.4-mini"));
    assert!(!model_supports_logprobs("openai/gpt-5.4"));
    assert!(!model_supports_logprobs("openai/gpt-5.4-nano"));

    // Non-reasoning models: YES logprobs
    assert!(model_supports_logprobs("openai/gpt-4.1"));
    assert!(model_supports_logprobs("openai/gpt-4.1-mini"));
    // GPT-5 base family: mandatory reasoning, no logprob path
    // (docs/LOGPROBS.md census; provider 400 measured 2026-07-27)
    assert!(!model_supports_logprobs("openai/gpt-5"));
    assert!(!model_supports_logprobs("openai/gpt-5-mini"));
    assert!(!model_supports_logprobs("openai/gpt-5-chat-latest"));
    assert!(model_supports_logprobs("openai/gpt-5.2-pro"));
    assert!(model_supports_logprobs("google/gemini-2.5-pro"));
    assert!(model_supports_logprobs("google/gemini-2.5-flash"));
    assert!(!model_supports_logprobs("google/gemini-3.1-pro-preview"));
    assert!(model_supports_logprobs("moonshotai/kimi-k2-0905"));
    assert!(model_supports_logprobs("deepseek/deepseek-chat"));
    assert!(model_supports_logprobs("deepseek/deepseek-v3.2"));
}

#[test]
fn test_should_use_json_mode_for_openai_and_local_models() {
    assert!(should_use_json_mode("openai/gpt-4.1"));
    assert!(should_use_json_mode("gemma4:31b"));
    assert!(!should_use_json_mode("google/gemma-4-31b-it"));
}
