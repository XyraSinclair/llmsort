//! Portable, durable records for finite-candidate, single-axis judgement runs.
//!
//! The record is the boundary between provider-backed execution and replay: once
//! persisted, callers can reload and project the response without a gateway.

pub mod edge;

use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use rand::seq::SliceRandom;
use rand::{rngs::StdRng, SeedableRng};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use llmsort::gateway::{
    ChatGateway, ChatRequest, ChatResponse, ProviderError, ReasoningEffort, Role,
};
use llmsort::rating_engine::{AttributeParams, EngineSpec, Observation, RaterParams, RatingEngine};
use llmsort::rerank::comparison::{
    PairwiseComparisonAttribute, PairwiseComparisonEntity, PairwiseComparisonSpec,
};
use llmsort::rerank::{
    multi_rerank, validate_multi_rerank_request, AttributeScoreSummary, ComparisonTrace,
    MultiRerankAttributeSpec, MultiRerankEntity, MultiRerankRequest, MultiRerankResponse,
    MultiRerankTopKSpec, RerankExecution, RerankRunOptions, RerankStopReason, TraceError,
    TraceSink,
};
use llmsort::trait_search::TraitSearchManager;

pub const JUDGEMENT_RUN_SCHEMA: &str = "cardinal.judgement-run.v1";
pub const JUDGEMENT_PROMPT_TEMPLATE_SLUG: &str = "canonical_v2";
// Counterbalancing spends two calls per pair, so 8·n calls preserve the old
// uncounterbalanced budget's approximately 4·n distinct-pair coverage.
const COMPARISONS_PER_ENTITY: usize = 8;
const RUN_REF_PREFIX: &str = "jrun_";
const PROVIDER_CALL_REF_PREFIX: &str = "pcall_";
const COMPARISON_CONCURRENCY: usize = 8;
const SCHEDULE_VERSION: u32 = 1;
const REQUEST_DIGEST_DOMAIN: &[u8] = b"cardinal.gateway-request.v1\0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JudgementPrivacy {
    Public,
    Private,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JudgementCandidate {
    pub id: String,
    pub text: String,
}

/// Finite-candidate, one-axis request accepted at the portable-run boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JudgementRunRequest {
    pub entities: Vec<JudgementCandidate>,
    pub axis_key: String,
    pub axis_prompt: String,
    pub requested_k: usize,
    pub model: String,
    pub privacy: JudgementPrivacy,
    /// Per-run cap on concurrent provider calls (1..=16). Free-tier judges
    /// allow ~20 requests/min account-wide, so the default 8-way burst 429s
    /// the whole run; `1` elicits strictly serially. Absent = the daemon
    /// default (`COMPARISON_CONCURRENCY`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comparison_concurrency: Option<usize>,
    /// Optional floor between provider request starts, in milliseconds
    /// (≤ 60_000). Free-tier judges are rate-limited per minute, so even a
    /// serial run can outrun the window when the model answers quickly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_request_interval_ms: Option<u64>,
    /// Optional OpenAI-compatible provider base URL (https). Absent = the
    /// daemon's configured OpenRouter endpoint. Lets one daemon elicit from
    /// several free-tier providers (Cerebras, Gemini, …); the caller supplies
    /// the matching key via `x-provider-key`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_base_url: Option<String>,
    /// Optional resample width (1..=8). Each planned comparison is drawn N
    /// times with a distinct draw-token nonce appended after every stable
    /// byte, so the serve's prefix cache makes draws 2..n nearly free while
    /// yielding independent samples of the same judgement. The engine's
    /// comparison budget is scaled by N so distinct-pair coverage is
    /// preserved. Absent/1 = legacy single-draw behavior.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nonce_draws: Option<u32>,
}

/// Validated request bytes that were used to construct the instrument.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NormalizedJudgementRunRequest {
    pub entities: Vec<JudgementCandidate>,
    pub axis_key: String,
    pub axis_prompt: String,
    pub requested_k: usize,
    pub model: String,
    pub privacy: JudgementPrivacy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comparison_concurrency: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_request_interval_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_base_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nonce_draws: Option<u32>,
}

impl JudgementRunRequest {
    pub fn normalize(mut self) -> Result<NormalizedJudgementRunRequest, JudgementRunError> {
        for entity in &mut self.entities {
            entity.id = entity.id.trim().to_string();
        }
        self.axis_key = self.axis_key.trim().to_string();
        self.axis_prompt = self.axis_prompt.trim().to_string();
        self.model = self.model.trim().to_string();

        let normalized = NormalizedJudgementRunRequest {
            entities: self.entities,
            axis_key: self.axis_key,
            axis_prompt: self.axis_prompt,
            requested_k: self.requested_k,
            model: self.model,
            privacy: self.privacy,
            comparison_concurrency: self.comparison_concurrency,
            min_request_interval_ms: self.min_request_interval_ms,
            provider_base_url: self
                .provider_base_url
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            nonce_draws: self.nonce_draws,
        };
        normalized
            .validate()
            .map_err(JudgementRunError::InvalidRequest)?;
        Ok(normalized)
    }
}

impl NormalizedJudgementRunRequest {
    fn validate(&self) -> Result<(), String> {
        if self.entities.len() < 2 {
            return Err("entities must contain at least 2 items".to_string());
        }
        if self.axis_key.is_empty() {
            return Err("axis_key must not be blank".to_string());
        }
        if self.axis_prompt.is_empty() {
            return Err("axis_prompt must not be blank".to_string());
        }
        if self.model.is_empty() {
            return Err("model must not be blank".to_string());
        }
        if self.requested_k == 0 || self.requested_k > self.entities.len() {
            return Err(format!(
                "requested_k must be between 1 and the entity count ({})",
                self.entities.len()
            ));
        }
        if let Some(concurrency) = self.comparison_concurrency {
            if !(1..=16).contains(&concurrency) {
                return Err("comparison_concurrency must be between 1 and 16".to_string());
            }
        }
        if let Some(interval) = self.min_request_interval_ms {
            if interval > 60_000 {
                return Err("min_request_interval_ms must be at most 60000".to_string());
            }
        }
        if let Some(draws) = self.nonce_draws {
            if !(1..=8).contains(&draws) {
                return Err("nonce_draws must be between 1 and 8".to_string());
            }
        }
        if let Some(url) = &self.provider_base_url {
            let loopback_http = url.starts_with("http://127.0.0.1")
                || url.starts_with("http://localhost")
                || url.starts_with("http://[::1]");
            if !url.starts_with("https://") && !loopback_http {
                return Err(
                    "provider_base_url must be https (or http on loopback for local engines)"
                        .to_string(),
                );
            }
        }

        let mut ids = HashSet::with_capacity(self.entities.len());
        for entity in &self.entities {
            if entity.id.is_empty() {
                return Err("entity ids must not be blank".to_string());
            }
            if entity.id.trim() != entity.id {
                return Err(format!("entity id is not normalized: {}", entity.id));
            }
            if entity.text.trim().is_empty() {
                return Err(format!("entity text must not be blank: {}", entity.id));
            }
            if !ids.insert(entity.id.as_str()) {
                return Err(format!("duplicate entity id: {}", entity.id));
            }
        }

        if self.axis_key.trim() != self.axis_key
            || self.axis_prompt.trim() != self.axis_prompt
            || self.model.trim() != self.model
        {
            return Err("request strings are not normalized".to_string());
        }
        Ok(())
    }
}

/// Maximum comparisons that the portable single-axis run planner may attempt.
///
/// `requested_k` affects proposal priority and early stopping, but the hard
/// comparison budget is currently eight attempts per entity for every valid
/// normalized request.
pub fn max_judgement_run_comparisons(request: &NormalizedJudgementRunRequest) -> usize {
    COMPARISONS_PER_ENTITY.saturating_mul(request.entities.len())
}

#[derive(Debug, Clone, Serialize)]
pub struct ExternalJudgementSchedule {
    pub schedule_version: u32,
    pub template_slug: String,
    pub template_hash: String,
    pub seed: u64,
    pub schedule_digest: String,
    pub comparisons: Vec<ScheduledComparison>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScheduledComparison {
    pub comparison_index: u32,
    pub entity_a_id: String,
    pub entity_b_id: String,
    pub swapped: bool,
    pub system_prompt: String,
    pub user_prompt: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum ExternalHigherRanked {
    A,
    B,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExternalJudgementResult {
    pub comparison_index: u32,
    pub entity_a_id: String,
    pub entity_b_id: String,
    pub swapped: bool,
    pub higher_ranked: ExternalHigherRanked,
    pub ratio: f64,
    pub confidence: f64,
    #[serde(default)]
    pub refused: bool,
    #[serde(default)]
    pub input_tokens: Option<u64>,
    #[serde(default)]
    pub output_tokens: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExternalJudgementRun {
    pub harness: String,
    pub harness_version: String,
    pub model: String,
    pub seed: u64,
    pub schedule_digest: String,
    pub results: Vec<ExternalJudgementResult>,
}

/// Digest binding an external schedule to the exact request it was rendered
/// for: template bytes, seed, axis, budget, and every entity id and text.
/// External results must return it, so verdicts collected under one prompt
/// rendering can never be landed under different texts, a different axis, or
/// a drifted template (independent review 2026-08-10, finding 1).
#[must_use]
pub fn external_schedule_digest(request: &NormalizedJudgementRunRequest, seed: u64) -> String {
    use sha2::{Digest, Sha256};
    let template = llmsort::prompts::PROMPT_V2;
    let mut hasher = Sha256::new();
    let mut frame = |bytes: &[u8]| {
        // Length-prefixed framing keeps adjacent fields from aliasing.
        Sha256::update(&mut hasher, (bytes.len() as u64).to_le_bytes());
        Sha256::update(&mut hasher, bytes);
    };
    frame(b"cardinald-external-schedule-v1");
    frame(template.template_hash().as_bytes());
    frame(&seed.to_le_bytes());
    frame(request.axis_key.as_bytes());
    frame(request.axis_prompt.as_bytes());
    frame(&(request.requested_k as u64).to_le_bytes());
    frame(&(request.entities.len() as u64).to_le_bytes());
    for entity in &request.entities {
        frame(entity.id.as_bytes());
        frame(entity.text.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JudgementRunProvenance {
    pub harness: String,
    pub harness_version: String,
    pub model: String,
}

/// Build the fixed external schedule for a normalized portable request.
#[must_use]
pub fn build_external_schedule(
    request: &NormalizedJudgementRunRequest,
    seed: u64,
) -> ExternalJudgementSchedule {
    let template = llmsort::prompts::PROMPT_V2;
    let pair_budget = max_judgement_run_comparisons(request) / 2;
    let mut comparisons = Vec::with_capacity(pair_budget * 2);

    for (pair_index, (entity_a_index, entity_b_index)) in
        seeded_matching_pairs(request.entities.len(), pair_budget, seed)
            .into_iter()
            .enumerate()
    {
        let entity_a = &request.entities[entity_a_index];
        let entity_b = &request.entities[entity_b_index];
        for swapped in [false, true] {
            let (presented_a, presented_b) = if swapped {
                (entity_b, entity_a)
            } else {
                (entity_a, entity_b)
            };
            let prompt = comparison_spec(request, &request.model, presented_a, presented_b)
                .prompt_instance();
            comparisons.push(ScheduledComparison {
                comparison_index: u32::try_from(pair_index * 2 + usize::from(swapped) + 1)
                    .expect("portable schedule budget fits UInt32"),
                entity_a_id: entity_a.id.clone(),
                entity_b_id: entity_b.id.clone(),
                swapped,
                system_prompt: prompt.system,
                user_prompt: prompt.user,
            });
        }
    }

    ExternalJudgementSchedule {
        schedule_version: SCHEDULE_VERSION,
        template_slug: template.slug.to_string(),
        template_hash: template.template_hash(),
        seed,
        schedule_digest: external_schedule_digest(request, seed),
        comparisons,
    }
}

fn seeded_matching_pairs(n: usize, count: usize, seed: u64) -> Vec<(usize, usize)> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut pairs = Vec::with_capacity(count);
    while pairs.len() < count {
        let mut rotation: Vec<usize> = (0..n).collect();
        rotation.shuffle(&mut rng);
        if n % 2 == 1 {
            rotation.push(n);
        }
        for _ in 0..rotation.len().saturating_sub(1) {
            let last = rotation.len() - 1;
            for offset in 0..rotation.len() / 2 {
                let left = rotation[offset];
                let right = rotation[last - offset];
                if left != n && right != n {
                    pairs.push((left.min(right), left.max(right)));
                    if pairs.len() == count {
                        return pairs;
                    }
                }
            }
            let tail = rotation.pop().expect("matching rotation is nonempty");
            rotation.insert(1, tail);
        }
    }
    pairs
}

fn comparison_spec<'a>(
    request: &'a NormalizedJudgementRunRequest,
    model: &'a str,
    entity_a: &'a JudgementCandidate,
    entity_b: &'a JudgementCandidate,
) -> PairwiseComparisonSpec<'a> {
    PairwiseComparisonSpec {
        model,
        attribute: PairwiseComparisonAttribute {
            id: &request.axis_key,
            prompt: &request.axis_prompt,
            prompt_template_slug: Some(JUDGEMENT_PROMPT_TEMPLATE_SLUG),
        },
        entity_a: PairwiseComparisonEntity {
            id: &entity_a.id,
            text: &entity_a.text,
        },
        entity_b: PairwiseComparisonEntity {
            id: &entity_b.id,
            text: &entity_b.text,
        },
    }
}

pub fn validate_external_judgement_run(
    request: &NormalizedJudgementRunRequest,
    external: &ExternalJudgementRun,
) -> Result<(), String> {
    if external.harness != "claude-code" {
        return Err("external.harness must be claude-code".to_string());
    }
    if external.harness_version.is_empty()
        || external.harness_version.chars().count() > 64
        || external.harness_version.chars().any(char::is_control)
    {
        return Err(
            "external.harness_version must contain 1 to 64 printable characters".to_string(),
        );
    }
    if external.model.trim().is_empty() || external.model.trim() != external.model {
        return Err("external.model must be nonblank and normalized".to_string());
    }
    if external.model.len() > 200 {
        return Err("external.model must not exceed 200 bytes".to_string());
    }
    if external.schedule_digest != external_schedule_digest(request, external.seed) {
        return Err(
            "external.schedule_digest does not match the issued schedule for this request"
                .to_string(),
        );
    }

    let schedule = build_external_schedule(request, external.seed);
    let scheduled: HashMap<u32, (&str, &str, bool)> = schedule
        .comparisons
        .iter()
        .map(|comparison| {
            (
                comparison.comparison_index,
                (
                    comparison.entity_a_id.as_str(),
                    comparison.entity_b_id.as_str(),
                    comparison.swapped,
                ),
            )
        })
        .collect();
    let entity_ids: HashSet<&str> = request
        .entities
        .iter()
        .map(|entity| entity.id.as_str())
        .collect();
    let mut indices = HashSet::with_capacity(external.results.len());
    let mut usable = 0usize;
    let mut input_tokens = 0u32;
    let mut output_tokens = 0u32;

    for result in &external.results {
        if !indices.insert(result.comparison_index) {
            return Err(format!(
                "external comparison_index is duplicated: {}",
                result.comparison_index
            ));
        }
        if !entity_ids.contains(result.entity_a_id.as_str())
            || !entity_ids.contains(result.entity_b_id.as_str())
        {
            return Err(format!(
                "external result {} references an entity outside the request",
                result.comparison_index
            ));
        }
        let Some(&(entity_a_id, entity_b_id, swapped)) = scheduled.get(&result.comparison_index)
        else {
            return Err(format!(
                "external comparison_index is outside the scheduled budget: {}",
                result.comparison_index
            ));
        };
        if result.entity_a_id != entity_a_id
            || result.entity_b_id != entity_b_id
            || result.swapped != swapped
        {
            return Err(format!(
                "external result {} does not match the schedule for seed {}",
                result.comparison_index, external.seed
            ));
        }
        if !result.ratio.is_finite() || !(1.0..=26.0).contains(&result.ratio) {
            return Err(format!(
                "external result {} ratio must be within [1,26]",
                result.comparison_index
            ));
        }
        if !result.confidence.is_finite() || !(0.0..=1.0).contains(&result.confidence) {
            return Err(format!(
                "external result {} confidence must be within [0,1]",
                result.comparison_index
            ));
        }
        let row_input = u32::try_from(result.input_tokens.unwrap_or(0)).map_err(|_| {
            format!(
                "external result {} input_tokens exceeds UInt32",
                result.comparison_index
            )
        })?;
        let row_output = u32::try_from(result.output_tokens.unwrap_or(0)).map_err(|_| {
            format!(
                "external result {} output_tokens exceeds UInt32",
                result.comparison_index
            )
        })?;
        input_tokens = input_tokens.checked_add(row_input).ok_or_else(|| {
            "external input token total exceeds the durable record limit".to_string()
        })?;
        output_tokens = output_tokens.checked_add(row_output).ok_or_else(|| {
            "external output token total exceeds the durable record limit".to_string()
        })?;
        usable += usize::from(!result.refused);
    }

    if usable == 0 {
        return Err(
            "external results must contain at least one non-refused comparison".to_string(),
        );
    }
    // Coverage floors (independent review 2026-08-10, finding 2): a partial
    // result set must not mint full-cohort scores. Every scheduled comparison
    // must be answered (a refusal is an answer), and every entity must carry
    // at least one non-refused measurement or the run is rejected rather than
    // landing flat priors as public scores.
    if indices.len() != scheduled.len() {
        return Err(format!(
            "external results must answer every scheduled comparison: {} of {} present",
            indices.len(),
            scheduled.len()
        ));
    }
    let mut measured: HashSet<&str> = HashSet::with_capacity(entity_ids.len());
    for result in &external.results {
        if !result.refused {
            measured.insert(result.entity_a_id.as_str());
            measured.insert(result.entity_b_id.as_str());
        }
    }
    if let Some(unmeasured) = entity_ids.iter().find(|id| !measured.contains(*id)) {
        return Err(format!(
            "entity {unmeasured} has no non-refused comparison; refusing to land an unmeasured score"
        ));
    }
    Ok(())
}

/// Exact resolved rerank invocation plus the solver constructor spec it produced.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JudgementInstrumentSpec {
    pub rerank_request: MultiRerankRequest,
    pub cache_enabled: bool,
    pub cache_only: bool,
    pub rng_seed: Option<u64>,
    pub engine_spec: Option<EngineSpec>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct JudgementRunUsage {
    pub provider_input_tokens: u32,
    pub provider_output_tokens: u32,
    pub provider_cost_nanodollars: i64,
    pub provider_cost_is_estimate: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JudgementAttributeScore {
    pub latent_mean: f64,
    pub latent_std: f64,
    #[serde(default)]
    pub z_score: f64,
    pub percentile: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JudgementEntityScore {
    pub id: String,
    #[serde(default)]
    pub rank: Option<usize>,
    pub feasible: bool,
    pub p_flip: f64,
    pub attribute_score: JudgementAttributeScore,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JudgementRunResponse {
    pub entities: Vec<JudgementEntityScore>,
    pub global_topk_error: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum JudgementRunTerminal {
    Completed {
        stop_reason: RerankStopReason,
        response: JudgementRunResponse,
    },
    Cancelled {
        response: JudgementRunResponse,
    },
    Failed {
        error: String,
    },
}

impl JudgementRunTerminal {
    #[must_use]
    pub fn status(&self) -> &'static str {
        match self {
            Self::Completed { .. } => "completed",
            Self::Cancelled { .. } => "cancelled",
            Self::Failed { .. } => "failed",
        }
    }

    #[must_use]
    pub fn completed_response(&self) -> Option<&JudgementRunResponse> {
        match self {
            Self::Completed { response, .. } => Some(response),
            Self::Cancelled { .. } | Self::Failed { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JudgementProviderCall {
    pub call_ref: String,
    pub sequence: usize,
    pub provider: String,
    pub model: String,
    pub gateway_request_digest: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub outcome: JudgementProviderCallOutcome,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum JudgementProviderCallOutcome {
    Succeeded {
        provider_call_id: Option<String>,
        provider_request_id: Option<String>,
        input_tokens: u32,
        output_tokens: u32,
        cost_nanodollars: i64,
        cost_is_estimate: bool,
    },
    Failed {
        provider_request_id: Option<String>,
        error_code: String,
        error: String,
    },
}

/// Self-contained terminal atom. No provider capability is needed to read it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JudgementRunRecord {
    pub schema: String,
    pub run_ref: String,
    pub request: NormalizedJudgementRunRequest,
    pub instrument: JudgementInstrumentSpec,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<JudgementRunProvenance>,
    pub comparison_trace: Vec<ComparisonTrace>,
    pub provider_calls: Vec<JudgementProviderCall>,
    pub usage: JudgementRunUsage,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub terminal: JudgementRunTerminal,
}

impl JudgementRunRecord {
    #[must_use]
    pub fn completed_response(&self) -> Option<&JudgementRunResponse> {
        self.terminal.completed_response()
    }
}

/// Atomic, one-record-per-file JSON store keyed by opaque `run_ref`.
#[derive(Debug, Clone)]
pub struct JudgementRunStore {
    root: PathBuf,
}

impl JudgementRunStore {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Allocate an opaque run reference for work that will be persisted later.
    #[must_use]
    pub fn allocate_run_ref(&self) -> String {
        new_opaque_ref(RUN_REF_PREFIX)
    }

    pub fn persist(&self, record: &JudgementRunRecord) -> Result<(), JudgementRunError> {
        validate_record(record)?;
        fs::create_dir_all(&self.root)?;
        let destination = self.record_path(&record.run_ref)?;
        let mut temporary = tempfile::NamedTempFile::new_in(&self.root)?;
        {
            let mut writer = BufWriter::new(temporary.as_file_mut());
            serde_json::to_writer(&mut writer, record)?;
            writer.write_all(b"\n")?;
            writer.flush()?;
        }
        temporary.as_file().sync_all()?;
        temporary
            .persist_noclobber(&destination)
            .map_err(|error| JudgementRunError::Io(error.error))?;
        File::open(&self.root)?.sync_all()?;
        Ok(())
    }

    pub fn load(&self, run_ref: &str) -> Result<JudgementRunRecord, JudgementRunError> {
        let path = self.record_path(run_ref)?;
        let reader = BufReader::new(File::open(path)?);
        let record: JudgementRunRecord = serde_json::from_reader(reader)?;
        validate_record(&record)?;
        if record.run_ref != run_ref {
            return Err(JudgementRunError::InvalidRecord(format!(
                "requested run_ref {run_ref} does not match stored {}",
                record.run_ref
            )));
        }
        Ok(record)
    }

    fn record_path(&self, run_ref: &str) -> Result<PathBuf, JudgementRunError> {
        validate_opaque_ref(run_ref, RUN_REF_PREFIX).map_err(JudgementRunError::InvalidRunRef)?;
        Ok(self.root.join(format!("{run_ref}.json")))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum JudgementRunError {
    #[error("invalid judgement request: {0}")]
    InvalidRequest(String),
    #[error("execution cannot produce a portable v1 record: {0}")]
    UnsupportedExecution(String),
    #[error("invalid run_ref: {0}")]
    InvalidRunRef(String),
    #[error("invalid judgement-run record: {0}")]
    InvalidRecord(String),
    #[error("judgement run {run_ref} failed: {source}")]
    Execution {
        run_ref: String,
        #[source]
        source: Box<llmsort::rerank::MultiRerankError>,
    },
    #[error("judgement run {run_ref} violated its execution contract: {error}")]
    ExecutionInvariant { run_ref: String, error: String },
    #[error(
        "judgement run {run_ref} failed and its terminal record could not be persisted: execution={execution}; persistence={persistence}"
    )]
    FailedRecordPersistence {
        run_ref: String,
        execution: String,
        persistence: String,
    },
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Execute, capture, and atomically persist one portable judgement run.
pub async fn execute_judgement_run(
    request: JudgementRunRequest,
    execution: RerankExecution<'_>,
    store: &JudgementRunStore,
) -> Result<JudgementRunRecord, JudgementRunError> {
    let run_ref = store.allocate_run_ref();
    execute_judgement_run_with_ref(request, run_ref, execution, store).await
}

/// Execute a portable judgement run using a caller-preallocated run reference.
pub async fn execute_judgement_run_with_ref(
    request: JudgementRunRequest,
    run_ref: String,
    execution: RerankExecution<'_>,
    store: &JudgementRunStore,
) -> Result<JudgementRunRecord, JudgementRunError> {
    validate_opaque_ref(&run_ref, RUN_REF_PREFIX).map_err(JudgementRunError::InvalidRunRef)?;
    let request = request.normalize()?;
    let rerank_request = build_rerank_request(&request);
    validate_multi_rerank_request(&rerank_request)
        .map_err(|error| JudgementRunError::InvalidRequest(error.to_string()))?;

    let (gateway, upstream_trace, run_options, cache_enabled) = execution
        .judgement_run_instrumentation()
        .map_err(|reason| JudgementRunError::UnsupportedExecution(reason.to_string()))?;

    let started_at = Utc::now();
    let trace = CapturingTraceSink::new(upstream_trace);
    let provider_calls = Arc::new(Mutex::new(Vec::new()));
    let recording_gateway = Arc::new(RecordingGateway {
        inner: gateway,
        calls: Arc::clone(&provider_calls),
        next_sequence: AtomicUsize::new(0),
    });
    let instrumented = execution.with_judgement_run_instrumentation(recording_gateway, &trace);

    let mut instrument = JudgementInstrumentSpec {
        rerank_request: rerank_request.clone(),
        cache_enabled,
        cache_only: run_options.cache_only,
        rng_seed: run_options.rng_seed,
        engine_spec: None,
    };
    let result = multi_rerank(rerank_request, instrumented).await;
    let finished_at = Utc::now();
    let mut comparison_trace = trace.events();
    comparison_trace.sort_by_key(|event| (event.comparison_index, event.timestamp_ms));
    let mut provider_calls = lock_unpoisoned(&provider_calls).clone();
    provider_calls.sort_by_key(|call| call.sequence);

    match result {
        Ok(response) => {
            let projection = match project_response(&request.axis_key, response) {
                Ok(projection) => projection,
                Err(error) => {
                    let record = failed_record(
                        run_ref.clone(),
                        request,
                        instrument,
                        None,
                        comparison_trace,
                        provider_calls,
                        started_at,
                        finished_at,
                        error.clone(),
                    );
                    persist_failed(store, &record, &error)?;
                    return Err(JudgementRunError::ExecutionInvariant { run_ref, error });
                }
            };
            instrument.engine_spec = Some(projection.engine_spec);
            // A run with zero successful comparisons has measured nothing:
            // its scores are the solver's flat priors, not judgements. Landing
            // them as "completed" poisons the public ledger with plausible-
            // looking zeros (observed live 2026-08-02: exhausted provider key
            // -> every attempt failed -> comparisons_used=0, stop_reason
            // budget_exhausted, flat scores landed). Fail loudly instead,
            // naming the dominant provider error.
            if projection.stop_reason != RerankStopReason::Cancelled
                && projection.comparisons_used == 0
            {
                let failed_calls = provider_calls
                    .iter()
                    .filter(|call| {
                        matches!(call.outcome, JudgementProviderCallOutcome::Failed { .. })
                    })
                    .count();
                let last_error = provider_calls
                    .iter()
                    .rev()
                    .find_map(|call| match &call.outcome {
                        JudgementProviderCallOutcome::Failed {
                            error_code, error, ..
                        } => Some(format!("{error_code}: {error}")),
                        JudgementProviderCallOutcome::Succeeded { .. } => None,
                    });
                let error = match last_error {
                    Some(last) => format!(
                        "no pairwise comparison succeeded ({failed_calls} provider attempts failed; last error: {last})"
                    ),
                    None => "no pairwise comparison was attempted; refusing to report unmeasured flat priors as a completed run".to_string(),
                };
                let record = failed_record(
                    run_ref.clone(),
                    request,
                    instrument,
                    None,
                    comparison_trace,
                    provider_calls,
                    started_at,
                    finished_at,
                    error.clone(),
                );
                persist_failed(store, &record, &error)?;
                return Err(JudgementRunError::ExecutionInvariant { run_ref, error });
            }
            let terminal = if projection.stop_reason == RerankStopReason::Cancelled {
                JudgementRunTerminal::Cancelled {
                    response: projection.response,
                }
            } else {
                JudgementRunTerminal::Completed {
                    stop_reason: projection.stop_reason,
                    response: projection.response,
                }
            };
            let record = JudgementRunRecord {
                schema: JUDGEMENT_RUN_SCHEMA.to_string(),
                run_ref,
                request,
                instrument,
                provenance: None,
                comparison_trace,
                provider_calls,
                usage: projection.usage,
                started_at,
                finished_at,
                terminal,
            };
            store.persist(&record)?;
            Ok(record)
        }
        Err(source) => {
            let execution_error = source.to_string();
            let record = failed_record(
                run_ref.clone(),
                request,
                instrument,
                None,
                comparison_trace,
                provider_calls,
                started_at,
                finished_at,
                execution_error.clone(),
            );
            persist_failed(store, &record, &execution_error)?;
            Err(JudgementRunError::Execution {
                run_ref,
                source: Box::new(source),
            })
        }
    }
}

/// Fit, capture, and persist a portable run from daemon-scheduled external judgements.
pub async fn execute_external_judgement_run_with_ref(
    request: JudgementRunRequest,
    run_ref: String,
    external: ExternalJudgementRun,
    store: &JudgementRunStore,
) -> Result<JudgementRunRecord, JudgementRunError> {
    validate_opaque_ref(&run_ref, RUN_REF_PREFIX).map_err(JudgementRunError::InvalidRunRef)?;
    let request = request.normalize()?;
    let rerank_request = build_rerank_request(&request);
    validate_multi_rerank_request(&rerank_request)
        .map_err(|error| JudgementRunError::InvalidRequest(error.to_string()))?;
    validate_external_judgement_run(&request, &external)
        .map_err(JudgementRunError::InvalidRequest)?;

    let started_at = Utc::now();
    let provenance = JudgementRunProvenance {
        harness: external.harness.clone(),
        harness_version: external.harness_version.clone(),
        model: external.model.clone(),
    };
    let mut instrument = JudgementInstrumentSpec {
        rerank_request: rerank_request.clone(),
        cache_enabled: false,
        cache_only: false,
        rng_seed: Some(external.seed),
        engine_spec: None,
    };

    let fit = match fit_external_results(&request, &rerank_request, &external) {
        Ok(fit) => fit,
        Err(error) => {
            let record = failed_record(
                run_ref.clone(),
                request,
                instrument,
                Some(provenance),
                Vec::new(),
                Vec::new(),
                started_at,
                Utc::now(),
                error.clone(),
            );
            persist_failed(store, &record, &error)?;
            return Err(JudgementRunError::ExecutionInvariant { run_ref, error });
        }
    };
    instrument.engine_spec = Some(fit.engine_spec);
    let record = JudgementRunRecord {
        schema: JUDGEMENT_RUN_SCHEMA.to_string(),
        run_ref,
        request,
        instrument,
        provenance: Some(provenance),
        comparison_trace: fit.comparison_trace,
        provider_calls: Vec::new(),
        usage: fit.usage,
        started_at,
        finished_at: Utc::now(),
        terminal: JudgementRunTerminal::Completed {
            stop_reason: RerankStopReason::BudgetExhausted,
            response: fit.response,
        },
    };
    store.persist(&record)?;
    Ok(record)
}

struct ExternalFit {
    response: JudgementRunResponse,
    engine_spec: EngineSpec,
    comparison_trace: Vec<ComparisonTrace>,
    usage: JudgementRunUsage,
}

fn fit_external_results(
    request: &NormalizedJudgementRunRequest,
    rerank_request: &MultiRerankRequest,
    external: &ExternalJudgementRun,
) -> Result<ExternalFit, String> {
    let (manager_config, topk) = llmsort::rerank::multi::build_trait_search_config(rerank_request);
    let rater_id = rerank_request
        .rater_id
        .as_deref()
        .ok_or_else(|| "external rerank request omitted rater_id".to_string())?;
    let mut raters = HashMap::new();
    raters.insert(rater_id.to_string(), RaterParams::default());
    let engine_config = llmsort::rerank::multi::build_engine_config(
        &RerankRunOptions {
            rng_seed: Some(external.seed),
            cache_only: false,
        },
        &topk,
    );

    let mut engines = HashMap::new();
    for attribute in &rerank_request.attributes {
        let engine = RatingEngine::new(
            rerank_request.entities.len(),
            AttributeParams::default(),
            raters.clone(),
            Some(engine_config.clone()),
        )
        .map_err(|error| error.to_string())?;
        engines.insert(attribute.id.clone(), engine);
    }
    let engine_spec = engines
        .values()
        .next()
        .ok_or_else(|| "external rerank request omitted its rating engine".to_string())?
        .spec();
    if engines.values().any(|engine| engine.spec() != engine_spec) {
        return Err("per-attribute engine specifications diverged".to_string());
    }
    let engine_spec_id = engine_spec.id().0;
    let mut manager =
        TraitSearchManager::new(manager_config, engines).map_err(|error| error.to_string())?;

    let entity_indices: HashMap<&str, usize> = request
        .entities
        .iter()
        .enumerate()
        .map(|(index, entity)| (entity.id.as_str(), index))
        .collect();
    let mut observations = Vec::with_capacity(external.results.len());
    let mut comparison_trace = Vec::with_capacity(external.results.len());
    let mut usage = JudgementRunUsage {
        provider_input_tokens: 0,
        provider_output_tokens: 0,
        provider_cost_nanodollars: 0,
        provider_cost_is_estimate: false,
    };

    for result in &external.results {
        let entity_a_index = *entity_indices
            .get(result.entity_a_id.as_str())
            .ok_or_else(|| format!("unknown external entity {}", result.entity_a_id))?;
        let entity_b_index = *entity_indices
            .get(result.entity_b_id.as_str())
            .ok_or_else(|| format!("unknown external entity {}", result.entity_b_id))?;
        let entity_a = &request.entities[entity_a_index];
        let entity_b = &request.entities[entity_b_index];
        let (presented_a_index, presented_b_index, presented_a, presented_b) = if result.swapped {
            (entity_b_index, entity_a_index, entity_b, entity_a)
        } else {
            (entity_a_index, entity_b_index, entity_a, entity_b)
        };
        let spec = comparison_spec(request, &external.model, presented_a, presented_b);
        let cache_key = spec.cache_key();
        let input_tokens = u32::try_from(result.input_tokens.unwrap_or(0))
            .map_err(|_| "validated external input token overflowed UInt32".to_string())?;
        let output_tokens = u32::try_from(result.output_tokens.unwrap_or(0))
            .map_err(|_| "validated external output token overflowed UInt32".to_string())?;
        usage.provider_input_tokens = usage
            .provider_input_tokens
            .checked_add(input_tokens)
            .ok_or_else(|| "validated external input token total overflowed UInt32".to_string())?;
        usage.provider_output_tokens = usage
            .provider_output_tokens
            .checked_add(output_tokens)
            .ok_or_else(|| "validated external output token total overflowed UInt32".to_string())?;

        let solver_observation = if result.refused {
            None
        } else {
            let effective = if result.swapped {
                match result.higher_ranked {
                    ExternalHigherRanked::A => ExternalHigherRanked::B,
                    ExternalHigherRanked::B => ExternalHigherRanked::A,
                }
            } else {
                result.higher_ranked
            };
            let (winner, loser) = match effective {
                ExternalHigherRanked::A => (entity_a_index, entity_b_index),
                ExternalHigherRanked::B => (entity_b_index, entity_a_index),
            };
            let observation = Observation::new(
                winner,
                loser,
                result.ratio,
                result.confidence,
                rater_id,
                1.0,
            );
            observations.push(observation.clone());
            Some(observation)
        };
        comparison_trace.push(ComparisonTrace {
            timestamp_ms: llmsort::rerank::trace::now_epoch_ms(),
            comparison_index: result.comparison_index as usize,
            attribute_id: request.axis_key.clone(),
            attribute_index: 0,
            attribute_prompt_hash: cache_key.attribute_prompt_hash,
            prompt_template_slug: cache_key.prompt_template_slug,
            template_hash: cache_key.template_hash,
            rendered_prompt_digest: spec.rendered_prompt_digest(),
            engine_spec_id: engine_spec_id.clone(),
            entity_a_id: presented_a.id.clone(),
            entity_b_id: presented_b.id.clone(),
            entity_a_index: presented_a_index,
            entity_b_index: presented_b_index,
            entity_a_hash: cache_key.entity_a_hash,
            entity_b_hash: cache_key.entity_b_hash,
            cache_key_hash: cache_key.key_hash,
            model: external.model.clone(),
            served_model: None,
            higher_ranked: (!result.refused).then(|| match result.higher_ranked {
                ExternalHigherRanked::A => "A".to_string(),
                ExternalHigherRanked::B => "B".to_string(),
            }),
            ratio: (!result.refused).then_some(result.ratio),
            confidence: (!result.refused).then_some(result.confidence),
            solver_observation,
            pairwise_logprob_posterior: None,
            output_logprob_token_count: None,
            pairwise_logprob_posterior_error: None,
            ledger_draws: None,
            refused: result.refused,
            cached: false,
            swapped: result.swapped,
            input_tokens,
            output_tokens,
            provider_cost_nanodollars: 0,
            provider_cost_is_estimate: false,
            error: None,
        });
    }

    manager
        .add_observations(&request.axis_key, &observations)
        .map_err(|error| error.to_string())?;
    manager
        .recompute_global_state()
        .map_err(|error| error.to_string())?;
    manager
        .ensure_all_attribute_units()
        .map_err(|error| error.to_string())?;
    let global_topk_error = manager.estimate_topk_error();
    let scores = manager
        .attribute_scores(&request.axis_key)
        .ok_or_else(|| "external fit omitted attribute scores".to_string())?;
    let stds = manager
        .attribute_std(&request.axis_key)
        .unwrap_or_else(|| vec![0.0; request.entities.len()]);
    let z_scores = manager
        .attribute_z_scores(&request.axis_key)
        .ok_or_else(|| "external fit omitted attribute z-scores".to_string())?;
    let percentiles = manager
        .attribute_percentiles(&request.axis_key)
        .ok_or_else(|| "external fit omitted attribute percentiles".to_string())?;
    let ranked = manager.ranked_indices();
    let mut ordered = ranked.clone();
    let ranked_set: HashSet<usize> = ranked.into_iter().collect();
    ordered.extend((0..request.entities.len()).filter(|index| !ranked_set.contains(index)));
    let entities = ordered
        .into_iter()
        .map(|index| {
            let state = manager.entity_state(index);
            JudgementEntityScore {
                id: request.entities[index].id.clone(),
                rank: state.rank,
                feasible: state.feasible,
                p_flip: finite_or_zero(state.p_flip).clamp(0.0, 1.0),
                attribute_score: JudgementAttributeScore {
                    latent_mean: finite_or_zero(scores[index]),
                    latent_std: finite_or_zero(stds[index]),
                    z_score: finite_or_zero(z_scores[index]),
                    percentile: finite_or_zero(percentiles[index]).clamp(0.0, 1.0),
                },
            }
        })
        .collect();

    comparison_trace.sort_by_key(|event| (event.comparison_index, event.timestamp_ms));
    Ok(ExternalFit {
        response: JudgementRunResponse {
            entities,
            global_topk_error,
        },
        engine_spec,
        comparison_trace,
        usage,
    })
}

fn finite_or_zero(value: f64) -> f64 {
    if value.is_finite() {
        value
    } else {
        0.0
    }
}

fn build_rerank_request(request: &NormalizedJudgementRunRequest) -> MultiRerankRequest {
    MultiRerankRequest {
        entities: request
            .entities
            .iter()
            .map(|entity| MultiRerankEntity {
                id: entity.id.clone(),
                text: entity.text.clone(),
            })
            .collect(),
        attributes: vec![MultiRerankAttributeSpec {
            id: request.axis_key.clone(),
            prompt: request.axis_prompt.clone(),
            prompt_template_slug: Some(JUDGEMENT_PROMPT_TEMPLATE_SLUG.to_string()),
            weight: 1.0,
        }],
        topk: MultiRerankTopKSpec {
            // Clamp the engine's identification target to n-1: at k == n the
            // top-k set is certain with zero comparisons, so the stop rule
            // fires immediately and the run returns flat priors (observed
            // live 2026-07-28: 0 comparisons, "tolerated_error_met", empty
            // ClickHouse landing). A request for all n ranked still gets a
            // real boundary to resolve and therefore real comparisons.
            k: request
                .requested_k
                .min(request.entities.len().saturating_sub(1))
                .max(1),
            weight_exponent: 1.3,
            tolerated_error: 0.1,
            band_size: 5,
            effective_resistance_max_active: 64,
            stop_sigma_inflate: 1.25,
            stop_min_consecutive: 2,
            min_explore_degree: 2,
            prune_p_topk_below: None,
        },
        gates: Vec::new(),
        comparison_budget: Some(
            max_judgement_run_comparisons(request)
                * request.nonce_draws.unwrap_or(1).max(1) as usize,
        ),
        nonce_draws: request.nonce_draws,
        latency_budget_ms: None,
        max_cost_nanodollars: None,
        model: Some(request.model.clone()),
        rater_id: Some(request.model.clone()),
        comparison_concurrency: Some(
            request
                .comparison_concurrency
                .unwrap_or(COMPARISON_CONCURRENCY),
        ),
        max_pair_repeats: None,
        randomize_presentation_order: true,
        counterbalance_pairs: true,
    }
}

struct Projection {
    response: JudgementRunResponse,
    usage: JudgementRunUsage,
    engine_spec: EngineSpec,
    stop_reason: RerankStopReason,
    comparisons_used: usize,
}

fn project_response(axis_key: &str, response: MultiRerankResponse) -> Result<Projection, String> {
    let MultiRerankResponse {
        entities,
        meta,
        pareto_front: _,
        attribute_correlations: _,
    } = response;
    if meta.warm_start_observations != 0 {
        return Err("warm-start observations entered a portable run".to_string());
    }
    let engine_spec = meta
        .engine_spec
        .ok_or_else(|| "rerank response omitted its engine spec".to_string())?;
    let mut projected = Vec::with_capacity(entities.len());
    for mut entity in entities {
        let AttributeScoreSummary {
            latent_mean,
            latent_std,
            z_score,
            percentile,
            ..
        } = entity
            .attribute_scores
            .remove(axis_key)
            .ok_or_else(|| format!("entity {} omitted axis {axis_key}", entity.id))?;
        projected.push(JudgementEntityScore {
            id: entity.id,
            rank: entity.rank,
            feasible: entity.feasible,
            p_flip: entity.p_flip,
            attribute_score: JudgementAttributeScore {
                latent_mean,
                latent_std,
                z_score,
                percentile,
            },
        });
    }
    Ok(Projection {
        response: JudgementRunResponse {
            entities: projected,
            global_topk_error: meta.global_topk_error,
        },
        usage: JudgementRunUsage {
            provider_input_tokens: meta.provider_input_tokens,
            provider_output_tokens: meta.provider_output_tokens,
            provider_cost_nanodollars: meta.provider_cost_nanodollars,
            provider_cost_is_estimate: meta.provider_cost_is_estimate,
        },
        engine_spec,
        stop_reason: meta.stop_reason,
        comparisons_used: meta.comparisons_used,
    })
}

#[allow(clippy::too_many_arguments)] // mirrors the record's fields; grouping would only rename them
fn failed_record(
    run_ref: String,
    request: NormalizedJudgementRunRequest,
    instrument: JudgementInstrumentSpec,
    provenance: Option<JudgementRunProvenance>,
    comparison_trace: Vec<ComparisonTrace>,
    provider_calls: Vec<JudgementProviderCall>,
    started_at: DateTime<Utc>,
    finished_at: DateTime<Utc>,
    error: String,
) -> JudgementRunRecord {
    let usage = usage_from_calls(&provider_calls);
    JudgementRunRecord {
        schema: JUDGEMENT_RUN_SCHEMA.to_string(),
        run_ref,
        request,
        instrument,
        provenance,
        comparison_trace,
        provider_calls,
        usage,
        started_at,
        finished_at,
        terminal: JudgementRunTerminal::Failed { error },
    }
}

fn persist_failed(
    store: &JudgementRunStore,
    record: &JudgementRunRecord,
    execution: &str,
) -> Result<(), JudgementRunError> {
    store
        .persist(record)
        .map_err(|persistence| JudgementRunError::FailedRecordPersistence {
            run_ref: record.run_ref.clone(),
            execution: execution.to_string(),
            persistence: persistence.to_string(),
        })
}

fn usage_from_calls(calls: &[JudgementProviderCall]) -> JudgementRunUsage {
    let mut usage = JudgementRunUsage {
        provider_input_tokens: 0,
        provider_output_tokens: 0,
        provider_cost_nanodollars: 0,
        provider_cost_is_estimate: false,
    };
    for call in calls {
        if let JudgementProviderCallOutcome::Succeeded {
            input_tokens,
            output_tokens,
            cost_nanodollars,
            cost_is_estimate,
            ..
        } = call.outcome
        {
            usage.provider_input_tokens = usage.provider_input_tokens.saturating_add(input_tokens);
            usage.provider_output_tokens =
                usage.provider_output_tokens.saturating_add(output_tokens);
            usage.provider_cost_nanodollars = usage
                .provider_cost_nanodollars
                .saturating_add(cost_nanodollars);
            usage.provider_cost_is_estimate |= cost_is_estimate;
        }
    }
    usage
}

fn usage_from_trace(trace: &[ComparisonTrace]) -> JudgementRunUsage {
    trace.iter().fold(
        JudgementRunUsage {
            provider_input_tokens: 0,
            provider_output_tokens: 0,
            provider_cost_nanodollars: 0,
            provider_cost_is_estimate: false,
        },
        |mut usage, event| {
            usage.provider_input_tokens = usage
                .provider_input_tokens
                .saturating_add(event.input_tokens);
            usage.provider_output_tokens = usage
                .provider_output_tokens
                .saturating_add(event.output_tokens);
            usage.provider_cost_nanodollars = usage
                .provider_cost_nanodollars
                .saturating_add(event.provider_cost_nanodollars);
            usage.provider_cost_is_estimate |= event.provider_cost_is_estimate;
            usage
        },
    )
}

fn validate_record(record: &JudgementRunRecord) -> Result<(), JudgementRunError> {
    if record.schema != JUDGEMENT_RUN_SCHEMA {
        return Err(JudgementRunError::InvalidRecord(format!(
            "unsupported schema {}",
            record.schema
        )));
    }
    validate_opaque_ref(&record.run_ref, RUN_REF_PREFIX)
        .map_err(JudgementRunError::InvalidRunRef)?;
    record
        .request
        .validate()
        .map_err(JudgementRunError::InvalidRecord)?;
    if record.finished_at < record.started_at {
        return Err(JudgementRunError::InvalidRecord(
            "finished_at precedes started_at".to_string(),
        ));
    }
    if let Some(provenance) = &record.provenance {
        if provenance.harness != "claude-code"
            || provenance.harness_version.is_empty()
            || provenance.harness_version.chars().count() > 64
            || provenance.harness_version.chars().any(char::is_control)
            || provenance.model.trim().is_empty()
            || provenance.model.trim() != provenance.model
        {
            return Err(JudgementRunError::InvalidRecord(
                "external provenance is invalid".to_string(),
            ));
        }
        if !record.provider_calls.is_empty() {
            return Err(JudgementRunError::InvalidRecord(
                "external run contains provider calls".to_string(),
            ));
        }
        if record.comparison_trace.iter().any(|event| {
            event.model != provenance.model
                || event.provider_cost_nanodollars != 0
                || event.provider_cost_is_estimate
        }) {
            return Err(JudgementRunError::InvalidRecord(
                "external trace does not match its zero-cost provenance".to_string(),
            ));
        }
    }

    let expected_request = build_rerank_request(&record.request);
    if serde_json::to_value(&expected_request)?
        != serde_json::to_value(&record.instrument.rerank_request)?
    {
        return Err(JudgementRunError::InvalidRecord(
            "instrument request does not match normalized request".to_string(),
        ));
    }

    for (expected_sequence, call) in record.provider_calls.iter().enumerate() {
        if call.sequence != expected_sequence {
            return Err(JudgementRunError::InvalidRecord(
                "provider calls are not in invocation order".to_string(),
            ));
        }
        validate_opaque_ref(&call.call_ref, PROVIDER_CALL_REF_PREFIX)
            .map_err(JudgementRunError::InvalidRecord)?;
        if call.finished_at < call.started_at {
            return Err(JudgementRunError::InvalidRecord(format!(
                "provider call {} finishes before it starts",
                call.call_ref
            )));
        }
    }

    match &record.terminal {
        JudgementRunTerminal::Completed { response, .. }
        | JudgementRunTerminal::Cancelled { response } => {
            let engine_spec = record.instrument.engine_spec.as_ref().ok_or_else(|| {
                JudgementRunError::InvalidRecord(
                    "successful terminal record omitted engine_spec".to_string(),
                )
            })?;
            validate_response_ids(&record.request, response)?;
            let engine_spec_id = engine_spec.id().0;
            if record.comparison_trace.iter().any(|event| {
                !event.engine_spec_id.is_empty() && event.engine_spec_id != engine_spec_id
            }) {
                return Err(JudgementRunError::InvalidRecord(
                    "comparison trace references a different engine spec".to_string(),
                ));
            }
            let expected_usage = if record.provenance.is_some() {
                usage_from_trace(&record.comparison_trace)
            } else {
                usage_from_calls(&record.provider_calls)
            };
            if expected_usage != record.usage {
                return Err(JudgementRunError::InvalidRecord(
                    "comparison usage totals do not match run totals".to_string(),
                ));
            }
        }
        JudgementRunTerminal::Failed { .. } => {}
    }
    Ok(())
}

fn validate_response_ids(
    request: &NormalizedJudgementRunRequest,
    response: &JudgementRunResponse,
) -> Result<(), JudgementRunError> {
    if response.entities.len() != request.entities.len() {
        return Err(JudgementRunError::InvalidRecord(
            "response entity count does not match request".to_string(),
        ));
    }
    let expected: HashSet<_> = request
        .entities
        .iter()
        .map(|entity| entity.id.as_str())
        .collect();
    let actual: HashSet<_> = response
        .entities
        .iter()
        .map(|entity| entity.id.as_str())
        .collect();
    if actual.len() != response.entities.len() || actual != expected {
        return Err(JudgementRunError::InvalidRecord(
            "response entity ids do not match request".to_string(),
        ));
    }
    Ok(())
}

fn new_opaque_ref(prefix: &str) -> String {
    format!("{prefix}{}", Uuid::new_v4().simple())
}

fn validate_opaque_ref(value: &str, prefix: &str) -> Result<(), String> {
    let suffix = value
        .strip_prefix(prefix)
        .ok_or_else(|| format!("expected {prefix} prefix"))?;
    if suffix.len() != 32 {
        return Err("opaque reference must contain a simple UUID".to_string());
    }
    let uuid =
        Uuid::parse_str(suffix).map_err(|_| "opaque reference UUID is invalid".to_string())?;
    if uuid.get_version_num() != 4 {
        return Err("opaque reference UUID must be version 4".to_string());
    }
    Ok(())
}

struct CapturingTraceSink<'a> {
    upstream: Option<&'a dyn TraceSink>,
    events: Mutex<Vec<ComparisonTrace>>,
}

impl<'a> CapturingTraceSink<'a> {
    fn new(upstream: Option<&'a dyn TraceSink>) -> Self {
        Self {
            upstream,
            events: Mutex::new(Vec::new()),
        }
    }

    fn events(&self) -> Vec<ComparisonTrace> {
        lock_unpoisoned(&self.events).clone()
    }
}

impl TraceSink for CapturingTraceSink<'_> {
    fn record(&self, event: ComparisonTrace) -> Result<(), TraceError> {
        lock_unpoisoned(&self.events).push(event.clone());
        if let Some(upstream) = self.upstream {
            upstream.record(event)?;
        }
        Ok(())
    }
}

struct RecordingGateway {
    inner: Arc<dyn ChatGateway>,
    calls: Arc<Mutex<Vec<JudgementProviderCall>>>,
    next_sequence: AtomicUsize,
}

#[async_trait::async_trait]
impl ChatGateway for RecordingGateway {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, ProviderError> {
        let sequence = self.next_sequence.fetch_add(1, Ordering::Relaxed);
        let call_ref = new_opaque_ref(PROVIDER_CALL_REF_PREFIX);
        let provider = request.model.provider().to_string();
        let model = request.model.model_id().to_string();
        let gateway_request_digest = gateway_request_digest(&request);
        let started_at = Utc::now();
        let result = self.inner.chat(request).await;
        let finished_at = Utc::now();
        let outcome = match &result {
            Ok(response) => JudgementProviderCallOutcome::Succeeded {
                provider_call_id: response.provider_call_id.clone(),
                provider_request_id: response.provider_request_id.clone(),
                input_tokens: response.input_tokens,
                output_tokens: response.output_tokens,
                cost_nanodollars: response.cost_nanodollars,
                cost_is_estimate: response.cost_is_estimate,
            },
            Err(error) => JudgementProviderCallOutcome::Failed {
                provider_request_id: error
                    .context()
                    .and_then(|context| context.request_id.clone()),
                error_code: error.code().to_string(),
                error: error.to_string(),
            },
        };
        lock_unpoisoned(&self.calls).push(JudgementProviderCall {
            call_ref,
            sequence,
            provider,
            model,
            gateway_request_digest,
            started_at,
            finished_at,
            outcome,
        });
        result
    }
}

fn gateway_request_digest(request: &ChatRequest) -> String {
    fn put(hasher: &mut blake3::Hasher, bytes: &[u8]) {
        hasher.update(&(bytes.len() as u64).to_be_bytes());
        hasher.update(bytes);
    }
    fn put_optional_u32(hasher: &mut blake3::Hasher, value: Option<u32>) {
        match value {
            Some(value) => {
                hasher.update(&[1]);
                hasher.update(&value.to_be_bytes());
            }
            None => {
                hasher.update(&[0]);
            }
        }
    }
    fn put_optional_bool(hasher: &mut blake3::Hasher, value: Option<bool>) {
        match value {
            Some(value) => hasher.update(&[1, u8::from(value)]),
            None => hasher.update(&[0]),
        };
    }

    let mut hasher = blake3::Hasher::new();
    hasher.update(REQUEST_DIGEST_DOMAIN);
    put(&mut hasher, request.model.provider().as_bytes());
    put(&mut hasher, request.model.model_id().as_bytes());
    hasher.update(&(request.messages.len() as u64).to_be_bytes());
    for message in &request.messages {
        let role = match message.role {
            Role::System => b"system".as_slice(),
            Role::User => b"user".as_slice(),
            Role::Assistant => b"assistant".as_slice(),
        };
        put(&mut hasher, role);
        put(&mut hasher, message.content.as_bytes());
    }
    hasher.update(&request.temperature.to_bits().to_be_bytes());
    put_optional_u32(&mut hasher, request.max_tokens);
    hasher.update(&[u8::from(request.json_mode), u8::from(request.logprobs)]);
    put_optional_u32(&mut hasher, request.top_logprobs);
    match &request.reasoning {
        Some(reasoning) => {
            hasher.update(&[1]);
            put_optional_bool(&mut hasher, reasoning.enabled);
            let effort = reasoning.effort.as_ref().map(|effort| match effort {
                ReasoningEffort::Xhigh => b"xhigh".as_slice(),
                ReasoningEffort::High => b"high".as_slice(),
                ReasoningEffort::Medium => b"medium".as_slice(),
                ReasoningEffort::Low => b"low".as_slice(),
                ReasoningEffort::Minimal => b"minimal".as_slice(),
                ReasoningEffort::None => b"none".as_slice(),
            });
            match effort {
                Some(effort) => put(&mut hasher, effort),
                None => put(&mut hasher, b""),
            }
            put_optional_u32(&mut hasher, reasoning.max_tokens);
            put_optional_bool(&mut hasher, reasoning.exclude);
        }
        None => {
            hasher.update(&[0]);
        }
    }
    match request.prompt_cache_key.as_deref() {
        Some(key) => put(&mut hasher, key.as_bytes()),
        None => put(&mut hasher, b""),
    }
    hasher.finalize().to_hex().to_string()
}

fn lock_unpoisoned<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
