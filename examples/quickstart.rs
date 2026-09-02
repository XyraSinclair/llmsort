//! Minimal end-to-end example for `llmsort`: sort a small list with
//! `sort_texts`, the stability-promised library entry point.
//!
//! To run:
//! - Set `OPENROUTER_API_KEY`
//! - `cargo run --example quickstart`

use std::sync::Arc;

use llmsort::gateway::NoopUsageSink;
use llmsort::rerank::{sort_texts, RerankExecution, SortOptions};
use llmsort::{Attribution, ProviderGateway};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // OpenRouter gateway — reads OPENROUTER_API_KEY from the environment.
    // NoopUsageSink discards the per-call usage stream; the run's own
    // accounting still arrives in `sorted.meta`.
    let gateway = ProviderGateway::from_env(Arc::new(NoopUsageSink))?;
    let execution =
        RerankExecution::new(Arc::new(gateway), Attribution::new("example::quickstart"));

    let sorted = sort_texts(
        vec![
            "Entropy is why your coffee cools down and never warms back up: \
             heat spreads out because spread-out is overwhelmingly more likely."
                .into(),
            "The second law of thermodynamics states that the total entropy of \
             an isolated system is non-decreasing over time."
                .into(),
            "Stuff just kind of tends to get messier unless you do something \
             about it, entropy-wise."
                .into(),
        ],
        "clarity of explanation",
        execution,
        SortOptions::default(), // default judge, 4·n comparison budget
    )
    .await?;

    println!("stop_reason: {:?}", sorted.meta.stop_reason);
    println!(
        "comparisons: {} used, {} cached",
        sorted.meta.comparisons_used, sorted.meta.comparisons_cached
    );
    for item in &sorted.items {
        println!(
            "{:>2}. {:+.3} ± {:.3}  {}",
            item.rank, item.latent_mean, item.latent_std, item.text
        );
    }
    println!(
        "cost: ${:.4}",
        sorted.meta.provider_cost_nanodollars as f64 / 1e9
    );
    Ok(())
}
