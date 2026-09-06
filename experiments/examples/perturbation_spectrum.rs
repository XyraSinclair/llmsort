//! P1 — the perturbation spectrum (IDEATION.md, logprob-efficiency-2026-09-05).
//!
//! For a fixed entity pool and one base attribute, elicit every unordered
//! pair through the evidence rail (`ratio_letter_v1`, decision-token PMF)
//! under a ladder of elicitation perturbations:
//!
//! - `nonce`   — base attribute, K draw-token nonces (null suffix; the
//!               production `nonce_draws` instrument);
//! - `jitter`  — whitespace-jitter variants of the attribute (semantically
//!               null, token stream perturbed mid-prompt; E8 apparatus);
//! - `para`    — paraphrase variants of the attribute (semantics held,
//!               framing genuinely re-worded);
//!
//! each in BOTH presentation orders (the orientation rung falls out of the
//! same rows). Output: one JSONL row per call with the PMF moments — the
//! per-rung scatter analysis lives in python beside the pack.
//!
//! Usage:
//!   CARDINAL_SERIATE_LOGPROB_MODELS=<model>:20 \
//!   cargo run --release -p llmsort-experiments --example perturbation_spectrum -- \
//!     <spec.json> <out.jsonl>
//!
//! spec.json:
//! {
//!   "model": "gemma4-31b",
//!   "base_url": "http://127.0.0.1:8023/v1",
//!   "concurrency": 8,
//!   "nonce_draws": 8,
//!   "entities": [{"id": "...", "text": "..."}, ...],
//!   "axes": [{"rung": "nonce|jitter|para", "variant": "base|j1|p1|...",
//!             "prompt": "..."}, ...]
//! }
//!
//! The `nonce` rung must appear exactly once (variant "base"); jitter/para
//! rungs get one call per (pair, orientation, variant).

use std::sync::Arc;

use futures::StreamExt as _;
use serde::{Deserialize, Serialize};

use llmsort::gateway::openrouter::OpenRouterAdapter;
use llmsort::gateway::{Attribution, GatewayConfig, NoopUsageSink, ProviderGateway};
use llmsort::rerank::comparison::{
    compare_pair, PairwiseComparisonAttribute, PairwiseComparisonEntity,
    PairwiseComparisonRequest, PairwiseComparisonSpec, RATIO_LETTER_SLUG,
};
use llmsort::rerank::types::{HigherRanked, PairwiseJudgement};

#[derive(Deserialize)]
struct Spec {
    model: String,
    base_url: String,
    #[serde(default = "default_concurrency")]
    concurrency: usize,
    #[serde(default = "default_nonce_draws")]
    nonce_draws: u32,
    entities: Vec<SpecEntity>,
    axes: Vec<SpecAxis>,
}

fn default_concurrency() -> usize {
    8
}
fn default_nonce_draws() -> u32 {
    8
}

#[derive(Deserialize)]
struct SpecEntity {
    id: String,
    text: String,
}

#[derive(Deserialize, Clone)]
struct SpecAxis {
    rung: String,
    variant: String,
    prompt: String,
}

#[derive(Serialize)]
struct Row {
    rung: String,
    variant: String,
    /// Presented slot A / slot B entity ids.
    entity_a: String,
    entity_b: String,
    /// 0 = canonical (pool order), 1 = flipped presentation.
    orientation: u8,
    /// Nonce draw index (0 for single-draw rungs).
    draw: u32,
    nonce: Option<String>,
    /// PMF moments, presented A-over-B (None = degraded/no logprobs).
    log_ratio_mean: Option<f64>,
    log_ratio_var: Option<f64>,
    visible_mass: Option<f64>,
    logprob_mode: Option<bool>,
    higher_ranked_a: bool,
    ratio: f64,
    confidence: f64,
    input_tokens: u32,
    output_tokens: u32,
    error: Option<String>,
}

struct Call {
    axis: SpecAxis,
    a: usize,
    b: usize,
    orientation: u8,
    draw: u32,
    nonce: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let spec_path = args.next().ok_or("usage: perturbation_spectrum <spec.json> <out.jsonl>")?;
    let out_path = args.next().ok_or("usage: perturbation_spectrum <spec.json> <out.jsonl>")?;
    let spec: Spec = serde_json::from_str(&std::fs::read_to_string(&spec_path)?)?;
    let n = spec.entities.len();
    if n < 2 {
        return Err("need at least 2 entities".into());
    }

    let adapter = OpenRouterAdapter::with_config(
        "local-unauthenticated",
        spec.base_url.clone(),
        std::time::Duration::from_secs(600),
        None,
        None,
    )?;
    let gateway: Arc<ProviderGateway<NoopUsageSink>> = Arc::new(ProviderGateway::with_config(
        adapter,
        Arc::new(NoopUsageSink),
        GatewayConfig::default(),
    ));

    // Resume: rows already streamed to out_path are not re-elicited.
    let mut have: std::collections::HashSet<String> = std::collections::HashSet::new();
    if let Ok(existing) = std::fs::read_to_string(&out_path) {
        for line in existing.lines() {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                if v.get("error").map_or(true, |e| e.is_null()) {
                    have.insert(format!(
                        "{}|{}|{}|{}|{}",
                        v["variant"].as_str().unwrap_or(""),
                        v["entity_a"].as_str().unwrap_or(""),
                        v["entity_b"].as_str().unwrap_or(""),
                        v["orientation"],
                        v["draw"]
                    ));
                }
            }
        }
        if !have.is_empty() {
            eprintln!("resume: {} rows already present, skipping", have.len());
        }
    }

    // Build the full call plan up front; every call is independent.
    let mut calls: Vec<Call> = Vec::new();
    for axis in &spec.axes {
        let draws: Vec<(u32, Option<String>)> = if axis.rung == "nonce" {
            (0..spec.nonce_draws)
                .map(|d| (d, Some(format!("pspec-{d:02}"))))
                .collect()
        } else {
            // Fresh single draw; nonce still set so no SQLite cache layer
            // (none is passed anyway) and no provider-side dedup ambiguity.
            vec![(0, Some("pspec-00".to_string()))]
        };
        for a in 0..n {
            for b in (a + 1)..n {
                for orientation in [0u8, 1u8] {
                    for (draw, nonce) in &draws {
                        let key = format!(
                            "{}|{}|{}|{}|{}",
                            axis.variant,
                            spec.entities[if orientation == 0 { a } else { b }].id,
                            spec.entities[if orientation == 0 { b } else { a }].id,
                            orientation,
                            draw
                        );
                        if have.contains(&key) {
                            continue;
                        }
                        calls.push(Call {
                            axis: axis.clone(),
                            a,
                            b,
                            orientation,
                            draw: *draw,
                            nonce: nonce.clone(),
                        });
                    }
                }
            }
        }
    }
    eprintln!(
        "perturbation_spectrum: {} entities, {} axes, {} calls, concurrency {}",
        n,
        spec.axes.len(),
        calls.len(),
        spec.concurrency
    );

    let spec = Arc::new(spec);
    let done = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let total = calls.len();
    // Stream rows to disk as they complete: the engine may be restarted
    // under us (GPU borrow arbitration) and a partial pack must survive.
    let out_file = Arc::new(std::sync::Mutex::new(std::io::BufWriter::new(
        std::fs::OpenOptions::new().create(true).append(true).open(&out_path)?,
    )));
    let rows: Vec<Row> = futures::stream::iter(calls)
        .map(|call| {
            let gateway = Arc::clone(&gateway);
            let spec = Arc::clone(&spec);
            let done = Arc::clone(&done);
            let out_file = Arc::clone(&out_file);
            async move {
                let (ai, bi) = if call.orientation == 0 {
                    (call.a, call.b)
                } else {
                    (call.b, call.a)
                };
                let ea = &spec.entities[ai];
                let eb = &spec.entities[bi];
                let request = PairwiseComparisonRequest {
                    spec: PairwiseComparisonSpec {
                        model: &spec.model,
                        attribute: PairwiseComparisonAttribute {
                            id: &call.axis.variant,
                            prompt: &call.axis.prompt,
                            prompt_template_slug: Some(RATIO_LETTER_SLUG),
                        },
                        entity_a: PairwiseComparisonEntity { id: &ea.id, text: &ea.text },
                        entity_b: PairwiseComparisonEntity { id: &eb.id, text: &eb.text },
                    },
                    cache_only: false,
                    attribution: Attribution::new("examples::perturbation_spectrum"),
                    nonce: call.nonce.clone(),
                };
                let result = compare_pair(gateway.as_ref(), None, request).await;
                let k = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                if k % 100 == 0 {
                    eprintln!("  {k}/{total}");
                }
                let row = match result {
                    Ok((judgement, usage)) => {
                        let m = usage.evidence_moments;
                        let (higher_ranked_a, ratio, confidence, refused) = match judgement {
                            PairwiseJudgement::Observation {
                                higher_ranked,
                                ratio,
                                confidence,
                            } => (higher_ranked == HigherRanked::A, ratio, confidence, false),
                            PairwiseJudgement::Refused => (false, f64::NAN, f64::NAN, true),
                        };
                        Row {
                            rung: call.axis.rung,
                            variant: call.axis.variant,
                            entity_a: ea.id.clone(),
                            entity_b: eb.id.clone(),
                            orientation: call.orientation,
                            draw: call.draw,
                            nonce: call.nonce,
                            log_ratio_mean: m.map(|m| m.log_ratio_mean),
                            log_ratio_var: m.map(|m| m.log_ratio_var),
                            visible_mass: m.map(|m| m.visible_mass),
                            logprob_mode: m.map(|m| m.logprob_mode),
                            higher_ranked_a,
                            ratio,
                            confidence,
                            input_tokens: usage.input_tokens,
                            output_tokens: usage.output_tokens,
                            error: refused.then(|| "refused".to_string()),
                        }
                    }
                    Err(err) => Row {
                        rung: call.axis.rung,
                        variant: call.axis.variant,
                        entity_a: ea.id.clone(),
                        entity_b: eb.id.clone(),
                        orientation: call.orientation,
                        draw: call.draw,
                        nonce: call.nonce,
                        log_ratio_mean: None,
                        log_ratio_var: None,
                        visible_mass: None,
                        logprob_mode: None,
                        higher_ranked_a: false,
                        ratio: f64::NAN,
                        confidence: f64::NAN,
                        input_tokens: 0,
                        output_tokens: 0,
                        error: Some(err.to_string()),
                    },
                };
                {
                    use std::io::Write as _;
                    let mut f = out_file.lock().expect("out file lock");
                    let line = serde_json::to_string(&row).expect("row serializes");
                    writeln!(f, "{line}").expect("row write");
                    f.flush().expect("row flush");
                }
                row
            }
        })
        .buffer_unordered(spec.concurrency)
        .collect()
        .await;
    let errors = rows.iter().filter(|r| r.error.is_some()).count();
    let no_moments = rows
        .iter()
        .filter(|r| r.error.is_none() && r.log_ratio_mean.is_none())
        .count();
    eprintln!(
        "perturbation_spectrum: wrote {} rows to {out_path} ({errors} errors, {no_moments} without moments)",
        rows.len()
    );
    Ok(())
}
