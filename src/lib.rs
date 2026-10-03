#![forbid(unsafe_code)]

//! # llmsort
//!
//! Sort a list by any fuzzy attribute with an LLM judge. llmsort asks pairwise
//! ratio questions, fits the answers into one robust score scale, and returns
//! the order, gap sizes, uncertainty, and provider cost.
//!
//! ## Quickstart
//!
//! Set `OPENROUTER_API_KEY`, then:
//!
//! ```no_run
//! use std::sync::Arc;
//!
//! use llmsort::gateway::NoopUsageSink;
//! use llmsort::rerank::{sort_texts, RerankExecution, SortOptions};
//! use llmsort::{Attribution, ProviderGateway};
//!
//! # async fn demo() -> Result<(), Box<dyn std::error::Error>> {
//! let gateway = ProviderGateway::from_env(Arc::new(NoopUsageSink))?;
//! let execution =
//!     RerankExecution::new(Arc::new(gateway), Attribution::new("app::sort"));
//! let sorted = sort_texts(
//!     vec![
//!         "First proposal...".into(),
//!         "Second proposal...".into(),
//!         "Third proposal...".into(),
//!     ],
//!     "expected impact",
//!     execution,
//!     SortOptions::default(),
//! )
//! .await?;
//!
//! for item in sorted.items {
//!     println!("{}. {:.3} ± {:.3}  {}", item.rank, item.latent_mean, item.latent_std, item.text);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! The stability-promised library surface is [`sort_texts`] and
//! [`sort_documents`] (plus their setwise siblings). The CLI promises `sort`
//! and `judge`; the judgement-packet format is also stable. Other public
//! modules support composition but may change before 1.0.
//!
//! ## The map
//!
//! Five rooms, dependencies pointing one way:
//!
//! | Room | Modules | Role |
//! |---|---|---|
//! | solve | [`rating_engine`], [`censored_likelihood`], [`discrete`], [`repeat_pooling`], [`gain_calibration`], [`bias_calibration`] | pure math: IRLS fusion, observation model, calibration |
//! | evidence | [`packet`], [`seriate`] | content-addressed judgement records; byte-identical fusion |
//! | elicit | [`prompts`], [`rerank::comparison`], [`rerank::decimal_ledger`] | ratio prompts, instruments, comparison execution |
//! | gateway | [`gateway`] | provider adapters, pricing, usage accounting |
//! | run | [`mod@rerank`], [`cache`], [`trait_search`], [`text_chunking`] | orchestration: sort, multi-attribute runs, traces, reports |
//!
//! Design rationale:
//! <https://github.com/XyraSinclair/llmsort/blob/main/docs/ALGORITHM.md>.
//! Mathematical contract:
//! <https://github.com/XyraSinclair/llmsort/blob/main/docs/MODEL.md>.
//! Complete walkthrough:
//! <https://github.com/XyraSinclair/llmsort/blob/main/docs/WORKED_EXAMPLE.md>.
//! Research claims and replayable evidence:
//! <https://github.com/XyraSinclair/llmsort/blob/main/PROGRAM.md>.

pub mod bias_calibration;
pub mod cache;
pub mod censored_likelihood;
pub mod discrete;
pub mod gain_calibration;
pub mod gateway;
pub mod packet;
pub mod prompts;
pub mod rating_engine;
pub mod repeat_pooling;
pub mod rerank;
pub mod seriate;
pub mod text_chunking;
pub mod trait_search;

#[cfg(feature = "sqlite-store")]
pub use cache::SqlitePairwiseCache;
pub use cache::{PairwiseCache, PairwiseCacheKey};
pub use discrete::{DiscreteDistribution, WeightedValue};
pub use gateway::{Attribution, ChatGateway, ProviderGateway, UsageSink};
pub use rerank::{
    multi_rerank, rerank, sort_documents, sort_documents_setwise, sort_texts, sort_texts_setwise,
    ComparisonError, ComparisonEvent, ComparisonObserver, ComparisonTrace, JsonlTraceSink,
    MultiRerankError, ObserverError, RerankExecution, SortError, SortOptions, SortedItem,
    SortedTexts, TraceError, TraceSink, TraceWorker, WarmStartData, WarmStartError,
    WarmStartProvider,
};

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
mod readme_doctests {}
