//! freelane — free-token elicitation driver for cardinald.
//!
//! Continuously re-elicits the public ledger's existing (lens, axis) cells
//! across OpenRouter's free-model pool, one cardinald run at a time, paced to
//! free-tier limits. Every run lands PRIVATE (`scry_judgements_private`): the
//! public `scores_current` projection is a ReplacingMergeTree keyed
//! (lens, axis_key, entity_id, entity_hash) with no model in the key, so a
//! public free-model run would displace curated board scores. Promotion of
//! free-judge output to any public surface is an editorial decision made
//! elsewhere, with rank-agreement evidence in hand.
//!
//! State lives in the ledger, never in local files: a (lens, axis, model)
//! cell is done iff `scry_judgements_private.comparisons` holds rows for it
//! under our owner scope. Restart is therefore always safe, and killing the
//! process loses at most the in-flight run (cardinald itself persists and
//! lands completed runs).
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
const DEFAULT_OWNER_SCOPE: &str = "freelane";
const DEFAULT_MAX_ENTITIES: usize = 60;
/// Attempt budget cardinald enforces per run: 8 comparisons per entity.
const COMPARISONS_PER_ENTITY: f64 = 8.0;
const POLL_INTERVAL: Duration = Duration::from_secs(15);
const IDLE_SLEEP: Duration = Duration::from_secs(3600);
const COOLDOWN_BASE: Duration = Duration::from_secs(3600);
const COOLDOWN_CAP: Duration = Duration::from_secs(6 * 3600);

struct Config {
    cardinald_url: String,
    clickhouse_url: String,
    rpm: u64,
    daily_budget: f64,
    owner_scope: String,
    model_denylist: HashSet<String>,
    max_entities: usize,
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
        Ok(Self {
            cardinald_url: std::env::var("FREELANE_CARDINALD_URL")
                .unwrap_or_else(|_| DEFAULT_CARDINALD_URL.to_string()),
            clickhouse_url,
            rpm,
            daily_budget,
            owner_scope: std::env::var("FREELANE_OWNER_SCOPE")
                .unwrap_or_else(|_| DEFAULT_OWNER_SCOPE.to_string()),
            model_denylist,
            max_entities,
            plan_only: std::env::args().any(|arg| arg == "--plan"),
        })
    }

    fn min_request_interval_ms(&self) -> u64 {
        (60_000f64 / self.rpm as f64).ceil() as u64
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
    fn new(capacity: f64) -> Self {
        Self {
            level: capacity,
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

async fn run_cell(config: &Config, axis: &Axis, model: &str) -> Result<RunOutcome, String> {
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|error| format!("could not build HTTP client: {error}"))?;
    let body = json!({
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
        "comparison_concurrency": 1,
        "min_request_interval_ms": config.min_request_interval_ms(),
    });
    let response = client
        .post(format!("{}/v1/runs", config.cardinald_url))
        .json(&body)
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
        "freelane: submitted {run_ref} lens={} axis={} model={model} n={}",
        axis.lens,
        axis.axis_key,
        axis.entities.len()
    );

    // 8·n serial comparisons, each costing the paced interval OR the model's
    // real latency, whichever is longer — free-tier latency runs ~30s/response
    // (cohere, observed 2026-09-04: 384 requests took 3h11m against a 2h05m
    // deadline, so the run "failed" here while cardinald landed it fine).
    // Budget 90s per comparison: generous enough that only a wedged daemon
    // trips it, which is the only thing this deadline is for.
    let per_comparison_ms = config.min_request_interval_ms().max(90_000);
    let deadline = std::time::Instant::now()
        + Duration::from_millis(per_comparison_ms * 8 * axis.entities.len() as u64)
        + Duration::from_secs(600);
    loop {
        tokio::time::sleep(POLL_INTERVAL).await;
        let polled: serde_json::Value = client
            .get(format!("{}/v1/runs/{run_ref}", config.cardinald_url))
            .send()
            .await
            .map_err(|error| format!("cardinald poll failed: {error}"))?
            .json()
            .await
            .map_err(|error| format!("cardinald poll response unreadable: {error}"))?;
        match polled.get("status").and_then(|value| value.as_str()) {
            Some("completed") => return Ok(RunOutcome::Completed),
            Some("cancelled") => return Ok(RunOutcome::Failed("cancelled".to_string())),
            Some("failed") => {
                let error = polled
                    .get("error")
                    .and_then(|value| value.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                return Ok(RunOutcome::Failed(error));
            }
            _ => {
                if std::time::Instant::now() > deadline {
                    return Ok(RunOutcome::Failed(format!(
                        "poll deadline exceeded for {run_ref}; leaving it to cardinald"
                    )));
                }
            }
        }
    }
}

#[tokio::main]
async fn main() {
    if let Err(error) = drive().await {
        eprintln!("freelane: {error}");
        std::process::exit(1);
    }
}

async fn drive() -> Result<(), String> {
    let config = Config::from_env()?;
    let ch = ClickHouse::from_url(&config.clickhouse_url)?;
    let mut bucket = Bucket::new(config.daily_budget);
    let mut cooldown: HashMap<String, (std::time::Instant, u32)> = HashMap::new();

    loop {
        let axes = discover_axes(&ch, config.max_entities).await?;
        let models = discover_free_models(&config.model_denylist).await?;
        let done = done_cells(&ch, &config.owner_scope).await?;

        let mut pending: Vec<(usize, usize)> = Vec::new(); // (model idx, axis idx)
        // Interleave across models so early coverage spans many judges
        // instead of one judge finishing every axis first.
        for axis_index in 0..axes.len() {
            for model_index in 0..models.len() {
                let axis = &axes[axis_index];
                let key = (
                    axis.lens.clone(),
                    axis.axis_key.clone(),
                    models[model_index].clone(),
                );
                if !done.contains(&key) {
                    pending.push((model_index, axis_index));
                }
            }
        }
        pending.sort_by_key(|(model_index, axis_index)| (*axis_index, *model_index));

        let estimated_requests: f64 = pending
            .iter()
            .map(|(_, axis_index)| axes[*axis_index].entities.len() as f64 * COMPARISONS_PER_ENTITY)
            .sum();
        println!(
            "freelane: {} axes × {} free models → {} pending cells (~{:.0} requests, ~{:.1} days at {:.0}/day)",
            axes.len(),
            models.len(),
            pending.len(),
            estimated_requests,
            estimated_requests / config.daily_budget,
            config.daily_budget,
        );

        if config.plan_only {
            for (model_index, axis_index) in &pending {
                let axis = &axes[*axis_index];
                println!(
                    "freelane: pending lens={} axis={} model={} n={}",
                    axis.lens,
                    axis.axis_key,
                    models[*model_index],
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

        for (model_index, axis_index) in pending {
            let axis = &axes[axis_index];
            let model = &models[model_index];
            if let Some((until, _)) = cooldown.get(model) {
                if *until > std::time::Instant::now() {
                    continue;
                }
            }
            let cost = axis.entities.len() as f64 * COMPARISONS_PER_ENTITY;
            let wait = bucket.wait_for(cost);
            if wait > 0.0 {
                println!("freelane: budget wait {:.0}s before next cell", wait);
                tokio::time::sleep(Duration::from_secs_f64(wait)).await;
                bucket.wait_for(cost);
            }
            bucket.charge(cost);
            match run_cell(&config, axis, model).await? {
                RunOutcome::Completed => {
                    println!(
                        "freelane: completed lens={} axis={} model={model}",
                        axis.lens, axis.axis_key
                    );
                    cooldown.remove(model);
                }
                RunOutcome::Failed(error) => {
                    let failures = cooldown.get(model).map(|(_, count)| *count).unwrap_or(0) + 1;
                    let pause = COOLDOWN_BASE
                        .saturating_mul(1u32 << (failures - 1).min(3))
                        .min(COOLDOWN_CAP);
                    println!(
                        "freelane: failed lens={} axis={} model={model} ({error}); cooling {}s (failure #{failures})",
                        axis.lens,
                        axis.axis_key,
                        pause.as_secs()
                    );
                    cooldown.insert(
                        model.clone(),
                        (std::time::Instant::now() + pause, failures),
                    );
                }
            }
        }
        // One sweep of the pending list done (some cells may have been
        // skipped on cool-down); re-discover and reconcile from the ledger.
    }
}
