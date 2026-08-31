//! Whitespace-jitter probe battery: K quasi-independent draws of the same
//! structured judgement.
//!
//! The problem this solves: the pairwise cache is content-addressed, so an
//! identical prompt can only ever hold ONE judgement — repeat elicitation
//! of the same pair collides into the cached answer, and even uncached,
//! byte-identical requests can be served correlated samples (provider
//! caching, deterministic decoding). `repeat_pooling` (DerSimonian–Laird
//! with a heterogeneity floor) has been waiting for genuinely distinct
//! draws to pool.
//!
//! The draw generator: deterministic whitespace jitter. Probe `k` widens a
//! few seed-chosen word gaps inside the attribute prompt from one space to
//! two or three. The attribute text is substituted into the rendered
//! prompt at several points BEFORE the elicitation instruction (system and
//! user), so each probe perturbs the token stream at various positions
//! while leaving semantics untouched — and because the cache keys on the
//! attribute-prompt hash, every probe is a distinct, replayable,
//! individually cached judgement.
//!
//! What the battery measures, per pair, across K probes:
//! - draw dispersion (mean, sd, spread) of the signed log-ratio;
//! - sign instability (how often probes disagree on direction);
//! - duplicate rate (probes returning the identical value — K identical
//!   draws means jitter found a deterministic judge, and repeats at this
//!   temperature buy nothing);
//!
//! and pooled across the graph: σ_w² (within-pair), the DL σ_b²
//! heterogeneity floor, and the naive-vs-floored solves.

use std::collections::HashMap;

use futures::stream::{self, StreamExt};
use serde::Serialize;

use llmsort::cache::PairwiseCache;
use llmsort::gateway::{Attribution, ChatGateway};
use llmsort::repeat_pooling::{pool_repeats, RepeatDraws};
use llmsort::rerank::comparison::{
    compare_pair, ComparisonError, PairwiseComparisonAttribute, PairwiseComparisonEntity,
    PairwiseComparisonRequest, PairwiseComparisonSpec,
};
use llmsort::rerank::sort::spearman;

/// Deterministic whitespace jitter. Probe 0 is the identity. Probe `k >= 1`
/// widens `1 + (k-1) % 3` single-space word gaps to two or three spaces;
/// gap positions and widths come from `blake3(text, k)`, so the variant is
/// a pure function of `(text, k)` — replayable across runs and machines.
#[must_use]
pub fn whitespace_jitter(text: &str, probe: u32) -> String {
    if probe == 0 {
        return text.to_string();
    }
    let bytes = text.as_bytes();
    // Single-space gaps only (never widen inside an existing run).
    let gaps: Vec<usize> = (0..bytes.len())
        .filter(|&i| {
            bytes[i] == b' '
                && (i == 0 || bytes[i - 1] != b' ')
                && (i + 1 == bytes.len() || bytes[i + 1] != b' ')
        })
        .collect();
    if gaps.is_empty() {
        // Degenerate input: no interior gaps to widen — pad the tail.
        return format!("{text}{}", " ".repeat(probe as usize));
    }
    let mut hasher = blake3::Hasher::new();
    hasher.update(text.as_bytes());
    hasher.update(&probe.to_le_bytes());
    let seed = hasher.finalize();
    let seed = seed.as_bytes();

    let want = usize::min(1 + ((probe as usize - 1) % 3), gaps.len());
    let mut chosen: Vec<(usize, usize)> = Vec::with_capacity(want); // (gap byte idx, extra spaces)
    let mut used = vec![false; gaps.len()];
    for m in 0..want {
        let raw = u32::from_le_bytes([
            seed[(4 * m) % 32],
            seed[(4 * m + 1) % 32],
            seed[(4 * m + 2) % 32],
            seed[(4 * m + 3) % 32],
        ]) as usize;
        let mut idx = raw % gaps.len();
        while used[idx] {
            idx = (idx + 1) % gaps.len();
        }
        used[idx] = true;
        let extra = 1 + ((seed[(m + 17) % 32] as usize) % 2); // +1 or +2 spaces
        chosen.push((gaps[idx], extra));
    }
    chosen.sort_unstable();

    let mut out = String::with_capacity(text.len() + 8);
    let mut cursor = 0usize;
    for (pos, extra) in chosen {
        out.push_str(&text[cursor..=pos]);
        for _ in 0..extra {
            out.push(' ');
        }
        cursor = pos + 1;
    }
    out.push_str(&text[cursor..]);
    out
}

/// Options for [`run_probe_battery`].
#[derive(Debug, Clone)]
pub struct ProbeBatteryOptions {
    /// Model slug (OpenRouter).
    pub model: String,
    /// Prompt template slug.
    pub template: String,
    /// Probes per pair (K). Probe 0 is the unjittered prompt.
    pub probes: u32,
    /// Concurrent comparisons.
    pub concurrency: usize,
}

impl Default for ProbeBatteryOptions {
    fn default() -> Self {
        Self {
            model: String::new(),
            template: "canonical_v2".to_string(),
            probes: 8,
            concurrency: 6,
        }
    }
}

/// Per-pair probe record.
#[derive(Debug, Clone, Serialize)]
pub struct PairProbes {
    pub i: usize,
    pub j: usize,
    /// Signed log-ratios toward `i`, one per non-refused probe, probe order.
    pub draws: Vec<f64>,
    pub refusals: usize,
    pub mean: f64,
    /// Sample standard deviation of the draws (0 when < 2 draws).
    pub sd: f64,
    /// max − min of the draws.
    pub spread: f64,
    /// Draws whose sign disagrees with the majority direction.
    pub sign_minority: usize,
    /// Distinct draw values (1 = the judge answered identically every time).
    pub distinct: usize,
}

/// Battery-level report.
#[derive(Debug, Serialize)]
pub struct ProbeBatteryReport {
    pub model: String,
    pub template: String,
    pub probes: u32,
    pub n_entities: usize,
    pub pairs: Vec<PairProbes>,
    /// Fraction of all draws (beyond each pair's first) identical to that
    /// pair's first draw — 1.0 means jitter never moved the answer.
    pub duplicate_rate: f64,
    /// Pooled within-pair per-draw variance (σ_w²), from the DL solve.
    pub sigma_w2: Option<f64>,
    /// DL between-pair heterogeneity floor (σ_b²).
    pub sigma_b2: Option<f64>,
    pub q_statistic: Option<f64>,
    pub degrees_of_freedom: Option<usize>,
    /// Spearman ρ between the naive-pooled and heterogeneity-floored solves.
    pub naive_vs_floored_spearman: Option<f64>,
    pub comparisons: usize,
    pub comparisons_cached: usize,
    pub cost_nanodollars: i64,
}

struct ProbeOutcome {
    log_ratio_toward_i: Option<f64>,
    cached: bool,
    cost_nanodollars: i64,
}

/// Run the whitespace-jitter probe battery over `entities` and `pairs`.
///
/// Every (pair, probe) cell is one `compare_pair` call with the attribute
/// prompt jittered by [`whitespace_jitter`]; forward order only, so draws
/// are pure repeats (order effects are the bench's axis, not this one).
pub async fn run_probe_battery(
    gateway: &dyn ChatGateway,
    cache: Option<&dyn PairwiseCache>,
    entities: &[String],
    pairs: &[(usize, usize)],
    attribute_prompt: &str,
    opts: ProbeBatteryOptions,
) -> Result<ProbeBatteryReport, ComparisonError> {
    let attribution = Attribution::new("cardinal::probe");
    let variants: Vec<String> = (0..opts.probes)
        .map(|k| whitespace_jitter(attribute_prompt, k))
        .collect();

    let mut plan: Vec<(usize, u32)> = Vec::new();
    for pair_idx in 0..pairs.len() {
        for k in 0..opts.probes {
            plan.push((pair_idx, k));
        }
    }

    let results: Vec<((usize, u32), Result<ProbeOutcome, ComparisonError>)> =
        stream::iter(plan.iter().map(|&(pair_idx, k)| {
            let (i, j) = pairs[pair_idx];
            let attr = variants[k as usize].as_str();
            let model = opts.model.as_str();
            let template = opts.template.as_str();
            let attribution = &attribution;
            let entity_i = entities[i].as_str();
            let entity_j = entities[j].as_str();
            async move {
                let id_a = format!("e{i}");
                let id_b = format!("e{j}");
                let spec = PairwiseComparisonSpec {
                    model,
                    attribute: PairwiseComparisonAttribute {
                        id: "probe",
                        prompt: attr,
                        prompt_template_slug: Some(template),
                    },
                    entity_a: PairwiseComparisonEntity {
                        id: &id_a,
                        text: entity_i,
                    },
                    entity_b: PairwiseComparisonEntity {
                        id: &id_b,
                        text: entity_j,
                    },
                };
                let outcome = compare_pair(
                    gateway,
                    cache,
                    PairwiseComparisonRequest {
                        spec,
                        cache_only: false,
                        attribution: attribution.clone(),
                        nonce: None,
                    },
                )
                .await
                .map(|(judgement, usage)| ProbeOutcome {
                    log_ratio_toward_i: llmsort::rerank::types::signed_log_ratio_toward_first(
                        &judgement, true,
                    ),
                    cached: usage.cached,
                    cost_nanodollars: usage.provider_cost_nanodollars,
                });
                ((pair_idx, k), outcome)
            }
        }))
        .buffer_unordered(opts.concurrency.max(1))
        .collect()
        .await;

    let mut per_pair: HashMap<usize, Vec<(u32, Option<f64>)>> = HashMap::new();
    let mut comparisons = 0usize;
    let mut comparisons_cached = 0usize;
    let mut cost = 0i64;
    for ((pair_idx, k), res) in results {
        let outcome = res?;
        comparisons += 1;
        if outcome.cached {
            comparisons_cached += 1;
        }
        cost += outcome.cost_nanodollars;
        per_pair
            .entry(pair_idx)
            .or_default()
            .push((k, outcome.log_ratio_toward_i));
    }

    let mut pair_reports: Vec<PairProbes> = Vec::with_capacity(pairs.len());
    let mut dup_extra = 0usize;
    let mut dup_total = 0usize;
    for (pair_idx, &(i, j)) in pairs.iter().enumerate() {
        let mut cells = per_pair.remove(&pair_idx).unwrap_or_default();
        cells.sort_unstable_by_key(|&(k, _)| k);
        let draws: Vec<f64> = cells.iter().filter_map(|&(_, m)| m).collect();
        let refusals = cells.len() - draws.len();
        let n = draws.len();
        let mean = if n > 0 {
            draws.iter().sum::<f64>() / n as f64
        } else {
            f64::NAN
        };
        let sd = if n > 1 {
            (draws.iter().map(|d| (d - mean).powi(2)).sum::<f64>() / (n as f64 - 1.0)).sqrt()
        } else {
            0.0
        };
        let spread = if n > 0 {
            let max = draws.iter().cloned().fold(f64::MIN, f64::max);
            let min = draws.iter().cloned().fold(f64::MAX, f64::min);
            max - min
        } else {
            0.0
        };
        let pos = draws.iter().filter(|d| **d > 0.0).count();
        let neg = draws.iter().filter(|d| **d < 0.0).count();
        let sign_minority = usize::min(pos, neg);
        let mut distinct_vals: Vec<u64> = draws.iter().map(|d| d.to_bits()).collect();
        distinct_vals.sort_unstable();
        distinct_vals.dedup();
        let distinct = distinct_vals.len();
        if n > 1 {
            dup_total += n - 1;
            dup_extra += draws[1..]
                .iter()
                .filter(|d| d.to_bits() == draws[0].to_bits())
                .count();
        }
        pair_reports.push(PairProbes {
            i,
            j,
            draws,
            refusals,
            mean,
            sd,
            spread,
            sign_minority,
            distinct,
        });
    }

    let repeat_draws: Vec<RepeatDraws> = pair_reports
        .iter()
        .filter(|p| !p.draws.is_empty())
        .map(|p| RepeatDraws {
            i: p.i,
            j: p.j,
            draws: p.draws.clone(),
        })
        .collect();
    let pooled = pool_repeats(entities.len(), &repeat_draws);
    let (sigma_w2, sigma_b2, q_statistic, degrees_of_freedom, naive_vs_floored_spearman) =
        match &pooled {
            Some(p) => (
                Some(p.sigma_w2),
                Some(p.sigma_b2),
                Some(p.q_statistic),
                Some(p.degrees_of_freedom),
                spearman(&p.scores, &p.scores_naive),
            ),
            None => (None, None, None, None, None),
        };

    let duplicate_rate = if dup_total > 0 {
        dup_extra as f64 / dup_total as f64
    } else {
        0.0
    };

    Ok(ProbeBatteryReport {
        model: opts.model,
        template: opts.template,
        probes: opts.probes,
        n_entities: entities.len(),
        pairs: pair_reports,
        duplicate_rate,
        sigma_w2,
        sigma_b2,
        q_statistic,
        degrees_of_freedom,
        naive_vs_floored_spearman,
        comparisons,
        comparisons_cached,
        cost_nanodollars: cost,
    })
}

/// Render a probe battery report as text.
#[must_use]
pub fn render_probe_report(report: &ProbeBatteryReport) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    let _ = writeln!(
        out,
        "probe battery: {} | template {} | K={} probes/pair | {} pairs over {} entities",
        report.model,
        report.template,
        report.probes,
        report.pairs.len(),
        report.n_entities
    );
    let _ = writeln!(
        out,
        "calls {} ({} cached) | cost ${:.4}",
        report.comparisons,
        report.comparisons_cached,
        report.cost_nanodollars as f64 / 1e9
    );
    let _ = writeln!(
        out,
        "duplicate rate {:.0}% (identical to probe 0){}",
        report.duplicate_rate * 100.0,
        if report.duplicate_rate >= 0.999 {
            " — judge is deterministic under jitter; repeats buy nothing here"
        } else {
            ""
        }
    );
    if let (Some(w), Some(b)) = (report.sigma_w2, report.sigma_b2) {
        let _ = writeln!(
            out,
            "pooled: sigma_w2 {:.4} | sigma_b2 (DL floor) {:.4} | Q {:.2} (df {}) | naive-vs-floored rho {}",
            w,
            b,
            report.q_statistic.unwrap_or(f64::NAN),
            report.degrees_of_freedom.unwrap_or(0),
            report
                .naive_vs_floored_spearman
                .map_or("n/a".to_string(), |r| format!("{r:.3}")),
        );
    }
    let _ = writeln!(
        out,
        "pair    draws refuse distinct sign-min      mean        sd    spread"
    );
    for p in &report.pairs {
        let _ = writeln!(
            out,
            "{:>3}-{:<3} {:>5} {:>6} {:>8} {:>8} {:>9.3} {:>9.3} {:>9.3}",
            p.i,
            p.j,
            p.draws.len(),
            p.refusals,
            p.distinct,
            p.sign_minority,
            p.mean,
            p.sd,
            p.spread
        );
    }
    out
}
