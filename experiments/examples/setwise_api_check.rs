//! Direct verification of the graduated crate API `sort_documents_setwise`
//! against the E6 harness's pool: same corpus, same seed-17 selection, same
//! design parameters. Prints the gauge, the order, and cost; correlate the
//! ranks against the harness pack's latents offline.
//!
//! Usage (from `research/`, OPENROUTER_API_KEY set):
//!   cargo run --release --example setwise_api_check -- <model> <criterion>

use std::sync::Arc;

use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use serde::Deserialize;

use llmsort::gateway::{NoopUsageSink, ProviderGateway};
use llmsort::rerank::setwise::{sort_documents_setwise, SetwiseOptions};
use llmsort::rerank::types::RerankDocument;
use llmsort::ChatGateway;

#[derive(Deserialize)]
struct CorpusItem {
    id: String,
    text: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let model = args
        .next()
        .unwrap_or_else(|| "openai/gpt-5.6-luna".to_string());
    let criterion = args.next().unwrap_or_else(|| {
        "fit for a funder who wants cheap high-leverage AI safety field-building".to_string()
    });

    // Reproduce the harness's seed-17 pool: eligible >= 1600 chars,
    // seeded shuffle, first 24, truncated to 1600 chars.
    let corpus_raw = std::fs::read_to_string("data/manifund/items/full.json")?;
    let corpus: Vec<CorpusItem> = serde_json::from_str(&corpus_raw)?;
    let mut eligible: Vec<&CorpusItem> = corpus
        .iter()
        .filter(|item| item.text.trim().chars().count() >= 1600)
        .collect();
    let mut rng = StdRng::seed_from_u64(17);
    eligible.shuffle(&mut rng);
    let documents: Vec<RerankDocument> = eligible[..24]
        .iter()
        .map(|item| RerankDocument {
            id: item.id.clone(),
            text: item.text.trim().chars().take(1600).collect::<String>(),
        })
        .collect();

    let gateway: Arc<dyn ChatGateway> =
        Arc::new(ProviderGateway::from_env(Arc::new(NoopUsageSink))?);
    let sorted = sort_documents_setwise(
        documents,
        &criterion,
        gateway,
        SetwiseOptions {
            model: Some(model),
            seed: Some(17),
            ..SetwiseOptions::default()
        },
    )
    .await?;

    println!(
        "calls {} ok {} malformed {} errored {} | components {} | ${:.4}",
        sorted.calls,
        sorted.calls_ok,
        sorted.calls_malformed,
        sorted.calls_errored,
        sorted.components,
        sorted.cost_nanodollars as f64 / 1e9,
    );
    if let Some(g) = &sorted.gauge {
        println!(
            "gauge: flip rate {:?} ({}/{} pairs, {} subsets repeated)",
            g.flip_rate, g.direction_flips, g.entity_pairs_compared, g.subsets_with_repeats
        );
    }
    for item in &sorted.items {
        println!(
            "{:2}. {:<60} {:+.3} ±{:.3}",
            item.rank, item.id, item.latent_mean, item.latent_std
        );
    }
    Ok(())
}
