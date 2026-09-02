//! Codex exec adapter for subscription-billed chat completions.
//!
//! Calls use the pooled local Codex subscription session and have zero marginal API cost.
//! Each child runs from a fresh scratch directory so repository `AGENTS.md` files cannot affect
//! judgments. `CODEX_HOME` is deliberately left untouched because the resolved Codex binary
//! (often a pooling shim) owns auth.

use std::path::PathBuf;
use std::process::Stdio;
use std::time::Instant;

use serde::Deserialize;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use super::error::{ErrorContext, ProviderError};
use super::types::{ChatRequest, ChatResponse, FinishReason, Message, Role};

const PROVIDER: &str = "codex";
const QUOTA_MARKERS: &[&str] = &["session limit", "usage limit", "rate limit", "resets "];
const SAFEGUARD_MARKERS: &[&str] = &[
    "automated abuse",
    "content policy",
    "flagged",
    "safeguard",
    "safety policy",
];

/// Process configuration for [`CodexAdapter`].
#[derive(Debug, Clone)]
pub struct CodexConfig {
    /// The Codex CLI binary. `Default` resolves `codex` on `PATH`, falling
    /// back to the codexpool shim (`~/.codexpool/bin/codex`); the
    /// `CARDINAL_CODEX_BINARY` env var overrides both (see
    /// [`super::ProviderGateway::from_env`]).
    pub binary: PathBuf,
    /// An optional Codex reasoning effort level.
    pub effort: Option<String>,
}

impl Default for CodexConfig {
    fn default() -> Self {
        let on_path = std::env::var_os("PATH").and_then(|path| {
            std::env::split_paths(&path)
                .map(|dir| dir.join("codex"))
                .find(|candidate| candidate.is_file())
        });
        let binary = on_path.unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("/nonexistent"))
                .join(".codexpool/bin/codex")
        });
        Self {
            binary,
            effort: None,
        }
    }
}

/// Adapter for Codex non-interactive chat completions.
#[derive(Debug, Clone, Default)]
pub struct CodexAdapter {
    config: CodexConfig,
}

impl CodexAdapter {
    /// Create an adapter with explicit process configuration.
    pub fn new(config: CodexConfig) -> Self {
        Self { config }
    }

    /// Execute a chat completion through `codex exec --json`.
    pub async fn chat(&self, req: &ChatRequest) -> Result<ChatResponse, ProviderError> {
        if req.model.model_id().is_empty() {
            return Err(ProviderError::invalid_request(
                "Codex model must not be empty",
            ));
        }

        let (system_prompt, prompt) = map_messages(&req.messages);
        let scratch = tempfile::Builder::new()
            .prefix("cardinal-codex-")
            .tempdir_in(std::env::temp_dir())
            .map_err(|error| {
                ProviderError::config(format!("failed to create Codex scratch directory: {error}"))
            })?;

        let mut command = Command::new(&self.config.binary);
        command
            .arg("exec")
            .arg("--json")
            .arg("--ephemeral")
            .arg("--skip-git-repo-check")
            .arg("--sandbox")
            .arg("read-only")
            .arg("-c")
            .arg("approval_policy=never")
            .arg("-m")
            .arg(req.model.model_id())
            .current_dir(scratch.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(system_prompt) = system_prompt {
            command.arg("-c").arg(format!(
                "developer_instructions={}",
                serde_json::to_string(&system_prompt).expect("serializing a string cannot fail")
            ));
        }
        if let Some(effort) = self.config.effort.as_deref() {
            command.arg("-c").arg(format!(
                "model_reasoning_effort={}",
                serde_json::to_string(effort).expect("serializing a string cannot fail")
            ));
        }
        command.arg("-");

        let start = Instant::now();
        let mut child = command.spawn().map_err(|error| {
            ProviderError::config(format!(
                "failed to spawn {}: {error}",
                self.config.binary.display()
            ))
        })?;

        let stdin_error = if let Some(mut stdin) = child.stdin.take() {
            match stdin.write_all(prompt.as_bytes()).await {
                Ok(()) => stdin.shutdown().await.err(),
                Err(error) => Some(error),
            }
        } else {
            None
        };

        let output = child.wait_with_output().await.map_err(|error| {
            ProviderError::provider(PROVIDER, format!("failed to wait for Codex: {error}"), true)
        })?;
        let latency = start.elapsed();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if !output.status.success() {
            let detail = format!(
                "Codex exited with {}; stdout: {}; stderr: {}",
                output.status,
                tail(&stdout, 500),
                tail(&stderr, 500)
            );
            return Err(classify_cli_error(detail, None));
        }
        if let Some(error) = stdin_error {
            return Err(ProviderError::provider(
                PROVIDER,
                format!("failed to write Codex prompt: {error}"),
                true,
            ));
        }

        let stream = parse_stream(&stdout).map_err(|error| {
            ProviderError::provider(
                PROVIDER,
                format!(
                    "invalid JSONL event stream: {error}; stdout: {:?}; stderr: {:?}",
                    tail(&stdout, 500),
                    tail(&stderr, 300)
                ),
                false,
            )
        })?;
        if let Some(error) = stream.error {
            return Err(classify_cli_error(error, stream.thread_id));
        }
        let content = stream.final_message.ok_or_else(|| {
            ProviderError::provider(
                PROVIDER,
                format!(
                    "missing final agent_message in JSONL event stream; stderr: {:?}",
                    tail(&stderr, 300)
                ),
                false,
            )
        })?;
        // Token counts feed run denominators; a missing usage event must not
        // silently report zero (the claude_code adapter errors identically).
        let usage = stream.usage.ok_or_else(|| {
            ProviderError::provider(
                PROVIDER,
                format!(
                    "missing token_count event in JSONL event stream; stderr: {:?}",
                    tail(&stderr, 300)
                ),
                false,
            )
        })?;

        Ok(ChatResponse {
            provider_call_id: None,
            provider_request_id: stream.thread_id,
            served_model: Some(req.model.model_id().to_string()),
            content,
            reasoning: None,
            reasoning_tokens: usage.reasoning_output_tokens,
            input_tokens: usage.input_tokens,
            output_tokens: usage.output_tokens,
            cost_nanodollars: 0,
            cost_is_estimate: false,
            upstream_cost_nanodollars: None,
            latency,
            finish_reason: FinishReason::Stop,
            output_logprobs: None,
            cache_read_tokens: usage.cached_input_tokens,
            cache_write_tokens: None,
        })
    }
}

#[derive(Deserialize)]
struct CodexEvent {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    thread_id: Option<String>,
    #[serde(default)]
    item: Option<CodexItem>,
    #[serde(default)]
    usage: Option<CodexUsage>,
    #[serde(default)]
    error: Option<CodexEventError>,
    #[serde(default)]
    message: Option<String>,
}

#[derive(Deserialize)]
struct CodexItem {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    text: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct CodexUsage {
    #[serde(default)]
    input_tokens: u32,
    #[serde(default)]
    output_tokens: u32,
    #[serde(default)]
    cached_input_tokens: Option<u32>,
    #[serde(default)]
    reasoning_output_tokens: Option<u32>,
}

#[derive(Deserialize)]
struct CodexEventError {
    message: String,
}

struct ParsedStream {
    thread_id: Option<String>,
    final_message: Option<String>,
    usage: Option<CodexUsage>,
    error: Option<String>,
}

fn parse_stream(stdout: &str) -> Result<ParsedStream, serde_json::Error> {
    let mut parsed = ParsedStream {
        thread_id: None,
        final_message: None,
        usage: None,
        error: None,
    };

    for line in stdout.lines().filter(|line| !line.trim().is_empty()) {
        let event: CodexEvent = serde_json::from_str(line)?;
        match event.kind.as_str() {
            "thread.started" => parsed.thread_id = event.thread_id,
            "item.completed" => {
                if let Some(item) = event.item {
                    if item.kind == "agent_message" {
                        parsed.final_message = item.text;
                    } else if item.kind == "error" {
                        parsed.error = item.text;
                    }
                }
            }
            "turn.completed" => parsed.usage = event.usage,
            "turn.failed" => {
                parsed.error = event
                    .error
                    .map(|error| error.message)
                    .or(event.message)
                    .or_else(|| Some("Codex turn failed".to_string()));
            }
            "error" => {
                parsed.error = event
                    .message
                    .or_else(|| event.error.map(|error| error.message));
            }
            _ => {}
        }
    }

    Ok(parsed)
}

fn map_messages(messages: &[Message]) -> (Option<String>, String) {
    let mut system_messages = Vec::new();
    let mut prompt = String::new();

    for message in messages {
        let label = match message.role {
            Role::System => {
                system_messages.push(message.content.as_str());
                continue;
            }
            Role::User => "User:\n",
            Role::Assistant => "Assistant:\n",
        };
        if !prompt.is_empty() {
            prompt.push_str("\n\n");
        }
        prompt.push_str(label);
        prompt.push_str(&message.content);
    }

    let system_prompt = (!system_messages.is_empty()).then(|| system_messages.join("\n\n"));
    (system_prompt, prompt)
}

fn classify_cli_error(message: String, request_id: Option<String>) -> ProviderError {
    let lowercase = message.to_ascii_lowercase();
    if QUOTA_MARKERS
        .iter()
        .any(|marker| lowercase.contains(marker))
    {
        let mut context = ErrorContext::new().with_code(message);
        if let Some(request_id) = request_id {
            context = context.with_request_id(request_id);
        }
        return ProviderError::rate_limited_subscription(context);
    }
    if SAFEGUARD_MARKERS
        .iter()
        .any(|marker| lowercase.contains(marker))
    {
        return ProviderError::refused(message);
    }

    let context = request_id.map(|request_id| ErrorContext::new().with_request_id(request_id));
    match context {
        Some(context) => ProviderError::provider_with_context(PROVIDER, message, true, context),
        None => ProviderError::provider(PROVIDER, message, true),
    }
}

fn tail(text: &str, max_chars: usize) -> String {
    let mut chars = text.chars().rev().take(max_chars).collect::<Vec<_>>();
    chars.reverse();
    chars.into_iter().collect()
}
