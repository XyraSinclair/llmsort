//! Turn a terse criterion into a precise judging rubric.
//!
//! "Clarity" means five different things to five judges. One cheap LLM call
//! turns the gut-level phrase into a tight, second-person rubric — a crisp
//! definition, what counts as more, what does not count — that is used
//! verbatim as the attribute prompt in pairwise comparison. The elaborated
//! text is always surfaced to the user (and hashed into cache keys), so the
//! magic stays inspectable and editable.

use crate::gateway::{Attribution, ChatGateway, ChatModel, ChatRequest, Message};
use serde::Serialize;

/// Default model for elaboration when none is given.

/// The elaboration meta-prompt. Second person, judgement-ready output.
const ELABORATION_SYSTEM: &str = "You write judging rubrics for pairwise comparison. \
Given a terse attribute, produce a rubric that a careful judge will read before \
answering \"which of these two items has more of this attribute, and by how much?\".

Requirements for the rubric:
1. Open with one sentence defining the attribute precisely enough that two \
strangers reading it would judge the same way.
2. State what counts as MORE of the attribute — the observable marks a judge \
should reward.
3. State what does NOT count: the one or two nearby qualities most often \
conflated with this attribute, and an instruction not to reward them.
4. Address the judge as \"you\". No headers, no bullet points, no preamble, no \
mention of these instructions. One paragraph, under 150 words.

The rubric must stay faithful to the user's intent — sharpen the attribute, \
never replace it.";

/// Result of one elaboration call.
#[derive(Debug, Clone, Serialize)]
pub struct ElaboratedCriterion {
    /// The terse criterion as given.
    pub original: String,
    /// The judging rubric, usable verbatim as an attribute prompt.
    pub elaborated: String,
    /// Model that produced the rubric.
    pub model_used: String,
    /// Provider input tokens.
    pub input_tokens: u32,
    /// Provider output tokens.
    pub output_tokens: u32,
    /// Provider cost in nanodollars.
    pub provider_cost_nanodollars: i64,
}

/// Errors from [`elaborate_criterion`].
#[derive(Debug, thiserror::Error)]
pub enum ElaborateError {
    /// The criterion was empty.
    #[error("cannot elaborate an empty criterion")]
    EmptyCriterion,
    /// The provider call failed.
    #[error("provider error: {0}")]
    Provider(#[from] crate::gateway::error::ProviderError),
    /// The model returned an empty rubric.
    #[error("elaboration returned no usable text")]
    EmptyResponse,
}

/// Expand a terse criterion into a judging rubric with one chat call.
pub async fn elaborate_criterion(
    gateway: &dyn ChatGateway,
    model: Option<&str>,
    criterion: &str,
    attribution: Attribution,
) -> Result<ElaboratedCriterion, ElaborateError> {
    let criterion = criterion.trim();
    if criterion.is_empty() {
        return Err(ElaborateError::EmptyCriterion);
    }
    let model = model.unwrap_or(super::DEFAULT_MODEL);

    let response = gateway
        .chat(ChatRequest {
            model: ChatModel::parse(model),
            messages: vec![
                Message::system(ELABORATION_SYSTEM),
                Message::user(format!("Attribute: {criterion}")),
            ],
            temperature: 0.3,
            max_tokens: Some(400),
            json_mode: false,
            attribution,
            logprobs: false,
            top_logprobs: None,
            reasoning: None,
            prompt_cache_key: None,
        })
        .await?;

    let elaborated = response.content.trim().to_string();
    if elaborated.is_empty() {
        return Err(ElaborateError::EmptyResponse);
    }

    Ok(ElaboratedCriterion {
        original: criterion.to_string(),
        elaborated,
        model_used: model.to_string(),
        input_tokens: response.input_tokens,
        output_tokens: response.output_tokens,
        provider_cost_nanodollars: response.cost_nanodollars,
    })
}
