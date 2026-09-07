//! Judge bakeoff: one standardized stability battery per judge model, run
//! against the production pairwise ratio-letter instrument, so small local
//! judges (8–14B dense, 3–4B-active MoE) and frontier references are
//! measured on identical calls over identical entities.
//!
//! Per (lens, axis) the battery reads, from the same records:
//! - slot bias: mean signed A-slot advantage and mean |m_AB + m_BA| in nats;
//! - retest: pair-level Spearman of signed log-ratios between two nonce draws of wording `a`;
//! - wording: pair-level Spearman of wording `a` against `b`, `c` (catalog modes);
//! - par/A/B %: share of answered calls at parity / entity A ahead / entity B ahead
//!   (a judge that never uses the lowercase half of the alphabet shows B ≈ 0);
//! - decisiveness: mean |canonical log-ratio| and visible PMF mass;
//! - health: logprob-mode fraction, refusals, failed calls, calls/s, cost.
//!
//! `report` folds every `records-*.json` in a pack into the inter-model
//! Spearman matrix (wording `a`, draw 0), agreement with the leave-one-out
//! consensus, and agreement with named reference judges.
//!
//! ```text
//! judge_bakeoff run --items items.json --axes axes.json --model <id> --pack <dir>
//! judge_bakeoff report --pack <dir> [--reference m1,m2]
//! ```
//! Endpoint: `OPENROUTER_BASE_URL` (local vLLM or OpenRouter),
//! `OPENROUTER_API_KEY`, and `CARDINAL_SERIATE_LOGPROB_MODELS="<model>:20"` so
//! the ratio-letter instrument reads the answer-position PMF.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use clap::{Parser, Subcommand};
use futures::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};

use llmsort::gateway::{Attribution, NoopUsageSink, ProviderGateway};
use llmsort::rerank::comparison::{ORDINAL_LETTER_SLUG, RATIO_LETTER_ATTR_LAST_SLUG};
use llmsort::rerank::sort::spearman;
use llmsort::rerank::{
    compare_pair, PairwiseComparisonAttribute, PairwiseComparisonEntity, PairwiseComparisonRequest,
    PairwiseComparisonSpec, PairwiseJudgement,
};

#[derive(Parser)]
#[command(name = "judge_bakeoff")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run the battery for one model and write `records-<model>.json`.
    Run {
        #[arg(long)]
        items: PathBuf,
        #[arg(long)]
        axes: PathBuf,
        #[arg(long)]
        model: String,
        #[arg(long)]
        pack: PathBuf,
        /// Items per lens (deterministic: first n by id).
        #[arg(long, default_value_t = 30)]
        n: usize,
        /// Even pair degree per item (circulant design).
        #[arg(long, default_value_t = 6)]
        degree: usize,
        #[arg(long, default_value_t = 12)]
        concurrency: usize,
        /// Nonce draws of wording `a` (draw 0 has no nonce).
        #[arg(long, default_value_t = 2)]
        draws: usize,
        /// Wording modes to run (catalog suffixes).
        #[arg(long, default_value = "a,b,c")]
        wordings: String,
        /// Entity text cap in chars (prompt budget).
        #[arg(long, default_value_t = 8000)]
        max_chars: usize,
        /// Print prompt tail + raw output + judgement for the first N calls.
        #[arg(long, default_value_t = 0)]
        debug_raw: usize,
        /// Pairwise instrument: `ratio` (52-letter ratio ladder, attr-last) or
        /// `ordinal` (direction only: A / B / =).
        #[arg(long, default_value = "ratio")]
        instrument: String,
    },
    /// Fold every records-*.json in the pack into REPORT.md.
    Report {
        #[arg(long)]
        pack: PathBuf,
        /// Comma-separated reference judges for the agreement column.
        #[arg(long, default_value = "")]
        reference: String,
    },
}

#[derive(Deserialize, Clone)]
struct Item {
    lens: String,
    id: String,
    text: String,
}

#[derive(Deserialize, Clone)]
struct Axis {
    lens: String,
    key: String,
    /// wording mode (`a`, `b`, `c`, …) → prompt text
    wordings: BTreeMap<String, String>,
}

#[derive(Serialize, Deserialize, Clone)]
struct CallRecord {
    lens: String,
    axis: String,
    wording: String,
    draw: usize,
    i: usize,
    j: usize,
    /// true = (i,j) presented as (A,B); false = swapped.
    order_ij: bool,
    presented_mean: Option<f64>,
    presented_var: Option<f64>,
    visible_mass: Option<f64>,
    logprob_mode: Option<bool>,
    refused: bool,
    failed: bool,
    input_tokens: u32,
    cache_read_tokens: Option<u32>,
    cost_nanodollars: i64,
    latency_ms: u64,
}

#[derive(Serialize, Deserialize)]
struct Pack {
    model: String,
    #[serde(default = "default_instrument")]
    instrument: String,
    started_utc: String,
    wall_secs: f64,
    calls: usize,
    n_per_lens: usize,
    degree: usize,
    item_ids: BTreeMap<String, Vec<String>>,
    records: Vec<CallRecord>,
}

fn default_instrument() -> String {
    "ratio".to_string()
}

fn truncate_chars(s: &str, max: usize) -> &str {
    match s.char_indices().nth(max) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}

/// Circulant design: (i, i+k mod n) for k in 1..=degree/2.
fn circulant_pairs(n: usize, degree: usize) -> Vec<(usize, usize)> {
    let mut pairs = Vec::new();
    for k in 1..=(degree / 2).max(1) {
        for i in 0..n {
            let j = (i + k) % n;
            if i != j {
                pairs.push((i, j));
            }
        }
    }
    pairs
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    match Cli::parse().cmd {
        Cmd::Run {
            items,
            axes,
            model,
            pack,
            n,
            degree,
            concurrency,
            draws,
            wordings,
            max_chars,
            debug_raw,
            instrument,
        } => {
            run(
                &items,
                &axes,
                &model,
                &pack,
                n,
                degree,
                concurrency,
                draws,
                &wordings,
                max_chars,
                debug_raw,
                &instrument,
            )
            .await
        }
        Cmd::Report { pack, reference } => report(&pack, &reference),
    }
}

#[allow(clippy::too_many_arguments)]
async fn run(
    items_path: &Path,
    axes_path: &Path,
    model: &str,
    pack_dir: &Path,
    n: usize,
    degree: usize,
    concurrency: usize,
    draws: usize,
    wordings: &str,
    max_chars: usize,
    debug_raw: usize,
    instrument: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let slug: &'static str = match instrument {
        "ratio" => RATIO_LETTER_ATTR_LAST_SLUG,
        "ordinal" => ORDINAL_LETTER_SLUG,
        other => return Err(format!("unknown --instrument {other:?} (ratio|ordinal)").into()),
    };
    let items: Vec<Item> = serde_json::from_str(&std::fs::read_to_string(items_path)?)?;
    let axes: Vec<Axis> = serde_json::from_str(&std::fs::read_to_string(axes_path)?)?;
    let wordings: Vec<String> = wordings.split(',').map(|s| s.trim().to_string()).collect();
    std::fs::create_dir_all(pack_dir)?;

    let mut by_lens: BTreeMap<String, Vec<Item>> = BTreeMap::new();
    for it in items {
        by_lens.entry(it.lens.clone()).or_default().push(it);
    }
    for v in by_lens.values_mut() {
        v.sort_by(|a, b| a.id.cmp(&b.id));
        v.truncate(n);
        for it in v.iter_mut() {
            it.text = truncate_chars(&it.text, max_chars).to_string();
        }
    }
    let by_lens = Arc::new(by_lens);
    let axes = Arc::new(axes);

    // Units: (lens, axis index, wording, draw, i, j, order_ij)
    let mut units: Vec<(String, usize, String, usize, usize, usize, bool)> = Vec::new();
    for (lens, pool) in by_lens.iter() {
        let pairs = circulant_pairs(pool.len(), degree);
        for (ai, axis) in axes.iter().enumerate() {
            if axis.lens != *lens {
                continue;
            }
            for w in &wordings {
                if !axis.wordings.contains_key(w) {
                    continue;
                }
                let n_draws = if w == "a" { draws.max(1) } else { 1 };
                for d in 0..n_draws {
                    for &(i, j) in &pairs {
                        units.push((lens.clone(), ai, w.clone(), d, i, j, true));
                        units.push((lens.clone(), ai, w.clone(), d, i, j, false));
                    }
                }
            }
        }
    }
    eprintln!(
        "judge_bakeoff: model={model} lenses={} axes={} calls={} concurrency={concurrency}",
        by_lens.len(),
        axes.len(),
        units.len()
    );

    let gateway = Arc::new(ProviderGateway::from_env(Arc::new(NoopUsageSink))?);
    let started = Instant::now();
    let started_utc = chrono::Utc::now().to_rfc3339();
    let total = units.len();
    let done = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let records: Vec<CallRecord> = stream::iter(units.into_iter().map(
        |(lens, ai, wording, draw, i, j, order_ij)| {
            let gateway = Arc::clone(&gateway);
            let by_lens = Arc::clone(&by_lens);
            let axes = Arc::clone(&axes);
            let model = model.to_string();
            let done = Arc::clone(&done);
            async move {
                let pool = &by_lens[&lens];
                let axis = &axes[ai];
                let prompt = &axis.wordings[&wording];
                let (sa, sb) = if order_ij { (i, j) } else { (j, i) };
                let axis_id = format!("{}#{}", axis.key, wording);
                let spec = PairwiseComparisonSpec {
                    model: &model,
                    attribute: PairwiseComparisonAttribute {
                        id: &axis_id,
                        prompt,
                        prompt_template_slug: Some(slug),
                    },
                    entity_a: PairwiseComparisonEntity {
                        id: &pool[sa].id,
                        text: &pool[sa].text,
                    },
                    entity_b: PairwiseComparisonEntity {
                        id: &pool[sb].id,
                        text: &pool[sb].text,
                    },
                };
                let nonce = (draw > 0).then(|| format!("retest-{draw}"));
                let t0 = Instant::now();
                let outcome = compare_pair(
                    gateway.as_ref(),
                    None,
                    PairwiseComparisonRequest {
                        nonce,
                        spec,
                        cache_only: false,
                        attribution: Attribution::new("cardinal::example::judge_bakeoff"),
                    },
                )
                .await;
                let latency_ms = t0.elapsed().as_millis() as u64;
                let k = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                if k % 200 == 0 || k == total {
                    eprintln!("  {k}/{total} calls, {:.1}s", started.elapsed().as_secs_f64());
                }
                let base = CallRecord {
                    lens: lens.clone(),
                    axis: axis.key.clone(),
                    wording: wording.clone(),
                    draw,
                    i,
                    j,
                    order_ij,
                    presented_mean: None,
                    presented_var: None,
                    visible_mass: None,
                    logprob_mode: None,
                    refused: false,
                    failed: false,
                    input_tokens: 0,
                    cache_read_tokens: None,
                    cost_nanodollars: 0,
                    latency_ms,
                };
                match outcome {
                    Ok((judgement, usage)) => {
                        let m = usage.evidence_moments;
                        if k <= debug_raw {
                            let prompt = usage.prompt_text.as_deref().unwrap_or("");
                            let tail: String = prompt.chars().rev().take(700).collect::<Vec<_>>().into_iter().rev().collect();
                            eprintln!(
                                "--- debug call {k}: {lens}/{axis_id} {i}-{j} order_ij={order_ij}\n  prompt tail: …{tail:?}\n  raw_output: {:?}\n  judgement: {judgement:?}\n  moments: {m:?}\n  logprobs: {}",
                                usage.raw_output.as_deref().unwrap_or(""),
                                usage.output_logprobs.as_ref().map(|l| format!("{} positions", l.len())).unwrap_or_else(|| "none".into())
                            );
                        }
                        CallRecord {
                            presented_mean: m.map(|e| e.log_ratio_mean),
                            presented_var: m.map(|e| e.log_ratio_var),
                            visible_mass: m.map(|e| e.visible_mass),
                            logprob_mode: m.map(|e| e.logprob_mode),
                            refused: matches!(judgement, PairwiseJudgement::Refused),
                            input_tokens: usage.input_tokens,
                            cache_read_tokens: usage.cache_read_tokens,
                            cost_nanodollars: usage.provider_cost_nanodollars,
                            ..base
                        }
                    }
                    Err(err) => {
                        if k <= 5 || k % 500 == 0 {
                            eprintln!("call failed ({lens}/{axis_id} {i}-{j}): {err}");
                        }
                        CallRecord {
                            failed: true,
                            ..base
                        }
                    }
                }
            }
        },
    ))
    .buffer_unordered(concurrency)
    .collect()
    .await;

    let wall_secs = started.elapsed().as_secs_f64();
    let pack = Pack {
        model: model.to_string(),
        instrument: instrument.to_string(),
        started_utc,
        wall_secs,
        calls: records.len(),
        n_per_lens: n,
        degree,
        item_ids: by_lens
            .iter()
            .map(|(l, v)| (l.clone(), v.iter().map(|it| it.id.clone()).collect()))
            .collect(),
        records,
    };
    let path = pack_dir.join(format!("records-{}.json", model.replace('/', "_")));
    std::fs::write(&path, serde_json::to_string(&pack)?)?;
    eprintln!(
        "pack: {} ({} calls, {:.0}s, {:.1} calls/s)",
        path.display(),
        pack.calls,
        wall_secs,
        pack.calls as f64 / wall_secs.max(1e-9)
    );
    print!("{}", battery_table(&pack));
    Ok(())
}

/// Canonical i-over-j reading of a record.
fn canonical(r: &CallRecord) -> Option<f64> {
    r.presented_mean.map(|m| if r.order_ij { m } else { -m })
}

/// Canonical signed log-ratio per pair (i over j), averaged over both
/// presentation orders; pairs missing either order are dropped. All
/// stability and agreement metrics are computed at this pair level: the
/// degree-6 ring design is too thin to fold into item scores without either
/// a shared-neighbour artifact (signed mean) or ring-integrated noise
/// (least squares), and pair-level needs neither.
fn pair_means(pack: &Pack, lens: &str, axis: &str, wording: &str, draw: usize) -> BTreeMap<(usize, usize), f64> {
    let mut by_pair: BTreeMap<(usize, usize), Vec<f64>> = BTreeMap::new();
    for r in &pack.records {
        if r.lens == lens && r.axis == axis && r.wording == wording && r.draw == draw {
            if let Some(m) = canonical(r) {
                by_pair.entry((r.i, r.j)).or_default().push(m);
            }
        }
    }
    by_pair
        .into_iter()
        .filter(|(_, ms)| ms.len() >= 2)
        .map(|(k, ms)| (k, ms.iter().sum::<f64>() / ms.len() as f64))
        .collect()
}

/// Spearman over the pairs both maps share.
fn pair_spearman(a: &BTreeMap<(usize, usize), f64>, b: &BTreeMap<(usize, usize), f64>) -> Option<f64> {
    let (mut xs, mut ys) = (Vec::new(), Vec::new());
    for (k, x) in a {
        if let Some(y) = b.get(k) {
            xs.push(*x);
            ys.push(*y);
        }
    }
    spearman(&xs, &ys)
}

/// Leave-one-out consensus: z-score every other model's pair vector over the
/// pairs all packs share, sum, and correlate with model `a`.
fn consensus_rho(scores: &[BTreeMap<(usize, usize), f64>], a: usize) -> Option<f64> {
    // Leave-one-out consensus: each other judge is z-scored over the pairs it
    // answered, and a pair's consensus is the mean z over the judges that
    // covered it. Requiring every judge to cover a pair would let one
    // high-refusal judge shrink the shared set to a handful of pairs.
    let others: Vec<usize> = (0..scores.len()).filter(|&b| b != a).collect();
    if others.len() < 2 {
        return None;
    }
    let min_cover = (others.len() / 2).max(2);
    let mut sum: BTreeMap<(usize, usize), (f64, usize)> = BTreeMap::new();
    for &b in &others {
        let keys: Vec<_> = scores[b].keys().copied().collect();
        let v: Vec<f64> = keys.iter().map(|k| scores[b][k]).collect();
        for (k, z) in keys.into_iter().zip(zscore(&v)) {
            let e = sum.entry(k).or_insert((0.0, 0));
            e.0 += z;
            e.1 += 1;
        }
    }
    let mut own = Vec::new();
    let mut cons = Vec::new();
    for (k, v) in &scores[a] {
        if let Some(&(z, n)) = sum.get(k) {
            if n >= min_cover {
                own.push(*v);
                cons.push(z / n as f64);
            }
        }
    }
    if own.len() < 10 {
        return None;
    }
    spearman(&own, &cons)
}

struct CellStats {
    slot_bias: f64,
    slot_abs: f64,
    retest_rho: Option<f64>,
    wording_b_rho: Option<f64>,
    wording_c_rho: Option<f64>,
    decisiveness: f64,
    /// Fraction of answered calls whose mean sits at parity / A ahead / B ahead.
    parity_frac: f64,
    a_ahead_frac: f64,
    b_ahead_frac: f64,
    visible_mass: f64,
    logprob_frac: f64,
    refused: usize,
    failed: usize,
}

fn cell_stats(pack: &Pack, lens: &str, axis: &str) -> CellStats {
    let rows: Vec<&CallRecord> = pack
        .records
        .iter()
        .filter(|r| r.lens == lens && r.axis == axis)
        .collect();
    // Slot bias from wording a, draw 0: pair both orders.
    let mut by_pair: BTreeMap<(usize, usize), (Option<f64>, Option<f64>)> = BTreeMap::new();
    for r in rows.iter().filter(|r| r.wording == "a" && r.draw == 0) {
        let e = by_pair.entry((r.i, r.j)).or_default();
        if r.order_ij {
            e.0 = r.presented_mean;
        } else {
            e.1 = r.presented_mean;
        }
    }
    let mut sb = Vec::new();
    for (a, b) in by_pair.values() {
        if let (Some(a), Some(b)) = (a, b) {
            sb.push((a + b) / 2.0);
        }
    }
    let slot_bias = mean(&sb);
    let slot_abs = mean(&sb.iter().map(|v| v.abs()).collect::<Vec<_>>());
    let s_a0 = pair_means(pack, lens, axis, "a", 0);
    let has = |w: &str, d: usize| rows.iter().any(|r| r.wording == w && r.draw == d);
    let rho_vs = |w: &str, d: usize| has(w, d).then(|| pair_spearman(&s_a0, &pair_means(pack, lens, axis, w, d))).flatten();
    let retest_rho = rho_vs("a", 1);
    let wording_b_rho = rho_vs("b", 0);
    let wording_c_rho = rho_vs("c", 0);
    let answered: Vec<f64> = rows.iter().filter_map(|r| r.presented_mean).collect();
    let frac = |pred: &dyn Fn(f64) -> bool| answered.iter().filter(|m| pred(**m)).count() as f64 / answered.len().max(1) as f64;
    const PARITY_NATS: f64 = 0.03;
    let decisive: Vec<f64> = answered.iter().map(|m| m.abs()).collect();
    let vm: Vec<f64> = rows.iter().filter_map(|r| r.visible_mass).collect();
    let logprob = rows.iter().filter(|r| r.logprob_mode == Some(true)).count();
    CellStats {
        slot_bias,
        slot_abs,
        retest_rho,
        wording_b_rho,
        wording_c_rho,
        decisiveness: mean(&decisive),
        parity_frac: frac(&|m| m.abs() < PARITY_NATS),
        a_ahead_frac: frac(&|m| m >= PARITY_NATS),
        b_ahead_frac: frac(&|m| m <= -PARITY_NATS),
        visible_mass: mean(&vm),
        logprob_frac: logprob as f64 / rows.len().max(1) as f64,
        refused: rows.iter().filter(|r| r.refused).count(),
        failed: rows.iter().filter(|r| r.failed).count(),
    }
}

fn mean(xs: &[f64]) -> f64 {
    if xs.is_empty() {
        f64::NAN
    } else {
        xs.iter().sum::<f64>() / xs.len() as f64
    }
}

fn fmt_rho(r: Option<f64>) -> String {
    r.map(|v| format!("{v:+.2}")).unwrap_or_else(|| "  n/a".into())
}

fn cells(pack: &Pack) -> Vec<(String, String)> {
    let mut set = BTreeSet::new();
    for r in &pack.records {
        set.insert((r.lens.clone(), r.axis.clone()));
    }
    set.into_iter().collect()
}

fn battery_table(pack: &Pack) -> String {
    let mut out = String::new();
    let cost: i64 = pack.records.iter().map(|r| r.cost_nanodollars).sum();
    out.push_str(&format!(
        "\n### {}  ({} calls, {:.0}s, {:.1} calls/s, ${:.3})\n\n",
        pack.model,
        pack.calls,
        pack.wall_secs,
        pack.calls as f64 / pack.wall_secs.max(1e-9),
        cost as f64 / 1e9
    ));
    out.push_str("| lens | axis | retest ρ | wording b ρ | wording c ρ | slot bias (nats) | slot |m| | decisive |m| | par/A/B % | vis mass | logprob | refused | failed |\n");
    out.push_str("|---|---|---|---|---|---|---|---|---|---|---|---|---|\n");
    for (lens, axis) in cells(pack) {
        let c = cell_stats(pack, &lens, &axis);
        out.push_str(&format!(
            "| {lens} | {axis} | {} | {} | {} | {:+.2} | {:.2} | {:.2} | {:.0}/{:.0}/{:.0} | {:.2} | {:.0}% | {} | {} |\n",
            fmt_rho(c.retest_rho),
            fmt_rho(c.wording_b_rho),
            fmt_rho(c.wording_c_rho),
            c.slot_bias,
            c.slot_abs,
            c.decisiveness,
            c.parity_frac * 100.0,
            c.a_ahead_frac * 100.0,
            c.b_ahead_frac * 100.0,
            c.visible_mass,
            c.logprob_frac * 100.0,
            c.refused,
            c.failed
        ));
    }
    out
}

fn zscore(xs: &[f64]) -> Vec<f64> {
    let m = mean(xs);
    let sd = (xs.iter().map(|x| (x - m).powi(2)).sum::<f64>() / xs.len().max(1) as f64).sqrt();
    xs.iter().map(|x| if sd > 0.0 { (x - m) / sd } else { 0.0 }).collect()
}

fn report(pack_dir: &Path, reference: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut packs: Vec<Pack> = Vec::new();
    for entry in std::fs::read_dir(pack_dir)? {
        let p = entry?.path();
        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if name.starts_with("records-") && name.ends_with(".json") {
            packs.push(serde_json::from_str(&std::fs::read_to_string(&p)?)?);
        }
    }
    packs.sort_by(|a, b| a.model.cmp(&b.model));
    if packs.is_empty() {
        return Err("no records-*.json in pack".into());
    }
    let refs: Vec<&str> = reference.split(',').map(str::trim).filter(|s| !s.is_empty()).collect();

    let mut out = String::new();
    out.push_str("# Judge bakeoff report\n\n");
    out.push_str(&format!(
        "{} models, generated {}\n\n## Per-model battery\n",
        packs.len(),
        chrono::Utc::now().to_rfc3339()
    ));
    for p in &packs {
        out.push_str(&battery_table(p));
    }

    // Cells common to all packs; scores from wording a, draw 0.
    let mut common: Option<BTreeSet<(String, String)>> = None;
    for p in &packs {
        let c: BTreeSet<_> = cells(p).into_iter().collect();
        common = Some(match common {
            None => c,
            Some(prev) => prev.intersection(&c).cloned().collect(),
        });
    }
    let common: Vec<(String, String)> = common.unwrap_or_default().into_iter().collect();
    // Item alignment: require identical item id lists per lens.
    let mut scores: Vec<BTreeMap<(String, String), BTreeMap<(usize, usize), f64>>> = Vec::new();
    for p in &packs {
        let mut m = BTreeMap::new();
        for (lens, axis) in &common {
            if p.item_ids[lens] != packs[0].item_ids[lens] {
                return Err(format!("item ids differ for {lens} between {} and {}", p.model, packs[0].model).into());
            }
            m.insert((lens.clone(), axis.clone()), pair_means(p, lens, axis, "a", 0));
        }
        scores.push(m);
    }

    out.push_str("\n## Inter-model agreement (pair-level Spearman of signed log-ratios, wording a, draw 0, mean over cells)\n\n| model |");
    for p in &packs {
        out.push_str(&format!(" {} |", short(&p.model)));
    }
    out.push_str(" consensus (LOO) |");
    for r in &refs {
        out.push_str(&format!(" vs {} |", short(r)));
    }
    out.push('\n');
    out.push_str("|---|");
    for _ in 0..packs.len() + 1 + refs.len() {
        out.push_str("---|");
    }
    out.push('\n');

    let pair_rho = |a: usize, b: usize| -> Option<f64> {
        let mut rs = Vec::new();
        for cell in &common {
            if let Some(r) = pair_spearman(&scores[a][cell], &scores[b][cell]) {
                rs.push(r);
            }
        }
        (!rs.is_empty()).then(|| mean(&rs))
    };
    let consensus_mean = |a: usize| -> Option<f64> {
        let mut rs = Vec::new();
        for cell in &common {
            let per_cell: Vec<BTreeMap<(usize, usize), f64>> = scores.iter().map(|m| m[cell].clone()).collect();
            if let Some(r) = consensus_rho(&per_cell, a) {
                rs.push(r);
            }
        }
        (!rs.is_empty()).then(|| mean(&rs))
    };
    let idx_of = |name: &str| packs.iter().position(|p| p.model == name);
    for (a, pa) in packs.iter().enumerate() {
        out.push_str(&format!("| {} |", short(&pa.model)));
        for b in 0..packs.len() {
            if a == b {
                out.push_str("   —  |");
            } else {
                out.push_str(&format!(" {} |", fmt_rho(pair_rho(a, b))));
            }
        }
        out.push_str(&format!(" {} |", fmt_rho(consensus_mean(a))));
        for r in &refs {
            let v = idx_of(r).filter(|&b| b != a).and_then(|b| pair_rho(a, b));
            out.push_str(&format!(" {} |", fmt_rho(v)));
        }
        out.push('\n');
    }

    out.push_str("\n## Per-cell agreement with consensus (LOO)\n\n| model |");
    for (lens, axis) in &common {
        out.push_str(&format!(" {}/{} |", short(lens), axis));
    }
    out.push_str("\n|---|");
    for _ in &common {
        out.push_str("---|");
    }
    out.push('\n');
    for (a, pa) in packs.iter().enumerate() {
        out.push_str(&format!("| {} |", short(&pa.model)));
        for cell in &common {
            let per_cell: Vec<BTreeMap<(usize, usize), f64>> = scores.iter().map(|m| m[cell].clone()).collect();
            out.push_str(&format!(" {} |", fmt_rho(consensus_rho(&per_cell, a))));
        }
        out.push('\n');
    }

    let path = pack_dir.join("REPORT.md");
    std::fs::write(&path, &out)?;
    print!("{out}");
    eprintln!("report: {}", path.display());
    Ok(())
}

fn short(model: &str) -> String {
    let s = model.rsplit('/').next().unwrap_or(model);
    if s.len() > 28 {
        s[..28].to_string()
    } else {
        s.to_string()
    }
}
