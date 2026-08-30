//! LLM pairwise comparison logic for reranking.
//!
//! Implements the contract between LLM JSON responses and solver observations.

use serde::Deserialize;
use tracing::warn;

use crate::cache::{
    CacheError, CachedJudgement, PairwiseCache, PairwiseCacheAttribute, PairwiseCacheEntity,
    PairwiseCacheKey, PairwiseCacheKeyParts, PairwiseCacheTemplate,
};
use crate::discrete::{DiscreteDistribution, WeightedValue};
use crate::gateway::{
    pairwise_logprob_posterior, truncate_output_logprobs, Attribution, ChatGateway, ChatModel,
    ChatRequest, ConfidenceSource, PairwiseAnswer, PairwiseLogprobPosterior, PairwisePreferredSide,
    ProviderError, RatioBucket, ReasoningConfig, ReasoningEffort, SignedLogRatioDistribution,
    TokenLogprob,
};
use crate::prompts::{
    prompt_by_slug, EntityRef, PromptInstance, PromptTemplate, DEFAULT_PROMPT,
    ORDINAL_OBSERVATION_RATIO, RATIO_LADDER,
};
use crate::text_chunking::count_tokens;

use super::decimal_ledger;
use super::types::{HigherRanked, PairwiseJudgement};

mod decimal_ledger_path;
mod execution;
mod parsing;
mod seriate;
mod types;

pub use execution::{compare_pair, estimate_pairwise_input_tokens};
pub use parsing::parse_pairwise_response;
pub(crate) use types::DECIMAL_LEDGER_DRAWS;
pub use types::{
    is_evidence_slug, pairwise_logprobs_top_n, pairwise_max_output_tokens, ComparisonError,
    ComparisonUsage, EvidenceMoments, PairwiseComparisonAttribute, PairwiseComparisonEntity,
    PairwiseComparisonRequest, PairwiseComparisonSpec, DECIMAL_LEDGER_SLUG, ORDINAL_LETTER_SLUG,
    PAIRWISE_BUCKET_LOGPROB_MAX_ATTEMPTS, PAIRWISE_LOGPROBS_TOP_N_DEFAULT,
    PAIRWISE_MAX_OUTPUT_TOKENS_DEFAULT, PAIRWISE_MAX_OUTPUT_TOKENS_GPT5,
    PAIRWISE_TYPICAL_OUTPUT_TOKENS, RATIO_LETTER_ATTR_LAST_SLUG, RATIO_LETTER_SLUG,
};

use decimal_ledger_path::compare_pair_decimal_ledger;
use execution::{
    cached_to_judgement, evidence_moments_from_cached, judgement_to_cached,
    ledger_logprobs_require_effort_none, model_supports_logprobs, should_use_json_mode,
    LADDER_RATIO_CAP,
};
use parsing::{
    compact_bucket_output_logprobs, fallback_stored_logprobs, pairwise_bucket_logprob_posterior,
};
use seriate::compare_pair_seriate;

#[cfg(test)]
mod tests;
