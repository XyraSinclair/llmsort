//! E14 — the funnel: a cheap screen over all n (setwise ring rounds=2, or
//! pointwise 0–100 as the folk contender), then pairwise refinement with
//! certified top-k on the top-M slice. Composed entirely from the promised
//! surface plus one inline pointwise stage — the way a user would compose it.
//!
//! The product question it measures: for "give me the best 10 of 150", how
//! close does screen+refine get to the full pairwise sort, at what fraction
//! of its cost — and does a tie-blocked pointwise screen lose top items at
//! the slice cut?
//!
//! Usage (from `research/`, OPENROUTER_API_KEY set):
//!   cargo run --release --example funnel_topk -- \
//!     <model> <setwise|point> <criterion> <out.json>
//!
//! Fixed frame (matches run7/run8 packs for offline comparison): corpus
//! `data/arxiv_abstracts.json`, all 150 items truncated to 1000 chars,
//! seed 18, M = 30, top-k = 10.

use std::sync::Arc;

use futures::StreamExt as _;
use serde::{Deserialize, Serialize};

use llmsort::gateway::{
    Attribution, ChatModel, ChatRequest, Message, NoopUsageSink, ProviderGateway, ReasoningConfig,
};
use llmsort::rerank::setwise::{sort_documents_setwise, SetwiseOptions};
use llmsort::rerank::types::RerankDocument;
use llmsort::rerank::{RerankExecution, RerankRunOptions, SortOptions};
use llmsort::ChatGateway;

const M: usize = 30;
const TOP_K: usize = 10;
const SEED: u64 = 18;

const POINT_SYSTEM: &str = "You are an expert subjective evaluator. You read one entity, then an attribute. You answer with one integer from 0 to 100: the entity's level of the attribute, where 50 is a typical entity of this kind, 0 far below all peers, 100 far above all peers. Nothing else — no words, no punctuation, no explanation.\nExample: 62";

#[derive(Deserialize)]
struct CorpusItem {
    id: String,
    text: String,
}

#[derive(Serialize)]
struct StageScore {
    id: String,
    mean: f64,
}

#[derive(Serialize)]
struct FunnelReport {
    model: String,
    stage1: String,
    criterion: String,
    n: usize,
    m: usize,
    top_k: usize,
    seed: u64,
    stage1_scores: Vec<StageScore>,
    stage1_calls: usize,
    stage1_calls_ok: usize,
    stage1_cost_nanodollars: i64,
    stage1_flip_rate: Option<f64>,
    slice_ids: Vec<String>,
    stage2_order: Vec<StageScore>,
    stage2_comparisons: usize,
    stage2_cost_nanodollars: i64,
    total_cost_nanodollars: i64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let model = args.next().expect("model slug");
    let stage1 = args.next().expect("stage1: setwise|point");
    let criterion = args.next().expect("criterion");
    let out_path = args.next().expect("output json path");

    let corpus_raw = std::fs::read_to_string("data/arxiv_abstracts.json")?;
    let corpus: Vec<CorpusItem> = serde_json::from_str(&corpus_raw)?;
    let documents: Vec<RerankDocument> = corpus
        .iter()
        .map(|item| RerankDocument {
            id: item.id.clone(),
            text: item.text.trim().chars().take(1000).collect::<String>(),
        })
        .collect();
    let n = documents.len();

    let gateway: Arc<dyn ChatGateway> =
        Arc::new(ProviderGateway::from_env(Arc::new(NoopUsageSink))?);

    // --- stage 1: screen all n ------------------------------------------
    let (stage1_scores, s1_calls, s1_ok, s1_cost, s1_flip): (
        Vec<StageScore>,
        usize,
        usize,
        i64,
        Option<f64>,
    ) = match stage1.as_str() {
        "setwise" => {
            let sorted = sort_documents_setwise(
                documents.clone(),
                &criterion,
                Arc::clone(&gateway),
                SetwiseOptions {
                    model: Some(model.clone()),
                    rounds: 2,
                    seed: Some(SEED),
                    ..SetwiseOptions::default()
                },
            )
            .await?;
            let scores = sorted
                .items
                .iter()
                .map(|i| StageScore {
                    id: i.id.clone(),
                    mean: i.latent_mean,
                })
                .collect();
            (
                scores,
                sorted.calls,
                sorted.calls_ok,
                sorted.cost_nanodollars,
                sorted.gauge.and_then(|g| g.flip_rate),
            )
        }
        "point" => {
            let results: Vec<(String, Option<f64>, i64)> =
                futures::stream::iter(documents.iter().map(|doc| {
                    let user = format!(
                        "<entity>\n{}\n</entity>\n\nRate the entity on <attribute>{}</attribute>: one integer from 0 to 100.\nanswer:",
                        doc.text, criterion
                    );
                    let request = ChatRequest {
                        reasoning: Some(ReasoningConfig::disabled()),
                        ..ChatRequest::new(
                            ChatModel::parse(model.clone()),
                            vec![Message::system(POINT_SYSTEM), Message::user(&user)],
                            Attribution::new("llmsort::example::funnel_topk"),
                        )
                        .max_tokens(8)
                    };
                    let gateway = Arc::clone(&gateway);
                    let id = doc.id.clone();
                    async move {
                        match gateway.chat(request).await {
                            Ok(resp) => {
                                let parsed = resp
                                    .content
                                    .trim()
                                    .trim_end_matches('.')
                                    .parse::<i64>()
                                    .ok()
                                    .filter(|v| (0..=100).contains(v))
                                    .map(|v| v as f64);
                                (id, parsed, resp.cost_nanodollars)
                            }
                            Err(_) => (id, None, 0),
                        }
                    }
                }))
                .buffer_unordered(8)
                .collect()
                .await;
            let ok = results.iter().filter(|r| r.1.is_some()).count();
            let cost = results.iter().map(|r| r.2).sum();
            // Unparsed items get the floor: honest for a screen (they cannot
            // be selected), loud in the report via calls_ok.
            let mut scores: Vec<StageScore> = results
                .into_iter()
                .map(|(id, v, _)| StageScore {
                    id,
                    mean: v.unwrap_or(-1.0),
                })
                .collect();
            scores.sort_by(|a, b| {
                b.mean
                    .partial_cmp(&a.mean)
                    .expect("finite")
                    .then(a.id.cmp(&b.id))
            });
            (scores, n, ok, cost, None)
        }
        other => panic!("unknown stage1 {other:?}"),
    };

    // --- stage 2: pairwise refine the top-M slice ------------------------
    let slice_ids: Vec<String> = stage1_scores.iter().take(M).map(|s| s.id.clone()).collect();
    let slice_docs: Vec<RerankDocument> = slice_ids
        .iter()
        .map(|id| {
            documents
                .iter()
                .find(|d| &d.id == id)
                .expect("slice id from corpus")
                .clone()
        })
        .collect();
    let execution = RerankExecution::new(
        Arc::clone(&gateway),
        Attribution::new("llmsort::example::funnel_topk"),
    )
    .run_options(RerankRunOptions {
        rng_seed: Some(SEED),
        ..RerankRunOptions::default()
    });
    let refined = llmsort::rerank::sort_documents(
        slice_docs,
        &criterion,
        execution,
        SortOptions {
            model: Some(model.clone()),
            top_k: Some(TOP_K),
            ..SortOptions::default()
        },
    )
    .await?;

    let report = FunnelReport {
        model,
        stage1,
        criterion,
        n,
        m: M,
        top_k: TOP_K,
        seed: SEED,
        stage1_scores,
        stage1_calls: s1_calls,
        stage1_calls_ok: s1_ok,
        stage1_cost_nanodollars: s1_cost,
        stage1_flip_rate: s1_flip,
        slice_ids,
        stage2_order: refined
            .items
            .iter()
            .map(|i| StageScore {
                id: i.id.clone(),
                mean: i.latent_mean,
            })
            .collect(),
        stage2_comparisons: refined.meta.comparisons_used,
        stage2_cost_nanodollars: refined.meta.provider_cost_nanodollars,
        total_cost_nanodollars: s1_cost + refined.meta.provider_cost_nanodollars,
    };
    std::fs::write(&out_path, serde_json::to_string_pretty(&report)?)?;
    println!(
        "funnel {} stage1={} : stage1 {}/{} ok ${:.4} flip {:?} | stage2 {} comparisons ${:.4} | total ${:.4} -> {}",
        report.criterion,
        report.stage1,
        report.stage1_calls_ok,
        report.stage1_calls,
        report.stage1_cost_nanodollars as f64 / 1e9,
        report.stage1_flip_rate,
        report.stage2_comparisons,
        report.stage2_cost_nanodollars as f64 / 1e9,
        report.total_cost_nanodollars as f64 / 1e9,
        out_path,
    );
    Ok(())
}
