//! Setwise (k-wise full-order) sorting — the graduated E6 instrument.
//!
//! One call shows the judge k entities in lettered slots and asks for the
//! complete order, most → least of the criterion. Each answer lowers to
//! k(k−1)/2 ordinal observations fused by the same robust solver as the
//! pairwise path. Chunk design: `rounds` seeded shuffles of the list, each
//! split into ⌈n/k⌉ balanced groups; every group is re-presented `repeats`
//! times in fresh slot orders, which yields the **order-sensitivity gauge**
//! for free — the fraction of entity pairs whose direction flips between
//! shuffled presentations of the same subset.
//!
//! ## When to use which sort
//!
//! [`sort_texts`](super::sort::sort_texts) (pairwise ratio) is the
//! certified instrument: magnitudes, error bars, active planning.
//! [`sort_texts_setwise`] is the *adequate-regime* instrument: an order
//! (not magnitudes) at roughly 1/3–1/4 of the pairwise dollars per item,
//! measured at or near the pairwise sort's own test–retest ceiling on
//! stable attributes.
//!
//! ## The operating rule (measured, not assumed)
//!
//! Run with `repeats: 2` (the default) and read `gauge.flip_rate` FIRST:
//! over 38 live cells spanning four model families, two corpus families,
//! entity sizes 400–8000 chars and three delimiter styles, every cell with
//! flip rate < 0.20 agreed with the pairwise sort at ρ ≥ 0.64 (median
//! 0.79), while every poorly-agreeing cell (ρ < 0.61) had flip rate
//! ≥ 0.21. Rule: **flip < 0.2 → trust the setwise order; ≥ 0.25 → shrink
//! k, raise `rounds`, or fall back to the pairwise sort** for that
//! attribute. k = 6–8 is the measured band (k = 12 collapsed exactly on
//! the flakiest attribute); dollars per item are ~flat in k, so take the
//! largest k the flip rate tolerates. Evidence pack:
//! `research/artifacts/live/best-worst-2026-08-22/` (PROGRAM.md E6).
//!
//! Precondition (E15): the gauge certifies POOL-level order, never
//! item-level distinctions. On pools with duplicates / near-duplicates /
//! boilerplate the flip rate stays at the clean baseline while the
//! relative order inside a degenerate cluster is seed noise (identical
//! texts a median ~13 ranks apart at n = 150, direction coin-flipping
//! across seeds) — but inside joint 2σ (twin gaps exceeded it in only
//! 0–4/45 pairs per cell). Read `latent_std`: rank gaps smaller than the
//! error bars are presentation, not measurement. Pack:
//! `research/artifacts/live/e15-degenerate-2026-08-25/`.
//!
//! The default chunk design is the **anchored ring** (E11): per round the
//! shuffled list is tiled by cyclic windows of k at stride k−overlap, so
//! consecutive groups share `overlap` anchors and the last window wraps —
//! the observation graph is connected in ONE round, ⌈n/(k−o)⌉ calls
//! instead of the 2·⌈n/k⌉ a disjoint design needs. Measured live
//! (deepseek + gpt-5.6-luna, n = 24): ring at 8 calls matches the
//! 12-call disjoint design's agreement; 4 calls (repeats: 1) is the
//! connected adequacy floor at ρ 0.70–0.79 — but flying gauge-blind.
//!
//! Prompt geometry is cache-native: the entities block is a byte-stable
//! prefix and the criterion arrives last, so providers with automatic
//! prefix caching serve repeat presentations at cache-read prices
//! (measured live: a repeated run's setwise arm cost 10× less).

use std::collections::HashMap;
use std::sync::Arc;

use futures::stream::StreamExt;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};

use crate::gateway::{Attribution, ChatGateway, ChatModel, ChatRequest, Message, ReasoningConfig};
use crate::rating_engine::{AttributeParams, Observation, RaterParams, RatingEngine};
use crate::seriate::atom::RATIO_LADDER;
use crate::seriate::instrument::ordinal::FIXED_BUCKET;
use crate::trait_search::compute_attribute_units;

use super::sort::SortedItem;
use super::types::RerankDocument;

const SLOT_LETTERS: [char; 12] = ['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L'];
const MAX_OUTPUT_TOKENS: u32 = 64;
// Deliberately cheaper than the pairwise default (`multi/request.rs`
// `DEFAULT_MODEL`, terra): luna is measured at its own pairwise ceiling on
// stable attributes (PROGRAM.md E6), and setwise promises an adequate
// order, not cardinal magnitudes.
const DEFAULT_MODEL: &str = "openai/gpt-5.6-luna";

const ORDER_SYSTEM: &str = "You are an expert subjective evaluator. You read a small set of entities in lettered slots, then an attribute. You answer with every slot letter exactly once, separated by spaces, ordered from the MOST of the attribute to the LEAST. Nothing else — no words, no punctuation, no explanation.\nExample: C A D B";

/// Chunk-design family: how groups tile each shuffled round.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SetwiseDesign {
    /// Cyclic overlapping windows (stride k−overlap, last wraps): the
    /// graph is ring-connected in one round. The measured default.
    #[default]
    Ring,
    /// Disjoint balanced groups; needs `rounds ≥ 2` to connect when
    /// n > k.
    Disjoint,
}

/// Options for [`sort_texts_setwise`] / [`sort_documents_setwise`].
#[derive(Debug, Clone)]
pub struct SetwiseOptions {
    /// Model slug (OpenRouter), e.g. `openai/gpt-5.6-luna` (the default —
    /// measured at its own pairwise ceiling on stable attributes).
    pub model: Option<String>,
    /// Slots per call. Measured band: 6–8. Clamped to the list size and to
    /// 12 (the slot alphabet). Default 8.
    pub k: usize,
    /// Chunk-design family. Default [`SetwiseDesign::Ring`].
    pub design: SetwiseDesign,
    /// Ring only: anchors shared between consecutive groups. Default 2.
    pub overlap: usize,
    /// Rounds of shuffle → tile. Default 1 — enough for the ring design at
    /// n ≲ 3k; for n ≫ k use `rounds: 2` (measured at n = 150: rounds 1
    /// falls to ρ 0.55–0.71 vs pairwise while rounds 2 lands within
    /// 0.02–0.07 of the pairwise test–retest ceiling at ~1/6 its cost —
    /// PROGRAM.md E13). A `Disjoint` design with `rounds: 1` and n > k
    /// cannot connect across groups (surfaced via `components`), use ≥ 2
    /// there.
    pub rounds: usize,
    /// Shuffled re-presentations of each group. At ≥ 2 (the default) the
    /// order-sensitivity gauge is measured; at 1 `gauge` is `None` and you
    /// are flying without the instrument this module exists to give you.
    pub repeats: usize,
    /// Seed for the shuffle design (also the solver's determinism anchor).
    pub seed: Option<u64>,
    /// Maximum concurrent judge calls. Default 8.
    pub concurrency: Option<usize>,
}

impl Default for SetwiseOptions {
    fn default() -> Self {
        Self {
            model: None,
            k: 8,
            design: SetwiseDesign::Ring,
            overlap: 2,
            rounds: 1,
            repeats: 2,
            seed: None,
            concurrency: None,
        }
    }
}

/// How much presentation order moves this judge on these entities for this
/// criterion: over subsets asked in ≥ 2 shuffled slot orders, the fraction
/// of entity pairs (ordered by both presentations) whose direction flips.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderSensitivity {
    /// Subsets that received ≥ 2 parsed presentations.
    pub subsets_with_repeats: usize,
    /// Presentation pairs compared.
    pub presentation_pairs: usize,
    /// Entity pairs ordered by both presentations of a pair.
    pub entity_pairs_compared: usize,
    /// Pairs whose direction flipped.
    pub direction_flips: usize,
    /// `direction_flips / entity_pairs_compared`; `None` when nothing was
    /// comparable. The dial: < 0.2 trust, ≥ 0.25 fall back.
    pub flip_rate: Option<f64>,
}

/// Result of a setwise sort.
#[derive(Debug, Clone, Serialize)]
pub struct SetwiseSorted {
    /// Items in rank order (best first). `latent_mean`/`latent_std` are on
    /// the solver's log-ratio scale but carry ordinal information only —
    /// the fixed lowering magnitude means spacing is not a measurement.
    pub items: Vec<SortedItem>,
    /// The order-sensitivity gauge; `None` when `repeats < 2`.
    pub gauge: Option<OrderSensitivity>,
    /// Judge calls attempted / parsed / malformed / errored.
    pub calls: usize,
    /// Calls whose answer parsed as a complete distinct-slot order.
    pub calls_ok: usize,
    /// Calls that returned text that did not parse (partial orders,
    /// repeated letters, prose). Never silently defaulted.
    pub calls_malformed: usize,
    /// Calls that failed at the provider.
    pub calls_errored: usize,
    /// First provider error observed, so errored calls are never
    /// unattributed (E15 logged 4–24% transport-error loss with no cause
    /// on record — this field is that cause).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_error: Option<String>,
    /// Up to three raw malformed answers (truncated), the debugging
    /// surface for parse failures without a trace file.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub malformed_samples: Vec<String>,
    /// Connected components of the observation graph. 1 means every item
    /// is comparable to every other; more means the ranking is only
    /// defined within components (raise `rounds`).
    pub components: usize,
    /// Provider tokens and cost across all calls.
    pub input_tokens: u64,
    /// Output tokens across all calls.
    pub output_tokens: u64,
    /// Total judge cost in nanodollars.
    pub cost_nanodollars: i64,
    /// Model slug the calls were made with.
    pub model_used: String,
}

/// Errors from [`sort_texts_setwise`] / [`sort_documents_setwise`].
#[derive(Debug, thiserror::Error)]
pub enum SetwiseSortError {
    /// The input list was empty.
    #[error("cannot sort an empty list")]
    EmptyInput,
    /// Two documents shared the same id.
    #[error("duplicate document id: {0}")]
    DuplicateId(String),
    /// Every judge call failed or was malformed; there is nothing to solve.
    #[error("no usable judge calls ({malformed} malformed, {errored} errored of {attempted}); first error: {}", .first_error.as_deref().unwrap_or("all answers malformed"))]
    NoUsableCalls {
        /// Calls attempted.
        attempted: usize,
        /// Calls whose answer did not parse.
        malformed: usize,
        /// Calls that failed at the provider.
        errored: usize,
        /// First provider error observed, if any call errored (vs parsed wrong).
        first_error: Option<String>,
    },
    /// The solver rejected the configuration.
    #[error("rating engine: {0}")]
    Engine(&'static str),
}

fn escape_xml_chars(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// One judged subset: sorted member indices plus `repeats` slot orders.
struct SubsetPlan {
    subset: Vec<usize>,
    presentations: Vec<Vec<usize>>,
}

/// Ring design: per round, shuffle the pool and take cyclic windows of k
/// at stride k−overlap; consecutive windows share `overlap` anchors and
/// the final window wraps, closing the ring — connected in one round.
fn draw_ring_design(
    n: usize,
    k: usize,
    overlap: usize,
    rounds: usize,
    repeats: usize,
    rng: &mut StdRng,
) -> Vec<SubsetPlan> {
    let stride = (k - overlap.min(k - 1)).max(1);
    let q = n.div_ceil(stride);
    let mut plans = Vec::with_capacity(q * rounds);
    for _ in 0..rounds {
        let mut pool: Vec<usize> = (0..n).collect();
        pool.shuffle(rng);
        for g in 0..q {
            // A cyclic window of k over n > k distinct items never
            // repeats an entity (stride ≥ 1).
            let order: Vec<usize> = (0..k).map(|j| pool[(g * stride + j) % n]).collect();
            let mut subset = order.clone();
            subset.sort_unstable();
            debug_assert!(subset.windows(2).all(|w| w[0] != w[1]));
            let presentations = (0..repeats.max(1))
                .map(|r| {
                    let mut o = order.clone();
                    if r > 0 {
                        o.shuffle(rng);
                    }
                    o
                })
                .collect();
            plans.push(SubsetPlan {
                subset,
                presentations,
            });
        }
    }
    plans
}

/// `rounds` seeded shuffles, each split into ⌈n/k⌉ balanced groups; each
/// group carries `repeats` shuffled slot orders.
fn draw_chunk_design(
    n: usize,
    k: usize,
    rounds: usize,
    repeats: usize,
    rng: &mut StdRng,
) -> Vec<SubsetPlan> {
    let q = n.div_ceil(k);
    let mut plans = Vec::with_capacity(q * rounds);
    for _ in 0..rounds {
        let mut pool: Vec<usize> = (0..n).collect();
        pool.shuffle(rng);
        for g in 0..q {
            let order: Vec<usize> = pool[g * n / q..(g + 1) * n / q].to_vec();
            let mut subset = order.clone();
            subset.sort_unstable();
            let presentations = (0..repeats.max(1))
                .map(|r| {
                    let mut o = order.clone();
                    if r > 0 {
                        o.shuffle(rng);
                    }
                    o
                })
                .collect();
            plans.push(SubsetPlan {
                subset,
                presentations,
            });
        }
    }
    plans
}

/// Cache-stable prefix: system-adjacent entities block in slot order.
fn entities_block(texts: &[String], order: &[usize]) -> String {
    let mut block = String::from("<entities>\n");
    for (slot, &idx) in order.iter().enumerate() {
        let letter = SLOT_LETTERS[slot];
        block.push_str(&format!(
            "<entity_{letter}>\n{}\n</entity_{letter}>\n",
            texts[idx]
        ));
    }
    block.push_str("</entities>");
    block
}

/// The criterion tail, appended after the byte-stable prefix.
fn criterion_tail(criterion: &str, group_len: usize) -> String {
    let letters: Vec<String> = (0..group_len)
        .map(|s| SLOT_LETTERS[s].to_string())
        .collect();
    format!(
        "\n\nCompare the entities by <attribute_name>: {} </attribute_name>.\n\nOrder every slot from {{{}}} from MOST of the attribute to LEAST, every letter exactly once.\nanswer:",
        escape_xml_chars(criterion),
        letters.join(", ")
    )
}

/// Parse a full order: exactly `want` distinct single slot letters.
/// Anything else is malformed — never a default.
fn parse_slots(raw: &str, k: usize, want: usize) -> Option<Vec<usize>> {
    let mut slots: Vec<usize> = Vec::new();
    for token in raw.split(|c: char| c.is_whitespace() || c == ',' || c == ';' || c == '>') {
        let t = token.trim_matches(|c: char| !c.is_ascii_alphanumeric());
        let mut chars = t.chars();
        let (Some(c), None) = (chars.next(), chars.next()) else {
            continue;
        };
        let pos = SLOT_LETTERS[..k].iter().position(|&l| l == c)?;
        if slots.contains(&pos) {
            return None;
        }
        slots.push(pos);
    }
    (slots.len() == want).then_some(slots)
}

/// Sort `texts` by `criterion` with the setwise instrument, best first.
///
/// Sugar over [`sort_documents_setwise`] with synthesized `item-<n>` ids.
pub async fn sort_texts_setwise(
    texts: Vec<String>,
    criterion: &str,
    gateway: Arc<dyn ChatGateway>,
    opts: SetwiseOptions,
) -> Result<SetwiseSorted, SetwiseSortError> {
    let documents = texts
        .into_iter()
        .enumerate()
        .map(|(i, text)| RerankDocument {
            id: format!("item-{i}"),
            text,
        })
        .collect();
    sort_documents_setwise(documents, criterion, gateway, opts).await
}

/// Sort `documents` by `criterion` with the setwise instrument, best first.
///
/// Read `gauge.flip_rate` before trusting the order (module docs carry the
/// measured thresholds). The judge sees the rendered document text as-is;
/// truncate long documents yourself if cost matters — dollars per item
/// scale ~linearly with entity bytes and ~flat in `k`.
pub async fn sort_documents_setwise(
    documents: Vec<RerankDocument>,
    criterion: &str,
    gateway: Arc<dyn ChatGateway>,
    opts: SetwiseOptions,
) -> Result<SetwiseSorted, SetwiseSortError> {
    if documents.is_empty() {
        return Err(SetwiseSortError::EmptyInput);
    }
    {
        let mut seen = std::collections::HashSet::new();
        for d in &documents {
            if !seen.insert(d.id.as_str()) {
                return Err(SetwiseSortError::DuplicateId(d.id.clone()));
            }
        }
    }
    let model = opts.model.clone().unwrap_or_else(|| DEFAULT_MODEL.into());
    let n = documents.len();
    if n == 1 {
        let d = &documents[0];
        return Ok(SetwiseSorted {
            items: vec![SortedItem {
                id: d.id.clone(),
                text: d.text.clone(),
                rank: 1,
                latent_mean: 0.0,
                latent_std: 0.0,
                z_score: 0.0,
                percentile: 0.5,
            }],
            gauge: None,
            calls: 0,
            calls_ok: 0,
            calls_malformed: 0,
            calls_errored: 0,
            first_error: None,
            malformed_samples: Vec::new(),
            components: 1,
            input_tokens: 0,
            output_tokens: 0,
            cost_nanodollars: 0,
            model_used: model,
        });
    }

    let k = opts.k.clamp(2, SLOT_LETTERS.len()).min(n);
    let mut rng = StdRng::seed_from_u64(opts.seed.unwrap_or(17));
    let plans = match opts.design {
        SetwiseDesign::Ring if n > k => draw_ring_design(
            n,
            k,
            opts.overlap,
            opts.rounds.max(1),
            opts.repeats,
            &mut rng,
        ),
        _ => draw_chunk_design(n, k, opts.rounds.max(1), opts.repeats, &mut rng),
    };
    let escaped: Vec<String> = documents
        .iter()
        .map(|d| escape_xml_chars(&d.text))
        .collect();
    let attribution = Attribution::new("rerank::setwise");
    let concurrency = opts.concurrency.unwrap_or(8).max(1);

    // One future per (plan, presentation); results keyed back by index.
    let calls_spec: Vec<(usize, Vec<usize>)> = plans
        .iter()
        .enumerate()
        .flat_map(|(pi, plan)| {
            plan.presentations
                .iter()
                .map(move |order| (pi, order.clone()))
        })
        .collect();
    let calls = calls_spec.len();
    let results: Vec<(usize, Vec<usize>, Result<crate::gateway::ChatResponse, _>)> =
        futures::stream::iter(calls_spec.into_iter().map(|(pi, order)| {
            let user = format!(
                "{}{}",
                entities_block(&escaped, &order),
                criterion_tail(criterion, order.len())
            );
            let request = ChatRequest::new(
                ChatModel::parse(model.clone()),
                vec![Message::system(ORDER_SYSTEM), Message::user(&user)],
                attribution.clone(),
            )
            .max_tokens(MAX_OUTPUT_TOKENS);
            // A two-token-per-slot answer never wants hybrid reasoning: on
            // reasoning-by-default providers the whole output budget burns as
            // thought and the content comes back empty (100% malformed,
            // measured 2026-08-24 — the CLI smoke failed exactly this way
            // whenever OPENROUTER_DISABLE_REASONING was not exported).
            let request = ChatRequest {
                reasoning: Some(ReasoningConfig::disabled()),
                ..request
            };
            let gateway = Arc::clone(&gateway);
            async move { (pi, order, gateway.chat(request).await) }
        }))
        .buffer_unordered(concurrency)
        .collect()
        .await;

    let ratio = RATIO_LADDER[usize::from(FIXED_BUCKET) - 1];
    let mut observations: Vec<Observation> = Vec::new();
    // subset -> per-presentation entity -> rank (for the gauge).
    let mut subset_ranks: HashMap<Vec<usize>, Vec<HashMap<usize, usize>>> = HashMap::new();
    let (mut calls_ok, mut calls_malformed, mut calls_errored) = (0usize, 0usize, 0usize);
    let (mut input_tokens, mut output_tokens, mut cost) = (0u64, 0u64, 0i64);
    let mut first_error: Option<String> = None;
    let mut malformed_samples: Vec<String> = Vec::new();

    for (pi, order, outcome) in results {
        match outcome {
            Err(e) => {
                calls_errored += 1;
                if first_error.is_none() {
                    first_error = Some(e.to_string());
                }
            }
            Ok(resp) => {
                input_tokens += u64::from(resp.input_tokens);
                output_tokens += u64::from(resp.output_tokens);
                cost += resp.cost_nanodollars;
                match parse_slots(&resp.content, order.len(), order.len()) {
                    None => {
                        calls_malformed += 1;
                        if malformed_samples.len() < 3 {
                            malformed_samples.push(resp.content.chars().take(120).collect());
                        }
                    }
                    Some(slots) => {
                        calls_ok += 1;
                        // slots: slot indices most → least; map to entities.
                        let ranked: Vec<usize> = slots.iter().map(|&s| order[s]).collect();
                        for (a, &hi) in ranked.iter().enumerate() {
                            for &lo in &ranked[a + 1..] {
                                observations.push(Observation::new(
                                    hi,
                                    lo,
                                    ratio,
                                    1.0,
                                    model.clone(),
                                    1.0,
                                ));
                            }
                        }
                        let rank_map: HashMap<usize, usize> =
                            ranked.iter().enumerate().map(|(r, &e)| (e, r)).collect();
                        subset_ranks
                            .entry(plans[pi].subset.clone())
                            .or_default()
                            .push(rank_map);
                    }
                }
            }
        }
    }

    if calls_ok == 0 {
        return Err(SetwiseSortError::NoUsableCalls {
            attempted: calls,
            malformed: calls_malformed,
            errored: calls_errored,
            first_error,
        });
    }

    let gauge = (opts.repeats >= 2).then(|| {
        let (mut with_repeats, mut pres_pairs, mut compared, mut flips) = (0, 0, 0, 0);
        for (subset, pres) in &subset_ranks {
            if pres.len() >= 2 {
                with_repeats += 1;
            }
            for a in 0..pres.len() {
                for b in (a + 1)..pres.len() {
                    pres_pairs += 1;
                    for x in 0..subset.len() {
                        for y in (x + 1)..subset.len() {
                            let (e, f) = (subset[x], subset[y]);
                            if let (Some((r1e, r1f)), Some((r2e, r2f))) = (
                                pres[a].get(&e).zip(pres[a].get(&f)),
                                pres[b].get(&e).zip(pres[b].get(&f)),
                            ) {
                                if r1e != r1f && r2e != r2f {
                                    compared += 1;
                                    if (r1e < r1f) != (r2e < r2f) {
                                        flips += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        OrderSensitivity {
            subsets_with_repeats: with_repeats,
            presentation_pairs: pres_pairs,
            entity_pairs_compared: compared,
            direction_flips: flips,
            flip_rate: (compared > 0).then(|| flips as f64 / compared as f64),
        }
    });

    let raters: HashMap<String, RaterParams> = [(model.clone(), RaterParams::default())].into();
    let mut engine = RatingEngine::new(n, AttributeParams::default(), raters, None)
        .map_err(SetwiseSortError::Engine)?;
    engine.add_observations(&observations);
    let summary = engine.solve();
    let scores = summary.scores;
    let stds: Vec<f64> = summary
        .diag_cov
        .iter()
        .map(|&v| v.max(0.0).sqrt())
        .collect();
    let (_scale, z, _min_norm, pct) = compute_attribute_units(&scores);

    let mut ranked_idx: Vec<usize> = (0..n).collect();
    ranked_idx.sort_by(|&a, &b| {
        scores[b]
            .partial_cmp(&scores[a])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let items: Vec<SortedItem> = ranked_idx
        .iter()
        .enumerate()
        .map(|(rank, &i)| SortedItem {
            id: documents[i].id.clone(),
            text: documents[i].text.clone(),
            rank: rank + 1,
            latent_mean: scores[i],
            latent_std: stds[i],
            z_score: z[i],
            percentile: pct[i],
        })
        .collect();

    Ok(SetwiseSorted {
        items,
        gauge,
        calls,
        calls_ok,
        calls_malformed,
        calls_errored,
        first_error,
        malformed_samples,
        components: summary.components,
        input_tokens,
        output_tokens,
        cost_nanodollars: cost,
        model_used: model,
    })
}
