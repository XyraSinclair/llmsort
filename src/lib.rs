#![forbid(unsafe_code)]

//! # llmsort
//!
//! Score a list by any fuzzy attribute with an LLM judge: pairwise ratio
//! questions fitted into consistent scores with error bars, at a known cost.
//!
//! Instead of asking an LLM to "rate this 1–10" (unreliable, miscalibrated),
//! llmsort asks pairwise ratio questions: "how many times more attribute
//! does A have than B?" A robust statistical solver (IRLS with Huber loss)
//! combines these noisy observations into globally consistent scores with
//! uncertainty estimates. The system selects the most informative pairs to
//! query and stops when the top-K ranking is sufficiently certain.
//!
//! The ontology, in five nouns: an **attribute** (any nameable dimension) over
//! entities, each holding a latent **magnitude** (only ratios are observable);
//! **instruments** (elicitation modes) emit **evidence** in one currency —
//! (E\[log-ratio\], honest variance) — which the solver fuses into a
//! **scaling**: every entity placed on a shared log-ratio scale with a
//! *reading* (magnitude ± uncertainty). A ranking is a scaling with the
//! spacing deleted.
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
//! The stability-promised surface is [`sort_texts`] / [`sort_documents`],
//! the `llmsort sort` and `llmsort judge` CLI verbs, and the packet format.
//! Everything else is exposed for composition but may move.
//!
//! Lineage: `cardinal-harness` → `ratiometer` → `llmsorting` → `llmsort`;
//! the research program that produced this engine lives in this repo's
//! `experiments/` crate and `research/` record. See `docs/ALGORITHM.md`
//! for the design rationale.

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
