//! Minimal end-to-end example for `llmsorting`.
//!
//! This ranks three entities by "clarity of explanation" and returns
//! quantitative scores with uncertainty estimates.
//!
//! To run:
//! - Set `OPENROUTER_API_KEY`
//! - `cargo run --example quickstart`

use std::sync::Arc;

use llmsort::gateway::NoopUsageSink;
use llmsort::rerank::{
    ModelLadderPolicy, MultiRerankAttributeSpec, MultiRerankEntity, MultiRerankRequest,
    MultiRerankTopKSpec, RerankRunOptions,
};
use llmsort::{Attribution, ProviderGateway, SqlitePairwiseCache};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // -- Infrastructure setup ------------------------------------------------

    // SQLite cache for pairwise judgments — re-running this example reuses prior
    // LLM calls, so you only pay for new comparisons.
    let cache = SqlitePairwiseCache::new(SqlitePairwiseCache::default_path())?;

    // OpenRouter gateway — reads OPENROUTER_API_KEY from the environment.
    // NoopUsageSink discards cost tracking; use StderrUsageSink to see costs.
    let gateway = ProviderGateway::from_env(Arc::new(NoopUsageSink))?;

    // Model ladder: starts with a high-quality model and can downgrade to a
    // cheaper one once uncertainty is low enough that precision doesn't matter.
    let model_policy = Arc::new(ModelLadderPolicy::default());

    let run_options = RerankRunOptions {
        rng_seed: None,    // None = random; set Some(42) for reproducibility
        cache_only: false, // false = make LLM calls; true = only use cached judgments
    };

    // -- The actual request --------------------------------------------------

    let req = MultiRerankRequest {
        // The items to rank. Each gets a stable id and the text the LLM will see.
        entities: vec![
            MultiRerankEntity {
                id: "a".into(),
                text: "Entity A text".into(),
            },
            MultiRerankEntity {
                id: "b".into(),
                text: "Entity B text".into(),
            },
            MultiRerankEntity {
                id: "c".into(),
                text: "Entity C text".into(),
            },
        ],

        // Attributes to score on. Each gets independent pairwise comparisons,
        // then they're combined into a weighted utility for the final ranking.
        attributes: vec![MultiRerankAttributeSpec {
            id: "clarity".into(),
            prompt: "clarity of explanation".into(),
            prompt_template_slug: Some("canonical_v2".into()),
            weight: 1.0, // relative weight in the combined utility
        }],

        // Top-k configuration: how to decide when we're done.
        topk: MultiRerankTopKSpec {
            k: 2, // we want to identify the best 2 out of 3

            // Defaults are good for most use cases — see docs/ALGORITHM.md
            // for what each parameter does and why.
            ..serde_json::from_str(
                "{,
            prune_p_topk_below: None,
        }",
            )
            .unwrap()
        },

        gates: vec![],                // no hard filters on any attribute
        comparison_budget: None,      // no cap on number of LLM calls
        latency_budget_ms: None,      // no wall-clock time limit
        max_cost_nanodollars: None,   // no provider-reported spend cap
        model: None,                  // use the model ladder's default
        rater_id: None,               // no logical rater grouping
        comparison_concurrency: None, // use internal default parallelism
        max_pair_repeats: None,       // allow re-asking the same pair if needed
        randomize_presentation_order: true,
        counterbalance_pairs: false,
    };

    // -- Run it --------------------------------------------------------------

    let resp = llmsort::rerank::multi_rerank(
        req,
        llmsort::rerank::RerankExecution::new(
            Arc::new(gateway),
            Attribution::new("example::quickstart"),
        )
        .cache(&cache)
        .model_policy(model_policy)
        .run_options(run_options),
    )
    .await?;

    // -- Interpret results ---------------------------------------------------

    println!("stop_reason: {:?}", resp.meta.stop_reason);
    println!(
        "comparisons: {} used, {} cached",
        resp.meta.comparisons_used, resp.meta.comparisons_cached
    );
    println!();

    for entity in &resp.entities {
        println!(
            "  {} (rank {:?}): utility = {:.3} ± {:.3}",
            entity.id, entity.rank, entity.u_mean, entity.u_std,
        );
    }

    Ok(())
}
