//! freelane — free-token elicitation driver for cardinald.
//!
//! Continuously re-elicits the public ledger's existing (lens, axis) cells
//! across every configured free-tier provider lane: OpenRouter's `:free`
//! pool (discovered live) plus any `FREELANE_PROVIDERS` extras (Cerebras,
//! Gemini, … — static model lists against OpenAI-compatible endpoints,
//! routed per run via cardinald's `provider_base_url` + `x-provider-key`).
//! Up to each lane's `concurrent_runs` cardinald runs are in flight at a
//! time (one per distinct model), paced so each lane's combined request
//! floor stays at its `rpm`. Every run lands PRIVATE
//! (`scry_judgements_private`): the public `scores_current` projection is a
//! ReplacingMergeTree keyed (lens, axis_key, entity_id, entity_hash) with no
//! model in the key, so a public free-model run would displace curated board
//! scores. Promotion of free-judge output to any public surface is an
//! editorial decision made elsewhere, with rank-agreement evidence in hand.
//!
//! Throughput honesty: free-tier limits are enforced per account
//! (OpenRouter: 20 req/min, 1000 req/day account-wide, verified
//! 2026-09-04), so concurrency within one lane does not multiply
//! throughput — each lane's daily budget binds. More lanes DO multiply
//! throughput: every extra provider is an independent quota pool.
//!
//! State lives in the ledger, never in local files: a (lens, axis, model)
//! cell is done iff `scry_judgements_private.comparisons` holds rows for it
//! under our owner scope. Restart is therefore always safe, and killing the
//! process loses at most the in-flight runs (cardinald itself persists and
//! lands completed runs). Daily buckets are likewise seeded from the
//! ledger's last-day landed comparisons at boot, so restarts cannot mint
//! fresh budget. Model slugs must be globally unique across lanes (bare
//! Cerebras/Gemini slugs never collide with OpenRouter's namespaced
//! `vendor/model:free` shape); freelane refuses to start on a duplicate.
//!
//! Config is environment-only (systemd `EnvironmentFile` is the intended
//! carrier): see `docs/FREELANE.md`.

use std::collections::HashMap;
use std::collections::HashSet;
use std::time::Duration;

use serde::Deserialize;
use serde_json::json;

const DEFAULT_CARDINALD_URL: &str = "http://127.0.0.1:8093";
// 10 leaves half the account-wide ~20 req/min free window as headroom for
// overlap (a prior run's tail, discovery churn, other estate use). With
// gateway retries disabled on paced runs, the paced rate is the real rate.
const DEFAULT_RPM: u64 = 10;
const DEFAULT_DAILY_BUDGET: f64 = 900.0;
const DEFAULT_CONCURRENT_RUNS: usize = 4;
const DEFAULT_OWNER_SCOPE: &str = "freelane";
const DEFAULT_MAX_ENTITIES: usize = 60;
/// Attempt budget cardinald enforces per run: 8 comparisons per entity.
const COMPARISONS_PER_ENTITY: f64 = 8.0;
const POLL_INTERVAL: Duration = Duration::from_secs(15);
const IDLE_SLEEP: Duration = Duration::from_secs(3600);
const COOLDOWN_BASE: Duration = Duration::from_secs(3600);
const COOLDOWN_CAP: Duration = Duration::from_secs(6 * 3600);

/// One extra provider lane, from the `FREELANE_PROVIDERS` JSON array.
/// Example:
/// `[{"name":"cerebras","base_url":"https://api.cerebras.ai/v1",
///    "key_env":"CEREBRAS_API_KEY","rpm":10,"concurrent_runs":2,
///    "models":[{"slug":"gpt-oss-120b","daily":350}]}]`
#[derive(Deserialize)]
struct ExtraProviderSpec {
    name: String,
    base_url: String,
    /// Env var holding the lane's key. Absent = an unauthenticated local
    /// engine (vLLM); freelane still sends a placeholder key so cardinald
    /// never falls back to its OpenRouter key for the lane.
    #[serde(default)]
    key_env: Option<String>,
    rpm: u64,
    #[serde(default = "default_extra_concurrent")]
    concurrent_runs: usize,
    /// Pacing off = a local engine that WANTS saturation (vLLM batching);
    /// cardinald then runs the lane's requests with its normal retrying
    /// gateway and no interval floor.
    #[serde(default = "default_true")]
    paced: bool,
    /// Concurrent provider calls within one run (cardinald clamps 1..=16).
    /// Keep 1 for rate-limited APIs; raise for local engines.
    #[serde(default = "default_one")]
    comparison_concurrency: usize,
    models: Vec<ExtraModelSpec>,
}

fn default_true() -> bool {
    true
}

fn default_one() -> usize {
    1
}

#[derive(Deserialize)]
struct ExtraModelSpec {
    slug: String,
    /// Per-model daily request budget (free tiers meter per model per day).
    daily: f64,
}

fn default_extra_concurrent() -> usize {
    2
}

struct Config {
    cardinald_url: String,
    clickhouse_url: String,
    rpm: u64,
    daily_budget: f64,
    concurrent_runs: usize,
    owner_scope: String,
    model_denylist: HashSet<String>,
    max_entities: usize,
    extra_providers: Vec<ExtraProviderSpec>,
    plan_only: bool,
}

impl Config {
    fn from_env() -> Result<Self, String> {
        let clickhouse_url = std::env::var("FREELANE_CLICKHOUSE_URL").map_err(|_| {
            "FREELANE_CLICKHOUSE_URL is required (ClickHouse HTTP endpoint holding \
             scry_judgements; same URL shape cardinald's landing accepts)"
                .to_string()
        })?;
        let parse = |name: &str| -> Result<Option<u64>, String> {
            match std::env::var(name) {
                Ok(value) => value
                    .trim()
                    .parse::<u64>()
                    .map(Some)
                    .map_err(|_| format!("{name} must be a positive integer")),
                Err(_) => Ok(None),
            }
        };
        let rpm = parse("FREELANE_RPM")?.unwrap_or(DEFAULT_RPM).clamp(1, 60);
        let daily_budget = parse("FREELANE_DAILY_BUDGET")?
            .map(|value| value as f64)
            .unwrap_or(DEFAULT_DAILY_BUDGET);
        let concurrent_runs = parse("FREELANE_CONCURRENT_RUNS")?
            .map(|value| (value as usize).clamp(1, 12))
            .unwrap_or(DEFAULT_CONCURRENT_RUNS);
        let max_entities = parse("FREELANE_MAX_ENTITIES")?
            .map(|value| (value as usize).clamp(2, 200))
            .unwrap_or(DEFAULT_MAX_ENTITIES);
        let model_denylist = std::env::var("FREELANE_MODEL_DENYLIST")
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|slug| !slug.is_empty())
            .map(str::to_string)
            .collect();
        let extra_providers: Vec<ExtraProviderSpec> = match std::env::var("FREELANE_PROVIDERS") {
            Ok(raw) if !raw.trim().is_empty() => serde_json::from_str(&raw)
                .map_err(|error| format!("FREELANE_PROVIDERS is invalid JSON: {error}"))?,
            _ => Vec::new(),
        };
        for provider in &extra_providers {
            if !provider.base_url.starts_with("https://") {
                return Err(format!(
                    "provider {} base_url must be https",
                    provider.name
                ));
            }
            if provider.models.is_empty() {
                return Err(format!("provider {} lists no models", provider.name));
            }
        }
        Ok(Self {
            cardinald_url: std::env::var("FREELANE_CARDINALD_URL")
                .unwrap_or_else(|_| DEFAULT_CARDINALD_URL.to_string()),
            clickhouse_url,
            rpm,
            daily_budget,
            concurrent_runs,
            owner_scope: std::env::var("FREELANE_OWNER_SCOPE")
                .unwrap_or_else(|_| DEFAULT_OWNER_SCOPE.to_string()),
            model_denylist,
            max_entities,
            extra_providers,
            plan_only: std::env::args().any(|arg| arg == "--plan"),
        })
    }
}

/// A provider lane the scheduler can draw cells from. Lane 0 is always
/// OpenRouter (discovered models, one shared bucket); extras have static
/// model lists and one bucket per model.
struct Lane {
    name: String,
    base_url: Option<String>,
    key: Option<String>,
    rpm: u64,
    concurrent_runs: usize,
    paced: bool,
    comparison_concurrency: usize,
    models: Vec<String>,
    /// Parallel to `models` for extra lanes; `None` = shared lane bucket.
    per_model_daily: Option<Vec<f64>>,
}

impl Lane {
    /// Per-run paced interval: each of up to `concurrent_runs` runs is spaced
    /// at K×(60s/rpm), so the lane's combined floor stays at `rpm` no matter
    /// how many are actually in flight. cardinald validates intervals ≤ 60s;
    /// the clamp can push the combined floor slightly above `rpm` at high K,
    /// where real model latency binds anyway. Unpaced lanes (local engines)
    /// get no floor at all.
    fn interval_ms(&self) -> u64 {
        if !self.paced {
            return 0;
        }
        (((60_000f64 * self.concurrent_runs as f64) / self.rpm as f64).ceil() as u64).min(60_000)
    }

    fn bucket_id(&self, model_index: usize) -> String {
        if self.per_model_daily.is_some() {
            format!("{}/{}", self.name, self.models[model_index])
        } else {
            self.name.clone()
        }
    }
}

/// Leaky bucket over elicitation requests: capacity = the daily budget,
/// refilled continuously at budget/day. Never a calendar window.
struct Bucket {
    level: f64,
    capacity: f64,
    refill_per_sec: f64,
    last: std::time::Instant,
}

impl Bucket {
    /// `initial` seeds the level (clamped to [0, capacity]) so a restart can
    /// account for spend the ledger already witnessed.
    fn new(capacity: f64, initial: f64) -> Self {
        Self {
            level: initial.clamp(0.0, capacity),
            capacity,
            refill_per_sec: capacity / 86_400.0,
            last: std::time::Instant::now(),
        }
    }

    /// Seconds until `cost` is affordable (0 when it already is).
    fn wait_for(&mut self, cost: f64) -> f64 {
        let now = std::time::Instant::now();
        self.level = (self.level + now.duration_since(self.last).as_secs_f64() * self.refill_per_sec)
            .min(self.capacity);
        self.last = now;
        if self.level >= cost {
            0.0
        } else {
            (cost - self.level) / self.refill_per_sec
        }
    }

    fn charge(&mut self, cost: f64) {
        self.level -= cost;
    }
}

#[derive(Clone)]
struct Axis {
    lens: String,
    axis_key: String,
    axis_prompt: String,
    entities: Vec<(String, String)>,
}

struct ClickHouse {
    client: reqwest::Client,
    endpoint: reqwest::Url,
    basic_auth: Option<(String, Option<String>)>,
}

impl ClickHouse {
    fn from_url(raw: &str) -> Result<Self, String> {
        let mut endpoint =
            reqwest::Url::parse(raw).map_err(|error| format!("invalid ClickHouse URL: {error}"))?;
        let basic_auth = if endpoint.username().is_empty() && endpoint.password().is_none() {
            None
        } else {
            let username = endpoint.username().to_string();
            let password = endpoint.password().map(str::to_string);
            endpoint.set_password(None).ok();
            endpoint.set_username("").ok();
            Some((username, password))
        };
        Ok(Self {
            client: reqwest::Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(30))
                .build()
                .map_err(|error| format!("could not build HTTP client: {error}"))?,
            endpoint,
            basic_auth,
        })
    }

    /// POST a query whose only string parameters are pre-bound via
    /// `param_<name>` query pairs; response rows come back as JSONEachRow.
    async fn query(
        &self,
        sql: &str,
        params: &[(&str, &str)],
    ) -> Result<Vec<serde_json::Value>, String> {
        let mut url = self.endpoint.clone();
        for (name, value) in params {
            url.query_pairs_mut()
                .append_pair(&format!("param_{name}"), value);
        }
        let mut request = self.client.post(url).body(sql.as_bytes().to_vec());
        if let Some((username, password)) = &self.basic_auth {
            request = request.basic_auth(username, password.as_ref());
        }
        let response = request
            .send()
            .await
            .map_err(|error| format!("ClickHouse request failed: {error}"))?;
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|error| format!("ClickHouse response unreadable: {error}"))?;
        if !status.is_success() {
            return Err(format!("ClickHouse HTTP {status}: {}", body.trim()));
        }
        body.lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| {
                serde_json::from_str(line)
                    .map_err(|error| format!("ClickHouse row parse failed: {error}"))
            })
            .collect()
    }
}

async fn discover_axes(ch: &ClickHouse, max_entities: usize) -> Result<Vec<Axis>, String> {
    let heads = ch
        .query(
            "SELECT lens, axis_key, any(axis_prompt) AS axis_prompt \
             FROM scry_judgements.scores_current \
             GROUP BY lens, axis_key \
             HAVING uniqExact(entity_id) >= 2 \
             ORDER BY lens, axis_key \
             FORMAT JSONEachRow",
            &[],
        )
        .await?;
    let mut axes = Vec::with_capacity(heads.len());
    for head in heads {
        let lens = str_field(&head, "lens")?;
        let axis_key = str_field(&head, "axis_key")?;
        let axis_prompt = str_field(&head, "axis_prompt")?;
        let rows = ch
            .query(
                &format!(
                    "SELECT entity_id, any(entity_text) AS entity_text \
                     FROM scry_judgements.scores_current \
                     WHERE lens = {{lens:String}} AND axis_key = {{axis:String}} \
                     GROUP BY entity_id \
                     ORDER BY max(latent_mean) DESC \
                     LIMIT {max_entities} \
                     FORMAT JSONEachRow"
                ),
                &[("lens", lens.as_str()), ("axis", axis_key.as_str())],
            )
            .await?;
        let entities = rows
            .iter()
            .map(|row| Ok((str_field(row, "entity_id")?, str_field(row, "entity_text")?)))
            .collect::<Result<Vec<_>, String>>()?;
        if entities.len() >= 2 {
            axes.push(Axis {
                lens,
                axis_key,
                axis_prompt,
                entities,
            });
        }
    }
    Ok(axes)
}

async fn done_cells(
    ch: &ClickHouse,
    owner_scope: &str,
) -> Result<HashSet<(String, String, String)>, String> {
    let rows = ch
        .query(
            "SELECT DISTINCT lens, axis_key, model \
             FROM scry_judgements_private.comparisons \
             WHERE owner_scope = {scope:String} \
             FORMAT JSONEachRow",
            &[("scope", owner_scope)],
        )
        .await?;
    rows.iter()
        .map(|row| {
            Ok((
                str_field(row, "lens")?,
                str_field(row, "axis_key")?,
                str_field(row, "model")?,
            ))
        })
        .collect()
}

/// Requests the ledger witnessed landing in the trailing day under our scope,
/// per model. Failed runs' attempts are invisible here, so this undercounts
/// true spend; cool-downs bound that error, and the alternative (a state
/// file) is worse.
async fn landed_last_day_by_model(
    ch: &ClickHouse,
    owner_scope: &str,
) -> Result<HashMap<String, f64>, String> {
    let rows = ch
        .query(
            "SELECT model, count() AS n \
             FROM scry_judgements_private.comparisons \
             WHERE owner_scope = {scope:String} \
               AND observed_at > now64(3) - INTERVAL 1 DAY \
             GROUP BY model \
             FORMAT JSONEachRow",
            &[("scope", owner_scope)],
        )
        .await?;
    let mut counts = HashMap::with_capacity(rows.len());
    for row in &rows {
        let model = str_field(row, "model")?;
        let value = row
            .get("n")
            .ok_or_else(|| "ClickHouse count row missing n".to_string())?;
        let count = value
            .as_f64()
            .or_else(|| value.as_str().and_then(|s| s.parse().ok()))
            .ok_or_else(|| "ClickHouse count not numeric".to_string())?;
        counts.insert(model, count);
    }
    Ok(counts)
}

#[derive(Deserialize)]
struct ModelsPage {
    data: Vec<serde_json::Value>,
}

async fn discover_free_models(denylist: &HashSet<String>) -> Result<Vec<String>, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("freelane/0.1 (llmsort-experiments)")
        .build()
        .map_err(|error| format!("could not build HTTP client: {error}"))?;
    let page: ModelsPage = client
        .get("https://openrouter.ai/api/v1/models")
        .send()
        .await
        .map_err(|error| format!("OpenRouter models request failed: {error}"))?
        .json()
        .await
        .map_err(|error| format!("OpenRouter models parse failed: {error}"))?;
    let mut slugs = Vec::new();
    for model in page.data {
        let Some(id) = model.get("id").and_then(|value| value.as_str()) else {
            continue;
        };
        if !id.ends_with(":free") || denylist.contains(id) {
            continue;
        }
        let priced_zero = |field: &str| {
            model
                .pointer(&format!("/pricing/{field}"))
                .and_then(|value| value.as_str())
                == Some("0")
        };
        if !priced_zero("prompt") || !priced_zero("completion") {
            continue;
        }
        // Text-to-text judges only: skip anything whose declared output
        // modalities exist and exclude text.
        let text_out = match model.pointer("/architecture/output_modalities") {
            Some(serde_json::Value::Array(modalities)) => {
                modalities.iter().any(|value| value.as_str() == Some("text"))
            }
            _ => true,
        };
        if text_out {
            slugs.push(id.to_string());
        }
    }
    slugs.sort();
    Ok(slugs)
}

fn str_field(row: &serde_json::Value, name: &str) -> Result<String, String> {
    row.get(name)
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .ok_or_else(|| format!("ClickHouse row missing string field {name}"))
}

enum RunOutcome {
    Completed,
    Failed(String),
}

struct InFlight {
    run_ref: String,
    lane_index: usize,
    model_index: usize,
    axis_index: usize,
    deadline: std::time::Instant,
}

#[derive(Clone, Copy)]
struct Cell {
    lane_index: usize,
    model_index: usize,
    axis_index: usize,
}

async fn submit_cell(
    client: &reqwest::Client,
    config: &Config,
    lane: &Lane,
    axis: &Axis,
    model: &str,
) -> Result<(String, std::time::Instant), String> {
    let mut body = json!({
        "entities": axis
            .entities
            .iter()
            .map(|(id, text)| json!({"id": id, "text": text}))
            .collect::<Vec<_>>(),
        "axis_key": axis.axis_key,
        "axis_prompt": axis.axis_prompt,
        "requested_k": axis.entities.len().min(10),
        "model": model,
        "privacy": "private",
        "owner_scope": config.owner_scope,
        "lens": axis.lens,
        "comparison_concurrency": lane.comparison_concurrency,
    });
    let interval_ms = lane.interval_ms();
    if interval_ms > 0 {
        body["min_request_interval_ms"] = json!(interval_ms);
    }
    if let Some(base_url) = &lane.base_url {
        body["provider_base_url"] = json!(base_url);
    }
    let mut request = client
        .post(format!("{}/v1/runs", config.cardinald_url))
        .json(&body);
    if let Some(key) = &lane.key {
        request = request.header("x-provider-key", key);
    } else if lane.base_url.is_some() {
        // Keyless local engine: send a placeholder so cardinald never falls
        // back to its OpenRouter key for this lane.
        request = request.header("x-provider-key", "local-unauthenticated");
    }
    let response = request
        .send()
        .await
        .map_err(|error| format!("cardinald submit failed: {error}"))?;
    let status = response.status();
    let submitted: serde_json::Value = response
        .json()
        .await
        .map_err(|error| format!("cardinald submit response unreadable: {error}"))?;
    if !status.is_success() {
        return Err(format!("cardinald submit HTTP {status}: {submitted}"));
    }
    let run_ref = str_field(&submitted, "run_ref")?;
    println!(
        "freelane: submitted {run_ref} lane={} lens={} axis={} model={model} n={} interval={}ms",
        lane.name,
        axis.lens,
        axis.axis_key,
        axis.entities.len(),
        lane.interval_ms(),
    );

    // 8·n serial comparisons, each costing the paced interval OR the model's
    // real latency, whichever is longer — free-tier latency runs ~30s/response
    // (cohere, observed 2026-09-04: 384 requests took 3h11m against a 2h05m
    // deadline, so the run "failed" here while cardinald landed it fine).
    // Budget 90s per comparison: generous enough that only a wedged daemon
    // trips it, which is the only thing this deadline is for.
    let per_comparison_ms = lane.interval_ms().max(90_000);
    let deadline = std::time::Instant::now()
        + Duration::from_millis(per_comparison_ms * 8 * axis.entities.len() as u64)
        + Duration::from_secs(600);
    Ok((run_ref, deadline))
}

/// One status probe: `None` while the run is still going.
async fn poll_run(
    client: &reqwest::Client,
    config: &Config,
    run_ref: &str,
) -> Result<Option<RunOutcome>, String> {
    let polled: serde_json::Value = client
        .get(format!("{}/v1/runs/{run_ref}", config.cardinald_url))
        .send()
        .await
        .map_err(|error| format!("cardinald poll failed: {error}"))?
        .json()
        .await
        .map_err(|error| format!("cardinald poll response unreadable: {error}"))?;
    match polled.get("status").and_then(|value| value.as_str()) {
        Some("completed") => Ok(Some(RunOutcome::Completed)),
        Some("cancelled") => Ok(Some(RunOutcome::Failed("cancelled".to_string()))),
        Some("failed") => {
            let error = polled
                .get("error")
                .and_then(|value| value.as_str())
                .unwrap_or("unknown")
                .to_string();
            Ok(Some(RunOutcome::Failed(error)))
        }
        _ => Ok(None),
    }
}

fn cool(
    cooldown: &mut HashMap<String, (std::time::Instant, u32)>,
    key: &str,
    lens: &str,
    axis_key: &str,
    error: &str,
) {
    let failures = cooldown.get(key).map(|(_, count)| *count).unwrap_or(0) + 1;
    let pause = COOLDOWN_BASE
        .saturating_mul(1u32 << (failures - 1).min(3))
        .min(COOLDOWN_CAP);
    println!(
        "freelane: failed lens={lens} axis={axis_key} judge={key} ({error}); cooling {}s (failure #{failures})",
        pause.as_secs()
    );
    cooldown.insert(
        key.to_string(),
        (std::time::Instant::now() + pause, failures),
    );
}

#[tokio::main]
async fn main() {
    if let Err(error) = drive().await {
        eprintln!("freelane: {error}");
        std::process::exit(1);
    }
}

fn build_extra_lanes(config: &Config) -> Result<Vec<Lane>, String> {
    let mut lanes = Vec::with_capacity(config.extra_providers.len());
    for spec in &config.extra_providers {
        let key = match &spec.key_env {
            Some(key_env) => Some(std::env::var(key_env).map_err(|_| {
                format!("provider {} key env {key_env} is unset", spec.name)
            })?),
            None => None,
        };
        lanes.push(Lane {
            name: spec.name.clone(),
            base_url: Some(spec.base_url.clone()),
            key,
            rpm: spec.rpm.clamp(1, 1200),
            concurrent_runs: spec.concurrent_runs.clamp(1, 12),
            paced: spec.paced,
            comparison_concurrency: spec.comparison_concurrency.clamp(1, 16),
            models: spec.models.iter().map(|m| m.slug.clone()).collect(),
            per_model_daily: Some(spec.models.iter().map(|m| m.daily).collect()),
        });
    }
    Ok(lanes)
}

async fn drive() -> Result<(), String> {
    let config = Config::from_env()?;
    let ch = ClickHouse::from_url(&config.clickhouse_url)?;
    let extra_lanes = build_extra_lanes(&config)?;

    // Seed every bucket from the ledger's trailing day.
    let landed = landed_last_day_by_model(&ch, &config.owner_scope).await?;
    let extra_model_set: HashSet<&str> = extra_lanes
        .iter()
        .flat_map(|lane| lane.models.iter().map(String::as_str))
        .collect();
    if extra_model_set.len()
        != extra_lanes
            .iter()
            .map(|lane| lane.models.len())
            .sum::<usize>()
    {
        return Err("duplicate model slug across provider lanes".to_string());
    }
    let openrouter_spent: f64 = landed
        .iter()
        .filter(|(model, _)| !extra_model_set.contains(model.as_str()))
        .map(|(_, count)| count)
        .sum();
    let mut buckets: HashMap<String, Bucket> = HashMap::new();
    buckets.insert(
        "openrouter".to_string(),
        Bucket::new(config.daily_budget, config.daily_budget - openrouter_spent),
    );
    for lane in &extra_lanes {
        let dailies = lane.per_model_daily.as_ref().expect("extra lanes are per-model");
        for (model_index, slug) in lane.models.iter().enumerate() {
            let capacity = dailies[model_index];
            let spent = landed.get(slug).copied().unwrap_or(0.0);
            buckets.insert(
                lane.bucket_id(model_index),
                Bucket::new(capacity, capacity - spent),
            );
        }
    }
    if !landed.is_empty() {
        let total: f64 = landed.values().sum();
        println!(
            "freelane: {total:.0} requests landed in the trailing day across {} models; buckets seeded from the ledger",
            landed.len()
        );
    }

    let mut cooldown: HashMap<String, (std::time::Instant, u32)> = HashMap::new();
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|error| format!("could not build HTTP client: {error}"))?;

    loop {
        let axes = discover_axes(&ch, config.max_entities).await?;
        let openrouter_models = discover_free_models(&config.model_denylist).await?;
        let done = done_cells(&ch, &config.owner_scope).await?;

        let mut lanes: Vec<Lane> = vec![Lane {
            name: "openrouter".to_string(),
            base_url: None,
            key: None,
            rpm: config.rpm,
            concurrent_runs: config.concurrent_runs,
            paced: true,
            comparison_concurrency: 1,
            models: openrouter_models,
            per_model_daily: None,
        }];
        for spec_lane in &extra_lanes {
            lanes.push(Lane {
                name: spec_lane.name.clone(),
                base_url: spec_lane.base_url.clone(),
                key: spec_lane.key.clone(),
                rpm: spec_lane.rpm,
                concurrent_runs: spec_lane.concurrent_runs,
                paced: spec_lane.paced,
                comparison_concurrency: spec_lane.comparison_concurrency,
                models: spec_lane.models.clone(),
                per_model_daily: spec_lane.per_model_daily.clone(),
            });
        }

        // Interleave across judges so early coverage spans many of them
        // instead of one judge finishing every axis first.
        let mut pending: Vec<Cell> = Vec::new();
        for axis_index in 0..axes.len() {
            for (lane_index, lane) in lanes.iter().enumerate() {
                for model_index in 0..lane.models.len() {
                    let axis = &axes[axis_index];
                    let key = (
                        axis.lens.clone(),
                        axis.axis_key.clone(),
                        lane.models[model_index].clone(),
                    );
                    if !done.contains(&key) {
                        pending.push(Cell {
                            lane_index,
                            model_index,
                            axis_index,
                        });
                    }
                }
            }
        }

        let judge_count: usize = lanes.iter().map(|lane| lane.models.len()).sum();
        let estimated_requests: f64 = pending
            .iter()
            .map(|cell| axes[cell.axis_index].entities.len() as f64 * COMPARISONS_PER_ENTITY)
            .sum();
        let daily_capacity: f64 = config.daily_budget
            + extra_lanes
                .iter()
                .flat_map(|lane| lane.per_model_daily.iter().flatten())
                .sum::<f64>();
        println!(
            "freelane: {} axes × {} judges over {} lanes → {} pending cells (~{:.0} requests, ~{:.1} days at {:.0}/day)",
            axes.len(),
            judge_count,
            lanes.len(),
            pending.len(),
            estimated_requests,
            estimated_requests / daily_capacity,
            daily_capacity,
        );

        if config.plan_only {
            for cell in &pending {
                let axis = &axes[cell.axis_index];
                let lane = &lanes[cell.lane_index];
                println!(
                    "freelane: pending lane={} lens={} axis={} model={} n={}",
                    lane.name,
                    axis.lens,
                    axis.axis_key,
                    lane.models[cell.model_index],
                    axis.entities.len()
                );
            }
            return Ok(());
        }

        if pending.is_empty() {
            println!("freelane: no pending cells; sleeping 1h before re-discovery");
            tokio::time::sleep(IDLE_SLEEP).await;
            continue;
        }

        let mut in_flight: Vec<InFlight> = Vec::new();
        loop {
            // Fill each lane toward its concurrency target: one run per
            // distinct judge, skipping cooling judges and cells whose bucket
            // cannot yet afford them (a cheaper cell further down may fit).
            let now = std::time::Instant::now();
            let mut idx = 0;
            while idx < pending.len() {
                let cell = pending[idx];
                let lane = &lanes[cell.lane_index];
                let lane_load = in_flight
                    .iter()
                    .filter(|run| run.lane_index == cell.lane_index)
                    .count();
                if lane_load >= lane.concurrent_runs {
                    idx += 1;
                    continue;
                }
                let judge_busy = in_flight.iter().any(|run| {
                    run.lane_index == cell.lane_index && run.model_index == cell.model_index
                });
                let judge_key = format!("{}:{}", lane.name, lane.models[cell.model_index]);
                let judge_cooling = cooldown
                    .get(&judge_key)
                    .is_some_and(|(until, _)| *until > now);
                if judge_busy || judge_cooling {
                    idx += 1;
                    continue;
                }
                let cost =
                    axes[cell.axis_index].entities.len() as f64 * COMPARISONS_PER_ENTITY;
                let bucket = buckets
                    .get_mut(&lane.bucket_id(cell.model_index))
                    .expect("bucket exists for every schedulable cell");
                if bucket.wait_for(cost) > 0.0 {
                    idx += 1;
                    continue;
                }
                bucket.charge(cost);
                let (run_ref, deadline) = submit_cell(
                    &client,
                    &config,
                    lane,
                    &axes[cell.axis_index],
                    &lane.models[cell.model_index],
                )
                .await?;
                in_flight.push(InFlight {
                    run_ref,
                    lane_index: cell.lane_index,
                    model_index: cell.model_index,
                    axis_index: cell.axis_index,
                    deadline,
                });
                pending.remove(idx);
            }

            if in_flight.is_empty() {
                if pending.is_empty() {
                    break;
                }
                // Everything is cooling or budget-starved: wait for the nearer
                // of the cheapest affordable submit or a cool-down expiry.
                let now = std::time::Instant::now();
                let mut budget_wait = f64::INFINITY;
                for cell in &pending {
                    let lane = &lanes[cell.lane_index];
                    let judge_key = format!("{}:{}", lane.name, lane.models[cell.model_index]);
                    if cooldown
                        .get(&judge_key)
                        .is_some_and(|(until, _)| *until > now)
                    {
                        continue;
                    }
                    let cost =
                        axes[cell.axis_index].entities.len() as f64 * COMPARISONS_PER_ENTITY;
                    if let Some(bucket) = buckets.get_mut(&lane.bucket_id(cell.model_index)) {
                        budget_wait = budget_wait.min(bucket.wait_for(cost));
                    }
                }
                let cool_wait = cooldown
                    .values()
                    .map(|(until, _)| until.saturating_duration_since(now).as_secs_f64())
                    .fold(f64::INFINITY, f64::min);
                let wait = budget_wait.min(cool_wait).clamp(15.0, 3600.0);
                println!("freelane: nothing submittable; waiting {wait:.0}s (budget/cool-down)");
                tokio::time::sleep(Duration::from_secs_f64(wait)).await;
                continue;
            }

            tokio::time::sleep(POLL_INTERVAL).await;

            let mut i = 0;
            while i < in_flight.len() {
                let outcome = poll_run(&client, &config, &in_flight[i].run_ref).await?;
                let run = &in_flight[i];
                let axis = &axes[run.axis_index];
                let lane = &lanes[run.lane_index];
                let judge_key = format!("{}:{}", lane.name, lane.models[run.model_index]);
                match outcome {
                    Some(RunOutcome::Completed) => {
                        println!(
                            "freelane: completed lane={} lens={} axis={} model={}",
                            lane.name, axis.lens, axis.axis_key, lane.models[run.model_index]
                        );
                        cooldown.remove(&judge_key);
                        in_flight.remove(i);
                    }
                    Some(RunOutcome::Failed(error)) => {
                        cool(&mut cooldown, &judge_key, &axis.lens, &axis.axis_key, &error);
                        in_flight.remove(i);
                    }
                    None => {
                        if std::time::Instant::now() > run.deadline {
                            let error = format!(
                                "poll deadline exceeded for {}; leaving it to cardinald",
                                run.run_ref
                            );
                            cool(&mut cooldown, &judge_key, &axis.lens, &axis.axis_key, &error);
                            in_flight.remove(i);
                        } else {
                            i += 1;
                        }
                    }
                }
            }

            if pending.is_empty() && in_flight.is_empty() {
                break;
            }
        }
        // One sweep of the pending list done (some cells may have been
        // skipped on cool-down); re-discover and reconcile from the ledger.
    }
}
