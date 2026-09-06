//! Measure a model's per-wording gains live: the JCB corpus judged through
//! all three wordings of the ratio question, solved jointly with
//! `solve_with_template_gains`. Prints the fitted gains (canonical_v2 = 1)
//! and the calibration payoff (rms vs naive).
//!
//! Usage: OPENROUTER_API_KEY=... cargo run --example wording_gains -- <model> [<model>...]

use std::sync::Arc;

use llmsort::gain_calibration::{solve_with_template_gains, GainObservation};
use llmsort::gateway::{Attribution, NoopUsageSink, ProviderGateway};
use llmsort::rerank::{
    compare_pair, HigherRanked, PairwiseComparisonAttribute, PairwiseComparisonEntity,
    PairwiseComparisonRequest, PairwiseComparisonSpec, PairwiseJudgement, WORDING_SLUGS,
};
use llmsort_experiments::{core_pairs, CORPUS, PRIMARY_ATTRIBUTE};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let models: Vec<String> = std::env::args().skip(1).collect();
    if models.is_empty() {
        return Err("usage: wording_gains <model> [<model>...]".into());
    }
    let gateway = ProviderGateway::from_env(Arc::new(NoopUsageSink))?;
    let attribution = Attribution::new("cardinal::example::wording_gains");

    for model in &models {
        let mut obs = Vec::new();
        let mut cost = 0i64;
        let mut refusals = 0usize;
        for &(i, j) in &core_pairs() {
            for slug in WORDING_SLUGS {
                let spec = PairwiseComparisonSpec {
                    model,
                    attribute: PairwiseComparisonAttribute {
                        id: "gains",
                        prompt: PRIMARY_ATTRIBUTE,
                        prompt_template_slug: Some(slug),
                    },
                    entity_a: PairwiseComparisonEntity {
                        id: "a",
                        text: CORPUS[i],
                    },
                    entity_b: PairwiseComparisonEntity {
                        id: "b",
                        text: CORPUS[j],
                    },
                };
                let (judgement, usage) = compare_pair(
                    &gateway,
                    None,
                    PairwiseComparisonRequest {
                        nonce: None,
                        spec,
                        cache_only: false,
                        attribution: attribution.clone(),
                    },
                )
                .await?;
                cost += usage.provider_cost_nanodollars;
                match judgement {
                    PairwiseJudgement::Observation {
                        higher_ranked,
                        ratio,
                        ..
                    } => {
                        let toward_i = match higher_ranked {
                            HigherRanked::A => 1.0,
                            HigherRanked::B => -1.0,
                        };
                        obs.push(GainObservation {
                            i,
                            j,
                            log_ratio: toward_i * ratio.max(1.0).ln(),
                            template: slug.to_string(),
                        });
                    }
                    PairwiseJudgement::Refused => refusals += 1,
                }
            }
        }
        match solve_with_template_gains(CORPUS.len(), &obs, "canonical_v2") {
            Some(solve) => {
                println!("model: {model}");
                for (template, gain) in &solve.gains {
                    println!("  gain {template:<14} {gain:.3}");
                }
                println!(
                    "  rms {:.4} vs naive {:.4} ({} rounds) · {} obs · {} refusals · ${:.4}",
                    solve.rms_residual,
                    solve.rms_residual_uncalibrated,
                    solve.iterations,
                    obs.len(),
                    refusals,
                    cost as f64 / 1e9,
                );
            }
            None => println!("model: {model} — no usable observations"),
        }
    }
    Ok(())
}
