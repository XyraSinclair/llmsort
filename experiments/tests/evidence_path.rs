//! End-to-end tests for the seriate evidence path (`ratio_letter_v1`):
//! single-token answers with top-k logprobs become PMF evidence whose
//! measured variance drives the solver, with loud degradation and cache
//! round-trips.

use std::process::{Command, Stdio};
use std::sync::{mpsc, Arc};
use std::time::Duration;

use llmsort::gateway::openrouter::OpenRouterAdapter;
use llmsort::gateway::{Attribution, GatewayConfig, NoopUsageSink, ProviderGateway};
use llmsort::prompts::PromptInstance;
use llmsort::rerank::{
    compare_pair, sort_texts, PairwiseComparisonAttribute, PairwiseComparisonEntity,
    PairwiseComparisonRequest, PairwiseComparisonSpec, RerankExecution, RerankRunOptions,
    SortOptions,
};
use llmsort::{ComparisonTrace, SqlitePairwiseCache, TraceError, TraceSink};
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

fn extract_between<'a>(s: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let i = s.find(start)? + start.len();
    let rest = &s[i..];
    let j = rest.find(end)?;
    Some(&rest[..j])
}

fn stars(s: &str) -> i64 {
    s.chars().filter(|&c| c == '*').count() as i64
}

/// Letter judge over the seriate prompt shape (<entity_A>/<entity_B>),
/// answering one letter with synthetic top-k logprobs.
#[derive(Clone, Copy)]
struct LetterJudge {
    /// Reject any request that asks for logprobs (reasoning-model style).
    reject_logprobs: bool,
    /// Omit logprobs from responses without erroring.
    omit_logprobs: bool,
}

impl Respond for LetterJudge {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap_or_default();
        let wants_logprobs = body
            .get("logprobs")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if self.reject_logprobs && wants_logprobs {
            return ResponseTemplate::new(400).set_body_json(json!({
                "error": { "message": "logprobs are not supported with reasoning models", "code": 400 }
            }));
        }
        let user = body["messages"]
            .as_array()
            .and_then(|m| {
                m.iter()
                    .find(|x| x["role"] == "user")
                    .and_then(|x| x["content"].as_str())
            })
            .unwrap_or("")
            .to_string();
        let a = extract_between(&user, "<entity_A>", "</entity_A>").unwrap_or("");
        let b = extract_between(&user, "<entity_B>", "</entity_B>").unwrap_or("");
        let d = stars(a) - stars(b);
        // Bucket 3 ('D'/'d') for small gaps, bucket 7 ('H'/'h') for large.
        let letter = match d {
            0 => 'A',
            1..=2 => 'D',
            3.. => 'H',
            -2..=-1 => 'd',
            _ => 'h',
        };
        let mut response = json!({
            "choices": [{
                "message": { "content": letter.to_string() },
                "finish_reason": "stop"
            }],
            "usage": { "prompt_tokens": 50, "completion_tokens": 1 }
        });
        if wants_logprobs && !self.omit_logprobs {
            // 0.7 on the answer, 0.2 on the adjacent bucket (same side),
            // 0.05 junk; 0.05 never shown.
            let adjacent = match letter {
                'A' => 'B',
                c if c.is_ascii_uppercase() => ((c as u8) - 1) as char,
                c => ((c as u8) - 1) as char,
            };
            response["choices"][0]["logprobs"] = json!({
                "content": [{
                    "token": letter.to_string(),
                    "logprob": -0.356_674_9, // 0.7
                    "top_logprobs": [
                        { "token": letter.to_string(), "logprob": -0.356_674_9 },
                        { "token": adjacent.to_string(), "logprob": -1.609_438_0 }, // 0.2
                        { "token": "The", "logprob": -2.995_732_3 },                 // 0.05
                    ]
                }]
            });
        }
        ResponseTemplate::new(200).set_body_json(response)
    }
}

async fn start_judge(judge: LetterJudge) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(judge)
        .mount(&server)
        .await;
    server
}

fn gateway_for(server: &MockServer) -> Arc<ProviderGateway<NoopUsageSink>> {
    let adapter =
        OpenRouterAdapter::with_config("sk-test", server.uri(), Duration::from_secs(5), None, None)
            .unwrap();
    Arc::new(ProviderGateway::with_config(
        adapter,
        Arc::new(NoopUsageSink),
        GatewayConfig {
            max_retries: 0,
            retry_base_delay: Duration::from_millis(0),
        },
    ))
}

fn letter_opts() -> SortOptions {
    SortOptions {
        model: Some("test/judge".into()),
        comparison_budget: Some(40),
        prompt_template_slug: Some("ratio_letter_v1".into()),
        ..Default::default()
    }
}

fn items() -> Vec<String> {
    vec![
        "alpha ****".into(),
        "bravo ***".into(),
        "charlie **".into(),
        "delta *".into(),
    ]
}

fn evidence_spec(slug: &str) -> PairwiseComparisonSpec<'_> {
    PairwiseComparisonSpec {
        model: "test/judge",
        attribute: PairwiseComparisonAttribute {
            id: "brightness",
            prompt: "brightness <with literal markup>",
            prompt_template_slug: Some(slug),
        },
        entity_a: PairwiseComparisonEntity {
            id: "alpha",
            text: "alpha & amber",
        },
        entity_b: PairwiseComparisonEntity {
            id: "beta",
            text: "beta > blue",
        },
    }
}

#[test]
fn evidence_prompt_instances_are_rendered_by_the_requested_seriate_instrument() {
    for slug in ["ratio_letter_v1", "ordinal_letter_v1"] {
        let instrument: Box<dyn llmsort::seriate::instrument::Instrument> = match slug {
            "ratio_letter_v1" => {
                Box::new(llmsort::seriate::instrument::ratio_letter::RatioLetterInstrument)
            }
            "ordinal_letter_v1" => {
                Box::new(llmsort::seriate::instrument::ordinal::OrdinalInstrument)
            }
            _ => unreachable!(),
        };
        let expected = instrument.render(
            &llmsort::seriate::Attribute::new("brightness", "brightness <with literal markup>"),
            &llmsort::seriate::Entity::new("alpha & amber"),
            &llmsort::seriate::Entity::new("beta > blue"),
        );
        let actual = evidence_spec(slug).prompt_instance();
        let canonical = evidence_spec("canonical_v2").prompt_instance();

        assert_eq!(actual.template_slug, slug);
        assert_eq!(actual.system, expected.system, "system bytes for {slug}");
        assert_eq!(actual.user, expected.user, "user bytes for {slug}");
        assert_ne!(actual.system, canonical.system, "{slug} must not fall back");
        assert_ne!(actual.user, canonical.user, "{slug} must not fall back");
    }
}

#[test]
fn rendered_prompt_digest_is_stable_sensitive_and_length_delimited() {
    fn prompt(system: &str, user: &str) -> PromptInstance {
        PromptInstance {
            template_slug: "identity_test".into(),
            system: system.into(),
            user: user.into(),
        }
    }

    let original = prompt("system bytes", "user bytes");
    let digest = original.rendered_digest();
    assert!(!digest.is_empty());
    assert_eq!(digest, original.rendered_digest());
    assert_ne!(
        digest,
        prompt("changed system", "user bytes").rendered_digest()
    );
    assert_ne!(
        digest,
        prompt("system bytes", "changed user").rendered_digest()
    );
    assert_ne!(
        prompt("ab", "c").rendered_digest(),
        prompt("a", "bc").rendered_digest(),
        "part lengths must prevent concatenation-boundary collisions"
    );
}

struct ChannelTraceSink(mpsc::Sender<ComparisonTrace>);

impl TraceSink for ChannelTraceSink {
    fn record(&self, event: ComparisonTrace) -> Result<(), TraceError> {
        self.0.send(event).map_err(|_| TraceError::Closed)
    }
}

#[tokio::test]
async fn evidence_trace_identity_matches_the_exact_provider_prompt_and_seriate_template() {
    let server = start_judge(LetterJudge {
        reject_logprobs: false,
        omit_logprobs: false,
    })
    .await;
    let (trace_tx, trace_rx) = mpsc::channel();
    let trace_sink = ChannelTraceSink(trace_tx);
    let execution = RerankExecution::new(gateway_for(&server), Attribution::new("test::identity"))
        .trace(&trace_sink);
    let mut opts = letter_opts();
    opts.comparison_budget = Some(2);
    opts.comparison_concurrency = Some(1);
    opts.counterbalance = true;

    sort_texts(
        vec!["alpha ****".into(), "delta *".into()],
        "brightness",
        execution,
        opts,
    )
    .await
    .unwrap();

    let requests = server.received_requests().await.unwrap();
    let events: Vec<_> = trace_rx.try_iter().collect();
    assert_eq!(events.len(), requests.len());
    assert!(!events.is_empty());

    let instrument = llmsort::seriate::instrument::ratio_letter::RatioLetterInstrument;
    let expected_template_hash = llmsort::seriate::instrument::Instrument::render(
        &instrument,
        &llmsort::seriate::Attribute::new("fingerprint", "fingerprint"),
        &llmsort::seriate::Entity::new("A"),
        &llmsort::seriate::Entity::new("B"),
    )
    .template
    .0
     .0;

    fn message_content<'a>(messages: &'a [serde_json::Value], role: &str) -> &'a str {
        messages
            .iter()
            .find(|message| message["role"] == role)
            .and_then(|message| message["content"].as_str())
            .unwrap()
    }

    let mut provider_digests: Vec<String> = requests
        .iter()
        .map(|request| {
            let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
            let messages = body["messages"].as_array().unwrap();
            PromptInstance {
                template_slug: "ratio_letter_v1".into(),
                system: message_content(messages, "system").into(),
                user: message_content(messages, "user").into(),
            }
            .rendered_digest()
        })
        .collect();
    provider_digests.sort();
    let mut trace_digests: Vec<_> = events
        .iter()
        .map(|event| event.rendered_prompt_digest.clone())
        .collect();
    trace_digests.sort();
    assert_eq!(
        trace_digests, provider_digests,
        "every trace row must identify one exact provider prompt"
    );
    for event in events {
        assert_eq!(event.prompt_template_slug, "ratio_letter_v1");
        assert_eq!(event.template_hash, expected_template_hash);
    }
}

#[tokio::test]
async fn cached_evidence_trace_preserves_rendered_identity_without_prompt_text() {
    let server = start_judge(LetterJudge {
        reject_logprobs: false,
        omit_logprobs: false,
    })
    .await;
    let dir = tempfile::tempdir().unwrap();
    let cache = SqlitePairwiseCache::new(dir.path().join("cache.sqlite")).unwrap();
    let gateway = gateway_for(&server);
    let run_options = RerankRunOptions {
        rng_seed: Some(19),
        cache_only: false,
    };
    let mut opts = letter_opts();
    opts.comparison_budget = Some(2);
    opts.comparison_concurrency = Some(1);
    opts.counterbalance = true;
    let entities = vec!["alpha ****".to_string(), "delta *".to_string()];

    sort_texts(
        entities.clone(),
        "brightness",
        RerankExecution::new(gateway.clone(), Attribution::new("test::cache-fill"))
            .cache(&cache)
            .run_options(run_options),
        opts.clone(),
    )
    .await
    .unwrap();

    let (trace_tx, trace_rx) = mpsc::channel();
    let trace_sink = ChannelTraceSink(trace_tx);
    sort_texts(
        entities.clone(),
        "brightness",
        RerankExecution::new(gateway.clone(), Attribution::new("test::cache-trace"))
            .cache(&cache)
            .run_options(RerankRunOptions {
                rng_seed: Some(19),
                cache_only: true,
            })
            .trace(&trace_sink),
        opts,
    )
    .await
    .unwrap();

    let events: Vec<_> = trace_rx.try_iter().collect();
    assert!(!events.is_empty());
    for event in events {
        assert!(event.cached);
        let spec = PairwiseComparisonSpec {
            model: &event.model,
            attribute: PairwiseComparisonAttribute {
                id: &event.attribute_id,
                prompt: "brightness",
                prompt_template_slug: Some("ratio_letter_v1"),
            },
            entity_a: PairwiseComparisonEntity {
                id: &event.entity_a_id,
                text: &entities[event.entity_a_index],
            },
            entity_b: PairwiseComparisonEntity {
                id: &event.entity_b_id,
                text: &entities[event.entity_b_index],
            },
        };
        let expected_key = spec.cache_key();
        let expected_digest = spec.rendered_prompt_digest();
        assert_eq!(event.prompt_template_slug, "ratio_letter_v1");
        assert_eq!(event.template_hash, expected_key.template_hash);
        assert_eq!(event.rendered_prompt_digest, expected_digest);

        let (_, usage) = compare_pair(
            gateway.as_ref(),
            Some(&cache),
            PairwiseComparisonRequest {
                nonce: None,
                spec,
                cache_only: true,
                attribution: Attribution::new("test::cached-usage"),
            },
        )
        .await
        .unwrap();
        assert!(usage.cached);
        assert!(usage.prompt_text.is_none());
        assert_eq!(usage.rendered_prompt_digest, event.rendered_prompt_digest);
    }
}

async fn trace_for_evidence_response(response: ResponseTemplate) -> ComparisonTrace {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(response)
        .mount(&server)
        .await;
    let (trace_tx, trace_rx) = mpsc::channel();
    let trace_sink = ChannelTraceSink(trace_tx);
    let mut opts = letter_opts();
    opts.comparison_budget = Some(1);
    opts.comparison_concurrency = Some(1);
    opts.counterbalance = false;

    sort_texts(
        vec!["alpha ****".into(), "delta *".into()],
        "brightness",
        RerankExecution::new(
            gateway_for(&server),
            Attribution::new("test::terminal-path"),
        )
        .trace(&trace_sink),
        opts,
    )
    .await
    .unwrap();

    let events: Vec<_> = trace_rx.try_iter().collect();
    assert_eq!(events.len(), 1);
    events.into_iter().next().unwrap()
}

fn assert_ratio_letter_trace_identity(event: &ComparisonTrace) {
    let entities = ["alpha ****", "delta *"];
    let spec = PairwiseComparisonSpec {
        model: &event.model,
        attribute: PairwiseComparisonAttribute {
            id: &event.attribute_id,
            prompt: "brightness",
            prompt_template_slug: Some("ratio_letter_v1"),
        },
        entity_a: PairwiseComparisonEntity {
            id: &event.entity_a_id,
            text: entities[event.entity_a_index],
        },
        entity_b: PairwiseComparisonEntity {
            id: &event.entity_b_id,
            text: entities[event.entity_b_index],
        },
    };
    let expected_key = spec.cache_key();
    assert_eq!(event.prompt_template_slug, "ratio_letter_v1");
    assert_eq!(event.template_hash, expected_key.template_hash);
    assert_eq!(event.rendered_prompt_digest, spec.rendered_prompt_digest());
}

#[tokio::test]
async fn evidence_parse_refusal_trace_retains_rendered_identity() {
    let event = trace_for_evidence_response(ResponseTemplate::new(200).set_body_json(json!({
        "choices": [{
            "message": { "content": "?" },
            "finish_reason": "stop"
        }],
        "usage": { "prompt_tokens": 11, "completion_tokens": 1 }
    })))
    .await;

    assert!(event.refused);
    assert!(event.error.is_none());
    assert_ratio_letter_trace_identity(&event);
}

#[tokio::test]
async fn evidence_provider_error_trace_retains_error_and_rendered_identity() {
    let event = trace_for_evidence_response(ResponseTemplate::new(502).set_body_json(json!({
        "error": { "message": "upstream unavailable", "code": "bad_gateway" }
    })))
    .await;

    assert!(
        event
            .error
            .as_deref()
            .is_some_and(|error| error.contains("upstream unavailable")),
        "trace must retain provider error: {:?}",
        event.error
    );
    assert_ratio_letter_trace_identity(&event);
}

#[tokio::test]
async fn letter_path_recovers_planted_order_with_logprob_evidence() {
    let server = start_judge(LetterJudge {
        reject_logprobs: false,
        omit_logprobs: false,
    })
    .await;
    let execution = RerankExecution::new(gateway_for(&server), Attribution::new("test::evidence"));
    let sorted = sort_texts(items(), "brightness", execution, letter_opts())
        .await
        .unwrap();
    let texts: Vec<&str> = sorted.items.iter().map(|i| i.text.as_str()).collect();
    assert_eq!(
        texts,
        vec!["alpha ****", "bravo ***", "charlie **", "delta *"]
    );
    // Every judgement carried PMF evidence in logprob mode with the
    // fixture's 0.95 visible mass.
    assert_eq!(
        sorted.meta.evidence_judgements,
        sorted.meta.comparisons_used
    );
    assert_eq!(
        sorted.meta.logprob_mode_judgements,
        sorted.meta.evidence_judgements
    );
    let mass = sorted.meta.evidence_visible_mass_mean.unwrap();
    assert!(
        (mass - 0.95).abs() < 1e-6,
        "visible mass diagnostic: {mass}"
    );
    // Counterbalancing still measured (letter judge is content-driven).
    assert!(sorted.meta.pairs_counterbalanced > 0);
    assert_eq!(sorted.meta.position_flips, 0);
}

#[tokio::test]
async fn logprob_rejection_degrades_loudly_to_sampled_mode() {
    let server = start_judge(LetterJudge {
        reject_logprobs: true,
        omit_logprobs: false,
    })
    .await;
    let execution = RerankExecution::new(gateway_for(&server), Attribution::new("test::evidence"));
    let sorted = sort_texts(items(), "brightness", execution, letter_opts())
        .await
        .unwrap();
    // Order still recovered from sampled single letters...
    let texts: Vec<&str> = sorted.items.iter().map(|i| i.text.as_str()).collect();
    assert_eq!(texts[0], "alpha ****");
    assert_eq!(texts[3], "delta *");
    // ...and run metadata says, loudly, that no judgement ran in logprob mode.
    assert!(sorted.meta.evidence_judgements > 0);
    assert_eq!(sorted.meta.logprob_mode_judgements, 0);
    let mass = sorted.meta.evidence_visible_mass_mean.unwrap();
    assert!(
        (mass - 1.0).abs() < 1e-9,
        "sampled mode: empirical mass 1.0"
    );
}

#[tokio::test]
async fn silently_missing_logprobs_also_degrade_to_sampled() {
    let server = start_judge(LetterJudge {
        reject_logprobs: false,
        omit_logprobs: true,
    })
    .await;
    let execution = RerankExecution::new(gateway_for(&server), Attribution::new("test::evidence"));
    let sorted = sort_texts(items(), "brightness", execution, letter_opts())
        .await
        .unwrap();
    assert!(sorted.meta.evidence_judgements > 0);
    assert_eq!(sorted.meta.logprob_mode_judgements, 0);
}

#[tokio::test]
async fn evidence_moments_survive_the_cache_round_trip() {
    let server = start_judge(LetterJudge {
        reject_logprobs: false,
        omit_logprobs: false,
    })
    .await;
    let dir = std::env::temp_dir().join(format!("cardinal-evidence-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let cache_path = dir.join("cache.sqlite");
    let _ = std::fs::remove_file(&cache_path);
    let cache = SqlitePairwiseCache::new(&cache_path).unwrap();

    let run = |seed: u64| {
        let gateway = gateway_for(&server);
        let cache = cache.clone();
        async move {
            let execution = RerankExecution::new(gateway, Attribution::new("test::evidence"))
                .cache(&cache)
                .run_options(llmsort::rerank::RerankRunOptions {
                    rng_seed: Some(seed),
                    cache_only: false,
                });
            sort_texts(items(), "brightness", execution, letter_opts())
                .await
                .unwrap()
        }
    };
    let first = run(7).await;
    assert_eq!(first.meta.comparisons_cached, 0);
    assert_eq!(
        first.meta.logprob_mode_judgements,
        first.meta.evidence_judgements
    );

    let second = run(7).await;
    assert!(second.meta.comparisons_cached > 0, "second run hits cache");
    // The cached judgements still carry evidence moments — the solver
    // consumes measured variance on replay too.
    assert_eq!(
        second.meta.evidence_judgements, second.meta.comparisons_used,
        "cached judgements must not lose their PMF moments"
    );
    let texts: Vec<&str> = second.items.iter().map(|i| i.text.as_str()).collect();
    assert_eq!(texts[0], "alpha ****");
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn cli_sort_with_letter_template_prints_evidence_diagnostics() {
    let server = start_judge(LetterJudge {
        reject_logprobs: false,
        omit_logprobs: false,
    })
    .await;
    let bin = {
        let cargo_bin = option_env!("CARGO_BIN_EXE_cardinal").filter(|p| !p.is_empty());
        match cargo_bin {
            Some(p) => std::path::PathBuf::from(p),
            None => panic!("CARGO_BIN_EXE_cardinal not set"),
        }
    };
    let mut child = Command::new(bin)
        .args([
            "sort",
            "--by",
            "brightness",
            "--model",
            "test/judge",
            "--no-cache",
            "--seed",
            "7",
            "--budget",
            "24",
            "--template",
            "ratio_letter_v1",
        ])
        .env("OPENROUTER_API_KEY", "sk-test")
        .env("OPENROUTER_BASE_URL", server.uri())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        use std::io::Write;
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(b"alpha ****\ndelta *\n")
            .unwrap();
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout, "alpha ****\ndelta *\n");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("evidence:") && stderr.contains("logprob-mode"),
        "stderr: {stderr}"
    );
}

/// Judge speaking the ordinal dialect: answers 'A', 'B', or '=' only.
#[derive(Clone, Copy)]
struct OrdinalJudge;

impl Respond for OrdinalJudge {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap_or_default();
        let wants_logprobs = body
            .get("logprobs")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let user = body["messages"]
            .as_array()
            .and_then(|m| {
                m.iter()
                    .find(|x| x["role"] == "user")
                    .and_then(|x| x["content"].as_str())
            })
            .unwrap_or("")
            .to_string();
        let a = extract_between(&user, "<entity_A>", "</entity_A>").unwrap_or("");
        let b = extract_between(&user, "<entity_B>", "</entity_B>").unwrap_or("");
        let d = stars(a) - stars(b);
        let answer = if d > 0 {
            "A"
        } else if d < 0 {
            "B"
        } else {
            "="
        };
        let mut response = json!({
            "choices": [{
                "message": { "content": answer },
                "finish_reason": "stop"
            }],
            "usage": { "prompt_tokens": 40, "completion_tokens": 1 }
        });
        if wants_logprobs {
            let other = if answer == "A" { "B" } else { "A" };
            response["choices"][0]["logprobs"] = json!({
                "content": [{
                    "token": answer,
                    "logprob": -0.105_360_5, // 0.9
                    "top_logprobs": [
                        { "token": answer, "logprob": -0.105_360_5 },
                        { "token": other, "logprob": -2.995_732_3 }, // 0.05
                    ]
                }]
            });
        }
        ResponseTemplate::new(200).set_body_json(response)
    }
}

#[tokio::test]
async fn ordinal_dialect_judge_full_pipeline_with_residual_diagnostic() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(OrdinalJudge)
        .mount(&server)
        .await;
    let execution = RerankExecution::new(gateway_for(&server), Attribution::new("test::evidence"));
    let mut opts = letter_opts();
    opts.prompt_template_slug = Some("ordinal_letter_v1".into());
    let sorted = sort_texts(items(), "brightness", execution, opts)
        .await
        .unwrap();
    let texts: Vec<&str> = sorted.items.iter().map(|i| i.text.as_str()).collect();
    assert_eq!(
        texts,
        vec!["alpha ****", "bravo ***", "charlie **", "delta *"],
        "direction-only logprob evidence recovers the planted order"
    );
    assert_eq!(
        sorted.meta.logprob_mode_judgements,
        sorted.meta.evidence_judgements
    );
    // Content-driven judge: both presentation orders yield exactly
    // reflected PMFs, so the order residual is ~0 nats.
    let residual = sorted.meta.evidence_order_residual_mean_abs.unwrap();
    assert!(residual < 1e-9, "unbiased judge residual: {residual}");
}

#[tokio::test]
async fn position_biased_letter_judge_shows_nonzero_order_residual() {
    /// Always answers 'D' (slot A wins) regardless of content.
    #[derive(Clone, Copy)]
    struct AlwaysSlotA;
    impl Respond for AlwaysSlotA {
        fn respond(&self, request: &Request) -> ResponseTemplate {
            let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap_or_default();
            let wants_logprobs = body
                .get("logprobs")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let mut response = json!({
                "choices": [{
                    "message": { "content": "D" },
                    "finish_reason": "stop"
                }],
                "usage": { "prompt_tokens": 40, "completion_tokens": 1 }
            });
            if wants_logprobs {
                response["choices"][0]["logprobs"] = json!({
                    "content": [{
                        "token": "D",
                        "logprob": -0.105_360_5,
                        "top_logprobs": [
                            { "token": "D", "logprob": -0.105_360_5 },
                            { "token": "A", "logprob": -2.995_732_3 },
                        ]
                    }]
                });
            }
            ResponseTemplate::new(200).set_body_json(response)
        }
    }
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(AlwaysSlotA)
        .mount(&server)
        .await;
    let execution = RerankExecution::new(gateway_for(&server), Attribution::new("test::evidence"));
    let sorted = sort_texts(items(), "brightness", execution, letter_opts())
        .await
        .unwrap();
    // Pure position bias: presented means point the same way in both
    // orders, so residuals are ~2x the per-order mean, far from zero.
    let residual = sorted.meta.evidence_order_residual_mean_abs.unwrap();
    assert!(
        residual > 0.5,
        "pure position bias must show a large order residual: {residual}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn calibrate_catches_position_prior_and_clears_honest_judge() {
    // Null pairs: identical text in both slots. The honest judge answers
    // parity ('A'); the biased one answers 'D' (slot A wins) every time.
    #[derive(Clone, Copy)]
    struct NullJudge {
        biased: bool,
    }
    impl Respond for NullJudge {
        fn respond(&self, request: &Request) -> ResponseTemplate {
            let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap_or_default();
            let wants_logprobs = body
                .get("logprobs")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let letter = if self.biased { "D" } else { "A" };
            let mut response = json!({
                "choices": [{
                    "message": { "content": letter },
                    "finish_reason": "stop"
                }],
                "usage": { "prompt_tokens": 40, "completion_tokens": 1 }
            });
            if wants_logprobs {
                response["choices"][0]["logprobs"] = json!({
                    "content": [{
                        "token": letter,
                        "logprob": -0.105_360_5,
                        "top_logprobs": [
                            { "token": letter, "logprob": -0.105_360_5 },
                        ]
                    }]
                });
            }
            ResponseTemplate::new(200).set_body_json(response)
        }
    }

    async fn run(biased: bool) -> String {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(NullJudge { biased })
            .mount(&server)
            .await;
        let bin = env!("CARGO_BIN_EXE_cardinal");
        let output = Command::new(bin)
            .args(["calibrate", "--models", "test/judge", "--nulls", "3"])
            .env("OPENROUTER_API_KEY", "sk-test")
            .env("OPENROUTER_BASE_URL", server.uri())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    }

    let honest = run(false).await;
    let honest_line = honest.lines().nth(1).unwrap_or_default().to_string();
    // Honest judge: parity ~1.0, bias ~0.
    assert!(honest_line.contains("1.000"), "honest: {honest_line}");
    assert!(honest_line.contains("0.0000"), "honest bias: {honest_line}");

    let biased = run(true).await;
    let biased_line = biased.lines().nth(1).unwrap_or_default().to_string();
    // Biased judge: P(A) ~1.0 and a large positive bias in nats.
    let bias_field: f64 = biased_line
        .split_whitespace()
        .nth(4)
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.0);
    assert!(
        bias_field > 0.2,
        "pure position prior must show large bias-nats: {biased_line}"
    );
}

#[tokio::test]
async fn frustration_diagnostic_zero_for_transitive_judge_high_for_cyclic() {
    // Transitive star judge: curl fraction ~0.
    let server = start_judge(LetterJudge {
        reject_logprobs: false,
        omit_logprobs: false,
    })
    .await;
    let execution =
        RerankExecution::new(gateway_for(&server), Attribution::new("test::frustration"));
    let sorted = sort_texts(items(), "brightness", execution, letter_opts())
        .await
        .unwrap();
    let frustration = sorted.meta.judgement_frustration_mean.unwrap();
    // Not ~0, and honestly so: the star judge is transitive in DIRECTION
    // but answers bucketed magnitudes (1.5 / 3.9), which are not
    // log-additive — quantization alone injects real curl (~0.13 here).
    // The diagnostic measuring that is a feature: magnitude inconsistency is
    // inconsistency. The pin separates quantization-level curl from
    // structural cycles (below).
    assert!(
        frustration < 0.2,
        "directionally transitive judge: quantization-level curl only: {frustration}"
    );

    // Cyclic judge: prefers by (stars + 2) mod 4 — a planted rock-paper-
    // scissors structure over the roster. The curl fraction must be large:
    // no scores can explain a cycle.
    #[derive(Clone, Copy)]
    struct CyclicJudge;
    impl Respond for CyclicJudge {
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
            let a = extract_between(&user, "<entity_A>", "</entity_A>").unwrap_or("");
            let b = extract_between(&user, "<entity_B>", "</entity_B>").unwrap_or("");
            // Rock-paper-scissors over star counts modulo the roster:
            // x beats y iff x == y+1 (mod 4).
            let sa = stars(a).rem_euclid(4);
            let sb = stars(b).rem_euclid(4);
            let a_wins = (sa - sb).rem_euclid(4) == 1;
            let letter = if sa == sb {
                'A'
            } else if a_wins {
                'H'
            } else {
                'h'
            };
            let mut response = json!({
                "choices": [{
                    "message": { "content": letter.to_string() },
                    "finish_reason": "stop"
                }],
                "usage": { "prompt_tokens": 40, "completion_tokens": 1 }
            });
            response["choices"][0]["logprobs"] = json!({
                "content": [{
                    "token": letter.to_string(),
                    "logprob": -0.105_360_5,
                    "top_logprobs": [
                        { "token": letter.to_string(), "logprob": -0.105_360_5 },
                    ]
                }]
            });
            ResponseTemplate::new(200).set_body_json(response)
        }
    }
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(CyclicJudge)
        .mount(&server)
        .await;
    let execution =
        RerankExecution::new(gateway_for(&server), Attribution::new("test::frustration"));
    let sorted = sort_texts(items(), "brightness", execution, letter_opts())
        .await
        .unwrap();
    let frustration = sorted.meta.judgement_frustration_mean.unwrap();
    assert!(
        frustration > 0.3,
        "planted rock-paper-scissors must show high curl: {frustration}"
    );
}
