//! ClickHouse landing for completed portable judgement runs.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{DateTime, Utc};
use reqwest::{Client, Url};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::judgement_run::{
    JudgementPrivacy, JudgementRunRecord, JudgementRunStore, JudgementRunTerminal,
};
use llmsort::gateway::types::DEFAULT_CHAT_TEMPERATURE;

const PUBLIC_COMPARISONS: &str = "scry_judgements.comparisons";
const PUBLIC_SCORES: &str = "scry_judgements.scores";
const PRIVATE_COMPARISONS: &str = "scry_judgements_private.comparisons";
const PRIVATE_SCORES: &str = "scry_judgements_private.scores";
// Default provenance label written into landed rows. Updated at each crate
// rename; older rows carry "cardinal-harness" (pre 2026-08-12) or
// "ratiometer" (pre 2026-08-15).
const HARNESS: &str = "llmsorting";

const COMPARISON_COLUMNS: &str = "observed_at,run_id,lens,axis_key,axis_prompt,axis_prompt_hash,harness,template_slug,template_hash,model,comparison_index,entity_a_id,entity_b_id,entity_a_hash,entity_b_hash,swapped,cached,refused,higher_ranked,ratio,confidence,log_ratio_mean,log_ratio_var,input_tokens,output_tokens,cost_nanodollars,cost_is_estimate,error,temperature,harness_version";
const PRIVATE_COMPARISON_COLUMNS: &str = "observed_at,run_id,owner_scope,lens,axis_key,axis_prompt,axis_prompt_hash,harness,template_slug,template_hash,model,comparison_index,entity_a_id,entity_b_id,entity_a_hash,entity_b_hash,swapped,cached,refused,higher_ranked,ratio,confidence,log_ratio_mean,log_ratio_var,input_tokens,output_tokens,cost_nanodollars,cost_is_estimate,error,temperature,harness_version";
const SCORE_COLUMNS: &str = "scored_at,run_id,lens,axis_key,axis_prompt,axis_prompt_hash,harness,model,seed,item_count,comparison_budget,comparisons_used,stop_reason,topk_error,run_cost_nanodollars,entity_id,entity_text,rank,latent_mean,latent_std,z_score,percentile,temperature,harness_version,entity_hash";
const PRIVATE_SCORE_COLUMNS: &str = "scored_at,run_id,owner_scope,lens,axis_key,axis_prompt,axis_prompt_hash,harness,model,seed,item_count,comparison_budget,comparisons_used,stop_reason,topk_error,run_cost_nanodollars,entity_id,entity_text,rank,latent_mean,latent_std,z_score,percentile,temperature,harness_version,entity_hash";

/// Credential-aware ClickHouse HTTP client. Its URL is scrubbed of userinfo.
pub struct ClickHouseLanding {
    client: Client,
    endpoint: Url,
    basic_auth: Option<(String, Option<String>)>,
}

impl ClickHouseLanding {
    pub fn from_url(raw: &str) -> Result<Self, String> {
        let mut endpoint = Url::parse(raw).map_err(|error| format!("invalid URL: {error}"))?;
        if !matches!(endpoint.scheme(), "http" | "https") {
            return Err("URL scheme must be http or https".to_string());
        }

        let basic_auth = if endpoint.username().is_empty() && endpoint.password().is_none() {
            None
        } else {
            let username = percent_decode(endpoint.username())?;
            let password = endpoint.password().map(percent_decode).transpose()?;
            endpoint
                .set_password(None)
                .map_err(|_| "could not remove URL password".to_string())?;
            endpoint
                .set_username("")
                .map_err(|_| "could not remove URL username".to_string())?;
            Some((username, password))
        };

        let client = Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|error| format!("could not build HTTP client: {error}"))?;
        Ok(Self {
            client,
            endpoint,
            basic_auth,
        })
    }

    async fn batch_exists(&self, table: &'static str, run_ref: &str) -> Result<bool, String> {
        columns_for_table(table).ok_or_else(|| "unknown landing table".to_string())?;
        let mut url = self.endpoint.clone();
        let query = format!(
            "SELECT count() FROM {table} WHERE run_id = {{run_id:String}} FORMAT TabSeparated"
        );
        url.query_pairs_mut().append_pair("param_run_id", run_ref);

        // The SQL travels as the POST body, never as a bodyless POST with
        // the query in the URL: ClickHouse rejects a POST carrying neither
        // Content-Length nor Transfer-Encoding with 411 Length Required
        // (bit live 2026-07-26 — four runs parked in landing_pending; an
        // explicit empty Vec body still went out header-less).
        let mut request = self.client.post(url).body(query.into_bytes());
        if let Some((username, password)) = &self.basic_auth {
            request = request.basic_auth(username, password.as_ref());
        }
        let response = request
            .send()
            .await
            .map_err(|error| format!("existence probe HTTP request failed: {error}"))?;
        if !response.status().is_success() {
            return Err(format!(
                "ClickHouse existence probe returned HTTP {}",
                response.status()
            ));
        }
        let count = response
            .text()
            .await
            .map_err(|error| format!("could not read existence probe response: {error}"))?
            .trim()
            .parse::<u64>()
            .map_err(|_| "ClickHouse existence probe returned an invalid count".to_string())?;
        Ok(count > 0)
    }

    async fn insert_if_missing(
        &self,
        table: &'static str,
        run_ref: &str,
        body: Vec<u8>,
    ) -> Result<(), String> {
        if self.batch_exists(table, run_ref).await? {
            return Ok(());
        }
        self.insert(table, body).await
    }

    async fn insert(&self, table: &'static str, body: Vec<u8>) -> Result<(), String> {
        let columns =
            columns_for_table(table).ok_or_else(|| "unknown landing table".to_string())?;
        let mut url = self.endpoint.clone();
        let query = format!("INSERT INTO {table} ({columns}) FORMAT JSONEachRow");
        url.query_pairs_mut()
            .append_pair("query", &query)
            .append_pair("date_time_input_format", "best_effort");

        let mut request = self
            .client
            .post(url)
            .header("content-type", "application/x-ndjson")
            .body(body);
        if let Some((username, password)) = &self.basic_auth {
            request = request.basic_auth(username, password.as_ref());
        }
        let response = request
            .send()
            .await
            .map_err(|error| format!("HTTP request failed: {error}"))?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(format!("ClickHouse returned HTTP {}", response.status()))
        }
    }

    /// Retry every recognized pending batch once.
    pub async fn replay_pending(&self, store: &JudgementRunStore) {
        let mut pending = match pending_files(store.root()) {
            Ok(paths) => paths,
            Err(error) => {
                eprintln!("cardinald: could not scan pending landings: {error}");
                return;
            }
        };
        pending.sort();
        for path in pending {
            let Some((run_ref, table)) = pending_descriptor(&path) else {
                continue;
            };
            let body = match fs::read(&path) {
                Ok(body) => body,
                Err(error) => {
                    eprintln!(
                        "cardinald: could not read pending landing {}: {error}",
                        path.display()
                    );
                    continue;
                }
            };
            if body.is_empty() {
                // Zero rows to land: the batch is vacuously landed. Remove
                // the file instead of replaying a bodyless insert that
                // ClickHouse rejects with 411 on every pass.
                if let Err(error) = fs::remove_file(&path) {
                    eprintln!(
                        "cardinald: could not remove empty pending landing {}: {error}",
                        path.display()
                    );
                }
                continue;
            }
            match self.insert_if_missing(table, run_ref, body).await {
                Ok(()) => {
                    if let Err(error) = fs::remove_file(&path) {
                        eprintln!(
                            "cardinald: landed pending batch but could not remove {}: {error}",
                            path.display()
                        );
                    }
                }
                Err(error) => {
                    eprintln!("cardinald: pending landing retry for {table} failed: {error}");
                }
            }
        }
    }
}

/// Persist both table batches before landing either one, preserving failed batches.
pub async fn land_completed_run(
    client: Option<&ClickHouseLanding>,
    store: &JudgementRunStore,
    record: &JudgementRunRecord,
    lens: &str,
    owner_scope: &str,
) -> bool {
    let batches = match completed_batches(record, lens, owner_scope) {
        Ok(batches) => batches,
        Err(error) => {
            eprintln!(
                "cardinald: could not construct landing rows for {}: {error}",
                record.run_ref
            );
            return false;
        }
    };

    let pending_batches: Vec<_> = batches
        .into_iter()
        .map(|batch| {
            let pending = pending_path(store.root(), &record.run_ref, batch.table);
            (batch, pending)
        })
        .collect();
    let mut all_preserved = true;
    for (batch, pending) in &pending_batches {
        if let Err(error) = write_pending(pending, &batch.body) {
            eprintln!(
                "cardinald: could not preserve pending batch {}: {error}",
                pending.display()
            );
            all_preserved = false;
        }
    }
    if !all_preserved {
        return false;
    }

    for (batch, pending) in pending_batches {
        let landed = match client {
            Some(client) => match client
                .insert_if_missing(batch.table, &record.run_ref, batch.body)
                .await
            {
                Ok(()) => true,
                Err(error) => {
                    eprintln!(
                        "cardinald: ClickHouse landing for {} into {} failed: {error}",
                        record.run_ref, batch.table
                    );
                    false
                }
            },
            None => {
                eprintln!(
                    "cardinald: ClickHouse URL is not configured; preserving {} batch for {}",
                    batch.table, record.run_ref
                );
                false
            }
        };

        if landed {
            if let Err(error) = fs::remove_file(&pending) {
                eprintln!(
                    "cardinald: landed batch but could not remove {}: {error}",
                    pending.display()
                );
            }
        }
    }
    true
}

struct LandingBatch {
    table: &'static str,
    body: Vec<u8>,
}

#[derive(Serialize)]
struct ComparisonRow<'a> {
    observed_at: String,
    run_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    owner_scope: Option<&'a str>,
    lens: &'a str,
    axis_key: &'a str,
    axis_prompt: &'a str,
    axis_prompt_hash: &'a str,
    harness: &'a str,
    template_slug: &'a str,
    template_hash: &'a str,
    model: &'a str,
    comparison_index: u32,
    entity_a_id: &'a str,
    entity_b_id: &'a str,
    entity_a_hash: String,
    entity_b_hash: String,
    swapped: u8,
    cached: u8,
    refused: u8,
    higher_ranked: &'a str,
    ratio: Option<f64>,
    confidence: Option<f64>,
    /// PMF-derived signed log-ratio moments (presented A-over-B) from the
    /// provider's decision-token logprobs — present only on evidence-rail
    /// rows (ratio_letter*/ordinal_letter with a logprob route). These are
    /// the measurements the solver weights by; persisting them keeps the
    /// elicited distribution auditable per row.
    #[serde(skip_serializing_if = "Option::is_none")]
    log_ratio_mean: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    log_ratio_var: Option<f64>,
    input_tokens: u32,
    output_tokens: u32,
    cost_nanodollars: u64,
    cost_is_estimate: u8,
    error: &'a str,
    temperature: f64,
    harness_version: String,
}

#[derive(Serialize)]
struct ScoreRow<'a> {
    scored_at: String,
    run_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    owner_scope: Option<&'a str>,
    lens: &'a str,
    axis_key: &'a str,
    axis_prompt: &'a str,
    axis_prompt_hash: &'a str,
    harness: &'a str,
    model: &'a str,
    seed: u64,
    item_count: u32,
    comparison_budget: u32,
    comparisons_used: u32,
    stop_reason: &'static str,
    topk_error: f64,
    run_cost_nanodollars: u64,
    entity_id: &'a str,
    entity_text: &'a str,
    rank: u32,
    latent_mean: f64,
    latent_std: f64,
    z_score: f64,
    percentile: f64,
    temperature: f64,
    harness_version: String,
    entity_hash: String,
}

fn completed_batches(
    record: &JudgementRunRecord,
    lens: &str,
    owner_scope: &str,
) -> Result<Vec<LandingBatch>, String> {
    let JudgementRunTerminal::Completed {
        stop_reason,
        response,
    } = &record.terminal
    else {
        return Err("run is not completed".to_string());
    };

    let private = record.request.privacy == JudgementPrivacy::Private;
    let row_owner_scope = private.then_some(owner_scope);
    let axis_prompt_hash = sha256_hex(&record.request.axis_prompt);
    let harness = record
        .provenance
        .as_ref()
        .map(|provenance| provenance.harness.as_str())
        .unwrap_or(HARNESS);
    let harness_version = record
        .provenance
        .as_ref()
        .map(|provenance| provenance.harness_version.as_str())
        .unwrap_or(env!("CARGO_PKG_VERSION"));
    let landing_model = record
        .provenance
        .as_ref()
        .map(|provenance| provenance.model.as_str())
        .unwrap_or(&record.request.model);
    let entities: HashMap<&str, &str> = record
        .request
        .entities
        .iter()
        .map(|entity| (entity.id.as_str(), entity.text.as_str()))
        .collect();

    let mut comparison_rows = Vec::with_capacity(record.comparison_trace.len());
    for trace in &record.comparison_trace {
        let entity_a_text = entities
            .get(trace.entity_a_id.as_str())
            .ok_or_else(|| format!("trace references unknown entity {}", trace.entity_a_id))?;
        let entity_b_text = entities
            .get(trace.entity_b_id.as_str())
            .ok_or_else(|| format!("trace references unknown entity {}", trace.entity_b_id))?;
        comparison_rows.push(ComparisonRow {
            observed_at: clickhouse_time(
                DateTime::from_timestamp_millis(trace.timestamp_ms).unwrap_or(record.finished_at),
                3,
            ),
            run_id: &record.run_ref,
            owner_scope: row_owner_scope,
            lens,
            axis_key: &record.request.axis_key,
            axis_prompt: &record.request.axis_prompt,
            axis_prompt_hash: &axis_prompt_hash,
            harness,
            template_slug: &trace.prompt_template_slug,
            template_hash: &trace.template_hash,
            model: &trace.model,
            comparison_index: u32::try_from(trace.comparison_index)
                .map_err(|_| "comparison index exceeds UInt32".to_string())?,
            entity_a_id: &trace.entity_a_id,
            entity_b_id: &trace.entity_b_id,
            entity_a_hash: sha256_hex(entity_a_text),
            entity_b_hash: sha256_hex(entity_b_text),
            swapped: u8::from(trace.swapped),
            cached: u8::from(trace.cached),
            refused: u8::from(trace.refused),
            higher_ranked: trace.higher_ranked.as_deref().unwrap_or(""),
            ratio: trace.ratio,
            confidence: trace.confidence,
            log_ratio_mean: trace
                .pairwise_logprob_posterior
                .as_ref()
                .and_then(|p| p.mean_signed_ln_ratio()),
            log_ratio_var: trace
                .pairwise_logprob_posterior
                .as_ref()
                .and_then(|p| p.variance_signed_ln_ratio()),
            input_tokens: trace.input_tokens,
            output_tokens: trace.output_tokens,
            cost_nanodollars: nonnegative_cost(trace.provider_cost_nanodollars),
            cost_is_estimate: u8::from(trace.provider_cost_is_estimate),
            error: trace.error.as_deref().unwrap_or(""),
            temperature: f64::from(DEFAULT_CHAT_TEMPERATURE),
            harness_version: harness_version.to_string(),
        });
    }

    let seed = record
        .instrument
        .rng_seed
        .ok_or_else(|| "completed run omitted its RNG seed".to_string())?;
    let comparison_budget = record
        .instrument
        .rerank_request
        .comparison_budget
        .ok_or_else(|| "completed run omitted its comparison budget".to_string())?;
    let comparisons_used = record
        .comparison_trace
        .iter()
        .filter(|trace| trace.solver_observation.is_some())
        .count();
    let mut score_rows = Vec::with_capacity(response.entities.len());
    for (position, score) in response.entities.iter().enumerate() {
        let entity_text = entities
            .get(score.id.as_str())
            .ok_or_else(|| format!("score references unknown entity {}", score.id))?;
        score_rows.push(ScoreRow {
            scored_at: clickhouse_time(record.finished_at, 3),
            run_id: &record.run_ref,
            owner_scope: row_owner_scope,
            lens,
            axis_key: &record.request.axis_key,
            axis_prompt: &record.request.axis_prompt,
            axis_prompt_hash: &axis_prompt_hash,
            harness,
            model: landing_model,
            seed,
            item_count: u32::try_from(record.request.entities.len())
                .map_err(|_| "item count exceeds UInt32".to_string())?,
            comparison_budget: u32::try_from(comparison_budget)
                .map_err(|_| "comparison budget exceeds UInt32".to_string())?,
            comparisons_used: u32::try_from(comparisons_used)
                .map_err(|_| "comparisons used exceeds UInt32".to_string())?,
            stop_reason: stop_reason_name(*stop_reason),
            topk_error: response.global_topk_error,
            run_cost_nanodollars: nonnegative_cost(record.usage.provider_cost_nanodollars),
            entity_id: &score.id,
            entity_text,
            rank: u32::try_from(score.rank.unwrap_or(position + 1))
                .map_err(|_| "entity rank exceeds UInt32".to_string())?,
            latent_mean: score.attribute_score.latent_mean,
            latent_std: score.attribute_score.latent_std,
            z_score: score.attribute_score.z_score,
            percentile: score.attribute_score.percentile,
            temperature: f64::from(DEFAULT_CHAT_TEMPERATURE),
            harness_version: harness_version.to_string(),
            entity_hash: sha256_hex(entity_text),
        });
    }

    // A batch with zero rows has nothing to land: reqwest sends an empty
    // body without Content-Length and ClickHouse answers 411 Length
    // Required, parking the batch in landing_pending forever (bit live
    // 2026-07-28: a degenerate 0-comparison run minted a zero-byte
    // comparisons batch that failed every replay). Emit only nonempty
    // batches.
    Ok(vec![
        LandingBatch {
            table: if private {
                PRIVATE_COMPARISONS
            } else {
                PUBLIC_COMPARISONS
            },
            body: json_lines(&comparison_rows)?,
        },
        LandingBatch {
            table: if private {
                PRIVATE_SCORES
            } else {
                PUBLIC_SCORES
            },
            body: json_lines(&score_rows)?,
        },
    ]
    .into_iter()
    .filter(|batch| !batch.body.is_empty())
    .collect())
}

fn json_lines<T: Serialize>(rows: &[T]) -> Result<Vec<u8>, String> {
    let mut body = Vec::new();
    for row in rows {
        serde_json::to_writer(&mut body, row)
            .map_err(|error| format!("could not serialize JSONEachRow: {error}"))?;
        body.push(b'\n');
    }
    Ok(body)
}

fn clickhouse_time(value: DateTime<Utc>, precision: usize) -> String {
    match precision {
        3 => value.format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
        _ => value.format("%Y-%m-%d %H:%M:%S%.6f").to_string(),
    }
}

fn sha256_hex(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

fn nonnegative_cost(value: i64) -> u64 {
    u64::try_from(value).unwrap_or(0)
}

fn stop_reason_name(reason: llmsort::rerank::RerankStopReason) -> &'static str {
    use llmsort::rerank::RerankStopReason;
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

fn columns_for_table(table: &str) -> Option<&'static str> {
    match table {
        PUBLIC_COMPARISONS => Some(COMPARISON_COLUMNS),
        PRIVATE_COMPARISONS => Some(PRIVATE_COMPARISON_COLUMNS),
        PUBLIC_SCORES => Some(SCORE_COLUMNS),
        PRIVATE_SCORES => Some(PRIVATE_SCORE_COLUMNS),
        _ => None,
    }
}

fn pending_path(root: &Path, run_ref: &str, table: &str) -> PathBuf {
    root.join(format!(
        "{run_ref}.landing_pending_{}.jsonl",
        table.replace('.', "__")
    ))
}

fn pending_descriptor(path: &Path) -> Option<(&str, &'static str)> {
    let file_name = path.file_name()?.to_str()?;
    let (run_ref, encoded) = file_name.split_once(".landing_pending_")?;
    if run_ref.is_empty() {
        return None;
    }
    let table = match encoded.strip_suffix(".jsonl")? {
        "scry_judgements__comparisons" => Some(PUBLIC_COMPARISONS),
        "scry_judgements__scores" => Some(PUBLIC_SCORES),
        "scry_judgements_private__comparisons" => Some(PRIVATE_COMPARISONS),
        "scry_judgements_private__scores" => Some(PRIVATE_SCORES),
        _ => None,
    }?;
    Some((run_ref, table))
}

fn pending_files(root: &Path) -> io::Result<Vec<PathBuf>> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    Ok(entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| pending_descriptor(path).is_some())
        .collect())
}

fn write_pending(path: &Path, body: &[u8]) -> io::Result<()> {
    if path.exists() {
        return Ok(());
    }
    let root = path
        .parent()
        .ok_or_else(|| io::Error::other("pending path has no parent"))?;
    fs::create_dir_all(root)?;
    let mut temporary = tempfile::NamedTempFile::new_in(root)?;
    {
        let mut writer = BufWriter::new(temporary.as_file_mut());
        writer.write_all(body)?;
        writer.flush()?;
    }
    temporary.as_file().sync_all()?;
    match temporary.persist_noclobber(path) {
        Ok(_) => File::open(root)?.sync_all(),
        Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(error.error),
    }
}

fn percent_decode(value: &str) -> Result<String, String> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return Err("invalid percent encoding in URL credentials".to_string());
            }
            let high = hex_value(bytes[index + 1])?;
            let low = hex_value(bytes[index + 2])?;
            decoded.push((high << 4) | low);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).map_err(|_| "URL credentials are not valid UTF-8".to_string())
}

fn hex_value(value: u8) -> Result<u8, String> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => Err("invalid percent encoding in URL credentials".to_string()),
    }
}
