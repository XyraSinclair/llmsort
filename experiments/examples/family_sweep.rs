//! E10 — the family sweep instrument (NORTH's native unit of work), live.
//!
//! For each pair, over one prompt prefix, judge by the attribute A, a
//! paraphrase A′ (must correlate), the negation ¬A (must anti-correlate),
//! in BOTH presentation orders (must anti-symmetrize); null pairs
//! (identical items) must read ratio ~1. From the SAME calls: the scaling
//! evidence AND the reliability reading — paraphrase ρ, negation ρ, order
//! residual, null calibration — with the cache economics measured
//! (cached-token fraction, cost per judgement).
//!
//! Two pools: the battery aphorisms (subtle subjective attribute) and the
//! country anchors (truth-anchored: population), so the reliability
//! reading is validated against ground truth where truth exists. Both
//! template orders run — attribute-first (`ratio_letter_v1`) vs
//! attribute-last (`ratio_letter_attrlast_v1`) — because the cached
//! fraction difference between them IS the E10 economics question.
//!
//! Usage: OPENROUTER_API_KEY=... cargo run -p llmsort-experiments \
//!   --example family_sweep -- [model] [pack_dir]
//! Defaults: openai/gpt-4.1-mini, research/artifacts/live/e10-family-sweep-2026-08-29

use std::collections::HashMap;
use std::sync::Arc;

use futures::stream::{self, StreamExt};
use serde::Serialize;

use llmsort::gateway::{Attribution, NoopUsageSink, ProviderGateway};
use llmsort::rerank::comparison::{RATIO_LETTER_ATTR_LAST_SLUG, RATIO_LETTER_SLUG};
use llmsort::rerank::sort::spearman;
use llmsort::rerank::{
    compare_pair, PairwiseComparisonAttribute, PairwiseComparisonEntity, PairwiseComparisonRequest,
    PairwiseComparisonSpec,
};
use llmsort_experiments::{
    ring_stride_pairs, CORPUS, NULL_INDICES, OPPOSITE_ATTRIBUTE, PARAPHRASE_ATTRIBUTE,
    PRIMARY_ATTRIBUTE,
};

const CONCURRENCY: usize = 8;

/// (variant name, canonical sign: negation readings flip).
const VARIANT_SIGNS: [(&str, f64); 3] = [("primary", 1.0), ("paraphrase", 1.0), ("negation", -1.0)];

#[derive(Serialize, Clone)]
struct CallRecord {
    pool: String,
    slug: String,
    i: usize,
    j: usize,
    /// true = (i,j) presented as (A,B); false = swapped.
    order_ij: bool,
    variant: String,
    is_null: bool,
    /// Presented A-over-B signed log-ratio from the PMF, if informative.
    presented_mean: Option<f64>,
    presented_var: Option<f64>,
    visible_mass: Option<f64>,
    logprob_mode: Option<bool>,
    refused: bool,
    input_tokens: u32,
    cache_read_tokens: Option<u32>,
    cost_nanodollars: i64,
}

struct Pool {
    name: &'static str,
    items: Vec<(String, String)>,
    pairs: Vec<(usize, usize)>,
    nulls: Vec<usize>,
    /// variant name -> attribute prompt text
    attrs: Vec<(&'static str, String)>,
    /// item id -> true magnitude (log scale applied later), when truth exists
    truth: Option<HashMap<String, f64>>,
}

fn battery_pool() -> Pool {
    Pool {
        name: "battery",
        items: CORPUS
            .iter()
            .enumerate()
            .map(|(k, t)| (format!("t{k}"), (*t).to_string()))
            .collect(),
        pairs: ring_stride_pairs(CORPUS.len(), &[1, 2, 4]),
        nulls: NULL_INDICES.to_vec(),
        attrs: vec![
            ("primary", PRIMARY_ATTRIBUTE.to_string()),
            ("paraphrase", PARAPHRASE_ATTRIBUTE.to_string()),
            ("negation", OPPOSITE_ATTRIBUTE.to_string()),
        ],
        truth: None,
    }
}

fn countries_pool() -> Result<Pool, Box<dyn std::error::Error>> {
    #[derive(serde::Deserialize)]
    struct Item {
        id: String,
        text: String,
    }
    let items: Vec<Item> = serde_json::from_str(&std::fs::read_to_string(
        "research/data/anchors_countries.json",
    )?)?;
    let truth: HashMap<String, f64> = serde_json::from_str(&std::fs::read_to_string(
        "research/data/anchors_countries_truth.json",
    )?)?;
    Ok(Pool {
        name: "countries",
        pairs: ring_stride_pairs(items.len(), &[1, 2]),
        nulls: Vec::new(),
        items: items.into_iter().map(|it| (it.id, it.text)).collect(),
        attrs: vec![
            (
                "primary",
                "population size: how many people live in it".to_string(),
            ),
            (
                "paraphrase",
                "the number of human inhabitants it has".to_string(),
            ),
            (
                "negation",
                "how few people live in it — the smallness of its population".to_string(),
            ),
        ],
        truth: Some(truth),
    })
}

/// Long-entity pool: 12 arXiv abstracts, so the pair prefix crosses the
/// provider cache floor (~1024 tokens on OpenAI) that the short pools sit
/// under — the cell where the family sweep's cache economics is actually
/// testable.
fn arxiv_pool() -> Result<Pool, Box<dyn std::error::Error>> {
    #[derive(serde::Deserialize)]
    struct Item {
        id: String,
        text: String,
    }
    let items: Vec<Item> = serde_json::from_str(&std::fs::read_to_string(
        "research/data/arxiv_abstracts.json",
    )?)?;
    let items: Vec<(String, String)> = items
        .into_iter()
        .take(12)
        .map(|it| (it.id, it.text))
        .collect();
    Ok(Pool {
        name: "arxiv12",
        pairs: ring_stride_pairs(items.len(), &[1, 2]),
        nulls: Vec::new(),
        items,
        attrs: vec![
            (
                "primary",
                "how methodologically rigorous the work described is".to_string(),
            ),
            (
                "paraphrase",
                "the strength of its experimental and technical rigor".to_string(),
            ),
            (
                "negation",
                "how methodologically loose or hand-wavy the work described is".to_string(),
            ),
        ],
        truth: None,
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let model = args
        .next()
        .unwrap_or_else(|| "openai/gpt-4.1-mini".to_string());
    let pack_dir = args
        .next()
        .unwrap_or_else(|| "research/artifacts/live/e10-family-sweep-2026-08-29".to_string());
    std::fs::create_dir_all(&pack_dir)?;

    let gateway = Arc::new(ProviderGateway::from_env(Arc::new(NoopUsageSink))?);
    let pools = vec![battery_pool(), countries_pool()?, arxiv_pool()?];

    let mut all_records: Vec<CallRecord> = Vec::new();
    for slug in [RATIO_LETTER_ATTR_LAST_SLUG, RATIO_LETTER_SLUG] {
        for pool in &pools {
            let records = run_sweep(&gateway, &model, slug, pool).await;
            summarize(slug, pool, &records);
            all_records.extend(records);
        }
    }

    let path = format!("{pack_dir}/records-{}.json", model.replace('/', "_"));
    std::fs::write(&path, serde_json::to_string_pretty(&all_records)?)?;
    eprintln!("pack: {path} ({} calls)", all_records.len());
    Ok(())
}

/// One unit = (pair, order): the family's three variants SEQUENTIALLY, so
/// the first call warms the prefix the next two are priced against.
async fn run_sweep(
    gateway: &Arc<ProviderGateway<NoopUsageSink>>,
    model: &str,
    slug: &'static str,
    pool: &Pool,
) -> Vec<CallRecord> {
    let mut units: Vec<(usize, usize, bool, bool)> = Vec::new();
    for &(i, j) in &pool.pairs {
        units.push((i, j, true, false));
        units.push((i, j, false, false));
    }
    for &k in &pool.nulls {
        units.push((k, k, true, true));
        units.push((k, k, false, true));
    }

    let results: Vec<Vec<CallRecord>> =
        stream::iter(units.into_iter().map(|(i, j, order_ij, is_null)| {
            let gateway = Arc::clone(gateway);
            let model = model.to_string();
            let pool_name = pool.name.to_string();
            let items = &pool.items;
            let attrs = &pool.attrs;
            async move {
                let mut rows = Vec::with_capacity(attrs.len());
                for (variant, prompt) in attrs {
                    let (slot_a, slot_b) = if order_ij { (i, j) } else { (j, i) };
                    let spec = PairwiseComparisonSpec {
                        model: &model,
                        attribute: PairwiseComparisonAttribute {
                            id: variant,
                            prompt,
                            prompt_template_slug: Some(slug),
                        },
                        entity_a: PairwiseComparisonEntity {
                            id: &items[slot_a].0,
                            text: &items[slot_a].1,
                        },
                        entity_b: PairwiseComparisonEntity {
                            id: &items[slot_b].0,
                            text: &items[slot_b].1,
                        },
                    };
                    let outcome = compare_pair(
                        gateway.as_ref(),
                        None,
                        PairwiseComparisonRequest {
                            spec,
                            cache_only: false,
                            attribution: Attribution::new("cardinal::example::family_sweep"),
                        },
                    )
                    .await;
                    let row = match outcome {
                        Ok((judgement, usage)) => {
                            let m = usage.evidence_moments;
                            CallRecord {
                                pool: pool_name.clone(),
                                slug: slug.to_string(),
                                i,
                                j,
                                order_ij,
                                variant: (*variant).to_string(),
                                is_null,
                                presented_mean: m.map(|e| e.log_ratio_mean),
                                presented_var: m.map(|e| e.log_ratio_var),
                                visible_mass: m.map(|e| e.visible_mass),
                                logprob_mode: m.map(|e| e.logprob_mode),
                                refused: matches!(
                                    judgement,
                                    llmsort::rerank::PairwiseJudgement::Refused
                                ),
                                input_tokens: usage.input_tokens,
                                cache_read_tokens: usage.cache_read_tokens,
                                cost_nanodollars: usage.provider_cost_nanodollars,
                            }
                        }
                        Err(err) => {
                            eprintln!("call failed ({pool_name}/{slug}/{variant} {i}-{j}): {err}");
                            CallRecord {
                                pool: pool_name.clone(),
                                slug: slug.to_string(),
                                i,
                                j,
                                order_ij,
                                variant: (*variant).to_string(),
                                is_null,
                                presented_mean: None,
                                presented_var: None,
                                visible_mass: None,
                                logprob_mode: None,
                                refused: false,
                                input_tokens: 0,
                                cache_read_tokens: None,
                                cost_nanodollars: 0,
                            }
                        }
                    };
                    rows.push(row);
                }
                rows
            }
        }))
        .buffer_unordered(CONCURRENCY)
        .collect()
        .await;
    results.into_iter().flatten().collect()
}

/// Canonical i-over-j reading: order swap reflects, negation flips.
fn canonical(rec: &CallRecord) -> Option<f64> {
    let sign_order = if rec.order_ij { 1.0 } else { -1.0 };
    let sign_variant = VARIANT_SIGNS
        .iter()
        .find(|(v, _)| *v == rec.variant)
        .map(|(_, s)| *s)?;
    rec.presented_mean.map(|m| m * sign_order * sign_variant)
}

fn summarize(slug: &str, pool: &Pool, records: &[CallRecord]) {
    let calls = records.len();
    let cost: i64 = records.iter().map(|r| r.cost_nanodollars).sum();
    let input: u64 = records.iter().map(|r| u64::from(r.input_tokens)).sum();
    let cache_read: u64 = records
        .iter()
        .map(|r| u64::from(r.cache_read_tokens.unwrap_or(0)))
        .sum();
    let refusals = records.iter().filter(|r| r.refused).count();
    let logprob = records
        .iter()
        .filter(|r| r.logprob_mode == Some(true))
        .count();

    // Per-pair canonical mean per variant (non-null pairs).
    let mut by_variant: HashMap<&str, HashMap<(usize, usize), Vec<f64>>> = HashMap::new();
    let mut order_residuals: Vec<f64> = Vec::new();
    for &(i, j) in &pool.pairs {
        for (variant, _) in &VARIANT_SIGNS {
            let both: Vec<&CallRecord> = records
                .iter()
                .filter(|r| !r.is_null && r.i == i && r.j == j && r.variant == *variant)
                .collect();
            let mut presented: Vec<(bool, f64)> = Vec::new();
            for r in &both {
                if let Some(c) = canonical(r) {
                    by_variant
                        .entry(variant)
                        .or_default()
                        .entry((i, j))
                        .or_default()
                        .push(c);
                }
                if let Some(m) = r.presented_mean {
                    presented.push((r.order_ij, m));
                }
            }
            if let (Some(ab), Some(ba)) = (
                presented.iter().find(|(o, _)| *o).map(|(_, m)| *m),
                presented.iter().find(|(o, _)| !*o).map(|(_, m)| *m),
            ) {
                order_residuals.push((ab + ba).abs());
            }
        }
    }
    let pair_means = |variant: &str| -> Vec<((usize, usize), f64)> {
        let mut v: Vec<((usize, usize), f64)> = by_variant
            .get(variant)
            .map(|m| {
                m.iter()
                    .map(|(k, vals)| (*k, vals.iter().sum::<f64>() / vals.len() as f64))
                    .collect()
            })
            .unwrap_or_default();
        v.sort_by_key(|(k, _)| *k);
        v
    };
    let rho_against_primary = |variant: &str| -> Option<f64> {
        let a = pair_means("primary");
        let b = pair_means(variant);
        let joint: Vec<(f64, f64)> = a
            .iter()
            .filter_map(|(k, ma)| b.iter().find(|(kb, _)| kb == k).map(|(_, mb)| (*ma, *mb)))
            .collect();
        let (xs, ys): (Vec<f64>, Vec<f64>) = joint.into_iter().unzip();
        spearman(&xs, &ys)
    };

    let null_abs: Vec<f64> = records
        .iter()
        .filter(|r| r.is_null)
        .filter_map(|r| r.presented_mean.map(f64::abs))
        .collect();
    let null_mean =
        (!null_abs.is_empty()).then(|| null_abs.iter().sum::<f64>() / null_abs.len() as f64);

    let truth_rho = pool.truth.as_ref().and_then(|truth| {
        let a = pair_means("primary");
        let (xs, ys): (Vec<f64>, Vec<f64>) = a
            .iter()
            .filter_map(|((i, j), m)| {
                let ti = truth.get(&pool.items[*i].0)?;
                let tj = truth.get(&pool.items[*j].0)?;
                Some((*m, ti.ln() - tj.ln()))
            })
            .unzip();
        spearman(&xs, &ys)
    });

    let residual_mean = (!order_residuals.is_empty())
        .then(|| order_residuals.iter().sum::<f64>() / order_residuals.len() as f64);

    println!(
        "{slug} · {} · {calls} calls ({refusals} refused, {logprob} logprob-mode) · ${:.4} · cached input {:.1}% ({cache_read}/{input})",
        pool.name,
        cost as f64 / 1e9,
        if input > 0 { 100.0 * cache_read as f64 / input as f64 } else { 0.0 },
    );
    println!(
        "  reliability: paraphrase rho {} · negation rho {} · order residual {} nats · null |m| {}{}",
        fmt(rho_against_primary("paraphrase")),
        fmt(rho_against_primary("negation")),
        fmt(residual_mean),
        fmt(null_mean),
        truth_rho
            .map(|r| format!(" · TRUTH rho {r:.3}"))
            .unwrap_or_default(),
    );
}

fn fmt(x: Option<f64>) -> String {
    x.map(|v| format!("{v:.3}"))
        .unwrap_or_else(|| "n/a".to_string())
}
