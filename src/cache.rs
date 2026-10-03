//! Cache for pairwise LLM judgements.
//!
//! The `PairwiseCache` trait and key types are always available.
//! Enable the `sqlite-store` feature for the `SqlitePairwiseCache` implementation.

use async_trait::async_trait;
use blake3;
#[cfg(feature = "sqlite-store")]
use fs2::FileExt;
#[cfg(feature = "sqlite-store")]
use rusqlite::{params, Connection};
#[cfg(feature = "sqlite-store")]
use serde::Serialize;
#[cfg(feature = "sqlite-store")]
use std::path::{Path, PathBuf};
#[cfg(feature = "sqlite-store")]
use std::sync::{Arc, Mutex};
#[cfg(feature = "sqlite-store")]
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct PairwiseCacheKey {
    pub model: String,
    pub prompt_template_slug: String,
    pub template_hash: String,
    pub attribute_id: String,
    pub attribute_prompt_hash: String,
    pub entity_a_id: String,
    pub entity_b_id: String,
    pub entity_a_hash: String,
    pub entity_b_hash: String,
    pub key_hash: String,
}

#[derive(Debug, Clone, Copy)]
pub struct PairwiseCacheEntity<'a> {
    pub id: &'a str,
    pub text: &'a str,
}

#[derive(Debug, Clone, Copy)]
pub struct PairwiseCacheAttribute<'a> {
    pub id: &'a str,
    pub prompt: &'a str,
}

#[derive(Debug, Clone, Copy)]
pub struct PairwiseCacheTemplate<'a> {
    pub slug: &'a str,
    pub template_hash: &'a str,
}

#[derive(Debug, Clone, Copy)]
pub struct PairwiseCacheKeyParts<'a> {
    pub model: &'a str,
    pub prompt_template: PairwiseCacheTemplate<'a>,
    pub attribute: PairwiseCacheAttribute<'a>,
    pub entity_a: PairwiseCacheEntity<'a>,
    pub entity_b: PairwiseCacheEntity<'a>,
}

impl PairwiseCacheKey {
    pub fn from_parts(parts: PairwiseCacheKeyParts<'_>) -> Self {
        let attribute_prompt_hash = hash_text(parts.attribute.prompt);
        let entity_a_hash = hash_text(parts.entity_a.text);
        let entity_b_hash = hash_text(parts.entity_b.text);
        let key_hash = hash_fields(&[
            parts.model,
            parts.prompt_template.slug,
            parts.prompt_template.template_hash,
            parts.attribute.id,
            &attribute_prompt_hash,
            parts.entity_a.id,
            &entity_a_hash,
            parts.entity_b.id,
            &entity_b_hash,
        ]);

        Self {
            model: parts.model.to_string(),
            prompt_template_slug: parts.prompt_template.slug.to_string(),
            template_hash: parts.prompt_template.template_hash.to_string(),
            attribute_id: parts.attribute.id.to_string(),
            attribute_prompt_hash,
            entity_a_id: parts.entity_a.id.to_string(),
            entity_b_id: parts.entity_b.id.to_string(),
            entity_a_hash,
            entity_b_hash,
            key_hash,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        model: &str,
        prompt_template_slug: &str,
        template_hash: &str,
        attribute_id: &str,
        attribute_prompt: &str,
        entity_a_id: &str,
        entity_a_text: &str,
        entity_b_id: &str,
        entity_b_text: &str,
    ) -> Self {
        Self::from_parts(PairwiseCacheKeyParts {
            model,
            prompt_template: PairwiseCacheTemplate {
                slug: prompt_template_slug,
                template_hash,
            },
            attribute: PairwiseCacheAttribute {
                id: attribute_id,
                prompt: attribute_prompt,
            },
            entity_a: PairwiseCacheEntity {
                id: entity_a_id,
                text: entity_a_text,
            },
            entity_b: PairwiseCacheEntity {
                id: entity_b_id,
                text: entity_b_text,
            },
        })
    }
}

#[derive(Debug, Clone)]
pub struct CachedJudgement {
    pub higher_ranked: Option<String>,
    pub ratio: Option<f64>,
    pub confidence: Option<f64>,
    pub refused: bool,
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
    pub provider_cost_nanodollars: Option<i64>,
    /// PMF-derived signed log-ratio mean (presented A-over-B coordinates);
    /// present only for evidence-mode judgements.
    pub log_ratio_mean: Option<f64>,
    /// PMF-derived log-ratio variance.
    pub log_ratio_var: Option<f64>,
    /// Probability mass visible at the answer position.
    pub visible_mass: Option<f64>,
    /// Whether the stored moments came from a logprob-mode call (true) or
    /// a sampled/frequency fallback (false). `None` for rows written
    /// before this column existed; readers infer from `visible_mass`.
    pub logprob_mode: Option<bool>,
    /// Credal-envelope bounds on the signed log-ratio and the conservation
    /// gap (decimal-ledger evidence only; `None` elsewhere). Persisted so
    /// the certificate survives the moment collapse — post-solve interval
    /// audits can ask whether adversarial resolution of unattributed mass
    /// could flip a ranking (coherence review F5, 2026-08-11).
    pub e_lo: Option<f64>,
    pub e_hi: Option<f64>,
    pub conservation_gap: Option<f64>,
}

#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[cfg(feature = "sqlite-store")]
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("cache lock poisoned")]
    Poisoned,
    #[error("task join error: {0}")]
    Join(String),
    #[error("serialization error: {0}")]
    Serde(String),
}

#[async_trait]
pub trait PairwiseCache: Send + Sync {
    async fn get(&self, key: &PairwiseCacheKey) -> Result<Option<CachedJudgement>, CacheError>;
    async fn put(&self, key: &PairwiseCacheKey, value: &CachedJudgement) -> Result<(), CacheError>;
}

#[cfg(feature = "sqlite-store")]
#[derive(Clone)]
pub struct SqlitePairwiseCache {
    path: PathBuf,
    conn: Arc<Mutex<Connection>>,
}

#[cfg(feature = "sqlite-store")]
impl SqlitePairwiseCache {
    pub fn new(path: impl AsRef<Path>) -> Result<Self, CacheError> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let conn = Connection::open(&path)?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;\
             PRAGMA synchronous=NORMAL;\
             CREATE TABLE IF NOT EXISTS pairwise_cache (\
               key_hash TEXT PRIMARY KEY,\
               model TEXT NOT NULL,\
               prompt_template_slug TEXT NOT NULL,\
               template_hash TEXT NOT NULL,\
               attribute_id TEXT NOT NULL,\
               attribute_prompt_hash TEXT NOT NULL,\
               entity_a_id TEXT NOT NULL,\
               entity_b_id TEXT NOT NULL,\
               entity_a_hash TEXT NOT NULL,\
               entity_b_hash TEXT NOT NULL,\
               higher_ranked TEXT,\
               ratio REAL,\
               confidence REAL,\
               refused INTEGER NOT NULL,\
               input_tokens INTEGER,\
               output_tokens INTEGER,\
               provider_cost_nanodollars INTEGER,\
               created_at INTEGER NOT NULL,\
               updated_at INTEGER NOT NULL,\
               hit_count INTEGER NOT NULL DEFAULT 0\
             );",
        )?;
        ensure_column(&conn, "template_hash", "TEXT NOT NULL DEFAULT ''")?;
        // Evidence-moment columns (PMF-derived judgements, seriate path).
        ensure_column(&conn, "log_ratio_mean", "REAL")?;
        ensure_column(&conn, "log_ratio_var", "REAL")?;
        ensure_column(&conn, "visible_mass", "REAL")?;
        ensure_column(&conn, "logprob_mode", "INTEGER")?;
        // Credal certificate columns (decimal-ledger evidence).
        ensure_column(&conn, "e_lo", "REAL")?;
        ensure_column(&conn, "e_hi", "REAL")?;
        ensure_column(&conn, "conservation_gap", "REAL")?;

        Ok(Self {
            path,
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn default_path() -> PathBuf {
        PathBuf::from(".cardinal_pairwise_cache.sqlite")
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn lock_exclusive(&self) -> Result<CacheLock, CacheError> {
        CacheLock::new(&self.path)
    }

    fn with_conn<F, R>(&self, f: F) -> Result<R, CacheError>
    where
        F: FnOnce(&Connection) -> Result<R, CacheError>,
    {
        let guard = self.conn.lock().map_err(|_| CacheError::Poisoned)?;
        f(&guard)
    }
}

#[cfg(feature = "sqlite-store")]
fn ensure_column(conn: &Connection, name: &str, spec: &str) -> Result<(), CacheError> {
    let mut stmt = conn.prepare("PRAGMA table_info(pairwise_cache)")?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let col_name: String = row.get(1)?;
        if col_name == name {
            return Ok(());
        }
    }
    let sql = format!("ALTER TABLE pairwise_cache ADD COLUMN {name} {spec}");
    conn.execute(&sql, [])?;
    Ok(())
}

#[cfg(feature = "sqlite-store")]
#[async_trait]
impl PairwiseCache for SqlitePairwiseCache {
    async fn get(&self, key: &PairwiseCacheKey) -> Result<Option<CachedJudgement>, CacheError> {
        let key_hash = key.key_hash.clone();
        let conn = self.clone();
        tokio::task::spawn_blocking(move || {
            conn.with_conn(|conn| {
                let mut stmt = conn.prepare(
                    "SELECT higher_ranked, ratio, confidence, refused, input_tokens, output_tokens,\
                            provider_cost_nanodollars, log_ratio_mean, log_ratio_var, visible_mass,\
                            logprob_mode, e_lo, e_hi, conservation_gap \
                     FROM pairwise_cache WHERE key_hash = ?1",
                )?;
                let mut rows = stmt.query(params![key_hash])?;
                if let Some(row) = rows.next()? {
                    let entry = CachedJudgement {
                        higher_ranked: row.get::<_, Option<String>>(0)?,
                        ratio: row.get::<_, Option<f64>>(1)?,
                        confidence: row.get::<_, Option<f64>>(2)?,
                        refused: row.get::<_, i64>(3)? != 0,
                        input_tokens: row.get::<_, Option<i64>>(4)?.map(|v| v as u32),
                        output_tokens: row.get::<_, Option<i64>>(5)?.map(|v| v as u32),
                        provider_cost_nanodollars: row.get::<_, Option<i64>>(6)?,
                        log_ratio_mean: row.get::<_, Option<f64>>(7)?,
                        log_ratio_var: row.get::<_, Option<f64>>(8)?,
                        visible_mass: row.get::<_, Option<f64>>(9)?,
                        logprob_mode: row.get::<_, Option<i64>>(10)?.map(|v| v != 0),
                        e_lo: row.get::<_, Option<f64>>(11)?,
                        e_hi: row.get::<_, Option<f64>>(12)?,
                        conservation_gap: row.get::<_, Option<f64>>(13)?,
                    };
                    conn.execute(
                        "UPDATE pairwise_cache \
                         SET hit_count = hit_count + 1, updated_at = ?1\
                         WHERE key_hash = ?2",
                        params![now_epoch(), key_hash],
                    )?;
                    Ok(Some(entry))
                } else {
                    Ok(None)
                }
            })
        })
        .await
        .map_err(|e| CacheError::Join(e.to_string()))?
    }

    async fn put(&self, key: &PairwiseCacheKey, value: &CachedJudgement) -> Result<(), CacheError> {
        let key = key.clone();
        let value = value.clone();
        let conn = self.clone();
        tokio::task::spawn_blocking(move || {
            conn.with_conn(|conn| {
                let now = now_epoch();
                conn.execute(
                    "INSERT INTO pairwise_cache (\
                        key_hash, model, prompt_template_slug, template_hash, attribute_id, attribute_prompt_hash,\
                        entity_a_id, entity_b_id, entity_a_hash, entity_b_hash,\
                        higher_ranked, ratio, confidence, refused,\
                        input_tokens, output_tokens, provider_cost_nanodollars,\
                        created_at, updated_at, log_ratio_mean, log_ratio_var, visible_mass, logprob_mode,\
                        e_lo, e_hi, conservation_gap\
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26)\
                     ON CONFLICT(key_hash) DO UPDATE SET \
                        higher_ranked = excluded.higher_ranked,\
                        ratio = excluded.ratio,\
                        confidence = excluded.confidence,\
                        refused = excluded.refused,\
                        input_tokens = excluded.input_tokens,\
                        output_tokens = excluded.output_tokens,\
                        provider_cost_nanodollars = excluded.provider_cost_nanodollars,\
                        updated_at = excluded.updated_at,\
                        log_ratio_mean = excluded.log_ratio_mean,\
                        log_ratio_var = excluded.log_ratio_var,\
                        visible_mass = excluded.visible_mass,\
                        logprob_mode = excluded.logprob_mode,\
                        e_lo = excluded.e_lo,\
                        e_hi = excluded.e_hi,\
                        conservation_gap = excluded.conservation_gap",
                    params![
                        key.key_hash,
                        key.model,
                        key.prompt_template_slug,
                        key.template_hash,
                        key.attribute_id,
                        key.attribute_prompt_hash,
                        key.entity_a_id,
                        key.entity_b_id,
                        key.entity_a_hash,
                        key.entity_b_hash,
                        value.higher_ranked,
                        value.ratio,
                        value.confidence,
                        if value.refused { 1 } else { 0 },
                        value.input_tokens.map(|v| v as i64),
                        value.output_tokens.map(|v| v as i64),
                        value.provider_cost_nanodollars,
                        now,
                        now,
                        value.log_ratio_mean,
                        value.log_ratio_var,
                        value.visible_mass,
                        value.logprob_mode.map(i64::from),
                        value.e_lo,
                        value.e_hi,
                        value.conservation_gap,
                    ],
                )?;
                Ok(())
            })
        })
        .await
        .map_err(|e| CacheError::Join(e.to_string()))?
    }
}

#[cfg(feature = "sqlite-store")]
#[derive(Debug)]
pub struct CacheLock {
    _file: std::fs::File,
}

#[cfg(feature = "sqlite-store")]
impl CacheLock {
    fn new(db_path: &Path) -> Result<Self, CacheError> {
        let mut lock_path = db_path.to_path_buf();
        lock_path.set_extension("lock");
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(lock_path)?;
        file.lock_exclusive()?;
        Ok(Self { _file: file })
    }
}

#[cfg(feature = "sqlite-store")]
#[derive(Debug, Serialize)]
pub struct CacheExportRow {
    pub key_hash: String,
    pub model: String,
    pub prompt_template_slug: String,
    pub template_hash: String,
    pub attribute_id: String,
    pub attribute_prompt_hash: String,
    pub entity_a_id: String,
    pub entity_b_id: String,
    pub entity_a_hash: String,
    pub entity_b_hash: String,
    pub higher_ranked: Option<String>,
    pub ratio: Option<f64>,
    pub confidence: Option<f64>,
    pub refused: bool,
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
    pub provider_cost_nanodollars: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
    pub hit_count: i64,
}

#[cfg(feature = "sqlite-store")]
#[derive(Debug, Clone, Serialize)]
pub struct CachePruneStats {
    pub deleted: usize,
    pub remaining: usize,
}

#[cfg(feature = "sqlite-store")]
impl SqlitePairwiseCache {
    pub async fn export_jsonl(&self, path: impl AsRef<Path>) -> Result<(), CacheError> {
        let path = path.as_ref().to_path_buf();
        let conn = self.clone();
        tokio::task::spawn_blocking(move || {
            conn.with_conn(|conn| {
                let mut stmt = conn.prepare(
                    "SELECT key_hash, model, prompt_template_slug, template_hash, attribute_id, attribute_prompt_hash,\n                            entity_a_id, entity_b_id, entity_a_hash, entity_b_hash,\n                            higher_ranked, ratio, confidence, refused,\n                            input_tokens, output_tokens, provider_cost_nanodollars,\n                            created_at, updated_at, hit_count\n                     FROM pairwise_cache ORDER BY updated_at DESC",
                )?;
                let mut rows = stmt.query([])?;
                let mut file = std::fs::File::create(path)?;
                while let Some(row) = rows.next()? {
                    let record = CacheExportRow {
                        key_hash: row.get(0)?,
                        model: row.get(1)?,
                        prompt_template_slug: row.get(2)?,
                        template_hash: row.get(3)?,
                        attribute_id: row.get(4)?,
                        attribute_prompt_hash: row.get(5)?,
                        entity_a_id: row.get(6)?,
                        entity_b_id: row.get(7)?,
                        entity_a_hash: row.get(8)?,
                        entity_b_hash: row.get(9)?,
                        higher_ranked: row.get(10)?,
                        ratio: row.get(11)?,
                        confidence: row.get(12)?,
                        refused: row.get::<_, i64>(13)? != 0,
                        input_tokens: row.get::<_, Option<i64>>(14)?.map(|v| v as u32),
                        output_tokens: row.get::<_, Option<i64>>(15)?.map(|v| v as u32),
                        provider_cost_nanodollars: row.get(16)?,
                        created_at: row.get(17)?,
                        updated_at: row.get(18)?,
                        hit_count: row.get(19)?,
                    };
                    let line = serde_json::to_string(&record)
                        .map_err(|e| CacheError::Serde(e.to_string()))?;
                    use std::io::Write;
                    writeln!(file, "{line}")?;
                }
                Ok(())
            })
        })
        .await
        .map_err(|e| CacheError::Join(e.to_string()))?
    }

    pub async fn prune(
        &self,
        max_age_days: Option<u64>,
        max_rows: Option<usize>,
    ) -> Result<CachePruneStats, CacheError> {
        let conn = self.clone();
        tokio::task::spawn_blocking(move || {
            conn.with_conn(|conn| {
                let mut deleted: usize = 0;
                if let Some(days) = max_age_days {
                    let cutoff = now_epoch().saturating_sub((days as i64).saturating_mul(86_400));
                    let removed = conn.execute(
                        "DELETE FROM pairwise_cache WHERE updated_at < ?1",
                        params![cutoff],
                    )?;
                    deleted = deleted.saturating_add(removed);
                }

                if let Some(max_rows) = max_rows {
                    if max_rows == 0 {
                        return Ok(CachePruneStats {
                            deleted,
                            remaining: 0,
                        });
                    }
                    let count: i64 =
                        conn.query_row("SELECT COUNT(*) FROM pairwise_cache", [], |row| {
                            row.get(0)
                        })?;
                    let keep = max_rows as i64;
                    if count > keep {
                        let removed = conn.execute(
                            "DELETE FROM pairwise_cache WHERE key_hash IN (\
                                SELECT key_hash FROM pairwise_cache \
                                ORDER BY updated_at DESC LIMIT -1 OFFSET ?1\
                             )",
                            params![keep],
                        )?;
                        deleted = deleted.saturating_add(removed);
                    }
                }

                let remaining: i64 =
                    conn.query_row("SELECT COUNT(*) FROM pairwise_cache", [], |row| row.get(0))?;
                Ok(CachePruneStats {
                    deleted,
                    remaining: remaining.max(0) as usize,
                })
            })
        })
        .await
        .map_err(|e| CacheError::Join(e.to_string()))?
    }
}

fn hash_text(text: &str) -> String {
    blake3::hash(text.as_bytes()).to_hex().to_string()
}

fn hash_fields(fields: &[&str]) -> String {
    let mut hasher = blake3::Hasher::new();
    for (idx, field) in fields.iter().enumerate() {
        if idx > 0 {
            hasher.update(b"|");
        }
        hasher.update(field.as_bytes());
    }
    hasher.finalize().to_hex().to_string()
}

#[cfg(feature = "sqlite-store")]
fn now_epoch() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
