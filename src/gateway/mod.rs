//! Provider gateway for chat completions.

pub mod claude_code;
pub mod codex;
pub mod error;
pub mod gemini_cli;
pub mod openrouter;
pub mod pricing;
pub mod types;
pub mod usage;

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use tokio::time::{sleep, sleep_until, Instant};

use claude_code::{ClaudeCodeAdapter, ClaudeCodeConfig};
use codex::{CodexAdapter, CodexConfig};
use gemini_cli::{GeminiCliAdapter, GeminiCliConfig};
use openrouter::OpenRouterAdapter;
use usage::{CallStatus, ProviderCallRecord, UsageSink as UsageSinkTrait};

pub use error::{ErrorContext, ProviderError, RateLimitSource};
pub use pricing::*;
pub use types::*;
pub use usage::{NoopUsageSink, StderrUsageSink, UsageSink};

#[async_trait::async_trait]
pub trait ChatGateway: Send + Sync {
    async fn chat(&self, req: ChatRequest) -> Result<ChatResponse, ProviderError>;
}

#[derive(Debug, Clone)]
pub struct GatewayConfig {
    pub max_retries: u32,
    pub retry_base_delay: Duration,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            max_retries: 2,
            retry_base_delay: Duration::from_secs(1),
        }
    }
}

/// Ceiling on any single retry sleep, including provider `retry_after` hints.
/// Operational plumbing, not a user knob: a provider asking for more than this
/// is better served by the caller's failure accounting than by a silent stall.
const MAX_RETRY_DELAY: Duration = Duration::from_secs(30);

pub struct ProviderGateway<U: UsageSinkTrait> {
    openrouter: Option<OpenRouterAdapter>,
    claude_code: ClaudeCodeAdapter,
    codex: CodexAdapter,
    gemini_cli: GeminiCliAdapter,
    usage_sink: Arc<U>,
    config: GatewayConfig,
    /// Shared rate-limit cooldown: when a provider says 429, every in-flight
    /// worker waits out one cooldown instead of retrying independently.
    cooldown_until: Mutex<Option<Instant>>,
}

#[async_trait::async_trait]
impl<U: UsageSinkTrait> ChatGateway for ProviderGateway<U> {
    async fn chat(&self, req: ChatRequest) -> Result<ChatResponse, ProviderError> {
        ProviderGateway::chat(self, req).await
    }
}

impl<U: UsageSinkTrait> ProviderGateway<U> {
    pub fn from_env(usage_sink: Arc<U>) -> Result<Self, ProviderError> {
        let openrouter = OpenRouterAdapter::from_env()?;
        let claude_code = ClaudeCodeAdapter::new(ClaudeCodeConfig {
            config_dir: std::env::var_os("CARDINAL_CLAUDE_CODE_CONFIG_DIR").map(Into::into),
            effort: std::env::var("CARDINAL_CLAUDE_CODE_EFFORT").ok(),
            ..ClaudeCodeConfig::default()
        });
        let codex = CodexAdapter::new(CodexConfig {
            binary: std::env::var_os("CARDINAL_CODEX_BINARY")
                .map(Into::into)
                .unwrap_or_else(|| CodexConfig::default().binary),
            effort: std::env::var("CARDINAL_CODEX_EFFORT").ok(),
        });
        let gemini_cli = GeminiCliAdapter::new(GeminiCliConfig {
            binary: std::env::var_os("CARDINAL_GEMINI_CLI_BINARY")
                .map(Into::into)
                .unwrap_or_else(|| GeminiCliConfig::default().binary),
            home: std::env::var_os("CARDINAL_GEMINI_CLI_HOME").map(Into::into),
        });
        Ok(Self {
            openrouter: Some(openrouter),
            claude_code,
            codex,
            gemini_cli,
            usage_sink,
            config: GatewayConfig::default(),
            cooldown_until: Mutex::new(None),
        })
    }

    pub fn with_config(
        openrouter: OpenRouterAdapter,
        usage_sink: Arc<U>,
        config: GatewayConfig,
    ) -> Self {
        Self {
            openrouter: Some(openrouter),
            claude_code: ClaudeCodeAdapter::default(),
            codex: CodexAdapter::default(),
            gemini_cli: GeminiCliAdapter::default(),
            usage_sink,
            config,
            cooldown_until: Mutex::new(None),
        }
    }

    pub fn claude_code(claude_code: ClaudeCodeAdapter, usage_sink: Arc<U>) -> Self {
        Self {
            openrouter: None,
            claude_code,
            codex: CodexAdapter::default(),
            gemini_cli: GeminiCliAdapter::default(),
            usage_sink,
            config: GatewayConfig::default(),
            cooldown_until: Mutex::new(None),
        }
    }

    pub fn with_adapters(
        openrouter: OpenRouterAdapter,
        claude_code: ClaudeCodeAdapter,
        usage_sink: Arc<U>,
        config: GatewayConfig,
    ) -> Self {
        Self {
            openrouter: Some(openrouter),
            claude_code,
            codex: CodexAdapter::default(),
            gemini_cli: GeminiCliAdapter::default(),
            usage_sink,
            config,
            cooldown_until: Mutex::new(None),
        }
    }

    pub fn codex(codex: CodexAdapter, usage_sink: Arc<U>) -> Self {
        Self {
            openrouter: None,
            claude_code: ClaudeCodeAdapter::default(),
            codex,
            gemini_cli: GeminiCliAdapter::default(),
            usage_sink,
            config: GatewayConfig::default(),
            cooldown_until: Mutex::new(None),
        }
    }

    pub async fn chat(&self, req: ChatRequest) -> Result<ChatResponse, ProviderError> {
        for attempt in 0..=self.config.max_retries {
            self.wait_for_cooldown().await;
            let result = match &req.model {
                ChatModel::OpenRouter(_) => match self.openrouter.as_ref() {
                    Some(openrouter) => openrouter.chat(&req).await,
                    None => Err(ProviderError::config(
                        "OpenRouter adapter is not configured",
                    )),
                },
                ChatModel::ClaudeCode(_) => self.claude_code.chat(&req).await,
                ChatModel::Codex(_) => self.codex.chat(&req).await,
                ChatModel::GeminiCli(_) => self.gemini_cli.chat(&req).await,
            };
            match result {
                Ok(resp) => {
                    self.record_usage(&req, &resp, CallStatus::Success, None)
                        .await;
                    return Ok(resp);
                }
                Err(err) => {
                    let code = err.code().to_string();
                    self.record_usage(&req, &ChatResponse::empty(), CallStatus::Error, Some(code))
                        .await;

                    if !err.is_retryable() || attempt == self.config.max_retries {
                        return Err(err);
                    }

                    let mut delay = backoff_delay(self.config.retry_base_delay, attempt);
                    if let ProviderError::RateLimited { retry_after, .. } = &err {
                        delay = delay.max(*retry_after).min(MAX_RETRY_DELAY);
                        // Shared backpressure: park every worker behind one
                        // cooldown; the gate at the top of the loop sleeps.
                        self.extend_cooldown(Instant::now() + delay).await;
                    } else {
                        delay = delay.min(MAX_RETRY_DELAY);
                        sleep(delay).await;
                    }
                }
            }
        }

        unreachable!("gateway retry loop must return within configured attempts")
    }

    /// Sleep until the shared rate-limit cooldown (if any) has passed.
    async fn wait_for_cooldown(&self) {
        let wait = {
            let guard = self.cooldown_until.lock().await;
            guard.filter(|until| *until > Instant::now())
        };
        if let Some(until) = wait {
            sleep_until(until).await;
        }
    }

    /// Push the shared cooldown out to at least `until` (never shortens it).
    async fn extend_cooldown(&self, until: Instant) {
        let mut guard = self.cooldown_until.lock().await;
        *guard = Some(guard.map_or(until, |existing| existing.max(until)));
    }

    async fn record_usage(
        &self,
        req: &ChatRequest,
        resp: &ChatResponse,
        status: CallStatus,
        error_code: Option<String>,
    ) {
        let mut record = ProviderCallRecord::new(
            req.model.provider(),
            "chat/completions",
            req.model.model_id(),
            req.attribution.caller,
        )
        .tokens(resp.input_tokens as i32, resp.output_tokens as i32)
        .cost(resp.cost_nanodollars)
        .upstream_cost(resp.upstream_cost_nanodollars)
        .cost_is_estimate(resp.cost_is_estimate)
        .user(req.attribution.user_id)
        .api_key(req.attribution.api_key_id)
        .job(req.attribution.job_id)
        .latency(resp.latency.as_millis() as i32);
        if let Some(request_id) = resp.provider_request_id.as_deref() {
            record = record.request_id(request_id);
        }

        let record = if status == CallStatus::Error {
            record.error(error_code.unwrap_or_else(|| "provider_error".to_string()))
        } else {
            record
        };

        self.usage_sink.record(record).await;
    }
}

fn backoff_delay(base: Duration, attempt: u32) -> Duration {
    let multiplier = 2u64.pow(attempt.min(5));
    base * multiplier as u32
}

impl ChatResponse {
    fn empty() -> Self {
        Self {
            provider_call_id: None,
            provider_request_id: None,
            served_model: None,
            content: String::new(),
            reasoning: None,
            reasoning_tokens: None,
            input_tokens: 0,
            output_tokens: 0,
            cost_nanodollars: 0,
            cost_is_estimate: false,
            upstream_cost_nanodollars: None,
            latency: Duration::from_millis(0),
            finish_reason: FinishReason::Unknown("error".to_string()),
            output_logprobs: None,
            cache_read_tokens: None,
            cache_write_tokens: None,
        }
    }
}
