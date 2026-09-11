//! Multi-criteria setwise: does one call that orders a window on m criteria
//! (m answer lines) recover the same per-criterion rankings as m separate
//! single-criterion calls — and does it inflate inter-criterion agreement
//! (cross-criterion halo)?
//!
//! Mirrors the scry extension's list sorter instrument exactly: ring windows
//! of k with overlap, first presentation in window order then shuffled
//! repeats (the flip gauge), lettered slots, `<attribute_name>` prompt,
//! exact-distinct-letter parse, Huber-IRLS ridge fit on ln 2.78 per ordered
//! pair. The joint arm shuffles the criteria order per call and asks for one
//! `i: C A D B` line per criterion.
//!
//! cargo run -p llmsort-experiments --example multi_criteria_setwise -- \
//!   --items lists/lw.json --label lw --model google/gemma-4-31b-it \
//!   --base-url https://openrouter.ai/api/v1 --api-key-env OPENROUTER_API_KEY \
//!   --criteria 'novelty=…;alpha=…;rigor=…' --out research/artifacts/live/<dir>

use std::collections::{BTreeMap, HashMap};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use clap::{Parser, ValueEnum};
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::Semaphore;

const SLOTS: &str = "ABCDEFGHIJKL";
const SYSTEM_SINGLE: &str = "You are an expert subjective evaluator. You read a small set of entities in lettered slots, then an attribute. You answer with every slot letter exactly once, separated by spaces, ordered from the MOST of the attribute to the LEAST. Nothing else — no words, no punctuation, no explanation.\nExample: C A D B";
const SYSTEM_JOINT: &str = "You are an expert subjective evaluator. You read a small set of entities in lettered slots, then a numbered list of attributes. For EACH attribute, on its own line, you answer with the attribute number, a colon, then every slot letter exactly once, separated by spaces, ordered from the MOST of that attribute to the LEAST. One line per attribute, in the numbered order. Nothing else — no words, no explanation.\nExample:\n1: C A D B\n2: A C B D";

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum Arm {
    Separate,
    Joint,
    Both,
}

#[derive(Parser, Debug)]
struct Args {
    /// JSON array of {id, text}.
    #[arg(long)]
    items: PathBuf,
    /// Short label for output files (e.g. lw, hn_top).
    #[arg(long)]
    label: String,
    /// `name=prompt;name=prompt;…`
    #[arg(long)]
    criteria: String,
    #[arg(long, default_value_t = 8)]
    k: usize,
    #[arg(long, default_value_t = 2)]
    overlap: usize,
    #[arg(long, default_value_t = 1)]
    rounds: usize,
    #[arg(long, default_value_t = 2)]
    repeats: usize,
    #[arg(long)]
    model: String,
    #[arg(long)]
    base_url: String,
    #[arg(long, default_value = "OPENROUTER_API_KEY")]
    api_key_env: String,
    #[arg(long, value_enum, default_value_t = Arm::Both)]
    arm: Arm,
    #[arg(long)]
    out: PathBuf,
    #[arg(long, default_value_t = 4)]
    concurrency: usize,
    #[arg(long, default_value_t = 3000)]
    max_chars: usize,
    #[arg(long, default_value_t = 7)]
    seed: u64,
    /// Sent as `reasoning_effort` when given. Cerebras qwen-3.8-27b spends its whole
    /// max_tokens budget on hidden reasoning unless this is "none" (2026-09-11).
    #[arg(long)]
    effort: Option<String>,
}

#[derive(Deserialize)]
struct Item {
    id: String,
    text: String,
}

#[derive(Clone)]
struct Plan {
    subset: Vec<usize>,
    presentations: Vec<Vec<usize>>,
}

fn design(n: usize, k: usize, overlap: usize, rounds: usize, repeats: usize, rng: &mut StdRng) -> Vec<Plan> {
    let stride = (k - overlap.min(k - 1)).max(1);
    let mut plans = Vec::new();
    for _ in 0..rounds {
        let mut pool: Vec<usize> = (0..n).collect();
        pool.shuffle(rng);
        let windows = if n > k { (n + stride - 1) / stride } else { 1 };
        for g in 0..windows {
            let order: Vec<usize> = if n > k { (0..k).map(|j| pool[(g * stride + j) % n]).collect() } else { pool.clone() };
            let mut presentations = Vec::new();
            for p in 0..repeats.max(1) {
                let mut o = order.clone();
                if p > 0 {
                    o.shuffle(rng);
                }
                presentations.push(o);
            }
            let mut subset = order.clone();
            subset.sort_unstable();
            plans.push(Plan { subset, presentations });
        }
    }
    plans
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&apos;")
}

fn entity_block(texts: &[String], order: &[usize]) -> (String, String) {
    let mut block = String::from("<entities>\n");
    for (slot, &idx) in order.iter().enumerate() {
        let l = &SLOTS[slot..slot + 1];
        block.push_str(&format!("<entity_{l}>\n{}\n</entity_{l}>\n", texts[idx]));
    }
    block.push_str("</entities>");
    let letters = (0..order.len()).map(|s| &SLOTS[s..s + 1]).collect::<Vec<_>>().join(", ");
    (block, letters)
}

fn prompt_single(texts: &[String], order: &[usize], criterion: &str) -> String {
    let (block, letters) = entity_block(texts, order);
    format!("{block}\n\nCompare the entities by <attribute_name>: {} </attribute_name>.\n\nOrder every slot from {{{letters}}} from MOST of the attribute to LEAST, every letter exactly once.\nanswer:", escape(criterion))
}

fn prompt_joint(texts: &[String], order: &[usize], criteria: &[&str]) -> String {
    let (block, letters) = entity_block(texts, order);
    let mut attrs = String::from("<attributes>\n");
    for (i, c) in criteria.iter().enumerate() {
        attrs.push_str(&format!("<attribute_{}>{}</attribute_{}>\n", i + 1, escape(c), i + 1));
    }
    attrs.push_str("</attributes>");
    format!("{block}\n\nCompare the entities by each attribute in turn:\n{attrs}\n\nFor each attribute, on its own line `<number>: <letters>`, order every slot from {{{letters}}} from MOST of that attribute to LEAST, every letter exactly once. {} lines.\nanswer:", criteria.len())
}

fn parse_slots(raw: &str, k: usize) -> Option<Vec<usize>> {
    let mut slots = Vec::new();
    for token in raw.split(|c: char| c.is_whitespace() || c == ',' || c == ';' || c == '>') {
        let t = token.trim_matches(|c: char| !c.is_ascii_alphanumeric());
        if t.chars().count() != 1 {
            continue;
        }
        let pos = SLOTS[..k].find(t)?;
        if slots.contains(&pos) {
            return None;
        }
        slots.push(pos);
    }
    (slots.len() == k).then_some(slots)
}

/// Joint answer: lines `i: letters` (also tolerates `i.` / `i)` / bare lines in order).
fn parse_joint(raw: &str, k: usize, m: usize) -> Vec<Option<Vec<usize>>> {
    let mut out: Vec<Option<Vec<usize>>> = vec![None; m];
    let mut bare = 0usize;
    for line in raw.lines() {
        let line = line.trim().trim_start_matches(|c| c == '*' || c == '-' || c == '`').trim();
        if line.is_empty() {
            continue;
        }
        let (idx, rest) = match line.find(|c: char| c == ':' || c == '.' || c == ')') {
            Some(p) if line[..p].trim().parse::<usize>().is_ok() => (line[..p].trim().parse::<usize>().unwrap(), &line[p + 1..]),
            _ => {
                bare += 1;
                (bare, line)
            }
        };
        if idx >= 1 && idx <= m && out[idx - 1].is_none() {
            out[idx - 1] = parse_slots(rest, k);
        }
    }
    out
}

struct Fit {
    scores: Vec<f64>,
    std: Vec<f64>,
    components: usize,
}

fn fit(n: usize, obs: &[(usize, usize, f64)]) -> Fit {
    let ridge = 1e-3;
    let delta = 1.0;
    let mut s = vec![0.0; n];
    let mut w = vec![1.0; obs.len()];
    let mut last_l: Option<Vec<f64>> = None;
    let chol = |a: &[f64]| -> Vec<f64> {
        let mut l = vec![0.0; n * n];
        for i in 0..n {
            for j in 0..=i {
                let mut sum = a[i * n + j];
                for m in 0..j {
                    sum -= l[i * n + m] * l[j * n + m];
                }
                l[i * n + j] = if i == j { sum.max(1e-12).sqrt() } else { sum / l[j * n + j] };
            }
        }
        l
    };
    let solve = |l: &[f64], rhs: &[f64]| -> Vec<f64> {
        let mut y = vec![0.0; n];
        for i in 0..n {
            let mut sum = rhs[i];
            for m in 0..i {
                sum -= l[i * n + m] * y[m];
            }
            y[i] = sum / l[i * n + i];
        }
        let mut x = vec![0.0; n];
        for i in (0..n).rev() {
            let mut sum = y[i];
            for m in i + 1..n {
                sum -= l[m * n + i] * x[m];
            }
            x[i] = sum / l[i * n + i];
        }
        x
    };
    for _ in 0..6 {
        let mut a = vec![0.0; n * n];
        let mut b = vec![0.0; n];
        for i in 0..n {
            a[i * n + i] = ridge;
        }
        for (e, &(hi, lo, r)) in obs.iter().enumerate() {
            let we = w[e];
            a[hi * n + hi] += we;
            a[lo * n + lo] += we;
            a[hi * n + lo] -= we;
            a[lo * n + hi] -= we;
            b[hi] += we * r;
            b[lo] -= we * r;
        }
        let l = chol(&a);
        s = solve(&l, &b);
        for (e, &(hi, lo, r)) in obs.iter().enumerate() {
            let res = (s[hi] - s[lo] - r).abs();
            w[e] = if res <= delta { 1.0 } else { delta / res };
        }
        last_l = Some(l);
    }
    let mut std = vec![0.0; n];
    if let Some(l) = &last_l {
        for i in 0..n {
            let mut unit = vec![0.0; n];
            unit[i] = 1.0;
            std[i] = (solve(l, &unit)[i] - 1.0 / (n as f64 * ridge)).max(0.0).sqrt();
        }
    }
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(p: &mut Vec<usize>, x: usize) -> usize {
        if p[x] != x {
            let r = find(p, p[x]);
            p[x] = r;
        }
        p[x]
    }
    for &(hi, lo, _) in obs {
        let a = find(&mut parent, hi);
        let b = find(&mut parent, lo);
        parent[a] = b;
    }
    let mut roots = std::collections::HashSet::new();
    for i in 0..n {
        roots.insert(find(&mut parent, i));
    }
    Fit { scores: s, std, components: roots.len() }
}

fn ranks(v: &[f64]) -> Vec<f64> {
    let mut idx: Vec<usize> = (0..v.len()).collect();
    idx.sort_by(|&a, &b| v[a].partial_cmp(&v[b]).unwrap());
    let mut r = vec![0.0; v.len()];
    let mut i = 0;
    while i < idx.len() {
        let mut j = i;
        while j + 1 < idx.len() && v[idx[j + 1]] == v[idx[i]] {
            j += 1;
        }
        let avg = (i + j) as f64 / 2.0;
        for &t in &idx[i..=j] {
            r[t] = avg;
        }
        i = j + 1;
    }
    r
}

fn spearman(a: &[f64], b: &[f64]) -> f64 {
    let (ra, rb) = (ranks(a), ranks(b));
    let m = ra.len() as f64;
    let (ma, mb) = (ra.iter().sum::<f64>() / m, rb.iter().sum::<f64>() / m);
    let cov: f64 = ra.iter().zip(&rb).map(|(x, y)| (x - ma) * (y - mb)).sum();
    let va: f64 = ra.iter().map(|x| (x - ma).powi(2)).sum();
    let vb: f64 = rb.iter().map(|y| (y - mb).powi(2)).sum();
    cov / (va * vb).sqrt()
}

fn topk_overlap(a: &[f64], b: &[f64], k: usize) -> f64 {
    let top = |v: &[f64]| {
        let mut idx: Vec<usize> = (0..v.len()).collect();
        idx.sort_by(|&x, &y| v[y].partial_cmp(&v[x]).unwrap());
        idx.into_iter().take(k).collect::<std::collections::HashSet<_>>()
    };
    top(a).intersection(&top(b)).count() as f64 / k as f64
}

#[derive(Serialize, Clone)]
struct CallTrace {
    arm: String,
    plan: usize,
    presentation: usize,
    order: Vec<usize>,
    criteria: Vec<String>,
    prompt_chars: usize,
    content: String,
    parsed: Vec<Option<Vec<usize>>>,
    input_tokens: u64,
    output_tokens: u64,
    cost_nanodollars: Option<u64>,
    finish_reason: Option<String>,
    served_model: Option<String>,
    secs: f64,
    error: Option<String>,
}

async fn call(client: &reqwest::Client, base: &str, key: &str, model: &str, system: &str, prompt: &str, max_tokens: u32, effort: Option<&str>) -> Result<(String, Value), String> {
    let mut body = json!({
        "model": model,
        "messages": [{"role":"system","content":system},{"role":"user","content":prompt}],
        "max_tokens": max_tokens,
        "temperature": 0,
    });
    if let Some(e) = effort {
        body["reasoning_effort"] = json!(e);
    }
    if base.contains("openrouter") {
        body["provider"] = json!({"data_collection": "deny"});
    }
    let mut last = String::new();
    for attempt in 0..4 {
        let resp = client
            .post(format!("{}/chat/completions", base.trim_end_matches('/')))
            .bearer_auth(key)
            .json(&body)
            .send()
            .await;
        match resp {
            Ok(r) => {
                let status = r.status();
                let v: Value = match r.json().await {
                    Ok(v) => v,
                    Err(e) => {
                        last = format!("http {status}: body not json: {e}");
                        tokio::time::sleep(std::time::Duration::from_millis(500 * (1 << attempt))).await;
                        continue;
                    }
                };
                if status.is_success() {
                    if let Some(c) = v["choices"][0]["message"]["content"].as_str() {
                        return Ok((c.to_string(), v));
                    }
                    last = format!("no content: {}", v.to_string().chars().take(200).collect::<String>());
                } else {
                    last = format!("http {status}: {}", v.to_string().chars().take(200).collect::<String>());
                }
            }
            Err(e) => last = e.to_string(),
        }
        tokio::time::sleep(std::time::Duration::from_millis(500 * (1 << attempt))).await;
    }
    Err(last)
}

#[derive(Serialize)]
struct ArmSummary {
    arm: String,
    calls: usize,
    ok: usize,
    malformed: usize,
    errored: usize,
    input_tokens: u64,
    output_tokens: u64,
    cost_dollars: f64,
    secs: f64,
    per_criterion: BTreeMap<String, CritSummary>,
    inter_criterion: BTreeMap<String, f64>,
}

#[derive(Serialize, Clone)]
struct CritSummary {
    parsed: usize,
    flip: Option<f64>,
    components: usize,
    scores: Vec<f64>,
    std: Vec<f64>,
}

fn summarize(arm: &str, names: &[String], n: usize, traces: &[CallTrace], plans: &[Plan]) -> ArmSummary {
    let m = names.len();
    let mut per = BTreeMap::new();
    let mut inter = BTreeMap::new();
    let mut fits: Vec<Vec<f64>> = Vec::new();
    for name in names.iter() {
        let mut obs: Vec<(usize, usize, f64)> = Vec::new();
        // subset key -> presentations -> item->rank
        let mut subset_ranks: HashMap<Vec<usize>, Vec<HashMap<usize, usize>>> = HashMap::new();
        let mut parsed = 0;
        for t in traces {
            let Some(slot_i) = t.criteria.iter().position(|c| c == name) else { continue };
            let Some(Some(slots)) = t.parsed.get(slot_i) else { continue };
            parsed += 1;
            let ranked: Vec<usize> = slots.iter().map(|&s| t.order[s]).collect();
            for a in 0..ranked.len() {
                for b in a + 1..ranked.len() {
                    obs.push((ranked[a], ranked[b], (2.78f64).ln()));
                }
            }
            let mut r = HashMap::new();
            for (pos, &it) in ranked.iter().enumerate() {
                r.insert(it, pos);
            }
            subset_ranks.entry(plans[t.plan].subset.clone()).or_default().push(r);
        }
        let (mut compared, mut flips) = (0usize, 0usize);
        for pres in subset_ranks.values() {
            for a in 0..pres.len() {
                for b in a + 1..pres.len() {
                    for (&e, &r1e) in &pres[a] {
                        for (&f, &r1f) in &pres[a] {
                            if e >= f {
                                continue;
                            }
                            let (Some(&r2e), Some(&r2f)) = (pres[b].get(&e), pres[b].get(&f)) else { continue };
                            compared += 1;
                            if (r1e < r1f) != (r2e < r2f) {
                                flips += 1;
                            }
                        }
                    }
                }
            }
        }
        let f = fit(n, &obs);
        fits.push(f.scores.clone());
        per.insert(
            name.clone(),
            CritSummary { parsed, flip: (compared > 0).then(|| flips as f64 / compared as f64), components: f.components, scores: f.scores, std: f.std },
        );
    }
    for i in 0..m {
        for j in i + 1..m {
            inter.insert(format!("{}~{}", names[i], names[j]), spearman(&fits[i], &fits[j]));
        }
    }
    ArmSummary {
        arm: arm.into(),
        calls: traces.len(),
        ok: traces.iter().filter(|t| t.error.is_none() && t.parsed.iter().all(|p| p.is_some())).count(),
        malformed: traces.iter().filter(|t| t.error.is_none() && t.parsed.iter().any(|p| p.is_none())).count(),
        errored: traces.iter().filter(|t| t.error.is_some()).count(),
        input_tokens: traces.iter().map(|t| t.input_tokens).sum(),
        output_tokens: traces.iter().map(|t| t.output_tokens).sum(),
        cost_dollars: traces.iter().filter_map(|t| t.cost_nanodollars).sum::<u64>() as f64 / 1e9,
        secs: traces.iter().map(|t| t.secs).sum(),
        per_criterion: per,
        inter_criterion: inter,
    }
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let key = std::env::var(&args.api_key_env).unwrap_or_else(|_| panic!("{} not set", args.api_key_env));
    let items: Vec<Item> = serde_json::from_slice(&std::fs::read(&args.items).expect("items")).expect("items json");
    let n = items.len();
    let texts: Vec<String> = items
        .iter()
        .map(|it| {
            let t: String = it.text.split_whitespace().collect::<Vec<_>>().join(" ");
            if t.chars().count() > args.max_chars { t.chars().take(args.max_chars).collect::<String>() + "…" } else { t }
        })
        .collect();
    let criteria: Vec<(String, String)> = args
        .criteria
        .split(';')
        .filter(|s| !s.trim().is_empty())
        .map(|s| {
            let (a, b) = s.split_once('=').expect("name=prompt");
            (a.trim().to_string(), b.trim().to_string())
        })
        .collect();
    let names: Vec<String> = criteria.iter().map(|c| c.0.clone()).collect();
    let m = criteria.len();
    assert!(m >= 2, "need ≥2 criteria");
    let k = args.k.min(n);
    let mut rng = StdRng::seed_from_u64(args.seed);
    let plans = design(n, k, args.overlap, args.rounds, args.repeats, &mut rng);
    std::fs::create_dir_all(&args.out).expect("out dir");
    let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(180)).build().unwrap();
    let sem = Arc::new(Semaphore::new(args.concurrency));
    let arms: Vec<Arm> = match args.arm {
        Arm::Both => vec![Arm::Separate, Arm::Joint],
        a => vec![a],
    };
    let mut summaries: Vec<ArmSummary> = Vec::new();
    let mut all_traces: Vec<CallTrace> = Vec::new();
    for arm in arms {
        let arm_name = format!("{arm:?}").to_lowercase();
        let mut jobs: Vec<(usize, usize, Vec<usize>, Vec<String>, String, &str, u32)> = Vec::new();
        for (pi, plan) in plans.iter().enumerate() {
            for (pj, pres) in plan.presentations.iter().enumerate() {
                match arm {
                    Arm::Separate => {
                        for (name, prompt) in &criteria {
                            jobs.push((pi, pj, pres.clone(), vec![name.clone()], prompt_single(&texts, pres, prompt), SYSTEM_SINGLE, 400));
                        }
                    }
                    Arm::Joint => {
                        let mut order: Vec<usize> = (0..m).collect();
                        order.shuffle(&mut rng);
                        let cnames: Vec<String> = order.iter().map(|&i| names[i].clone()).collect();
                        let cprompts: Vec<&str> = order.iter().map(|&i| criteria[i].1.as_str()).collect();
                        jobs.push((pi, pj, pres.clone(), cnames, prompt_joint(&texts, pres, &cprompts), SYSTEM_JOINT, 400 * m as u32));
                    }
                    Arm::Both => unreachable!(),
                }
            }
        }
        eprintln!("[{}] arm={arm_name} n={n} k={k} plans={} calls={}", args.label, plans.len(), jobs.len());
        let started = Instant::now();
        let mut handles = Vec::new();
        for (pi, pj, order, cnames, prompt, system, max_tokens) in jobs {
            let (client, sem, base, key, model, arm_name, effort) = (client.clone(), sem.clone(), args.base_url.clone(), key.clone(), args.model.clone(), arm_name.clone(), args.effort.clone());
            handles.push(tokio::spawn(async move {
                let _p = sem.acquire().await.unwrap();
                let t0 = Instant::now();
                let res = call(&client, &base, &key, &model, system, &prompt, max_tokens, effort.as_deref()).await;
                let secs = t0.elapsed().as_secs_f64();
                match res {
                    Ok((content, v)) => {
                        let parsed = if cnames.len() == 1 { vec![parse_slots(&content, order.len())] } else { parse_joint(&content, order.len(), cnames.len()) };
                        CallTrace {
                            arm: arm_name,
                            plan: pi,
                            presentation: pj,
                            order,
                            criteria: cnames,
                            prompt_chars: prompt.chars().count(),
                            content,
                            parsed,
                            input_tokens: v["usage"]["prompt_tokens"].as_u64().unwrap_or(0),
                            output_tokens: v["usage"]["completion_tokens"].as_u64().unwrap_or(0),
                            cost_nanodollars: v["usage"]["cost"].as_f64().map(|c| (c * 1e9) as u64),
                            finish_reason: v["choices"][0]["finish_reason"].as_str().map(String::from),
                            served_model: v["provider"].as_str().or(v["model"].as_str()).map(String::from),
                            secs,
                            error: None,
                        }
                    }
                    Err(e) => CallTrace {
                        arm: arm_name,
                        plan: pi,
                        presentation: pj,
                        order,
                        parsed: vec![None; cnames.len()],
                        criteria: cnames,
                        prompt_chars: prompt.chars().count(),
                        content: String::new(),
                        input_tokens: 0,
                        output_tokens: 0,
                        cost_nanodollars: None,
                        finish_reason: None,
                        served_model: None,
                        secs,
                        error: Some(e),
                    },
                }
            }));
        }
        let mut traces = Vec::new();
        for h in handles {
            traces.push(h.await.unwrap());
        }
        let mut s = summarize(&arm_name, &names, n, &traces, &plans);
        s.secs = started.elapsed().as_secs_f64();
        eprintln!(
            "[{}] arm={arm_name} done: ok={} malformed={} errored={} in={} out={} ${:.4} {:.0}s",
            args.label, s.ok, s.malformed, s.errored, s.input_tokens, s.output_tokens, s.cost_dollars, s.secs
        );
        summaries.push(s);
        all_traces.extend(traces);
    }
    // Trace.
    let mut f = std::fs::File::create(args.out.join(format!("trace-{}.jsonl", args.label))).unwrap();
    for t in &all_traces {
        writeln!(f, "{}", serde_json::to_string(t).unwrap()).unwrap();
    }
    // Cross-arm readouts.
    let mut md = String::new();
    md.push_str(&format!("## {} — n={n} k={k} overlap={} rounds={} repeats={} model={} seed={}\n\n", args.label, args.overlap, args.rounds, args.repeats, args.model, args.seed));
    md.push_str("| arm | calls | ok | malformed | errored | in tok | out tok | $ | wall s |\n|---|---|---|---|---|---|---|---|---|\n");
    for s in &summaries {
        md.push_str(&format!("| {} | {} | {} | {} | {} | {} | {} | {:.4} | {:.0} |\n", s.arm, s.calls, s.ok, s.malformed, s.errored, s.input_tokens, s.output_tokens, s.cost_dollars, s.secs));
    }
    md.push_str("\n| criterion | arm | parsed | flip | components |\n|---|---|---|---|---|\n");
    for s in &summaries {
        for (name, c) in &s.per_criterion {
            md.push_str(&format!("| {name} | {} | {} | {} | {} |\n", s.arm, c.parsed, c.flip.map(|f| format!("{f:.3}")).unwrap_or("—".into()), c.components));
        }
    }
    md.push_str("\n| pair | arm | inter-criterion ρ |\n|---|---|---|\n");
    for s in &summaries {
        for (pair, rho) in &s.inter_criterion {
            md.push_str(&format!("| {pair} | {} | {rho:+.3} |\n", s.arm));
        }
    }
    if summaries.len() == 2 {
        let (a, b) = (&summaries[0], &summaries[1]);
        md.push_str("\n| criterion | ρ(separate, joint) | top-10 overlap |\n|---|---|---|\n");
        for name in &names {
            let (sa, sb) = (&a.per_criterion[name].scores, &b.per_criterion[name].scores);
            md.push_str(&format!("| {name} | {:+.3} | {:.2} |\n", spearman(sa, sb), topk_overlap(sa, sb, 10)));
        }
        let inflation: Vec<f64> = a.inter_criterion.keys().map(|p| b.inter_criterion[p] - a.inter_criterion[p]).collect();
        md.push_str(&format!("\nmean inter-criterion ρ inflation (joint − separate): {:+.3}\n", inflation.iter().sum::<f64>() / inflation.len() as f64));
    }
    std::fs::write(args.out.join(format!("summary-{}.md", args.label)), &md).unwrap();
    let ids: Vec<&str> = items.iter().map(|i| i.id.as_str()).collect();
    std::fs::write(
        args.out.join(format!("summary-{}.json", args.label)),
        serde_json::to_string_pretty(&json!({"ids": ids, "criteria": criteria, "arms": summaries})).unwrap(),
    )
    .unwrap();
    print!("{md}");
}
