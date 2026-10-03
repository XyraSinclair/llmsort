//! Core types for the provider gateway.

use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

mod pairwise;

// =============================================================================
// ATTRIBUTION
// =============================================================================

/// Attribution for cost tracking and debugging.
///
/// Every request through the gateway carries attribution so we know:
/// - Who made the request (user_id)
/// - Which API key initiated the request (api_key_id)
/// - What job it's part of (job_id)
/// - Which code path triggered it (caller)
#[derive(Debug, Clone, Default)]
pub struct Attribution {
    /// User who initiated the request (if known).
    pub user_id: Option<Uuid>,
    /// API key that initiated the request (if known).
    pub api_key_id: Option<Uuid>,
    /// Job this request is part of (for rating jobs, batch jobs, etc.).
    pub job_id: Option<Uuid>,
    /// Which code path made this call, for debugging.
    /// Use a static string like "scry::embed" or "job_executor::compare".
    pub caller: &'static str,
}

impl Attribution {
    pub fn new(caller: &'static str) -> Self {
        Self {
            caller,
            ..Default::default()
        }
    }

    pub fn with_user(mut self, user_id: Uuid) -> Self {
        self.user_id = Some(user_id);
        self
    }

    pub fn with_api_key(mut self, api_key_id: Uuid) -> Self {
        self.api_key_id = Some(api_key_id);
        self
    }

    pub fn with_job(mut self, job_id: Uuid) -> Self {
        self.job_id = Some(job_id);
        self
    }
}

// =============================================================================
// CHAT TYPES
// =============================================================================

/// Chat message role.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

/// A chat message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
        }
    }
}

/// Chat model specification.
#[derive(Debug, Clone)]
pub enum ChatModel {
    /// OpenRouter model, e.g. "anthropic/claude-3-5-haiku"
    OpenRouter(String),
    /// Claude Code model or alias, e.g. "fable".
    ClaudeCode(String),
    /// Codex model, e.g. "gpt-5.6-sol".
    Codex(String),
    /// Gemini CLI model, e.g. "gemini-2.5-pro".
    GeminiCli(String),
}

impl ChatModel {
    /// Canonical entry point for user-supplied model slugs.
    ///
    /// Slugs prefixed with `claude-code/` or `codex/` use their subscription
    /// adapters. All other slugs use OpenRouter.
    pub fn parse(slug: impl Into<String>) -> Self {
        let slug = slug.into();
        if let Some(model_id) = slug.strip_prefix("claude-code/") {
            ChatModel::ClaudeCode(model_id.to_string())
        } else if let Some(model_id) = slug.strip_prefix("codex/") {
            ChatModel::Codex(model_id.to_string())
        } else if let Some(model_id) = slug.strip_prefix("gemini-cli/") {
            ChatModel::GeminiCli(model_id.to_string())
        } else {
            ChatModel::OpenRouter(slug)
        }
    }

    pub fn openrouter(model_id: impl Into<String>) -> Self {
        ChatModel::OpenRouter(model_id.into())
    }

    pub fn claude_code(model_id: impl Into<String>) -> Self {
        ChatModel::ClaudeCode(model_id.into())
    }

    pub fn codex(model_id: impl Into<String>) -> Self {
        ChatModel::Codex(model_id.into())
    }

    pub fn gemini_cli(model_id: impl Into<String>) -> Self {
        ChatModel::GeminiCli(model_id.into())
    }

    pub fn model_id(&self) -> &str {
        match self {
            ChatModel::OpenRouter(id) => id,
            ChatModel::ClaudeCode(id) => id,
            ChatModel::Codex(id) => id,
            ChatModel::GeminiCli(id) => id,
        }
    }

    pub fn provider(&self) -> &'static str {
        match self {
            ChatModel::OpenRouter(_) => "openrouter",
            ChatModel::ClaudeCode(_) => "claude-code",
            ChatModel::Codex(_) => "codex",
            ChatModel::GeminiCli(_) => "gemini-cli",
        }
    }
}

/// Request for chat completion.
#[derive(Debug, Clone)]
pub struct ChatRequest {
    /// Model to use.
    pub model: ChatModel,
    /// Messages in the conversation.
    pub messages: Vec<Message>,
    /// Sampling temperature (0.0 - 2.0).
    pub temperature: f32,
    /// Maximum tokens to generate.
    pub max_tokens: Option<u32>,
    /// Whether to request JSON output.
    pub json_mode: bool,
    /// Attribution for cost tracking.
    pub attribution: Attribution,
    /// Whether to request token-level logprobs in the response.
    ///
    /// When true, the provider returns log-probabilities for output tokens.
    /// These are useful for diagnostics and future answer-level rescoring, but
    /// decimal ratio ladders do not admit a valid confidence estimate from a
    /// single token-position peek.
    pub logprobs: bool,
    /// Number of top alternative logprobs to return per token position.
    /// Only meaningful when `logprobs` is true. Typically 5-20.
    pub top_logprobs: Option<u32>,
    /// Optional normalized reasoning configuration for providers that support it.
    pub reasoning: Option<ReasoningConfig>,
    /// OpenAI-style cache-routing hint (`prompt_cache_key`): should be
    /// derived from the STABLE content (template + attribute + entities)
    /// and independent of any nonce or padding, so repeat draws route to
    /// the same provider cache slot. None = omit.
    pub prompt_cache_key: Option<String>,
}

pub const DEFAULT_CHAT_TEMPERATURE: f32 = 0.0;

impl ChatRequest {
    pub fn new(model: ChatModel, messages: Vec<Message>, attribution: Attribution) -> Self {
        Self {
            model,
            messages,
            temperature: DEFAULT_CHAT_TEMPERATURE,
            max_tokens: None,
            json_mode: false,
            attribution,
            logprobs: false,
            top_logprobs: None,
            reasoning: None,
            prompt_cache_key: None,
        }
    }

    pub fn temperature(mut self, t: f32) -> Self {
        self.temperature = t;
        self
    }

    pub fn max_tokens(mut self, max: u32) -> Self {
        self.max_tokens = Some(max);
        self
    }

    pub fn json(mut self) -> Self {
        self.json_mode = true;
        self
    }

    /// Request token-level logprobs with the specified number of alternatives.
    pub fn with_logprobs(mut self, top_n: u32) -> Self {
        self.logprobs = true;
        self.top_logprobs = Some(top_n);
        self
    }

    pub fn reasoning(mut self, reasoning: ReasoningConfig) -> Self {
        self.reasoning = Some(reasoning);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ReasoningEffort {
    Xhigh,
    High,
    Medium,
    Low,
    Minimal,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ReasoningConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<ReasoningEffort>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude: Option<bool>,
}

impl ReasoningConfig {
    pub fn disabled() -> Self {
        Self {
            enabled: Some(false),
            effort: None,
            max_tokens: None,
            exclude: None,
        }
    }

    pub fn low() -> Self {
        Self {
            enabled: None,
            effort: Some(ReasoningEffort::Low),
            max_tokens: None,
            exclude: None,
        }
    }
}

/// Reason the model stopped generating.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FinishReason {
    Stop,
    Length,
    ContentFilter,
    ToolCalls,
    Unknown(String),
}

impl From<Option<String>> for FinishReason {
    fn from(s: Option<String>) -> Self {
        match s.as_deref() {
            Some("stop") => FinishReason::Stop,
            Some("length") => FinishReason::Length,
            Some("content_filter") => FinishReason::ContentFilter,
            Some("tool_calls") => FinishReason::ToolCalls,
            Some(other) => FinishReason::Unknown(other.to_string()),
            None => FinishReason::Unknown("none".to_string()),
        }
    }
}

/// A single token's logprob entry with alternatives.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenLogprob {
    /// The token string.
    pub token: String,
    /// Log-probability of this token.
    pub logprob: f64,
    /// Top alternative tokens at this position (if requested).
    pub top_alternatives: Vec<TokenAlternative>,
}

/// An alternative token at a given position.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenAlternative {
    /// The alternative token string.
    pub token: String,
    /// Log-probability of this alternative.
    pub logprob: f64,
}

/// Response from chat completion.
#[derive(Debug, Clone)]
pub struct ChatResponse {
    /// Provider-native completion identifier, when returned in the response body.
    pub provider_call_id: Option<String>,
    /// Provider request identifier, when returned in response headers.
    pub provider_request_id: Option<String>,
    /// Model identifier the provider reports it actually served, when available.
    ///
    /// Measurement runs should assert this against the requested model; routed
    /// channels (OpenRouter variants, Claude Code aliases) can serve a
    /// different concrete model than the one named in the request.
    pub served_model: Option<String>,
    /// Generated content.
    pub content: String,
    /// Provider-returned reasoning text, if available.
    pub reasoning: Option<String>,
    /// Reasoning token count, when the provider reports it separately.
    pub reasoning_tokens: Option<u32>,
    /// Input tokens consumed.
    pub input_tokens: u32,
    /// Output tokens generated.
    pub output_tokens: u32,
    /// Cost in nanodollars.
    ///
    /// If `cost_is_estimate` is true, this came from the local fallback estimate rather than
    /// an exact pricing-table entry or provider-reported cost.
    pub cost_nanodollars: i64,
    /// True when `cost_nanodollars` used fallback pricing because no exact local pricing entry
    /// or provider-reported upstream cost was available.
    pub cost_is_estimate: bool,
    /// Provider-reported upstream inference cost (nanodollars), if available.
    ///
    /// For OpenRouter this is derived from `usage.cost_details.upstream_inference_cost`.
    /// Used for auditing pricing drift vs our internal token pricing registry.
    pub upstream_cost_nanodollars: Option<i64>,
    /// Time taken for the request.
    pub latency: Duration,
    /// Why the model stopped.
    pub finish_reason: FinishReason,
    /// Per-token logprobs for the output, if requested and available.
    ///
    /// This is raw provider metadata. A valid ladder-level posterior may require
    /// continuation rescoring rather than naive inspection of one emitted token.
    pub output_logprobs: Option<Vec<TokenLogprob>>,
    /// Input tokens served from provider prompt cache (if reported).
    pub cache_read_tokens: Option<u32>,
    /// Input tokens written to provider prompt cache (if reported).
    pub cache_write_tokens: Option<u32>,
}

/// Provenance for confidence-like information retained with an observation.
///
/// These values are descriptive metadata. Solver precision must come from a
/// measured response distribution, not from collapsing this enum to a scalar.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConfidenceSource {
    /// Model self-reported confidence (from JSON output field).
    SelfReported(f64),
    /// Derived from a valid answer-level logprob scoring path.
    Logprob {
        /// Shannon entropy of the ratio token distribution (lower = more certain).
        entropy: f64,
        /// Probability mass on the selected ratio token.
        top_prob: f64,
        /// Probability mass within one ladder step of the selected ratio.
        neighborhood_prob: f64,
    },
    /// Future: provider-reported internal coherence metrics.
    LabsCoherence {
        /// Provider-computed internal consistency score.
        internal_consistency: f64,
        /// Provider-computed epistemic uncertainty estimate.
        epistemic_uncertainty: f64,
    },
    /// Weighted blend of multiple confidence sources.
    Blended {
        /// The blended scalar confidence value.
        value: f64,
        /// Contributing sources and their weights.
        components: Vec<(String, f64)>,
    },
}

pub use pairwise::*;
