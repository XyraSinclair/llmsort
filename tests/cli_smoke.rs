use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

use llmsort::rerank::{
    AttributeScoreSummary, MultiRerankAttributeSpec, MultiRerankEntity, MultiRerankEntityResult,
    MultiRerankMeta, MultiRerankRequest, MultiRerankResponse, MultiRerankTopKSpec,
    RerankStopReason,
};
use tempfile::tempdir;

#[allow(dead_code)]
fn llmsort_bin() -> PathBuf {
    let cargo_bin = option_env!("CARGO_BIN_EXE_llmsort").filter(|path| !path.is_empty());
    if let Some(path) = cargo_bin {
        let path = PathBuf::from(path);
        if path.exists() {
            return path;
        }
    }

    let test_exe =
        std::env::current_exe().expect("failed to resolve current integration test binary path");
    let deps_dir = test_exe.parent().unwrap_or_else(|| {
        panic!(
            "integration test binary path has no parent directory: {}",
            test_exe.display()
        )
    });
    let target_dir = deps_dir.parent().unwrap_or_else(|| {
        panic!(
            "integration test binary parent has no target directory: {}",
            deps_dir.display()
        )
    });
    let fallback = target_dir.join(format!("cardinal{}", std::env::consts::EXE_SUFFIX));

    if fallback.exists() {
        return fallback;
    }

    panic!(
        "failed to locate compiled llmsort binary; CARGO_BIN_EXE_llmsort={:?}; \
         integration test binary={}; fallback path={}. Run `cargo test --test cli_smoke` \
         so Cargo builds the llmsort binary before the smoke tests run",
        cargo_bin,
        test_exe.display(),
        fallback.display()
    );
}

#[test]
fn cli_validate_example_request_smoke() {
    let request_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/multi-rerank-request.json");

    let bin = llmsort_bin();
    let output = Command::new(&bin)
        .args(["validate", "--request"])
        .arg(&request_path)
        .output()
        .unwrap_or_else(|err| {
            panic!(
                "failed to run cardinal validate at {}: {err}",
                bin.display()
            )
        });

    assert!(
        output.status.success(),
        "cardinal validate exited with {}; stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("valid request:"),
        "stdout did not confirm validation: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        output.stderr.is_empty(),
        "validate should not warn on the checked-in example request; stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn cli_rerank_validates_before_gateway_setup() {
    let dir = tempdir().unwrap();
    let request_path = dir.path().join("invalid-request.json");
    let out_path = dir.path().join("out.json");
    std::fs::write(
        &request_path,
        serde_json::json!({
            "entities": [
                {"id": "a", "text": "A"},
                {"id": "b", "text": "B"}
            ],
            "attributes": [
                {"id": "clarity", "prompt": "clarity", "weight": 1.0}
            ],
            "topk": {"k": 0}
        })
        .to_string(),
    )
    .unwrap();

    let bin = llmsort_bin();
    let output = Command::new(&bin)
        .args(["rerank", "--request"])
        .arg(&request_path)
        .arg("--out")
        .arg(&out_path)
        .env_remove("OPENROUTER_API_KEY")
        .output()
        .unwrap_or_else(|err| panic!("failed to run cardinal rerank at {}: {err}", bin.display()));

    assert!(
        !output.status.success(),
        "invalid request should fail before gateway setup"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("topk.k must be >= 1"),
        "expected validation error before gateway setup; stderr={stderr}"
    );
}

#[test]
fn cli_rerank_cache_only_is_keyless() {
    let dir = tempdir().unwrap();
    let request_path = dir.path().join("request.json");
    let out_path = dir.path().join("out.json");
    let cache_path = dir.path().join("empty-cache.sqlite");
    std::fs::write(
        &request_path,
        serde_json::json!({
            "entities": [
                {"id": "a", "text": "A"},
                {"id": "b", "text": "B"}
            ],
            "attributes": [
                {"id": "clarity", "prompt": "clarity", "weight": 1.0}
            ],
            "topk": {"k": 1},
            "comparison_budget": 1
        })
        .to_string(),
    )
    .unwrap();

    let output = Command::new(llmsort_bin())
        .args(["rerank", "--request"])
        .arg(&request_path)
        .arg("--out")
        .arg(&out_path)
        .arg("--cache")
        .arg(&cache_path)
        .arg("--cache-only")
        .env_remove("OPENROUTER_API_KEY")
        .output()
        .expect("run keyless cache-only rerank");

    assert!(!output.status.success(), "empty cache should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("cache_only") || stderr.contains("cache miss"),
        "expected a cache miss after keyless setup; stderr={stderr}"
    );
    assert!(
        !stderr.contains("OPENROUTER_API_KEY"),
        "cache-only mode must not require a provider key; stderr={stderr}"
    );
}

#[test]
fn cli_report_json_smoke() {
    let dir = tempdir().unwrap();

    let request_path = dir.path().join("request.json");
    let response_path = dir.path().join("response.json");
    let out_path = dir.path().join("report.json");
    let md_path = dir.path().join("report.md");

    let req = MultiRerankRequest {
        nonce_draws: None,
        entities: vec![
            MultiRerankEntity {
                id: "a".into(),
                text: "Entity A text".into(),
            },
            MultiRerankEntity {
                id: "b".into(),
                text: "Entity B text".into(),
            },
        ],
        attributes: vec![MultiRerankAttributeSpec {
            id: "clarity".into(),
            prompt: "clarity of explanation".into(),
            prompt_template_slug: Some("canonical_v2".into()),
            weight: 1.0,
        }],
        topk: MultiRerankTopKSpec {
            k: 1,
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
        comparison_budget: Some(1),
        latency_budget_ms: None,
        max_cost_nanodollars: None,
        model: Some("openai/gpt-5-mini".into()),
        rater_id: None,
        comparison_concurrency: Some(1),
        max_pair_repeats: Some(1),
        randomize_presentation_order: true,
        counterbalance_pairs: false,
    };

    let mut a_scores = HashMap::new();
    a_scores.insert(
        "clarity".to_string(),
        AttributeScoreSummary {
            latent_mean: 1.0,
            latent_std: 0.1,
            z_score: 0.5,
            min_normalized: 2.0,
            percentile: 0.75,
        },
    );
    let mut b_scores = HashMap::new();
    b_scores.insert(
        "clarity".to_string(),
        AttributeScoreSummary {
            latent_mean: 0.0,
            latent_std: 0.2,
            z_score: -0.5,
            min_normalized: 1.0,
            percentile: 0.25,
        },
    );

    let resp = MultiRerankResponse {
        pareto_front: Vec::new(),
        attribute_correlations: Vec::new(),
        entities: vec![
            MultiRerankEntityResult {
                id: "a".into(),
                rank: Some(1),
                feasible: true,
                u_mean: 1.0,
                u_std: 0.1,
                p_flip: 0.01,
                attribute_scores: a_scores,
            },
            MultiRerankEntityResult {
                id: "b".into(),
                rank: Some(2),
                feasible: true,
                u_mean: 0.0,
                u_std: 0.2,
                p_flip: 0.02,
                attribute_scores: b_scores,
            },
        ],
        meta: MultiRerankMeta {
            global_topk_error: 0.2,
            tolerated_error: req.topk.tolerated_error,
            k: req.topk.k,
            band_size: req.topk.band_size,
            comparisons_attempted: 3,
            comparisons_used: 2,
            comparisons_refused: 1,
            comparisons_cached: 1,
            comparison_budget: 3,
            latency_ms: 1,
            model_used: "openai/gpt-5-mini".into(),
            rater_id_used: "openai/gpt-5-mini".into(),
            engine_spec: None,
            warm_start_observations: 0,
            provider_input_tokens: 123,
            provider_output_tokens: 45,
            provider_cost_nanodollars: 123_456_789,
            provider_cost_is_estimate: false,
            entities_pruned: 0,
            evidence_judgements: 0,
            logprob_mode_judgements: 0,
            evidence_visible_mass_mean: None,
            evidence_order_residual_mean_abs: None,
            evidence_sigma_w: None,
            evidence_obs_sigma_rms: None,
            judgement_frustration_mean: None,
            pairs_counterbalanced: 0,
            position_flips: 0,
            stop_reason: RerankStopReason::BudgetExhausted,
        },
    };

    std::fs::write(&request_path, serde_json::to_string_pretty(&req).unwrap()).unwrap();
    std::fs::write(&response_path, serde_json::to_string_pretty(&resp).unwrap()).unwrap();

    let bin = llmsort_bin();
    let status = Command::new(&bin)
        .args(["report", "--format", "json"])
        .arg("--request")
        .arg(&request_path)
        .arg("--response")
        .arg(&response_path)
        .arg("--out")
        .arg(&out_path)
        .status()
        .unwrap_or_else(|err| panic!("failed to run cardinal report at {}: {err}", bin.display()));
    assert!(status.success(), "cardinal report exited with {status}");

    let raw = std::fs::read_to_string(&out_path).unwrap();
    let v: serde_json::Value = serde_json::from_str(&raw).unwrap();

    assert!(
        v.get("request_hash")
            .and_then(|h| h.as_str())
            .unwrap()
            .len()
            >= 16
    );
    assert_eq!(
        v.pointer("/summary/stop_reason")
            .and_then(|s| s.as_str())
            .unwrap(),
        "budget_exhausted"
    );
    assert_eq!(
        v.pointer("/summary/provider_input_tokens")
            .and_then(|n| n.as_u64())
            .unwrap(),
        123
    );
    assert_eq!(
        v.pointer("/summary/provider_output_tokens")
            .and_then(|n| n.as_u64())
            .unwrap(),
        45
    );
    assert_eq!(
        v.pointer("/summary/provider_cost_nanodollars")
            .and_then(|n| n.as_i64())
            .unwrap(),
        123_456_789
    );
    assert_eq!(
        v.pointer("/attributes/0/id")
            .and_then(|s| s.as_str())
            .unwrap(),
        "clarity"
    );
    assert_eq!(
        v.pointer("/top_entities/0/id")
            .and_then(|s| s.as_str())
            .unwrap(),
        "a"
    );

    let status = Command::new(&bin)
        .args(["report"])
        .arg("--request")
        .arg(&request_path)
        .arg("--response")
        .arg(&response_path)
        .arg("--out")
        .arg(&md_path)
        .status()
        .unwrap_or_else(|err| panic!("failed to run cardinal report at {}: {err}", bin.display()));
    assert!(status.success(), "cardinal report exited with {status}");

    let markdown = std::fs::read_to_string(&md_path).unwrap();
    assert!(markdown.contains("## Run Status"));
    assert!(markdown.contains("Stop reason: `budget_exhausted`"));
    assert!(markdown.contains("Comparison budget: 3"));
    assert!(markdown.contains("Provider tokens input/output/total: 123/45/168"));
    assert!(markdown.contains("Provider cost: $0.123456789"));
    assert!(markdown.contains("## Warnings / Degraded State"));
    assert!(markdown.contains("non-converged stop reason `budget_exhausted`"));
    assert!(markdown.contains("Global top-k error 0.2000 exceeds tolerated error 0.1000"));
    assert!(markdown.contains("1 comparison(s) were refused"));
    assert!(markdown.contains("1 comparison(s) came from cache"));
    assert!(markdown.contains("budget before meeting the stopping tolerance"));
    assert!(markdown.contains("`clarity`: latent 1.000 ± 0.100"));
}

#[test]
fn cli_report_rejects_unsupported_format_before_writing() {
    let dir = tempdir().unwrap();
    let bin = llmsort_bin();
    let out_path = dir.path().join("report.html");

    let output = Command::new(&bin)
        .args(["report", "--format", "html"])
        .arg("--request")
        .arg(dir.path().join("request.json"))
        .arg("--response")
        .arg(dir.path().join("response.json"))
        .arg("--out")
        .arg(&out_path)
        .output()
        .unwrap_or_else(|err| panic!("failed to run cardinal report at {}: {err}", bin.display()));

    assert!(
        !output.status.success(),
        "report should reject unsupported output formats"
    );
    assert!(
        !out_path.exists(),
        "report should not write output for unsupported formats"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("invalid value") && stderr.contains("json") && stderr.contains("markdown"),
        "expected supported-format validation error; stderr={stderr}"
    );
}

#[test]
fn cli_report_rejects_zero_top_n_before_writing() {
    let dir = tempdir().unwrap();
    let bin = llmsort_bin();
    let out_path = dir.path().join("report.md");

    let output = Command::new(&bin)
        .args(["report", "--top-n", "0"])
        .arg("--request")
        .arg(dir.path().join("request.json"))
        .arg("--response")
        .arg(dir.path().join("response.json"))
        .arg("--out")
        .arg(&out_path)
        .output()
        .unwrap_or_else(|err| panic!("failed to run cardinal report at {}: {err}", bin.display()));

    assert!(!output.status.success(), "report should reject --top-n 0");
    assert!(
        !out_path.exists(),
        "report should not write output when --top-n is invalid"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("invalid value") && stderr.contains("--top-n"),
        "expected top-n validation error; stderr={stderr}"
    );
}

#[test]
fn cli_report_rejects_stale_response_entities() {
    let dir = tempdir().unwrap();
    let request_path = dir.path().join("request.json");
    let response_path = dir.path().join("response.json");
    let out_path = dir.path().join("report.json");

    let req = MultiRerankRequest {
        nonce_draws: None,
        entities: vec![
            MultiRerankEntity {
                id: "a".into(),
                text: "Entity A text".into(),
            },
            MultiRerankEntity {
                id: "b".into(),
                text: "Entity B text".into(),
            },
        ],
        attributes: vec![MultiRerankAttributeSpec {
            id: "clarity".into(),
            prompt: "clarity of explanation".into(),
            prompt_template_slug: Some("canonical_v2".into()),
            weight: 1.0,
        }],
        topk: MultiRerankTopKSpec {
            k: 1,
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
        comparison_budget: Some(1),
        latency_budget_ms: None,
        max_cost_nanodollars: None,
        model: Some("openai/gpt-5-mini".into()),
        rater_id: None,
        comparison_concurrency: Some(1),
        max_pair_repeats: Some(1),
        randomize_presentation_order: true,
        counterbalance_pairs: false,
    };

    let mut scores = HashMap::new();
    scores.insert(
        "clarity".to_string(),
        AttributeScoreSummary {
            latent_mean: 1.0,
            latent_std: 0.1,
            z_score: 0.5,
            min_normalized: 2.0,
            percentile: 0.75,
        },
    );

    let resp = MultiRerankResponse {
        pareto_front: Vec::new(),
        attribute_correlations: Vec::new(),
        entities: vec![MultiRerankEntityResult {
            id: "ghost".into(),
            rank: Some(1),
            feasible: true,
            u_mean: 1.0,
            u_std: 0.1,
            p_flip: 0.01,
            attribute_scores: scores,
        }],
        meta: MultiRerankMeta {
            global_topk_error: 0.0,
            tolerated_error: req.topk.tolerated_error,
            k: req.topk.k,
            band_size: req.topk.band_size,
            comparisons_attempted: 1,
            comparisons_used: 1,
            comparisons_refused: 0,
            comparisons_cached: 0,
            comparison_budget: 1,
            latency_ms: 1,
            model_used: "openai/gpt-5-mini".into(),
            rater_id_used: "openai/gpt-5-mini".into(),
            engine_spec: None,
            warm_start_observations: 0,
            provider_input_tokens: 1,
            provider_output_tokens: 1,
            provider_cost_nanodollars: 1,
            provider_cost_is_estimate: false,
            entities_pruned: 0,
            evidence_judgements: 0,
            logprob_mode_judgements: 0,
            evidence_visible_mass_mean: None,
            evidence_order_residual_mean_abs: None,
            evidence_sigma_w: None,
            evidence_obs_sigma_rms: None,
            judgement_frustration_mean: None,
            pairs_counterbalanced: 0,
            position_flips: 0,
            stop_reason: RerankStopReason::ToleratedErrorMet,
        },
    };

    std::fs::write(&request_path, serde_json::to_string_pretty(&req).unwrap()).unwrap();
    std::fs::write(&response_path, serde_json::to_string_pretty(&resp).unwrap()).unwrap();

    let bin = llmsort_bin();
    let output = Command::new(&bin)
        .args(["report", "--format", "json"])
        .arg("--request")
        .arg(&request_path)
        .arg("--response")
        .arg(&response_path)
        .arg("--out")
        .arg(&out_path)
        .output()
        .unwrap_or_else(|err| panic!("failed to run cardinal report at {}: {err}", bin.display()));

    assert!(
        !output.status.success(),
        "report should reject a response that does not match the request"
    );
    assert!(
        !out_path.exists(),
        "report should not write output for a stale response"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("response entity 'ghost' is not present in the request"),
        "expected stale response validation error; stderr={stderr}"
    );
}
