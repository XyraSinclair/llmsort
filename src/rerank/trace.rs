//! Comparison trace capture for rerank runs.

use serde::{Deserialize, Serialize};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::mpsc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::gateway::PairwiseLogprobPosterior;
use crate::rating_engine::Observation;
use crate::rerank::decimal_ledger::LedgerDrawsRecord;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonTrace {
    pub timestamp_ms: i64,
    pub comparison_index: usize,
    pub attribute_id: String,
    pub attribute_index: usize,
    pub attribute_prompt_hash: String,
    pub prompt_template_slug: String,
    pub template_hash: String,
    /// Content identity of the exact rendered system and user message bytes.
    #[serde(default)]
    pub rendered_prompt_digest: String,
    /// Content identity of the [`crate::rating_engine::EngineSpec`] this row entered.
    #[serde(default)]
    pub engine_spec_id: String,
    pub entity_a_id: String,
    pub entity_b_id: String,
    pub entity_a_index: usize,
    pub entity_b_index: usize,
    pub entity_a_hash: String,
    pub entity_b_hash: String,
    pub cache_key_hash: String,
    pub model: String,
    /// Provider-reported served model for this row's live call, when the
    /// adapter surfaces one (e.g. Claude Code modelUsage keys). Absent for
    /// cached rows and providers that do not report it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub served_model: Option<String>,
    pub higher_ranked: Option<String>,
    pub ratio: Option<f64>,
    pub confidence: Option<f64>,
    /// Exact observation routed to solver ingestion for this row.
    ///
    /// The engine's deterministic input filters still apply; replay feeds the
    /// same value through those filters. Absent for refusals, provider errors,
    /// routing failures, and traces written before the replay contract existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub solver_observation: Option<Observation>,
    /// Compact posterior inferred from provider output logprobs, when
    /// available. This records probability mass over preferred side, ratio
    /// bucket, semantic answer, and signed log-ratio without dumping raw token
    /// logprobs into every trace row.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pairwise_logprob_posterior: Option<PairwiseLogprobPosterior>,
    /// Number of output token logprob entries returned by the provider. This
    /// lets audits distinguish provider-level absence from posterior parsing
    /// failures.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_logprob_token_count: Option<usize>,
    /// Diagnostic reason when `pairwise_logprob_posterior` is absent for a
    /// non-cached observation where logprobs were requested.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pairwise_logprob_posterior_error: Option<String>,
    /// Raw decimal-ledger draw trajectories + grammar version — the
    /// estimator-replay seam. Present only for live rows whose evidence came
    /// from the exact-atom ledger; `decimal_ledger::analyze(&record.draws)`
    /// reproduces this row's moments and certificate offline (see
    /// `examples/replay_trace.rs`). Absent for cached rows (the cache stores
    /// collapsed moments, not draws), point/MC rows, and older traces.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ledger_draws: Option<LedgerDrawsRecord>,
    /// PMF-derived signed log-ratio moments in PRESENTED coordinates
    /// (seriate/ledger evidence rails). This is the per-row measurement the
    /// solver weights by; persisted so landings and replays keep it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_moments: Option<crate::rerank::comparison::EvidenceMoments>,
    pub refused: bool,
    pub cached: bool,
    /// Whether entity A/B presentation order was swapped to counteract
    /// position bias.  When true, the entity at index `entity_a_index` was
    /// shown in the "B" position and vice versa.
    #[serde(default)]
    pub swapped: bool,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub provider_cost_nanodollars: i64,
    #[serde(default)]
    pub provider_cost_is_estimate: bool,
    pub error: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum TraceError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serde(String),
    #[error("trace channel closed")]
    Closed,
    #[error("trace worker failed: {0}")]
    Join(String),
}

pub trait TraceSink: Send + Sync {
    fn record(&self, event: ComparisonTrace) -> Result<(), TraceError>;
}

#[derive(Clone)]
pub struct JsonlTraceSink {
    sender: mpsc::Sender<ComparisonTrace>,
}

pub struct TraceWorker {
    handle: Option<std::thread::JoinHandle<Result<(), TraceError>>>,
}

impl TraceWorker {
    pub fn join(mut self) -> Result<(), TraceError> {
        let handle = self.handle.take();
        match handle {
            Some(handle) => match handle.join() {
                Ok(result) => result,
                Err(_) => Err(TraceError::Join("trace worker panicked".to_string())),
            },
            None => Ok(()),
        }
    }
}

impl JsonlTraceSink {
    pub fn new(path: impl AsRef<Path>) -> Result<(Self, TraceWorker), TraceError> {
        let file = std::fs::File::create(path)?;
        let (sender, receiver) = mpsc::channel::<ComparisonTrace>();
        let handle = std::thread::spawn(move || write_trace_loop(file, receiver));
        Ok((
            Self { sender },
            TraceWorker {
                handle: Some(handle),
            },
        ))
    }
}

impl TraceSink for JsonlTraceSink {
    fn record(&self, event: ComparisonTrace) -> Result<(), TraceError> {
        self.sender.send(event).map_err(|_| TraceError::Closed)
    }
}

fn write_trace_loop(
    file: std::fs::File,
    receiver: mpsc::Receiver<ComparisonTrace>,
) -> Result<(), TraceError> {
    let mut writer = BufWriter::new(file);
    for event in receiver {
        let line = serde_json::to_string(&event).map_err(|e| TraceError::Serde(e.to_string()))?;
        writeln!(writer, "{line}")?;
    }
    writer.flush()?;
    Ok(())
}

pub fn now_epoch_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
