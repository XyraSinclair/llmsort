use std::fs::{self, File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Write};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use axum::extract::rejection::JsonRejection;
use axum::extract::{Path as AxumPath, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use llmsort::gateway::openrouter::OpenRouterAdapter;
use llmsort::gateway::{
    Attribution, ChatGateway, ChatRequest, ChatResponse, ErrorContext, GatewayConfig, ModelPricing,
    NoopUsageSink, ProviderError, ProviderGateway, OPENROUTER_PRICING_AS_OF,
};
use llmsort::rerank::comparison::{estimate_pairwise_input_tokens, pairwise_max_output_tokens};
use llmsort::rerank::{RerankExecution, RerankRunOptions, RerankStopReason};
use llmsort_experiments::judgement_run::{
    build_external_schedule, execute_external_judgement_run_with_ref,
    execute_judgement_run_with_ref, max_judgement_run_comparisons, validate_external_judgement_run,
    ExternalJudgementRun, ExternalJudgementSchedule, JudgementCandidate, JudgementPrivacy,
    JudgementRunRecord, JudgementRunRequest, JudgementRunStore, JudgementRunTerminal,
    NormalizedJudgementRunRequest,
};
use llmsort_experiments::landing::{land_completed_run, ClickHouseLanding};
use llmsort_experiments::openpriors::{AccountId, Instrument, InstrumentRegistration};
use llmsort_experiments::openpriors_registry::{Registry, RegistryError};
use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;
use uuid::Uuid;

const DEFAULT_ADDR: &str = "127.0.0.1:8093";
const DEFAULT_MAX_CONCURRENT_RUNS: usize = 4;
const DEFAULT_MAX_QUEUED_RUNS: usize = 32;
const DEFAULT_RUN_DIR: &str = ".cardinald/runs";
const DEFAULT_INSTRUMENT_DIR: &str = ".cardinald/instruments";
const MAX_ENTITIES: usize = 200;
const MAX_ENTITY_TEXT_BYTES: usize = 8192;
const MAX_AXIS_PROMPT_BYTES: usize = 4096;
const MAX_SUBMITTED_BY_BYTES: usize = 128;
const ESTIMATE_SAFETY_NUMERATOR: i64 = 5;
const ESTIMATE_SAFETY_DENOMINATOR: i64 = 4;

#[derive(Clone)]
struct AppState {
    store: JudgementRunStore,
    semaphore: Arc<Semaphore>,
    admission: Arc<Semaphore>,
    clickhouse: Option<Arc<ClickHouseLanding>>,
    registry: Arc<Registry>,
}

#[derive(Debug, Deserialize)]
struct JudgementRequestFields {
    entities: Vec<JudgementCandidate>,
    axis_key: String,
    axis_prompt: String,
    requested_k: usize,
    model: String,
    #[serde(default)]
    comparison_concurrency: Option<usize>,
    #[serde(default)]
    min_request_interval_ms: Option<u64>,
    #[serde(default)]
    provider_base_url: Option<String>,
    #[serde(default)]
    nonce_draws: Option<u32>,
}

impl JudgementRequestFields {
    fn into_run_request(self, privacy: JudgementPrivacy) -> JudgementRunRequest {
        JudgementRunRequest {
            entities: self.entities,
            axis_key: self.axis_key,
            axis_prompt: self.axis_prompt,
            requested_k: self.requested_k,
            model: self.model,
            privacy,
            comparison_concurrency: self.comparison_concurrency,
            min_request_interval_ms: self.min_request_interval_ms,
            provider_base_url: self.provider_base_url,
            nonce_draws: self.nonce_draws,
        }
    }
}

#[derive(Debug, Deserialize)]
struct CreateRunRequest {
    #[serde(flatten)]
    judgement: JudgementRequestFields,
    privacy: JudgementPrivacy,
    #[serde(default)]
    owner_scope: Option<String>,
    /// Public contributor attribution (openpriors invariant 4: trust
    /// attaches to accounts). The nucleus derives it from authentication —
    /// cardinald stores and lands it verbatim; empty means unattributed.
    #[serde(default)]
    submitted_by: Option<String>,
    #[serde(default)]
    lens: Option<String>,
    #[serde(default)]
    mode: Option<CreateRunMode>,
    #[serde(default)]
    external: Option<ExternalJudgementRun>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
enum CreateRunMode {
    External,
}

#[derive(Debug, Deserialize)]
struct ScheduleRequest {
    #[serde(flatten)]
    judgement: JudgementRequestFields,
    privacy: JudgementPrivacy,
    #[serde(default)]
    owner_scope: Option<String>,
    #[serde(default)]
    lens: Option<String>,
}

enum QueuedRun {
    Adaptive {
        request: JudgementRunRequest,
        gateway: Arc<dyn ChatGateway>,
    },
    External {
        request: JudgementRunRequest,
        external: ExternalJudgementRun,
    },
}

#[derive(Debug, Serialize)]
struct AcceptedRun {
    run_ref: String,
    status: &'static str,
}

#[derive(Debug, Serialize)]
struct EstimateResponse {
    max_spend_nanodollars: i64,
    planned_comparisons: usize,
    price: EstimatePrice,
    bound_method: String,
}

#[derive(Debug, Serialize)]
struct EstimatePrice {
    model: String,
    prompt_nanodollars_per_token: i64,
    completion_nanodollars_per_token: i64,
    as_of: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DaemonRunMetadata {
    run_ref: String,
    request: NormalizedJudgementRunRequest,
    owner_scope: String,
    // Pre-attribution metadata files deserialize to "" (unattributed).
    #[serde(default)]
    submitted_by: String,
    lens: String,
    created_at: DateTime<Utc>,
    status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct GetRunResponse {
    run_ref: String,
    status: String,
    privacy: JudgementPrivacy,
    owner_scope: String,
    /// Contributor account attribution; empty = unattributed.
    submitted_by: String,
    lens: String,
    axis_key: String,
    axis_prompt: String,
    model: String,
    entity_ids: Vec<String>,
    /// SHA-256 hex of each entity's text, aligned with `entity_ids`, so
    /// downstream services can verify the judged text and not merely an id
    /// alias (independent review 2026-08-10, finding 4).
    entity_text_hashes: Vec<String>,
    created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response: Option<CompletedResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct CompletedResponse {
    scores: Vec<ApiScore>,
    stop_reason: &'static str,
    comparisons_used: usize,
    provider_input_tokens: u32,
    provider_output_tokens: u32,
    cost_nanodollars: i64,
    cost_is_estimate: bool,
}

#[derive(Debug, Serialize)]
struct ApiScore {
    entity_id: String,
    rank: usize,
    latent_mean: f64,
    latent_std: f64,
    z_score: f64,
    percentile: f64,
}

struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            message: message.into(),
        }
    }

    fn conflict(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            message: message.into(),
        }
    }

    fn not_found() -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: "run not found".to_string(),
        }
    }

    fn too_many_requests(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::TOO_MANY_REQUESTS,
            message: message.into(),
        }
    }

    fn internal() -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "internal server error".to_string(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(serde_json::json!({ "error": self.message })),
        )
            .into_response()
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address: SocketAddr = env_or("CARDINALD_ADDR", DEFAULT_ADDR)
        .parse()
        .map_err(|error| format!("invalid CARDINALD_ADDR: {error}"))?;
    let max_concurrent =
        parse_positive_usize("CARDINALD_MAX_CONCURRENT_RUNS", DEFAULT_MAX_CONCURRENT_RUNS)?;
    let max_queued = parse_positive_usize("CARDINALD_MAX_QUEUED_RUNS", DEFAULT_MAX_QUEUED_RUNS)?;
    let run_dir = PathBuf::from(env_or("CARDINALD_RUN_DIR", DEFAULT_RUN_DIR));
    fs::create_dir_all(&run_dir)?;
    let store = JudgementRunStore::new(run_dir);

    let clickhouse = std::env::var("CARDINALD_CLICKHOUSE_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(|value| ClickHouseLanding::from_url(&value).map(Arc::new))
        .transpose()
        .map_err(|error| format!("invalid CARDINALD_CLICKHOUSE_URL: {error}"))?;
    if let Some(client) = clickhouse.as_deref() {
        client.replay_pending(&store).await;
    }
    recover_interrupted_runs(&store, clickhouse.as_deref()).await;

    let registry = Arc::new(
        Registry::open(env_or("CARDINALD_INSTRUMENT_DIR", DEFAULT_INSTRUMENT_DIR))
            .map_err(|error| format!("instrument registry failed to open: {error}"))?,
    );
    let seeded = registry
        .seed_builtins()
        .map_err(|error| format!("builtin instrument seeding failed: {error}"))?;
    eprintln!(
        "cardinald: instrument registry ready ({} builtins: {})",
        seeded.len(),
        seeded
            .iter()
            .map(|r| format!("{}={}", r.name, &r.instrument.0[..12]))
            .collect::<Vec<_>>()
            .join(", ")
    );

    let state = AppState {
        store,
        semaphore: Arc::new(Semaphore::new(max_concurrent)),
        admission: Arc::new(Semaphore::new(max_queued)),
        clickhouse,
        registry,
    };
    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/v1/estimate", post(estimate_run))
        .route("/v1/schedule", post(schedule_run))
        .route("/v1/runs", post(create_run))
        .route("/v1/runs/{run_ref}", get(get_run))
        .route(
            "/v1/instruments",
            get(list_instruments).post(register_instrument),
        )
        .route("/v1/instruments/{hash}", get(get_instrument))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(address).await?;
    eprintln!("cardinald: listening on {address}");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn healthz() -> &'static str {
    "ok"
}

#[derive(Debug, Deserialize)]
struct RegisterInstrumentRequest {
    name: String,
    owner: String,
    instrument: Instrument,
}

/// `POST /v1/instruments` — validate, content-address, persist, and alias an
/// instrument. Idempotent on identical content; `(owner, name)` never
/// silently re-points (409 on conflict).
async fn register_instrument(
    State(state): State<AppState>,
    payload: Result<Json<RegisterInstrumentRequest>, JsonRejection>,
) -> Result<Json<InstrumentRegistration>, ApiError> {
    let Json(request) =
        payload.map_err(|rejection| ApiError::bad_request(rejection.body_text()))?;
    state
        .registry
        .register(request.instrument, &request.name, &AccountId(request.owner))
        .map(Json)
        .map_err(|error| match error {
            RegistryError::NameConflict { .. } => ApiError::conflict(error.to_string()),
            RegistryError::Invalid(_) | RegistryError::BadAlias => {
                ApiError::bad_request(error.to_string())
            }
            RegistryError::Io(_) | RegistryError::Corrupt { .. } => {
                eprintln!("cardinald: instrument registry error: {error}");
                ApiError::internal()
            }
        })
}

/// `GET /v1/instruments` — every registration with its currency.
async fn list_instruments(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({ "instruments": state.registry.list() }))
}

/// `GET /v1/instruments/{hash}` — the full instrument plus its aliases.
async fn get_instrument(
    State(state): State<AppState>,
    AxumPath(hash): AxumPath<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let (instrument, registrations) = state.registry.get(&hash).ok_or(ApiError {
        status: StatusCode::NOT_FOUND,
        message: "instrument not found".to_string(),
    })?;
    Ok(Json(serde_json::json!({
        "instrument_hash": hash,
        "currency": instrument.currency(),
        "instrument": instrument,
        "registrations": registrations,
    })))
}

async fn schedule_run(
    State(state): State<AppState>,
    payload: Result<Json<ScheduleRequest>, JsonRejection>,
) -> Result<Json<ExternalJudgementSchedule>, ApiError> {
    // Schedules render 8xN prompts per call; ride the same admission gate as
    // runs so a local caller cannot spin unbounded free prompt rendering
    // (independent review 2026-08-10, finding 3).
    let _admission_permit = Arc::clone(&state.admission)
        .try_acquire_owned()
        .map_err(|_| ApiError::too_many_requests("the run queue is full; retry shortly"))?;
    let Json(payload) = payload.map_err(|_| ApiError::bad_request("invalid JSON request body"))?;
    validate_caps(&payload.judgement)?;
    validate_run_context(
        payload.privacy,
        payload.owner_scope.as_deref(),
        payload.lens.as_deref(),
    )?;
    let normalized = payload
        .judgement
        .into_run_request(payload.privacy)
        .normalize()
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let seed = rand::random::<u64>();
    Ok(Json(build_external_schedule(&normalized, seed)))
}

async fn estimate_run(
    payload: Result<Json<JudgementRequestFields>, JsonRejection>,
) -> Result<Json<EstimateResponse>, ApiError> {
    let Json(payload) = payload.map_err(|_| ApiError::bad_request("invalid JSON request body"))?;
    validate_caps(&payload)?;

    let normalized = payload
        .into_run_request(JudgementPrivacy::Public)
        .normalize()
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let pricing = llmsort::gateway::get_pricing(&normalized.model)
        .filter(|pricing| pricing.provider == "openrouter")
        .ok_or_else(|| ApiError::conflict("price_unknown"))?;
    let planned_comparisons = max_judgement_run_comparisons(&normalized);

    let mut entities_by_length: Vec<&JudgementCandidate> = normalized.entities.iter().collect();
    entities_by_length.sort_unstable_by_key(|entity| std::cmp::Reverse(entity.text.len()));
    let input_tokens = estimate_pairwise_input_tokens(
        &normalized.axis_key,
        &normalized.axis_prompt,
        Some(llmsort::rerank::default_template_slug(Some(
            &normalized.model,
        ))),
        &entities_by_length[0].text,
        &entities_by_length[1].text,
    );
    let output_tokens = pairwise_max_output_tokens(&normalized.model);
    let max_spend_nanodollars =
        checked_estimate_bound(planned_comparisons, input_tokens, output_tokens, pricing)?;

    Ok(Json(EstimateResponse {
        max_spend_nanodollars,
        planned_comparisons,
        price: EstimatePrice {
            model: normalized.model,
            prompt_nanodollars_per_token: pricing.input_nanos_per_token,
            completion_nanodollars_per_token: pricing.output_nanos_per_token,
            as_of: OPENROUTER_PRICING_AS_OF,
        },
        bound_method: format!(
            "ceil(1.25 × {planned_comparisons} comparisons × \
             ({input_tokens} canonical_v2 input tokens from the two longest texts × prompt price + \
             {output_tokens} max-output tokens × completion price))"
        ),
    }))
}

fn checked_estimate_bound(
    planned_comparisons: usize,
    input_tokens: u32,
    output_tokens: u32,
    pricing: ModelPricing,
) -> Result<i64, ApiError> {
    if pricing.input_nanos_per_token < 0 || pricing.output_nanos_per_token < 0 {
        return Err(ApiError::internal());
    }
    let comparisons = i64::try_from(planned_comparisons).map_err(|_| ApiError::internal())?;
    let input_cost = i64::from(input_tokens)
        .checked_mul(pricing.input_nanos_per_token)
        .ok_or_else(ApiError::internal)?;
    let output_cost = i64::from(output_tokens)
        .checked_mul(pricing.output_nanos_per_token)
        .ok_or_else(ApiError::internal)?;
    let per_comparison = input_cost
        .checked_add(output_cost)
        .ok_or_else(ApiError::internal)?;
    let subtotal = comparisons
        .checked_mul(per_comparison)
        .ok_or_else(ApiError::internal)?;
    let rounding = ESTIMATE_SAFETY_DENOMINATOR
        .checked_sub(1)
        .ok_or_else(ApiError::internal)?;
    let scaled = subtotal
        .checked_mul(ESTIMATE_SAFETY_NUMERATOR)
        .and_then(|value| value.checked_add(rounding))
        .ok_or_else(ApiError::internal)?;
    scaled
        .checked_div(ESTIMATE_SAFETY_DENOMINATOR)
        .ok_or_else(ApiError::internal)
}

async fn recover_interrupted_runs(
    store: &JudgementRunStore,
    clickhouse: Option<&ClickHouseLanding>,
) {
    let entries = match fs::read_dir(store.root()) {
        Ok(entries) => entries,
        Err(error) => {
            eprintln!("cardinald: could not scan run metadata during startup: {error}");
            return;
        }
    };
    let mut run_refs = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                eprintln!("cardinald: could not read a run metadata entry: {error}");
                continue;
            }
        };
        let Some(file_name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        let Some(run_ref) = file_name.strip_suffix(".cardinald.json") else {
            continue;
        };
        if valid_run_ref(run_ref) {
            run_refs.push(run_ref.to_string());
        }
    }
    run_refs.sort();

    for run_ref in run_refs {
        let metadata = match load_metadata(store.root(), &run_ref) {
            Ok(metadata) => metadata,
            Err(error) => {
                eprintln!("cardinald: could not read run metadata for {run_ref}: {error}");
                continue;
            }
        };
        if metadata.status != "running" {
            continue;
        }

        let record_path = store.root().join(format!("{run_ref}.json"));
        if !record_path.is_file() {
            mark_metadata_failed(
                store,
                metadata,
                "daemon restarted before the run finished; resubmit",
            );
            continue;
        }

        let record = match store.load(&run_ref) {
            Ok(record) => record,
            Err(error) => {
                eprintln!("cardinald: could not load terminal run {run_ref}: {error}");
                continue;
            }
        };
        let landing_preserved = if matches!(record.terminal, JudgementRunTerminal::Completed { .. })
        {
            land_completed_run(
                clickhouse,
                store,
                &record,
                &metadata.lens,
                &metadata.owner_scope,
                &metadata.submitted_by,
            )
            .await
        } else {
            true
        };
        if landing_preserved {
            update_metadata_from_record(store, metadata, &record);
        }
    }
}

async fn create_run(
    State(state): State<AppState>,
    headers: HeaderMap,
    payload: Result<Json<CreateRunRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<AcceptedRun>), ApiError> {
    let Json(payload) = payload.map_err(|_| ApiError::bad_request("invalid JSON request body"))?;
    validate_caps(&payload.judgement)?;

    let owner_scope = payload.owner_scope.unwrap_or_default();
    let submitted_by = normalize_submitted_by(payload.submitted_by)?;
    let lens = payload.lens.unwrap_or_else(|| "api".to_string());
    validate_run_context(payload.privacy, Some(&owner_scope), Some(&lens))?;

    let request = payload.judgement.into_run_request(payload.privacy);
    let normalized = request
        .clone()
        .normalize()
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    let admission_permit = Arc::clone(&state.admission)
        .try_acquire_owned()
        .map_err(|_| ApiError::too_many_requests("run queue is at capacity"))?;
    let queued_run = match (payload.mode, payload.external) {
        (None, None) => {
            let provider_key = provider_key(&headers)?;
            let paced = normalized.min_request_interval_ms.unwrap_or(0) > 0;
            // A paced run exists because the provider rate window is tight
            // (free-tier judges). Gateway retries fire BELOW the pacer, so
            // they triple the real request rate and — with retry_after
            // clamped under the provider's demanded wait — turn one seed
            // 429 into a self-starving storm that consumes the shared
            // window with doomed retries (observed live 2026-09-04: every
            // attempt of every paced run failing rate_limited_remote while
            // an unpaced burst succeeded). Paced runs therefore get zero
            // gateway retries: every real call is paced, failures consume
            // engine budget honestly, and the caller's cool-down handles a
            // saturated model.
            let gateway_config = if paced {
                GatewayConfig {
                    max_retries: 0,
                    ..GatewayConfig::default()
                }
            } else {
                GatewayConfig::default()
            };
            let mut gateway = build_gateway(
                provider_key,
                gateway_config,
                normalized.provider_base_url.as_deref(),
            )
            .map_err(|_| ApiError::unauthorized("provider key is invalid"))?;
            if let Some(interval_ms) = normalized.min_request_interval_ms {
                if interval_ms > 0 {
                    gateway = Arc::new(PacedGateway {
                        inner: gateway,
                        min_interval: Duration::from_millis(interval_ms),
                        next_start: tokio::sync::Mutex::new(tokio::time::Instant::now()),
                    });
                }
            }
            QueuedRun::Adaptive { request, gateway }
        }
        (Some(CreateRunMode::External), Some(external)) => {
            validate_external_judgement_run(&normalized, &external)
                .map_err(ApiError::bad_request)?;
            QueuedRun::External { request, external }
        }
        (Some(CreateRunMode::External), None) => {
            return Err(ApiError::bad_request(
                "external is required when mode is external",
            ));
        }
        (None, Some(_)) => {
            return Err(ApiError::bad_request(
                "mode must be external when external is supplied",
            ));
        }
    };
    let run_ref = state.store.allocate_run_ref();
    let metadata = DaemonRunMetadata {
        run_ref: run_ref.clone(),
        request: normalized,
        owner_scope,
        submitted_by,
        lens,
        created_at: Utc::now(),
        status: "running".to_string(),
        error: None,
    };
    persist_new_metadata(state.store.root(), &metadata).map_err(|error| {
        eprintln!("cardinald: could not allocate run metadata: {error}");
        ApiError::internal()
    })?;

    let task_state = state.clone();
    let task_metadata = metadata.clone();
    let task_run_ref = run_ref.clone();
    tokio::spawn(async move {
        let permit = match Arc::clone(&task_state.semaphore).acquire_owned().await {
            Ok(permit) => permit,
            Err(_) => {
                mark_metadata_failed(
                    &task_state.store,
                    task_metadata,
                    "run queue shut down before execution",
                );
                return;
            }
        };
        let failure_store = task_state.store.clone();
        let failure_metadata = task_metadata.clone();
        let failure_run_ref = task_run_ref.clone();
        let joined = tokio::task::spawn_blocking(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    eprintln!("cardinald: could not start run executor: {error}");
                    mark_metadata_failed(
                        &task_state.store,
                        task_metadata,
                        "judgement run executor could not start",
                    );
                    return;
                }
            };
            runtime.block_on(execute_queued_run(
                task_state,
                task_metadata,
                task_run_ref,
                queued_run,
                permit,
                admission_permit,
            ));
        })
        .await;
        if joined.is_err() {
            eprintln!("cardinald: run {failure_run_ref} executor stopped unexpectedly");
            mark_metadata_failed(
                &failure_store,
                failure_metadata,
                "judgement run executor stopped unexpectedly",
            );
        }
    });

    Ok((
        StatusCode::ACCEPTED,
        Json(AcceptedRun {
            run_ref,
            status: "running",
        }),
    ))
}

async fn execute_queued_run(
    state: AppState,
    metadata: DaemonRunMetadata,
    run_ref: String,
    queued_run: QueuedRun,
    permit: tokio::sync::OwnedSemaphorePermit,
    admission_permit: tokio::sync::OwnedSemaphorePermit,
) {
    let result = match queued_run {
        QueuedRun::Adaptive { request, gateway } => {
            let seed = rand::random::<u64>();
            let execution = RerankExecution::new(gateway, Attribution::new("cardinald::run"))
                .run_options(RerankRunOptions {
                    rng_seed: Some(seed),
                    cache_only: false,
                });
            execute_judgement_run_with_ref(request, run_ref.clone(), execution, &state.store).await
        }
        QueuedRun::External { request, external } => {
            execute_external_judgement_run_with_ref(
                request,
                run_ref.clone(),
                external,
                &state.store,
            )
            .await
        }
    };
    drop(permit);
    drop(admission_permit);

    match result {
        Ok(record) => {
            let landing_preserved =
                if matches!(record.terminal, JudgementRunTerminal::Completed { .. }) {
                    land_completed_run(
                        state.clickhouse.as_deref(),
                        &state.store,
                        &record,
                        &metadata.lens,
                        &metadata.owner_scope,
                        &metadata.submitted_by,
                    )
                    .await
                } else {
                    true
                };
            if landing_preserved {
                update_metadata_from_record(&state.store, metadata, &record);
            }
        }
        Err(_) => match state.store.load(&run_ref) {
            Ok(record) => {
                update_metadata_from_record(&state.store, metadata, &record);
            }
            Err(_) => {
                eprintln!("cardinald: run {run_ref} failed without a terminal record");
                mark_metadata_failed(
                    &state.store,
                    metadata,
                    "judgement run failed before its terminal record was persisted",
                );
            }
        },
    }
}

async fn get_run(
    State(state): State<AppState>,
    AxumPath(run_ref): AxumPath<String>,
) -> Result<Json<GetRunResponse>, ApiError> {
    if !valid_run_ref(&run_ref) {
        return Err(ApiError::not_found());
    }
    let metadata = load_metadata(state.store.root(), &run_ref).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            ApiError::not_found()
        } else {
            eprintln!("cardinald: could not read run metadata for {run_ref}: {error}");
            ApiError::internal()
        }
    })?;

    let record_path = state.store.root().join(format!("{run_ref}.json"));
    if record_path.is_file() {
        let record = state.store.load(&run_ref).map_err(|error| {
            eprintln!("cardinald: could not load terminal run {run_ref}: {error}");
            ApiError::internal()
        })?;
        Ok(Json(project_terminal(metadata, record)))
    } else {
        let entity_ids = metadata
            .request
            .entities
            .iter()
            .map(|entity| entity.id.clone())
            .collect();
        let entity_text_hashes = entity_text_hashes(&metadata.request.entities);
        Ok(Json(GetRunResponse {
            run_ref: metadata.run_ref,
            status: metadata.status,
            privacy: metadata.request.privacy,
            owner_scope: metadata.owner_scope,
            submitted_by: metadata.submitted_by,
            lens: metadata.lens,
            axis_key: metadata.request.axis_key,
            axis_prompt: metadata.request.axis_prompt,
            model: metadata.request.model,
            entity_ids,
            entity_text_hashes,
            created_at: metadata.created_at,
            response: None,
            error: metadata.error,
        }))
    }
}

fn project_terminal(metadata: DaemonRunMetadata, record: JudgementRunRecord) -> GetRunResponse {
    let (status, response, error) = match &record.terminal {
        JudgementRunTerminal::Completed {
            stop_reason,
            response,
        } => (
            "completed".to_string(),
            Some(project_completed(&record, *stop_reason, response)),
            None,
        ),
        JudgementRunTerminal::Cancelled { response } => (
            "cancelled".to_string(),
            Some(project_completed(
                &record,
                RerankStopReason::Cancelled,
                response,
            )),
            None,
        ),
        JudgementRunTerminal::Failed { error } => ("failed".to_string(), None, Some(error.clone())),
    };
    let entity_ids = record
        .request
        .entities
        .iter()
        .map(|entity| entity.id.clone())
        .collect();
    let entity_text_hashes = entity_text_hashes(&record.request.entities);
    GetRunResponse {
        run_ref: record.run_ref,
        status,
        privacy: record.request.privacy,
        owner_scope: metadata.owner_scope,
        submitted_by: metadata.submitted_by,
        lens: metadata.lens,
        axis_key: record.request.axis_key,
        axis_prompt: record.request.axis_prompt,
        model: record.request.model,
        entity_ids,
        entity_text_hashes,
        created_at: metadata.created_at,
        response,
        error,
    }
}

fn entity_text_hashes(entities: &[JudgementCandidate]) -> Vec<String> {
    use sha2::{Digest, Sha256};
    entities
        .iter()
        .map(|entity| format!("{:x}", Sha256::digest(entity.text.as_bytes())))
        .collect()
}

fn project_completed(
    record: &JudgementRunRecord,
    stop_reason: RerankStopReason,
    response: &llmsort_experiments::judgement_run::JudgementRunResponse,
) -> CompletedResponse {
    let scores = response
        .entities
        .iter()
        .enumerate()
        .map(|(position, entity)| ApiScore {
            entity_id: entity.id.clone(),
            rank: entity.rank.unwrap_or(position + 1),
            latent_mean: entity.attribute_score.latent_mean,
            latent_std: entity.attribute_score.latent_std,
            z_score: entity.attribute_score.z_score,
            percentile: entity.attribute_score.percentile,
        })
        .collect();
    CompletedResponse {
        scores,
        stop_reason: stop_reason_name(stop_reason),
        comparisons_used: record
            .comparison_trace
            .iter()
            .filter(|trace| trace.solver_observation.is_some())
            .count(),
        provider_input_tokens: record.usage.provider_input_tokens,
        provider_output_tokens: record.usage.provider_output_tokens,
        cost_nanodollars: record.usage.provider_cost_nanodollars,
        cost_is_estimate: record.usage.provider_cost_is_estimate,
    }
}

/// Attribution is a name the nucleus vouched for, never free text: trimmed,
/// bounded, printable. Empty (the default) means unattributed.
fn normalize_submitted_by(value: Option<String>) -> Result<String, ApiError> {
    let value = value.unwrap_or_default().trim().to_string();
    if value.len() > MAX_SUBMITTED_BY_BYTES {
        return Err(ApiError::bad_request(format!(
            "submitted_by must not exceed {MAX_SUBMITTED_BY_BYTES} bytes"
        )));
    }
    if value.chars().any(char::is_control) {
        return Err(ApiError::bad_request(
            "submitted_by must not contain control characters",
        ));
    }
    Ok(value)
}

fn validate_caps(payload: &JudgementRequestFields) -> Result<(), ApiError> {
    if payload.entities.len() > MAX_ENTITIES {
        return Err(ApiError::bad_request(format!(
            "entities must contain at most {MAX_ENTITIES} items"
        )));
    }
    if let Some(entity) = payload
        .entities
        .iter()
        .find(|entity| entity.text.len() > MAX_ENTITY_TEXT_BYTES)
    {
        return Err(ApiError::bad_request(format!(
            "entity text exceeds {MAX_ENTITY_TEXT_BYTES} bytes: {}",
            entity.id
        )));
    }
    if payload.axis_prompt.len() > MAX_AXIS_PROMPT_BYTES {
        return Err(ApiError::bad_request(format!(
            "axis_prompt exceeds {MAX_AXIS_PROMPT_BYTES} bytes"
        )));
    }
    Ok(())
}

fn validate_run_context(
    privacy: JudgementPrivacy,
    owner_scope: Option<&str>,
    lens: Option<&str>,
) -> Result<(), ApiError> {
    match privacy {
        JudgementPrivacy::Public if owner_scope.is_some_and(|value| !value.is_empty()) => {
            return Err(ApiError::bad_request(
                "owner_scope must be empty for public runs",
            ));
        }
        JudgementPrivacy::Private if owner_scope.is_none_or(|value| value.trim().is_empty()) => {
            return Err(ApiError::bad_request(
                "owner_scope must be nonblank for private runs",
            ));
        }
        JudgementPrivacy::Public | JudgementPrivacy::Private => {}
    }
    if lens.is_some_and(|value| value.trim().is_empty()) {
        return Err(ApiError::bad_request("lens must not be blank"));
    }
    Ok(())
}

fn provider_key(headers: &HeaderMap) -> Result<String, ApiError> {
    let header_key = match headers.get("x-provider-key") {
        Some(value) => Some(
            value
                .to_str()
                .map_err(|_| ApiError::unauthorized("x-provider-key is invalid"))?
                .to_string(),
        ),
        None => None,
    };
    header_key
        .filter(|value| !value.is_empty())
        .or_else(|| std::env::var("CARDINALD_OPENROUTER_KEY").ok())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::unauthorized("OpenRouter provider key is required"))
}

fn build_gateway(
    provider_key: String,
    gateway_config: GatewayConfig,
    base_url_override: Option<&str>,
) -> Result<Arc<dyn ChatGateway>, ProviderError> {
    let base_url = base_url_override
        .map(str::to_string)
        .unwrap_or_else(|| env_or("OPENROUTER_BASE_URL", "https://openrouter.ai/api/v1"));
    let timeout = std::env::var("OPENROUTER_TIMEOUT_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or(Duration::from_secs(120));
    let referer = std::env::var("OPENROUTER_REFERER").ok();
    let app_title = std::env::var("OPENROUTER_APP_TITLE").ok();
    let adapter = OpenRouterAdapter::with_config(
        provider_key.clone(),
        base_url,
        timeout,
        referer,
        app_title,
    )?;
    let gateway = ProviderGateway::with_config(adapter, Arc::new(NoopUsageSink), gateway_config);
    Ok(Arc::new(SecretScrubbingGateway {
        inner: gateway,
        secret: provider_key,
    }))
}

/// Enforces a floor between provider request starts. Free-tier judges are
/// rate-limited per minute (~20 req/min on OpenRouter `:free` slugs), so even
/// a `comparison_concurrency: 1` run can outrun the window when the model
/// answers quickly.
struct PacedGateway {
    inner: Arc<dyn ChatGateway>,
    min_interval: Duration,
    next_start: tokio::sync::Mutex<tokio::time::Instant>,
}

#[async_trait::async_trait]
impl ChatGateway for PacedGateway {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, ProviderError> {
        {
            let mut next_start = self.next_start.lock().await;
            let now = tokio::time::Instant::now();
            if *next_start > now {
                tokio::time::sleep_until(*next_start).await;
            }
            *next_start = tokio::time::Instant::now() + self.min_interval;
        }
        self.inner.chat(request).await
    }
}

struct SecretScrubbingGateway<G> {
    inner: G,
    secret: String,
}

#[async_trait::async_trait]
impl<G: ChatGateway> ChatGateway for SecretScrubbingGateway<G> {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, ProviderError> {
        self.inner
            .chat(request)
            .await
            .map_err(|error| scrub_provider_error(error, &self.secret))
    }
}

fn scrub_provider_error(error: ProviderError, secret: &str) -> ProviderError {
    let scrub = |value: String| value.replace(secret, "[REDACTED]");
    let scrub_context = |context: Option<ErrorContext>| {
        context.map(|mut context| {
            context.provider_code = context.provider_code.map(&scrub);
            context.request_id = context.request_id.map(&scrub);
            context
        })
    };
    match error {
        ProviderError::BudgetExceeded {
            limit_usd,
            spend_usd,
            retry_after,
        } => ProviderError::BudgetExceeded {
            limit_usd,
            spend_usd,
            retry_after,
        },
        ProviderError::RateLimited {
            retry_after,
            limit_source,
            context,
        } => ProviderError::RateLimited {
            retry_after,
            limit_source,
            context: scrub_context(context),
        },
        ProviderError::InvalidRequest { message, context } => ProviderError::InvalidRequest {
            message: scrub(message),
            context: scrub_context(context),
        },
        ProviderError::Refused { message, context } => ProviderError::Refused {
            message: scrub(message),
            context: scrub_context(context),
        },
        ProviderError::Provider {
            provider,
            message,
            retryable,
            context,
        } => ProviderError::Provider {
            provider,
            message: scrub(message),
            retryable,
            context: scrub_context(context),
        },
        ProviderError::Timeout(duration, context) => {
            ProviderError::Timeout(duration, scrub_context(context))
        }
        ProviderError::Http(error) => ProviderError::Http(error),
        ProviderError::Config(message) => ProviderError::Config(scrub(message)),
    }
}

fn persist_new_metadata(root: &Path, metadata: &DaemonRunMetadata) -> io::Result<()> {
    fs::create_dir_all(root)?;
    let path = metadata_path(root, &metadata.run_ref);
    let file = OpenOptions::new().write(true).create_new(true).open(path)?;
    let mut writer = BufWriter::new(file);
    serde_json::to_writer(&mut writer, metadata).map_err(io::Error::other)?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    writer.get_ref().sync_all()?;
    File::open(root)?.sync_all()
}

fn persist_metadata(root: &Path, metadata: &DaemonRunMetadata) -> io::Result<()> {
    fs::create_dir_all(root)?;
    let path = metadata_path(root, &metadata.run_ref);
    let mut temporary = tempfile::NamedTempFile::new_in(root)?;
    {
        let mut writer = BufWriter::new(temporary.as_file_mut());
        serde_json::to_writer(&mut writer, metadata).map_err(io::Error::other)?;
        writer.write_all(b"\n")?;
        writer.flush()?;
    }
    temporary.as_file().sync_all()?;
    temporary.persist(path).map_err(|error| error.error)?;
    File::open(root)?.sync_all()
}

fn load_metadata(root: &Path, run_ref: &str) -> io::Result<DaemonRunMetadata> {
    let reader = BufReader::new(File::open(metadata_path(root, run_ref))?);
    serde_json::from_reader(reader).map_err(io::Error::other)
}

fn metadata_path(root: &Path, run_ref: &str) -> PathBuf {
    root.join(format!("{run_ref}.cardinald.json"))
}

fn update_metadata_from_record(
    store: &JudgementRunStore,
    mut metadata: DaemonRunMetadata,
    record: &JudgementRunRecord,
) {
    metadata.status = record.terminal.status().to_string();
    metadata.error = match &record.terminal {
        JudgementRunTerminal::Failed { error } => Some(error.clone()),
        JudgementRunTerminal::Completed { .. } | JudgementRunTerminal::Cancelled { .. } => None,
    };
    if let Err(error) = persist_metadata(store.root(), &metadata) {
        eprintln!(
            "cardinald: could not update run metadata for {}: {error}",
            record.run_ref
        );
    }
}

fn mark_metadata_failed(store: &JudgementRunStore, mut metadata: DaemonRunMetadata, error: &str) {
    metadata.status = "failed".to_string();
    metadata.error = Some(error.to_string());
    if let Err(persistence_error) = persist_metadata(store.root(), &metadata) {
        eprintln!(
            "cardinald: could not persist failure metadata for {}: {persistence_error}",
            metadata.run_ref
        );
    }
}

fn valid_run_ref(value: &str) -> bool {
    let Some(suffix) = value.strip_prefix("jrun_") else {
        return false;
    };
    suffix.len() == 32
        && suffix.bytes().all(|byte| byte.is_ascii_hexdigit())
        && Uuid::parse_str(suffix).is_ok_and(|uuid| uuid.get_version_num() == 4)
}

fn stop_reason_name(reason: RerankStopReason) -> &'static str {
    match reason {
        RerankStopReason::ToleratedErrorMet => "tolerated_error_met",
        RerankStopReason::CertifiedStop => "certified_stop",
        RerankStopReason::BudgetExhausted => "budget_exhausted",
        RerankStopReason::LatencyBudgetExceeded => "latency_budget_exceeded",
        RerankStopReason::CostBudgetExhausted => "cost_budget_exhausted",
        RerankStopReason::ConsecutiveFailures => "consecutive_failures",
        RerankStopReason::Cancelled => "cancelled",
        RerankStopReason::NoProposals => "no_proposals",
        RerankStopReason::NoNewPairs => "no_new_pairs",
    }
}

fn env_or(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.to_string())
}

fn parse_positive_usize(name: &str, default: usize) -> Result<usize, String> {
    match std::env::var(name) {
        Ok(value) => value
            .parse::<usize>()
            .ok()
            .filter(|value| *value > 0)
            .ok_or_else(|| format!("{name} must be a positive integer")),
        Err(_) => Ok(default),
    }
}
