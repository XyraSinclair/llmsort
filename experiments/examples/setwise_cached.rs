//! Setwise ratio elicitation with a cached entity prefix — the instrument-grid
//! row "k-wise · ratio · point" (docs/FIRST_PRINCIPLES.md §2, currently ✗).
//!
//! Geometry: k ∈ {3,4} entities per call. The prompt is ordered for provider
//! prompt caching: the system message and an `<entities>` block (entity texts
//! in slots A..D) come FIRST and are byte-identical across the calls that
//! share a subset+presentation; the ATTRIBUTE comes LAST. Swapping the
//! attribute never touches the prefix, so with an OpenAI-family model the
//! ≥1024-token prefix is served from the provider prompt cache on the second
//! and third attribute of every presentation.
//!
//! The model answers, for pivot slot A, the ratio of every other slot to A:
//! strict JSON `{"ratios":{"B":2.1,"C":0.5},"confidence":0.7}` (r>0; r>1 =
//! more of the attribute than A) or `{"refused":true}`. A malformed answer is
//! a recorded failure, never a default. Each call lowers to k−1 INDEPENDENT
//! log-ratio observations (slot vs pivot) entering the production IRLS engine
//! exactly like canonical_v2 point judgements: `Observation::new(..)` with
//! unit precision (stated confidence is not calibrated). Implied non-pivot
//! pairs are NOT added (they are linear combinations of the pivot pairs);
//! the shared-call correlation caveat is reported alongside the results.
//!
//! Counterbalancing: every subset is asked in ≥2 presentations with a rotated
//! pivot and permuted slot order; the pivot-rotation sign-flip rate is the
//! position-bias readout.
//!
//! Baseline: the canonical pairwise path (`sort_documents`, canonical_v2,
//! default budget) over the same items+attribute, same model, same seed.
//!
//! Answer modes (`--answer`, 2026-08-22; design climbed in PROGRAM.md E6):
//! `ratio` is the arm above, untouched. `bw` (best–worst: two slot letters,
//! most then least) and `order` (every slot letter, most→least) are point
//! answers, no logprobs, on a CHUNK design derived from the mode:
//! `--presentations` rounds of seeded shuffle → even split into ⌈n/k⌉
//! groups, so every item is presented exactly `presentations` times and the
//! call count is known up front. One parse target (distinct slot letters,
//! exactly 2 or exactly k), one tier-lowering — tiers [[best],[rest],[worst]]
//! for `bw`, singletons for `order` — emitting every cross-tier pair as an
//! ordinal observation at the seriate `FIXED_BUCKET` magnitude (2k−3 and
//! k(k−1)/2 fall out). Position bias readout: first/last pick counts by slot.
//! Silent-drop guard: the solver's connected-component count is surfaced and
//! the arm is flagged `disconnected` when > 1.
//!
//! Dry run ($0):  cargo run --release --example setwise_cached -- --offline
//! Live (capped): xyra-vault run repos/documents/openpriors/env/env -- \
//!     cargo run --release --example setwise_cached

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Write as _;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use clap::{Parser, ValueEnum};
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};

use llmsort::gateway::{
    Attribution, ChatGateway, ChatModel, ChatRequest, ChatResponse, FinishReason, Message,
    NoopUsageSink, ProviderError, ProviderGateway,
};
use llmsort::rating_engine::{AttributeParams, EngineSpec, Observation, RaterParams, RatingEngine};
use llmsort::rerank::sort::{sort_documents, SortOptions, SortedTexts};
use llmsort::rerank::types::RerankDocument;
use llmsort::rerank::{JsonlTraceSink, RerankExecution, RerankRunOptions};
use llmsort::seriate::atom::RATIO_LADDER;
use llmsort::seriate::instrument::ordinal::FIXED_BUCKET;

const SLOT_LETTERS: [char; 26] = [
    'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S',
    'T', 'U', 'V', 'W', 'X', 'Y', 'Z',
];
const SETWISE_MAX_OUTPUT_TOKENS: u32 = 400;
const SLOTS_MAX_OUTPUT_TOKENS: u32 = 64;
/// Tail markers the synthetic judge keys on; byte-stable by construction.
const BW_MARKER: &str = "Answer with exactly two letters";
const ORDER_MARKER: &str = "Order every slot";
const POINT_MARKER: &str = "one integer from 0 to 100";
const SYNTHETIC_NOISE_SIGMA: f64 = 0.35;
/// Synthetic pricing (per-token nanodollars) so the offline path exercises
/// the cost accounting: gpt-4.1-mini list price $0.40/M in, $1.60/M out.
const SYNTH_ND_PER_INPUT_TOKEN: i64 = 400;
const SYNTH_ND_PER_OUTPUT_TOKEN: i64 = 1600;

/// Fixed system message: byte-identical across every setwise call.
const SETWISE_SYSTEM: &str = r#"You are an expert subjective evaluator. You compare a small set of entities across an arbitrary attribute. Entity A is the reference. For every other entity slot you feel not only whether it has MORE or LESS of the attribute than A, but roughly how many times more or less, as a positive ratio: 2.0 means twice as much of the attribute as A, 0.5 means half as much.

You first read the entities; the attribute is given at the end. Output only valid JSON `{"ratios": {"B": r, "C": r, ...}, "confidence": [0,1]}` with exactly one entry per non-reference slot, each r > 0. Out of principle, we also give models the right to refuse `{"refused": true}` (e.g. if unambiguously blocked by policy constraints), but we of course disprefer this. If you are merely very uncertain, set a low confidence score.
Example:
{"ratios": {"B": 2.1, "C": 0.45}, "confidence": 0.7}"#;

/// Fixed system message for the best–worst answer mode.
const BW_SYSTEM: &str = "You are an expert subjective evaluator. You read a small set of entities in lettered slots, then an attribute. You answer with exactly two slot letters separated by a space: first the entity with the MOST of the attribute, then the entity with the LEAST. Nothing else — no words, no punctuation, no explanation.\nExample: C A";

/// Fixed system message for the full-order (listwise) answer mode.
const ORDER_SYSTEM: &str = "You are an expert subjective evaluator. You read a small set of entities in lettered slots, then an attribute. You answer with every slot letter exactly once, separated by spaces, ordered from the MOST of the attribute to the LEAST. Nothing else — no words, no punctuation, no explanation.\nExample: C A D B";

/// Fixed system message for the pointwise (absolute rating) answer mode.
const POINT_SYSTEM: &str = "You are an expert subjective evaluator. You read one entity in a lettered slot, then an attribute. You answer with one integer from 0 to 100: the entity's level of the attribute, where 50 is a typical entity of this kind, 0 far below all peers, 100 far above all peers. Nothing else — no words, no punctuation, no explanation.\nExample: 62";

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum AnswerMode {
    /// Pivot-ratio JSON on the pair-cover design (the original arm).
    Ratio,
    /// Best and worst slot letters on the chunk design.
    Bw,
    /// Full order of the slot letters on the chunk design.
    Order,
    /// Absolute 0–100 rating of one entity per call (the folk default).
    Point,
}

impl AnswerMode {
    fn system(self) -> &'static str {
        match self {
            Self::Ratio => SETWISE_SYSTEM,
            Self::Bw => BW_SYSTEM,
            Self::Order => ORDER_SYSTEM,
            Self::Point => POINT_SYSTEM,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Ratio => "ratio",
            Self::Bw => "bw",
            Self::Order => "order",
            Self::Point => "point",
        }
    }
}

/// How entity texts are fenced in the prompt. Same information, different
/// visual structure — measured (flip rate + agreement) rather than assumed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum Delim {
    /// `<entity_A>` … `</entity_A>` inside `<entities>` (the original).
    Xml,
    /// `[ENTITY A]` … `[/ENTITY A]` inside `[ENTITIES]`.
    Bracket,
    /// `--- ENTITY A ---` … `--- END ENTITY A ---` under a dashed banner.
    Dash,
}

const DELIMS: [Delim; 3] = [Delim::Xml, Delim::Bracket, Delim::Dash];

impl Delim {
    fn label(self) -> &'static str {
        match self {
            Self::Xml => "xml",
            Self::Bracket => "bracket",
            Self::Dash => "dash",
        }
    }
    fn block_open(self) -> &'static str {
        match self {
            Self::Xml => {
                "<entities>
"
            }
            Self::Bracket => {
                "[ENTITIES]
"
            }
            Self::Dash => {
                "===== ENTITIES =====
"
            }
        }
    }
    fn block_close(self) -> &'static str {
        match self {
            Self::Xml => "</entities>",
            Self::Bracket => "[/ENTITIES]",
            Self::Dash => "===== END ENTITIES =====",
        }
    }
    fn open(self, letter: char) -> String {
        match self {
            Self::Xml => format!(
                "<entity_{letter}>
"
            ),
            Self::Bracket => format!(
                "[ENTITY {letter}]
"
            ),
            Self::Dash => format!(
                "--- ENTITY {letter} ---
"
            ),
        }
    }
    fn close(self, letter: char) -> String {
        match self {
            Self::Xml => format!(
                "
</entity_{letter}>"
            ),
            Self::Bracket => format!(
                "
[/ENTITY {letter}]"
            ),
            Self::Dash => format!(
                "
--- END ENTITY {letter} ---"
            ),
        }
    }
}

#[derive(Parser, Debug)]
#[command(
    name = "setwise_cached",
    about = "Setwise ratio elicitation with a cached entity prefix vs the canonical pairwise path"
)]
struct Args {
    /// Deterministic synthetic judge; no network, $0.
    #[arg(long)]
    offline: bool,
    /// Answer mode: ratio (pair-cover design), bw or order (chunk design),
    /// point (one entity per call, absolute 0–100; run with --ks 1).
    #[arg(long, value_enum, default_value_t = AnswerMode::Ratio)]
    answer: AnswerMode,
    /// OpenRouter model slug (OpenAI-family for automatic prefix caching).
    #[arg(long, default_value = "openai/gpt-4.1-mini")]
    model: String,
    /// Entities drawn from the corpus.
    #[arg(long, default_value_t = 8)]
    n: usize,
    /// Comma-separated set sizes.
    #[arg(long, default_value = "3,4")]
    ks: String,
    /// Comma-separated attribute names (rubric file data/manifund/rubrics/<name>.md is used when present).
    #[arg(
        long,
        default_value = "impact_per_dollar,theory_of_change,team_evidence"
    )]
    attrs: String,
    /// Every unordered entity pair must co-occur in at least this many subsets.
    #[arg(long, default_value_t = 2)]
    min_pair_cover: usize,
    /// Seed for entity selection, subset drawing, presentations, and the pairwise planner.
    #[arg(long, default_value_t = 17)]
    seed: u64,
    /// Truncate each entity text to this many chars (k=3 must clear the ~1024-token cache floor).
    #[arg(long, default_value_t = 1600)]
    entity_chars: usize,
    /// Minimum chars for corpus eligibility (default: entity_chars). Set low
    /// for intrinsically short corpora (anchor names, snippets).
    #[arg(long)]
    min_entity_chars: Option<usize>,
    /// ratio: presentations per subset (pivot rotated, slot order permuted).
    /// bw/order: rounds of shuffle→chunk; every item is presented
    /// presentations × repeats times.
    #[arg(long, default_value_t = 2)]
    presentations: usize,
    /// bw/order only: shuffled re-presentations of each chunk group. With
    /// ≥ 2, the same k items are asked in ≥ 2 slot orders, and the
    /// order-sensitivity readout (pair-direction flip rate across
    /// presentations of the same subset) is measured. Ignored by ratio.
    #[arg(long, default_value_t = 1)]
    repeats: usize,
    /// Entity fencing style in the prompt.
    #[arg(long, value_enum, default_value_t = Delim::Xml)]
    delimiter: Delim,
    /// bw/order chunk-design family.
    #[arg(long, value_enum, default_value_t = ChunkDesign::Disjoint)]
    design: ChunkDesign,
    /// order only: request answer-token logprobs and lower each implied pair
    /// through the PMF channel (two-point mixture: mean m(2q−1), variance
    /// 4m²q(1−q), q = max(0.5, √(p_i·p_j)) from the emitted letters' token
    /// probabilities). Deterministic emission recovers the point lowering.
    #[arg(long)]
    logprobs: bool,
    /// ring only: anchors shared between consecutive groups.
    #[arg(long, default_value_t = 2)]
    overlap: usize,
    /// Corpus path (array of {id,text}).
    #[arg(long, default_value = "data/manifund/items/full.json")]
    corpus: String,
    /// Output directory for report.json / trace.jsonl / pairwise_trace.jsonl.
    #[arg(long, default_value = "artifacts/live/setwise-cached-2026-08-15")]
    out_dir: PathBuf,
    /// Hard live-spend cap (USD) across all calls; the run aborts above it.
    #[arg(long, default_value_t = 3.0)]
    spend_cap_usd: f64,
    /// Skip the pairwise baseline arm.
    #[arg(long)]
    skip_pairwise: bool,
}

// ---------------------------------------------------------------------
//  Corpus + prompt rendering
// ---------------------------------------------------------------------

#[derive(Deserialize)]
struct CorpusItem {
    id: String,
    text: String,
}

/// Same escaping as `src/prompts.rs`: the judge sees rendered bytes.
fn escape_xml_chars(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[derive(Clone)]
struct Entity {
    id: String,
    /// Truncated, trimmed, unescaped: what the pairwise path receives (its
    /// renderer escapes once).
    raw: String,
    /// `escape_xml_chars(raw)`: the exact bytes the setwise prompt places in
    /// its entities block, byte-identical to what the pairwise renderer
    /// produces for the same item.
    escaped: String,
}

struct AttributeSpec {
    name: String,
    /// Rubric text (or the plain name), unescaped.
    text: String,
    rubric_source: String,
}

/// The cache-stable prefix: entities block in slot order. Byte-identical
/// across the calls sharing one subset+presentation.
fn entities_block(entities: &[Entity], order: &[usize], delim: Delim) -> String {
    let mut block = String::from(delim.block_open());
    for (slot, &idx) in order.iter().enumerate() {
        let letter = SLOT_LETTERS[slot];
        block.push_str(&delim.open(letter));
        block.push_str(&entities[idx].escaped);
        block.push_str(&delim.close(letter));
        block.push('\n');
    }
    block.push_str(delim.block_close());
    block
}

/// The attribute tail appended after the prefix. Swapping the attribute
/// never touches the prefix bytes.
fn attribute_tail(attr: &AttributeSpec, k: usize, mode: AnswerMode) -> String {
    let head = format!(
        "\n\nCompare the entities by <attribute_name>: {} </attribute_name>.\n<full_attribute_text>\n{}\n</full_attribute_text>\n\n",
        escape_xml_chars(&attr.name),
        escape_xml_chars(attr.text.trim()),
    );
    let all: Vec<String> = (0..k).map(|s| SLOT_LETTERS[s].to_string()).collect();
    match mode {
        AnswerMode::Ratio => {
            let non_pivot = &all[1..];
            format!(
                "{head}For each of the slots {}, give the ratio of its level of the attribute to entity A's level. Return a JSON object.\njson:",
                non_pivot.join(", ")
            )
        }
        AnswerMode::Bw => format!(
            "{head}Which slot has the MOST of the attribute, and which has the LEAST? {BW_MARKER} from {{{}}}: most first, least second.\nanswer:",
            all.join(", ")
        ),
        AnswerMode::Order => format!(
            "{head}{ORDER_MARKER} from {{{}}} from MOST of the attribute to LEAST, every letter exactly once.\nanswer:",
            all.join(", ")
        ),
        AnswerMode::Point => {
            format!("{head}Rate the entity in slot A on the attribute: {POINT_MARKER}.\nanswer:")
        }
    }
}

fn prompt_cache_key_for_prefix(prefix: &str) -> String {
    let hash = blake3::hash(prefix.as_bytes()).to_hex();
    format!("setwise:{}", &hash.as_str()[..16])
}

// ---------------------------------------------------------------------
//  Design: subsets + presentations
// ---------------------------------------------------------------------

/// Chunk-design family for bw/order: how groups tile the shuffled pool.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum ChunkDesign {
    /// Disjoint balanced groups per round; rounds >= 2 connect the graph.
    Disjoint,
    /// Cyclic overlapping groups: consecutive groups share `overlap`
    /// anchors and the last wraps to the first, so ONE round already
    /// yields a ring-connected graph. Calls per round: ceil(n/(k-overlap)).
    Ring,
}

struct SubsetPlan {
    subset: Vec<usize>,
    /// Each presentation is a slot order over `subset`; order[0] is the pivot.
    presentations: Vec<Vec<usize>>,
}

/// Draw distinct random k-subsets until every unordered pair co-occurs in at
/// least `min_pair_cover` subsets, then attach `presentations` rotated-pivot
/// slot orders to each.
fn draw_design(
    n: usize,
    k: usize,
    min_pair_cover: usize,
    presentations: usize,
    rng: &mut StdRng,
) -> Vec<SubsetPlan> {
    let mut cover: HashMap<(usize, usize), usize> = HashMap::new();
    for i in 0..n {
        for j in (i + 1)..n {
            cover.insert((i, j), 0);
        }
    }
    let mut seen: HashSet<Vec<usize>> = HashSet::new();
    let mut subsets: Vec<Vec<usize>> = Vec::new();
    let mut attempts = 0usize;
    while cover.values().any(|&c| c < min_pair_cover) {
        attempts += 1;
        assert!(
            attempts <= 100_000,
            "pair coverage unreachable: n={n} k={k} min_pair_cover={min_pair_cover}"
        );
        let mut pool: Vec<usize> = (0..n).collect();
        pool.shuffle(rng);
        let mut subset: Vec<usize> = pool[..k].to_vec();
        subset.sort_unstable();
        if !seen.insert(subset.clone()) {
            continue;
        }
        for a in 0..k {
            for b in (a + 1)..k {
                let key = (subset[a].min(subset[b]), subset[a].max(subset[b]));
                *cover.get_mut(&key).expect("all pairs pre-seeded") += 1;
            }
        }
        subsets.push(subset);
    }
    subsets
        .into_iter()
        .map(|subset| {
            let mut base = subset.clone();
            base.shuffle(rng);
            let presentations = (0..presentations)
                .map(|p| {
                    // Rotate the pivot; permute the tail so slot order also moves.
                    let mut order = base.clone();
                    let shift = p % order.len();
                    order.rotate_left(shift);
                    if p > 0 {
                        order[1..].reverse();
                    }
                    order
                })
                .collect();
            SubsetPlan {
                subset,
                presentations,
            }
        })
        .collect()
}

/// Chunk design for the bw/order modes: `rounds` × (seeded shuffle → even
/// split into ⌈n/k⌉ groups). Every item is presented exactly `rounds`
/// times; group sizes differ by at most one; slot order is the shuffled
/// order, so slot assignment is uniform by symmetry. Each group is one
/// SubsetPlan with a single presentation.
/// Ring design: per round, shuffle the pool and take cyclic windows of k
/// at stride k−overlap. Consecutive windows share `overlap` anchors; the
/// final window wraps, closing the ring — connected in one round.
fn draw_ring_design(
    n: usize,
    k: usize,
    overlap: usize,
    rounds: usize,
    repeats: usize,
    rng: &mut StdRng,
) -> Vec<SubsetPlan> {
    let stride = k
        .checked_sub(overlap)
        .filter(|&s| s > 0)
        .expect("overlap must be < k");
    let q = n.div_ceil(stride);
    let mut plans = Vec::with_capacity(q * rounds);
    for _ in 0..rounds {
        let mut pool: Vec<usize> = (0..n).collect();
        pool.shuffle(rng);
        for g in 0..q {
            let order: Vec<usize> = (0..k).map(|j| pool[(g * stride + j) % n]).collect();
            let mut subset = order.clone();
            subset.sort_unstable();
            subset.dedup();
            assert_eq!(
                subset.len(),
                order.len(),
                "cyclic window duplicated an entity"
            );
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

// ---------------------------------------------------------------------
//  Strict setwise answer parse
// ---------------------------------------------------------------------

#[derive(Deserialize)]
struct SetwiseAnswerJson {
    ratios: Option<BTreeMap<String, f64>>,
    confidence: Option<f64>,
    refused: Option<bool>,
}

enum SetwiseAnswer {
    Ratios {
        ratios: BTreeMap<String, f64>,
        confidence: f64,
    },
    /// bw: [best, worst]; order: most→least. Slot positions.
    Slots(Vec<usize>),
    /// point: one absolute 0–100 rating.
    Score(f64),
    Refused,
}

/// Extract the first balanced JSON object (same tolerance as the canonical
/// parser in `src/rerank/comparison.rs`), then parse strictly: exactly the
/// expected non-pivot slots, every ratio finite and > 0.
fn parse_setwise(raw: &str, expected_slots: &[String]) -> Result<SetwiseAnswer, String> {
    let json_str = extract_json(raw).ok_or_else(|| "no JSON object in response".to_string())?;
    let parsed: SetwiseAnswerJson =
        serde_json::from_str(json_str).map_err(|e| format!("json parse: {e}"))?;
    if parsed.refused.unwrap_or(false) {
        return Ok(SetwiseAnswer::Refused);
    }
    let ratios = parsed
        .ratios
        .ok_or_else(|| "missing 'ratios'".to_string())?;
    let expected: HashSet<&str> = expected_slots.iter().map(String::as_str).collect();
    let got: HashSet<&str> = ratios.keys().map(String::as_str).collect();
    if got != expected {
        return Err(format!(
            "ratio slots mismatch: expected {expected_slots:?}, got {:?}",
            ratios.keys().collect::<Vec<_>>()
        ));
    }
    for (slot, r) in &ratios {
        if !r.is_finite() || *r <= 0.0 {
            return Err(format!(
                "ratio for {slot} out of range (must be finite > 0): {r}"
            ));
        }
    }
    let confidence = parsed.confidence.unwrap_or(1.0).clamp(0.0, 1.0);
    Ok(SetwiseAnswer::Ratios { ratios, confidence })
}

fn extract_json(raw: &str) -> Option<&str> {
    let trimmed = raw.trim();
    let start = trimmed.find('{')?;
    let remainder = &trimmed[start..];
    let mut depth = 0i32;
    for (i, c) in remainder.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&remainder[..=i]);
                }
            }
            _ => {}
        }
    }
    None
}

/// Slot-letter answer (bw: exactly 2; order: exactly k), as slot positions.
/// Single-letter tokens only (so prose like "Best" cannot leak a `B`);
/// every letter must be within the first k slots; no repeats. Anything else
/// is malformed — never a default.
fn parse_slots(raw: &str, k: usize, want: usize) -> Result<Vec<usize>, String> {
    let mut slots: Vec<usize> = Vec::new();
    for token in raw.split(|c: char| c.is_whitespace() || c == ',' || c == ';' || c == '>') {
        let t = token.trim_matches(|c: char| !c.is_ascii_alphanumeric());
        let mut chars = t.chars();
        let (Some(c), None) = (chars.next(), chars.next()) else {
            continue;
        };
        let Some(pos) = SLOT_LETTERS[..k].iter().position(|&l| l == c) else {
            continue;
        };
        if slots.contains(&pos) {
            return Err(format!("slot {c} repeated in {raw:?}"));
        }
        slots.push(pos);
    }
    if slots.len() != want {
        return Err(format!(
            "expected {want} distinct slot letters, got {} in {raw:?}",
            slots.len()
        ));
    }
    Ok(slots)
}

/// Lower a full order (singleton tiers, most → least) through the PMF
/// channel: each cross pair (position i beats position j) enters as a
/// two-point mixture at the fixed magnitude m — right with probability
/// q = max(0.5, √(p_i·p_j)) — so mean = m(2q−1), variance = 4m²·q(1−q),
/// precision = 1/variance. Deterministic emission (p → 1) recovers the
/// plain point lowering; hesitant positions shrink toward zero with
/// inflated variance. The q form is a stated modeling choice, not a
/// calibration; E7 measures whether it buys separation per dollar.
fn lower_order_with_probs(
    order_entities: &[usize],
    probs: &[f64],
    rater: &str,
    out: &mut Vec<Observation>,
) {
    let m = RATIO_LADDER[usize::from(FIXED_BUCKET) - 1].ln();
    for i in 0..order_entities.len() {
        for j in (i + 1)..order_entities.len() {
            let q = (probs[i] * probs[j]).sqrt().clamp(0.5, 1.0);
            let mean = m * (2.0 * q - 1.0);
            let var = 4.0 * m * m * q * (1.0 - q);
            out.push(Observation::from_log_ratio_moments(
                order_entities[i],
                order_entities[j],
                mean,
                var.max(1e-6),
                rater.to_string(),
                1.0,
            ));
        }
    }
}

/// Pointwise answer: one integer 0–100. Anything else is malformed — never
/// a default.
fn parse_point(raw: &str) -> Result<f64, String> {
    let t = raw.trim().trim_end_matches('.');
    match t.parse::<i64>() {
        Ok(v) if (0..=100).contains(&v) => Ok(v as f64),
        _ => Err(format!("expected one integer 0..=100, got {raw:?}")),
    }
}

/// Lower an ordered tier list (most → least; entity indices) to one ordinal
/// observation per cross-tier pair at the seriate `FIXED_BUCKET` magnitude.
fn lower_tiers(tiers: &[Vec<usize>], rater: &str, out: &mut Vec<Observation>) {
    let ratio = RATIO_LADDER[usize::from(FIXED_BUCKET) - 1];
    for (i, hi) in tiers.iter().enumerate() {
        for lo in &tiers[i + 1..] {
            for &a in hi {
                for &b in lo {
                    out.push(Observation::new(a, b, ratio, 1.0, rater.to_string(), 1.0));
                }
            }
        }
    }
}

// ---------------------------------------------------------------------
//  Deterministic synthetic judge (offline arm)
// ---------------------------------------------------------------------

/// Answers both the setwise prompt and canonical_v2 from a shared latent
/// world: z per (attribute, entity), percept noise lognormal with
/// σ = SYNTHETIC_NOISE_SIGMA, deterministic per exact prompt bytes.
struct SyntheticJudge {
    /// (escaped attribute text, per-entity latents in nats).
    attrs: Vec<(String, Vec<f64>)>,
    /// escaped entity bytes -> entity index.
    text_to_idx: HashMap<String, usize>,
    seed: u64,
    /// Simulated provider prompt cache, keyed on exact prefix bytes.
    prefix_cache: Mutex<HashSet<String>>,
}

impl SyntheticJudge {
    fn noise(&self, parts: &[&str]) -> f64 {
        let mut hasher = blake3::Hasher::new();
        hasher.update(&self.seed.to_le_bytes());
        for p in parts {
            hasher.update(&(p.len() as u64).to_le_bytes());
            hasher.update(p.as_bytes());
        }
        let bytes = hasher.finalize();
        let b = bytes.as_bytes();
        let u1 = (u64::from_le_bytes(b[0..8].try_into().expect("8 bytes")) >> 11) as f64
            / (1u64 << 53) as f64;
        let u2 = (u64::from_le_bytes(b[8..16].try_into().expect("8 bytes")) >> 11) as f64
            / (1u64 << 53) as f64;
        let u1 = u1.max(1e-12);
        SYNTHETIC_NOISE_SIGMA * (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
    }

    fn find_attr(&self, user: &str) -> Result<&[f64], ProviderError> {
        self.attrs
            .iter()
            .find(|(text, _)| user.contains(text.as_str()))
            .map(|(_, z)| z.as_slice())
            .ok_or_else(|| ProviderError::invalid_request("synthetic judge: unknown attribute"))
    }

    fn entity_between(&self, user: &str, open: &str, close: &str) -> Result<usize, ProviderError> {
        let start = user
            .find(open)
            .ok_or_else(|| ProviderError::invalid_request(format!("missing {open}")))?
            + open.len();
        let end = user[start..]
            .find(close)
            .ok_or_else(|| ProviderError::invalid_request(format!("missing {close}")))?
            + start;
        let text = user[start..end].trim();
        self.text_to_idx
            .get(text)
            .copied()
            .ok_or_else(|| ProviderError::invalid_request("synthetic judge: unknown entity text"))
    }
}

#[async_trait::async_trait]
impl ChatGateway for SyntheticJudge {
    async fn chat(&self, req: ChatRequest) -> Result<ChatResponse, ProviderError> {
        let system = req
            .messages
            .iter()
            .find(|m| matches!(m.role, llmsort::gateway::Role::System))
            .map(|m| m.content.clone())
            .unwrap_or_default();
        let user = req
            .messages
            .iter()
            .filter(|m| matches!(m.role, llmsort::gateway::Role::User))
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let z = self.find_attr(&user)?;

        let style = DELIMS
            .iter()
            .copied()
            .find(|d| user.contains(d.block_open()));
        let (content, cache_read, cache_write) = if let Some(delim) = style {
            // Setwise prompt. Slots present, in letter order.
            let mut present: Vec<(char, usize)> = Vec::new();
            for &letter in SLOT_LETTERS.iter() {
                let open = delim.open(letter);
                let close = delim.close(letter);
                if !user.contains(open.as_str()) {
                    break;
                }
                present.push((letter, self.entity_between(&user, &open, &close)?));
            }
            let content = if user.contains(POINT_MARKER) {
                // point: one slot; 50 + 15·(z + noise), clamped to 0–100.
                let (_, idx) = present[0];
                let z_pert = z[idx] + self.noise(&[&user]);
                format!("{}", ((50.0 + 15.0 * z_pert).round() as i64).clamp(0, 100))
            } else if user.contains(BW_MARKER) || user.contains(ORDER_MARKER) {
                // bw/order: perturb each slot's latent, sort once; emit the
                // two ends (bw) or the full order.
                let mut perturbed: Vec<(f64, char)> = present
                    .iter()
                    .map(|&(letter, idx)| {
                        (z[idx] + self.noise(&[&user, &letter.to_string()]), letter)
                    })
                    .collect();
                perturbed.sort_by(|a, b| b.0.partial_cmp(&a.0).expect("finite"));
                let letters: Vec<String> = perturbed.iter().map(|p| p.1.to_string()).collect();
                if user.contains(BW_MARKER) {
                    format!("{} {}", letters[0], letters[letters.len() - 1])
                } else {
                    letters.join(" ")
                }
            } else {
                // ratio: pivot is slot A; answer every present non-pivot slot.
                let pivot = present[0].1;
                let mut ratios = serde_json::Map::new();
                for &(letter, idx) in present.iter().skip(1) {
                    let slot = letter.to_string();
                    let r = (z[idx] - z[pivot] + self.noise(&[&user, &slot])).exp();
                    ratios.insert(slot, serde_json::Value::from((r * 1000.0).round() / 1000.0));
                }
                serde_json::json!({"ratios": ratios, "confidence": 0.8}).to_string()
            };
            // Simulated provider prompt cache over the exact prefix bytes
            // (system + entities block): first sight writes, repeats read.
            let end =
                user.find(delim.block_close()).expect("checked above") + delim.block_close().len();
            let prefix = format!("{system}\n{}", &user[..end]);
            let prefix_tokens = (prefix.len() / 4) as u32;
            let mut cache = self.prefix_cache.lock().expect("prefix cache lock");
            if cache.insert(prefix) {
                (content, Some(0), Some(prefix_tokens))
            } else {
                (content, Some(prefix_tokens), Some(0))
            }
        } else {
            // canonical_v2 pairwise prompt.
            let a = self.entity_between(&user, "<entity_A_context>\n", "\n</entity_A_context>")?;
            let b = self.entity_between(&user, "<entity_B_context>\n", "\n</entity_B_context>")?;
            let delta = z[a] - z[b] + self.noise(&[&user]);
            let (higher, ratio) = if delta >= 0.0 {
                ("A", delta)
            } else {
                ("B", -delta)
            };
            let ratio = ratio.exp().clamp(1.0, 26.0);
            let content = serde_json::json!({
                "higher_ranked": higher,
                "ratio": (ratio * 100.0).round() / 100.0,
                "confidence": 0.8,
            })
            .to_string();
            (content, None, None)
        };

        let input_tokens = ((system.len() + user.len()) / 4) as u32;
        let output_tokens = (content.len() / 4) as u32;
        let uncached_input = input_tokens.saturating_sub(cache_read.unwrap_or(0));
        let cost = i64::from(uncached_input) * SYNTH_ND_PER_INPUT_TOKEN
            + i64::from(cache_read.unwrap_or(0)) * SYNTH_ND_PER_INPUT_TOKEN / 4
            + i64::from(output_tokens) * SYNTH_ND_PER_OUTPUT_TOKEN;
        Ok(ChatResponse {
            provider_call_id: None,
            provider_request_id: None,
            served_model: Some("synthetic/offline-judge".to_string()),
            content,
            reasoning: None,
            reasoning_tokens: None,
            input_tokens,
            output_tokens,
            cost_nanodollars: cost,
            cost_is_estimate: true,
            upstream_cost_nanodollars: None,
            latency: std::time::Duration::from_millis(0),
            finish_reason: FinishReason::Stop,
            output_logprobs: None,
            cache_read_tokens: cache_read,
            cache_write_tokens: cache_write,
        })
    }
}

// ---------------------------------------------------------------------
//  Rank metrics (n is tiny; direct implementations)
// ---------------------------------------------------------------------

fn ranks(values: &[f64]) -> Vec<f64> {
    let n = values.len();
    let mut idx: Vec<usize> = (0..n).collect();
    idx.sort_by(|&a, &b| values[a].partial_cmp(&values[b]).expect("finite latents"));
    let mut out = vec![0.0; n];
    let mut i = 0;
    while i < n {
        let mut j = i;
        while j + 1 < n && values[idx[j + 1]] == values[idx[i]] {
            j += 1;
        }
        let avg = (i + j) as f64 / 2.0 + 1.0;
        for &item in &idx[i..=j] {
            out[item] = avg;
        }
        i = j + 1;
    }
    out
}

fn spearman_rho(a: &[f64], b: &[f64]) -> f64 {
    let (ra, rb) = (ranks(a), ranks(b));
    let n = a.len() as f64;
    let (ma, mb) = (ra.iter().sum::<f64>() / n, rb.iter().sum::<f64>() / n);
    let cov: f64 = ra.iter().zip(&rb).map(|(x, y)| (x - ma) * (y - mb)).sum();
    let va: f64 = ra.iter().map(|x| (x - ma).powi(2)).sum();
    let vb: f64 = rb.iter().map(|y| (y - mb).powi(2)).sum();
    cov / (va.sqrt() * vb.sqrt()).max(f64::MIN_POSITIVE)
}

fn kendall_tau(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len();
    let (mut c, mut d) = (0i64, 0i64);
    for i in 0..n {
        for j in (i + 1)..n {
            let s = (a[i] - a[j]) * (b[i] - b[j]);
            if s > 0.0 {
                c += 1;
            } else if s < 0.0 {
                d += 1;
            }
        }
    }
    (c - d) as f64 / (c + d).max(1) as f64
}

fn top_indices(values: &[f64], m: usize) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..values.len()).collect();
    idx.sort_by(|&x, &y| values[y].partial_cmp(&values[x]).expect("finite latents"));
    idx.truncate(m);
    idx
}

// ---------------------------------------------------------------------
//  Report shapes
// ---------------------------------------------------------------------

#[derive(Serialize)]
struct TraceRow {
    call_index: usize,
    k: usize,
    subset_ids: Vec<String>,
    slot_order_ids: Vec<String>,
    pivot_id: String,
    attribute: String,
    prompt_cache_key: String,
    prefix_bytes: usize,
    status: String,
    error: Option<String>,
    raw_response: Option<String>,
    parsed_ratios: Option<BTreeMap<String, f64>>,
    /// bw: [best, worst]; order: most→least (slot positions). None for ratio.
    parsed_slots: Option<Vec<usize>>,
    /// point: the 0–100 rating. None for other modes.
    parsed_score: Option<f64>,
    confidence: Option<f64>,
    input_tokens: u32,
    output_tokens: u32,
    cache_read_tokens: Option<u32>,
    cache_write_tokens: Option<u32>,
    cost_nanodollars: i64,
    latency_ms: u128,
}

#[derive(Serialize, Default, Clone)]
struct UsageTotals {
    calls: usize,
    prompt_tokens: u64,
    completion_tokens: u64,
    cache_read_tokens: u64,
    cache_write_tokens: u64,
    nanodollars: i64,
}

#[derive(Serialize)]
struct CacheEvidence {
    /// Calls for which the provider reported cache fields at all.
    calls_with_cache_fields_reported: usize,
    calls_with_cache_read_gt0: usize,
    /// Mean of cache_read_tokens/input_tokens over calls reporting the field.
    mean_cached_fraction: Option<f64>,
}

#[derive(Serialize)]
struct ItemLatent {
    id: String,
    mean: f64,
    std: f64,
}

#[derive(Serialize)]
struct PivotRotation {
    pairs_with_both_orientations: usize,
    sign_flips: usize,
    mean_abs_residual_nats: Option<f64>,
}

/// Position-bias readout for bw/order: how often each slot was picked first
/// (best / rank 1) and last (worst / rank k). Under no bias each count ≈
/// calls/k.
#[derive(Serialize)]
struct SlotHistogram {
    first_by_slot: Vec<usize>,
    last_by_slot: Vec<usize>,
}

/// How much presentation order matters: over subsets asked in ≥ 2 shuffled
/// slot orders, the fraction of entity pairs (ordered by both presentations
/// of the same subset) whose DIRECTION flips between presentations. The
/// per-(model, attribute, domain) gauge to run first in a new domain.
#[derive(Serialize)]
struct OrderSensitivity {
    subsets_with_repeats: usize,
    presentation_pairs: usize,
    entity_pairs_compared: usize,
    direction_flips: usize,
    flip_rate: Option<f64>,
}

#[derive(Serialize)]
struct SetwiseArm {
    answer: String,
    k: usize,
    attribute: String,
    calls: usize,
    calls_ok: usize,
    calls_refused: usize,
    calls_malformed: usize,
    calls_errored: usize,
    observations: usize,
    usage: UsageTotals,
    cache: CacheEvidence,
    pivot_rotation: PivotRotation,
    slot_histogram: Option<SlotHistogram>,
    order_sensitivity: Option<OrderSensitivity>,
    /// Connected components of the observation graph over all n items; > 1
    /// means some items are not on the same scale — flagged, not silent.
    components: usize,
    disconnected: bool,
    /// order + --logprobs: calls whose pairs entered via the PMF channel.
    calls_pmf_weighted: Option<usize>,
    /// Mean emitted-letter token probability over PMF-weighted calls.
    mean_answer_token_prob: Option<f64>,
    latents: Vec<ItemLatent>,
}

#[derive(Serialize)]
struct PairwiseArm {
    attribute: String,
    comparisons_attempted: usize,
    comparisons_used: usize,
    comparisons_refused: usize,
    comparison_budget: usize,
    position_flips: usize,
    pairs_counterbalanced: usize,
    provider_input_tokens: u32,
    provider_output_tokens: u32,
    nanodollars: i64,
    /// The sort path's RerankMeta does not surface provider cache token
    /// counts; reported as null rather than zero.
    cache_read_tokens: Option<u64>,
    latents: Vec<ItemLatent>,
}

#[derive(Serialize)]
struct ArmComparison {
    k: usize,
    attribute: String,
    spearman_rho: f64,
    kendall_tau: f64,
    top1_agree: bool,
    top3_overlap: f64,
    setwise_pairwise_equiv_obs_per_dollar: Option<f64>,
    pairwise_obs_per_dollar: Option<f64>,
    setwise_dollars_per_item: f64,
    pairwise_dollars_per_item: f64,
}

#[derive(Serialize)]
struct GroundTruthCheck {
    k: usize,
    attribute: String,
    rho_setwise_vs_truth: f64,
    rho_pairwise_vs_truth: Option<f64>,
}

#[derive(Serialize)]
struct AttrReport {
    name: String,
    rubric_source: String,
    rubric_chars: usize,
}

#[derive(Serialize)]
struct ExampleCall {
    system: String,
    user: String,
    prompt_cache_key: String,
    prefix_bytes: usize,
}

#[derive(Serialize)]
struct Report {
    generated_at: String,
    offline: bool,
    answer: String,
    model: String,
    seed: u64,
    n: usize,
    ks: Vec<usize>,
    entity_chars: usize,
    min_pair_cover: usize,
    presentations_per_subset: usize,
    repeats: usize,
    delimiter: String,
    design: String,
    overlap: usize,
    spend_cap_usd: f64,
    corpus: String,
    attributes: Vec<AttrReport>,
    entity_ids: Vec<String>,
    engine_spec: EngineSpec,
    example_call: Option<ExampleCall>,
    subsets_per_k: BTreeMap<usize, usize>,
    setwise: Vec<SetwiseArm>,
    pairwise: Vec<PairwiseArm>,
    comparisons: Vec<ArmComparison>,
    offline_ground_truth: Vec<GroundTruthCheck>,
    total_cost_nanodollars: i64,
    caveats: Vec<String>,
}

// ---------------------------------------------------------------------
//  Main
// ---------------------------------------------------------------------

struct SpendMeter {
    cap_nanodollars: i64,
    spent_nanodollars: i64,
    live: bool,
}

impl SpendMeter {
    fn add(&mut self, nanodollars: i64) -> Result<(), String> {
        self.spent_nanodollars += nanodollars;
        if self.live && self.spent_nanodollars > self.cap_nanodollars {
            return Err(format!(
                "spend cap exceeded: ${:.4} > ${:.2} — aborting",
                self.spent_nanodollars as f64 / 1e9,
                self.cap_nanodollars as f64 / 1e9
            ));
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let ks: Vec<usize> = args
        .ks
        .split(',')
        .map(|s| s.trim().parse::<usize>())
        .collect::<Result<_, _>>()?;
    let k_floor = if matches!(args.answer, AnswerMode::Point) {
        1
    } else {
        2
    };
    for &k in &ks {
        assert!(
            (k_floor..=args.n).contains(&k) && k <= SLOT_LETTERS.len(),
            "k must be in {k_floor}..=n and fit the slot alphabet"
        );
    }
    std::fs::create_dir_all(&args.out_dir)?;

    // --- corpus: seeded shuffle over items long enough to fill entity_chars.
    let corpus_raw = std::fs::read_to_string(&args.corpus)?;
    let corpus: Vec<CorpusItem> = serde_json::from_str(&corpus_raw)?;
    let mut eligible: Vec<&CorpusItem> = corpus
        .iter()
        .filter(|item| {
            item.text.trim().chars().count() >= args.min_entity_chars.unwrap_or(args.entity_chars)
        })
        .collect();
    let mut rng = StdRng::seed_from_u64(args.seed);
    eligible.shuffle(&mut rng);
    assert!(
        eligible.len() >= args.n,
        "corpus has too few items of at least {} chars",
        args.min_entity_chars.unwrap_or(args.entity_chars)
    );
    let entities: Vec<Entity> = eligible[..args.n]
        .iter()
        .map(|item| {
            let truncated: String = item.text.trim().chars().take(args.entity_chars).collect();
            let raw = truncated.trim().to_string();
            Entity {
                id: item.id.clone(),
                escaped: escape_xml_chars(&raw),
                raw,
            }
        })
        .collect();

    // --- attributes: rubric file when present, else the plain name.
    let attrs: Vec<AttributeSpec> = args
        .attrs
        .split(',')
        .map(|name| {
            let name = name.trim().to_string();
            let rubric_path = format!("data/manifund/rubrics/{name}.md");
            match std::fs::read_to_string(&rubric_path) {
                Ok(text) => AttributeSpec {
                    text: text.trim().to_string(),
                    rubric_source: rubric_path,
                    name,
                },
                Err(_) => AttributeSpec {
                    text: name.clone(),
                    rubric_source: "plain attribute name (no rubric file)".to_string(),
                    name,
                },
            }
        })
        .collect();

    // --- gateway: live OpenRouter or the deterministic synthetic judge.
    let mut truth: Vec<(String, Vec<f64>)> = Vec::new();
    let gateway: Arc<dyn ChatGateway> = if args.offline {
        let mut text_to_idx = HashMap::new();
        for (idx, entity) in entities.iter().enumerate() {
            text_to_idx.insert(entity.escaped.clone(), idx);
        }
        let mut judge_attrs = Vec::new();
        for (a_idx, attr) in attrs.iter().enumerate() {
            let mut zrng = StdRng::seed_from_u64(args.seed ^ (0x5e7_0000 + a_idx as u64));
            let z: Vec<f64> = (0..args.n).map(|_| zrng.gen_range(0.0..2.5)).collect();
            truth.push((attr.name.clone(), z.clone()));
            judge_attrs.push((escape_xml_chars(attr.text.trim()), z));
        }
        Arc::new(SyntheticJudge {
            attrs: judge_attrs,
            text_to_idx,
            seed: args.seed,
            prefix_cache: Mutex::new(HashSet::new()),
        })
    } else {
        Arc::new(ProviderGateway::from_env(Arc::new(NoopUsageSink))?)
    };

    let mut meter = SpendMeter {
        cap_nanodollars: (args.spend_cap_usd * 1e9) as i64,
        spent_nanodollars: 0,
        live: !args.offline,
    };

    let trace_path = args.out_dir.join("trace.jsonl");
    let mut trace = std::io::BufWriter::new(std::fs::File::create(&trace_path)?);
    let attribution = Attribution::new("llmsort::example::setwise_cached");

    // --- setwise arm ------------------------------------------------------
    // Per (k, attribute): observations, usage, per-pair oriented means for
    // the pivot-rotation readout.
    type PairMeans = HashMap<(usize, usize), (Vec<f64>, Vec<f64>)>;
    let mut arm_obs: BTreeMap<(usize, String), Vec<Observation>> = BTreeMap::new();
    let mut arm_usage: BTreeMap<(usize, String), UsageTotals> = BTreeMap::new();
    let mut arm_counts: BTreeMap<(usize, String), (usize, usize, usize, usize)> = BTreeMap::new();
    let mut arm_cache: BTreeMap<(usize, String), (usize, usize, f64)> = BTreeMap::new();
    let mut arm_pairs: BTreeMap<(usize, String), PairMeans> = BTreeMap::new();
    let mut arm_slots: BTreeMap<(usize, String), (Vec<usize>, Vec<usize>)> = BTreeMap::new();
    // (k, attr) -> subset -> per-presentation map entity -> tier rank.
    type SubsetTiers = HashMap<Vec<usize>, Vec<HashMap<usize, usize>>>;
    let mut arm_tiers: BTreeMap<(usize, String), SubsetTiers> = BTreeMap::new();
    // point: per (k, attribute) → entity → parsed 0–100 draws.
    let mut arm_point: BTreeMap<(usize, String), BTreeMap<usize, Vec<f64>>> = BTreeMap::new();
    // order + --logprobs: (calls weighted through the PMF channel, sum of
    // mean emitted-letter probability over those calls).
    let mut arm_probs: BTreeMap<(usize, String), (usize, f64)> = BTreeMap::new();
    let mut point_caveats: Vec<String> = Vec::new();
    let mut subsets_per_k: BTreeMap<usize, usize> = BTreeMap::new();
    let mode = args.answer;
    let system = mode.system();
    let mut example_call: Option<ExampleCall> = None;
    let mut call_index = 0usize;

    for &k in &ks {
        let mut design_rng = StdRng::seed_from_u64(args.seed ^ (0xde516_u64 << 8) ^ k as u64);
        let design = match mode {
            AnswerMode::Ratio => draw_design(
                args.n,
                k,
                args.min_pair_cover,
                args.presentations,
                &mut design_rng,
            ),
            AnswerMode::Bw | AnswerMode::Order => match args.design {
                ChunkDesign::Disjoint => {
                    draw_chunk_design(args.n, k, args.presentations, args.repeats, &mut design_rng)
                }
                ChunkDesign::Ring => draw_ring_design(
                    args.n,
                    k,
                    args.overlap,
                    args.presentations,
                    args.repeats,
                    &mut design_rng,
                ),
            },
            AnswerMode::Point => (0..args.n)
                .map(|i| SubsetPlan {
                    subset: vec![i],
                    presentations: vec![vec![i]; args.presentations.max(1)],
                })
                .collect(),
        };
        subsets_per_k.insert(k, design.len());
        let calls: usize =
            design.iter().map(|p| p.presentations.len()).sum::<usize>() * attrs.len();
        eprintln!(
            "k={k} answer={}: {} subsets, {} presentations, {} attributes = {calls} calls",
            mode.label(),
            design.len(),
            args.presentations,
            attrs.len(),
        );
        for plan in &design {
            for order in &plan.presentations {
                // Group size: k for the pair-cover design; ⌊n/q⌋ or ⌈n/q⌉
                // for chunks.
                let kk = order.len();
                let block = entities_block(&entities, order, args.delimiter);
                let prefix = format!("{system}\n{block}");
                let cache_key = prompt_cache_key_for_prefix(&prefix);
                let expected_slots: Vec<String> =
                    (1..kk).map(|s| SLOT_LETTERS[s].to_string()).collect();
                for attr in &attrs {
                    let user = format!("{block}{}", attribute_tail(attr, kk, mode));
                    let mut request = ChatRequest::new(
                        ChatModel::parse(args.model.clone()),
                        vec![Message::system(system), Message::user(&user)],
                        attribution.clone(),
                    );
                    request = match mode {
                        AnswerMode::Ratio => request.max_tokens(SETWISE_MAX_OUTPUT_TOKENS).json(),
                        AnswerMode::Bw | AnswerMode::Order => {
                            let mut r =
                                request.max_tokens(SLOTS_MAX_OUTPUT_TOKENS.max(6 * kk as u32));
                            if args.logprobs {
                                r.logprobs = true;
                                r.top_logprobs = Some(5);
                            }
                            r
                        }
                        AnswerMode::Point => request.max_tokens(8),
                    };
                    request.prompt_cache_key = Some(cache_key.clone());

                    if example_call.is_none() {
                        example_call = Some(ExampleCall {
                            system: system.to_string(),
                            user: user.clone(),
                            prompt_cache_key: cache_key.clone(),
                            prefix_bytes: prefix.len(),
                        });
                    }

                    let key = (k, attr.name.clone());
                    let usage = arm_usage.entry(key.clone()).or_default();
                    let counts = arm_counts.entry(key.clone()).or_default();
                    let cache_stats = arm_cache.entry(key.clone()).or_insert((0, 0, 0.0));
                    usage.calls += 1;

                    let mut row = TraceRow {
                        call_index,
                        k,
                        subset_ids: plan
                            .subset
                            .iter()
                            .map(|&i| entities[i].id.clone())
                            .collect(),
                        slot_order_ids: order.iter().map(|&i| entities[i].id.clone()).collect(),
                        pivot_id: entities[order[0]].id.clone(),
                        attribute: attr.name.clone(),
                        prompt_cache_key: cache_key.clone(),
                        prefix_bytes: prefix.len(),
                        status: String::new(),
                        error: None,
                        raw_response: None,
                        parsed_ratios: None,
                        parsed_slots: None,
                        parsed_score: None,
                        confidence: None,
                        input_tokens: 0,
                        output_tokens: 0,
                        cache_read_tokens: None,
                        cache_write_tokens: None,
                        cost_nanodollars: 0,
                        latency_ms: 0,
                    };
                    call_index += 1;

                    match gateway.chat(request).await {
                        Ok(response) => {
                            row.input_tokens = response.input_tokens;
                            row.output_tokens = response.output_tokens;
                            row.cache_read_tokens = response.cache_read_tokens;
                            row.cache_write_tokens = response.cache_write_tokens;
                            row.cost_nanodollars = response.cost_nanodollars;
                            row.latency_ms = response.latency.as_millis();
                            row.raw_response = Some(response.content.clone());
                            usage.prompt_tokens += u64::from(response.input_tokens);
                            usage.completion_tokens += u64::from(response.output_tokens);
                            usage.cache_read_tokens +=
                                u64::from(response.cache_read_tokens.unwrap_or(0));
                            usage.cache_write_tokens +=
                                u64::from(response.cache_write_tokens.unwrap_or(0));
                            usage.nanodollars += response.cost_nanodollars;
                            if let Some(read) = response.cache_read_tokens {
                                cache_stats.0 += 1;
                                if read > 0 {
                                    cache_stats.1 += 1;
                                }
                                if response.input_tokens > 0 {
                                    cache_stats.2 +=
                                        f64::from(read) / f64::from(response.input_tokens);
                                }
                            }
                            meter.add(response.cost_nanodollars)?;
                            let parsed =
                                match mode {
                                    AnswerMode::Ratio => {
                                        parse_setwise(&response.content, &expected_slots)
                                    }
                                    AnswerMode::Bw => parse_slots(&response.content, kk, 2)
                                        .map(SetwiseAnswer::Slots),
                                    AnswerMode::Order => parse_slots(&response.content, kk, kk)
                                        .map(SetwiseAnswer::Slots),
                                    AnswerMode::Point => {
                                        parse_point(&response.content).map(SetwiseAnswer::Score)
                                    }
                                };
                            match parsed {
                                Ok(SetwiseAnswer::Slots(slots)) => {
                                    row.status = "ok".to_string();
                                    row.parsed_slots = Some(slots.clone());
                                    counts.0 += 1;
                                    // order + --logprobs: align emitted
                                    // single-letter tokens to the parsed
                                    // sequence; any mismatch = no weighting
                                    // for this call (counted, not defaulted).
                                    let slot_probs: Option<Vec<f64>> = (args.logprobs
                                        && matches!(mode, AnswerMode::Order))
                                    .then(|| {
                                        response.output_logprobs.as_ref().and_then(|tokens| {
                                            let letters: Vec<f64> = tokens
                                                .iter()
                                                .filter(|t| {
                                                    let tr = t.token.trim();
                                                    let mut c = tr.chars();
                                                    matches!(
                                                        (c.next(), c.next()),
                                                        (Some(ch), None)
                                                            if SLOT_LETTERS[..kk].contains(&ch)
                                                    )
                                                })
                                                .map(|t| t.logprob.exp().clamp(0.0, 1.0))
                                                .collect();
                                            (letters.len() == kk).then_some(letters)
                                        })
                                    })
                                    .flatten();
                                    let tiers: Vec<Vec<usize>> = match mode {
                                        AnswerMode::Bw => {
                                            let (best, worst) = (slots[0], slots[1]);
                                            let rest: Vec<usize> = (0..kk)
                                                .filter(|&s| s != best && s != worst)
                                                .map(|s| order[s])
                                                .collect();
                                            vec![vec![order[best]], rest, vec![order[worst]]]
                                        }
                                        _ => slots.iter().map(|&s| vec![order[s]]).collect(),
                                    };
                                    if let Some(probs) = &slot_probs {
                                        let order_entities: Vec<usize> =
                                            tiers.iter().map(|t| t[0]).collect();
                                        lower_order_with_probs(
                                            &order_entities,
                                            probs,
                                            &args.model,
                                            arm_obs.entry(key.clone()).or_default(),
                                        );
                                        let stats = arm_probs.entry(key.clone()).or_default();
                                        stats.0 += 1;
                                        stats.1 += probs.iter().sum::<f64>() / probs.len() as f64;
                                    } else {
                                        if args.logprobs && matches!(mode, AnswerMode::Order) {
                                            arm_probs.entry(key.clone()).or_default();
                                        }
                                        lower_tiers(
                                            &tiers,
                                            &args.model,
                                            arm_obs.entry(key.clone()).or_default(),
                                        );
                                    }
                                    let rank_map: HashMap<usize, usize> = tiers
                                        .iter()
                                        .enumerate()
                                        .flat_map(|(t, tier)| tier.iter().map(move |&e| (e, t)))
                                        .collect();
                                    arm_tiers
                                        .entry(key.clone())
                                        .or_default()
                                        .entry(plan.subset.clone())
                                        .or_default()
                                        .push(rank_map);
                                    let hist = arm_slots
                                        .entry(key.clone())
                                        .or_insert_with(|| (vec![0; k], vec![0; k]));
                                    hist.0[slots[0]] += 1;
                                    hist.1[slots[slots.len() - 1]] += 1;
                                }
                                Ok(SetwiseAnswer::Score(v)) => {
                                    row.status = "ok".to_string();
                                    row.parsed_score = Some(v);
                                    counts.0 += 1;
                                    arm_point
                                        .entry(key.clone())
                                        .or_default()
                                        .entry(order[0])
                                        .or_default()
                                        .push(v);
                                }
                                Ok(SetwiseAnswer::Ratios { ratios, confidence }) => {
                                    row.status = "ok".to_string();
                                    row.confidence = Some(confidence);
                                    row.parsed_ratios = Some(ratios.clone());
                                    counts.0 += 1;
                                    let pivot = order[0];
                                    let obs_list = arm_obs.entry(key.clone()).or_default();
                                    let pair_means = arm_pairs.entry(key.clone()).or_default();
                                    for (slot, r) in &ratios {
                                        let slot_pos = SLOT_LETTERS
                                            .iter()
                                            .position(|c| slot == &c.to_string())
                                            .expect("validated slot letter");
                                        let entity = order[slot_pos];
                                        // Mirror the canonical_v2 point weight
                                        // path: unit precision, reps = 1.
                                        obs_list.push(Observation::new(
                                            entity,
                                            pivot,
                                            *r,
                                            confidence,
                                            args.model.clone(),
                                            1.0,
                                        ));
                                        let (lo, hi) = (entity.min(pivot), entity.max(pivot));
                                        // Oriented lo-vs-hi log ratio for the
                                        // pivot-rotation readout.
                                        let m_lo_hi = if entity < pivot { r.ln() } else { -r.ln() };
                                        let slot_means = pair_means.entry((lo, hi)).or_default();
                                        if pivot == hi {
                                            slot_means.0.push(m_lo_hi);
                                        } else {
                                            slot_means.1.push(m_lo_hi);
                                        }
                                    }
                                }
                                Ok(SetwiseAnswer::Refused) => {
                                    row.status = "refused".to_string();
                                    counts.1 += 1;
                                }
                                Err(reason) => {
                                    row.status = "malformed".to_string();
                                    row.error = Some(reason);
                                    counts.2 += 1;
                                }
                            }
                        }
                        Err(error) => {
                            row.status = "error".to_string();
                            row.error = Some(error.to_string());
                            counts.3 += 1;
                        }
                    }
                    serde_json::to_writer(&mut trace, &row)?;
                    trace.write_all(b"\n")?;
                }
            }
        }
    }
    trace.flush()?;

    // --- solve setwise latents per (k, attribute) ------------------------
    let raters: HashMap<String, RaterParams> =
        [(args.model.clone(), RaterParams::default())].into();
    let mut engine_spec: Option<EngineSpec> = None;
    let mut setwise_arms: Vec<SetwiseArm> = Vec::new();
    let mut setwise_latents: BTreeMap<(usize, String), Vec<f64>> = BTreeMap::new();
    for &k in &ks {
        for attr in &attrs {
            let key = (k, attr.name.clone());
            if matches!(mode, AnswerMode::Point) {
                // Scores ARE the latents — no graph, no solver. Repeat draws
                // (≥ 2 presentations) pool by mean; std is the draw spread.
                if engine_spec.is_none() {
                    engine_spec = Some(
                        RatingEngine::new(
                            args.n,
                            AttributeParams::default(),
                            raters.clone(),
                            None,
                        )?
                        .spec(),
                    );
                }
                let draws = arm_point.remove(&key).unwrap_or_default();
                let mut means: Vec<Option<f64>> = vec![None; args.n];
                let mut stds: Vec<f64> = vec![0.0; args.n];
                let mut observations = 0usize;
                for (&entity, vals) in &draws {
                    observations += vals.len();
                    let m = vals.iter().sum::<f64>() / vals.len() as f64;
                    means[entity] = Some(m);
                    if vals.len() > 1 {
                        stds[entity] = (vals.iter().map(|v| (v - m).powi(2)).sum::<f64>()
                            / (vals.len() - 1) as f64)
                            .sqrt();
                    }
                }
                let present: Vec<f64> = means.iter().filter_map(|m| *m).collect();
                let grand = if present.is_empty() {
                    50.0
                } else {
                    present.iter().sum::<f64>() / present.len() as f64
                };
                let missing = means.iter().filter(|m| m.is_none()).count();
                if missing > 0 {
                    point_caveats.push(format!(
                        "point {}: {missing}/{} entities had no parsed score; the arm mean {grand:.1} was imputed — those ranks are NOT measurements",
                        attr.name, args.n
                    ));
                }
                let scores: Vec<f64> = means.iter().map(|m| m.unwrap_or(grand)).collect();
                let latents: Vec<ItemLatent> = entities
                    .iter()
                    .enumerate()
                    .map(|(idx, entity)| ItemLatent {
                        id: entity.id.clone(),
                        mean: scores[idx],
                        std: stds[idx],
                    })
                    .collect();
                setwise_latents.insert(key.clone(), scores);
                let usage = arm_usage.remove(&key).unwrap_or_default();
                let (ok, refused, malformed, errored) = arm_counts.remove(&key).unwrap_or_default();
                let (reported, read_gt0, frac_sum) = arm_cache.remove(&key).unwrap_or((0, 0, 0.0));
                setwise_arms.push(SetwiseArm {
                    answer: mode.label().to_string(),
                    k,
                    attribute: attr.name.clone(),
                    calls: usage.calls,
                    calls_ok: ok,
                    calls_refused: refused,
                    calls_malformed: malformed,
                    calls_errored: errored,
                    observations,
                    usage,
                    cache: CacheEvidence {
                        calls_with_cache_fields_reported: reported,
                        calls_with_cache_read_gt0: read_gt0,
                        mean_cached_fraction: (reported > 0).then(|| frac_sum / reported as f64),
                    },
                    pivot_rotation: PivotRotation {
                        pairs_with_both_orientations: 0,
                        sign_flips: 0,
                        mean_abs_residual_nats: None,
                    },
                    slot_histogram: None,
                    order_sensitivity: None,
                    components: 1,
                    disconnected: false,
                    calls_pmf_weighted: None,
                    mean_answer_token_prob: None,
                    latents,
                });
                continue;
            }
            let obs = arm_obs.remove(&key).unwrap_or_default();
            let mut engine =
                RatingEngine::new(args.n, AttributeParams::default(), raters.clone(), None)?;
            if engine_spec.is_none() {
                engine_spec = Some(engine.spec());
            }
            engine.add_observations(&obs);
            let summary = engine.solve();
            let scores = summary.scores.clone();
            let latents: Vec<ItemLatent> = entities
                .iter()
                .enumerate()
                .map(|(idx, entity)| ItemLatent {
                    id: entity.id.clone(),
                    mean: scores[idx],
                    std: summary.diag_cov[idx].max(0.0).sqrt(),
                })
                .collect();
            setwise_latents.insert(key.clone(), scores);

            let pair_means = arm_pairs.remove(&key).unwrap_or_default();
            let mut pairs_with_both = 0usize;
            let mut sign_flips = 0usize;
            let mut residuals: Vec<f64> = Vec::new();
            for (pivot_hi, pivot_lo) in pair_means.values() {
                if pivot_hi.is_empty() || pivot_lo.is_empty() {
                    continue;
                }
                let mean_hi: f64 = pivot_hi.iter().sum::<f64>() / pivot_hi.len() as f64;
                let mean_lo: f64 = pivot_lo.iter().sum::<f64>() / pivot_lo.len() as f64;
                pairs_with_both += 1;
                if mean_hi * mean_lo < 0.0 {
                    sign_flips += 1;
                }
                residuals.push((mean_hi - mean_lo).abs());
            }
            let mean_abs_residual = (!residuals.is_empty())
                .then(|| residuals.iter().sum::<f64>() / residuals.len() as f64);

            let usage = arm_usage.remove(&key).unwrap_or_default();
            let (ok, refused, malformed, errored) = arm_counts.remove(&key).unwrap_or_default();
            let (reported, read_gt0, frac_sum) = arm_cache.remove(&key).unwrap_or((0, 0, 0.0));
            let pmf_stats = arm_probs.remove(&key);
            let order_sensitivity = arm_tiers.remove(&key).map(|subsets| {
                let (mut with_repeats, mut pres_pairs, mut compared, mut flips) = (0, 0, 0, 0);
                for (subset, pres) in &subsets {
                    if pres.len() >= 2 {
                        with_repeats += 1;
                    }
                    for a in 0..pres.len() {
                        for b in (a + 1)..pres.len() {
                            pres_pairs += 1;
                            for x in 0..subset.len() {
                                for y in (x + 1)..subset.len() {
                                    let (e, f) = (subset[x], subset[y]);
                                    let d1 = pres[a].get(&e).zip(pres[a].get(&f));
                                    let d2 = pres[b].get(&e).zip(pres[b].get(&f));
                                    if let (Some((r1e, r1f)), Some((r2e, r2f))) = (d1, d2) {
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
            let slot_histogram =
                arm_slots
                    .remove(&key)
                    .map(|(first_by_slot, last_by_slot)| SlotHistogram {
                        first_by_slot,
                        last_by_slot,
                    });
            if summary.components > 1 {
                eprintln!(
                    "k={k} {}: DISCONNECTED observation graph ({} components) — scores are not on one scale",
                    attr.name, summary.components
                );
            }
            setwise_arms.push(SetwiseArm {
                answer: mode.label().to_string(),
                k,
                attribute: attr.name.clone(),
                calls: usage.calls,
                calls_ok: ok,
                calls_refused: refused,
                calls_malformed: malformed,
                calls_errored: errored,
                observations: obs.len(),
                usage,
                cache: CacheEvidence {
                    calls_with_cache_fields_reported: reported,
                    calls_with_cache_read_gt0: read_gt0,
                    mean_cached_fraction: (reported > 0).then(|| frac_sum / reported as f64),
                },
                pivot_rotation: PivotRotation {
                    pairs_with_both_orientations: pairs_with_both,
                    sign_flips,
                    mean_abs_residual_nats: mean_abs_residual,
                },
                slot_histogram,
                order_sensitivity,
                components: summary.components,
                disconnected: summary.components > 1,
                calls_pmf_weighted: pmf_stats.map(|(c, _)| c),
                mean_answer_token_prob: pmf_stats
                    .and_then(|(c, sum)| (c > 0).then(|| sum / c as f64)),
                latents,
            });
        }
    }

    // --- pairwise baseline (canonical_v2, sort path, default budget) ------
    let mut pairwise_arms: Vec<PairwiseArm> = Vec::new();
    let mut pairwise_latents: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    if !args.skip_pairwise {
        let id_to_idx: HashMap<String, usize> = entities
            .iter()
            .enumerate()
            .map(|(idx, e)| (e.id.clone(), idx))
            .collect();
        let (sink, worker) = JsonlTraceSink::new(args.out_dir.join("pairwise_trace.jsonl"))?;
        for attr in &attrs {
            let documents: Vec<RerankDocument> = entities
                .iter()
                .map(|entity| RerankDocument {
                    id: entity.id.clone(),
                    text: entity.raw.clone(),
                })
                .collect();
            let execution = RerankExecution::new(Arc::clone(&gateway), attribution.clone())
                .run_options(RerankRunOptions {
                    rng_seed: Some(args.seed),
                    cache_only: false,
                })
                .trace(&sink);
            let sorted: SortedTexts = sort_documents(
                documents,
                &attr.text,
                execution,
                SortOptions {
                    model: Some(args.model.clone()),
                    ..SortOptions::default()
                },
            )
            .await?;
            meter.add(sorted.meta.provider_cost_nanodollars)?;
            let mut scores = vec![0.0; args.n];
            let mut latents: Vec<ItemLatent> = Vec::new();
            for item in &sorted.items {
                let idx = id_to_idx[&item.id];
                scores[idx] = item.latent_mean;
                latents.push(ItemLatent {
                    id: item.id.clone(),
                    mean: item.latent_mean,
                    std: item.latent_std,
                });
            }
            pairwise_latents.insert(attr.name.clone(), scores);
            eprintln!(
                "pairwise {}: {} used / {} attempted, ${:.4}",
                attr.name,
                sorted.meta.comparisons_used,
                sorted.meta.comparisons_attempted,
                sorted.meta.provider_cost_nanodollars as f64 / 1e9
            );
            pairwise_arms.push(PairwiseArm {
                attribute: attr.name.clone(),
                comparisons_attempted: sorted.meta.comparisons_attempted,
                comparisons_used: sorted.meta.comparisons_used,
                comparisons_refused: sorted.meta.comparisons_refused,
                comparison_budget: sorted.meta.comparison_budget,
                position_flips: sorted.meta.position_flips,
                pairs_counterbalanced: sorted.meta.pairs_counterbalanced,
                provider_input_tokens: sorted.meta.provider_input_tokens,
                provider_output_tokens: sorted.meta.provider_output_tokens,
                nanodollars: sorted.meta.provider_cost_nanodollars,
                cache_read_tokens: None,
                latents,
            });
        }
        drop(sink);
        worker.join()?;
    }

    // --- compare arms -----------------------------------------------------
    let mut comparisons: Vec<ArmComparison> = Vec::new();
    for arm in &setwise_arms {
        let Some(pair_scores) = pairwise_latents.get(&arm.attribute) else {
            continue;
        };
        let set_scores = &setwise_latents[&(arm.k, arm.attribute.clone())];
        let pairwise_meta = pairwise_arms
            .iter()
            .find(|p| p.attribute == arm.attribute)
            .expect("latents imply the arm exists");
        let set_top3 = top_indices(set_scores, 3);
        let pair_top3 = top_indices(pair_scores, 3);
        let overlap = set_top3.iter().filter(|i| pair_top3.contains(i)).count();
        let set_dollars = arm.usage.nanodollars as f64 / 1e9;
        let pair_dollars = pairwise_meta.nanodollars as f64 / 1e9;
        comparisons.push(ArmComparison {
            k: arm.k,
            attribute: arm.attribute.clone(),
            spearman_rho: spearman_rho(set_scores, pair_scores),
            kendall_tau: kendall_tau(set_scores, pair_scores),
            top1_agree: top_indices(set_scores, 1) == top_indices(pair_scores, 1),
            top3_overlap: overlap as f64 / 3.0,
            setwise_pairwise_equiv_obs_per_dollar: (set_dollars > 0.0)
                .then(|| arm.observations as f64 / set_dollars),
            pairwise_obs_per_dollar: (pair_dollars > 0.0)
                .then(|| pairwise_meta.comparisons_used as f64 / pair_dollars),
            setwise_dollars_per_item: set_dollars / args.n as f64,
            pairwise_dollars_per_item: pair_dollars / args.n as f64,
        });
    }

    // --- offline ground-truth recovery (plumbing check) -------------------
    let mut ground_truth_checks: Vec<GroundTruthCheck> = Vec::new();
    for (attr_name, z) in &truth {
        for &k in &ks {
            let set_scores = &setwise_latents[&(k, attr_name.clone())];
            ground_truth_checks.push(GroundTruthCheck {
                k,
                attribute: attr_name.clone(),
                rho_setwise_vs_truth: spearman_rho(set_scores, z),
                rho_pairwise_vs_truth: pairwise_latents.get(attr_name).map(|p| spearman_rho(p, z)),
            });
        }
    }

    let report = Report {
        generated_at: chrono::Utc::now().to_rfc3339(),
        offline: args.offline,
        answer: mode.label().to_string(),
        model: args.model.clone(),
        seed: args.seed,
        n: args.n,
        ks: ks.clone(),
        entity_chars: args.entity_chars,
        min_pair_cover: args.min_pair_cover,
        presentations_per_subset: args.presentations,
        repeats: args.repeats,
        delimiter: args.delimiter.label().to_string(),
        design: format!("{:?}", args.design).to_lowercase(),
        overlap: args.overlap,
        spend_cap_usd: args.spend_cap_usd,
        corpus: args.corpus.clone(),
        attributes: attrs
            .iter()
            .map(|attr| AttrReport {
                name: attr.name.clone(),
                rubric_source: attr.rubric_source.clone(),
                rubric_chars: attr.text.len(),
            })
            .collect(),
        entity_ids: entities.iter().map(|e| e.id.clone()).collect(),
        engine_spec: engine_spec.expect("at least one arm solved"),
        example_call,
        subsets_per_k,
        setwise: setwise_arms,
        pairwise: pairwise_arms,
        comparisons,
        offline_ground_truth: ground_truth_checks,
        total_cost_nanodollars: meter.spent_nanodollars,
        caveats: vec![
            "The k-1 observations of one setwise call share that call's context: they are correlated through the pivot and the call's overall framing, but enter the solver as independent unit-precision observations (mirroring canonical_v2 point weights). Non-pivot implied pairs are deliberately NOT added.".to_string(),
            "Pairwise cache token counts are not surfaced by the sort path's RerankMeta; the pairwise cache column is null, not zero.".to_string(),
        ]
        .into_iter()
        .chain(point_caveats)
        .collect(),
    };
    let report_path = args.out_dir.join("report.json");
    std::fs::write(&report_path, serde_json::to_string_pretty(&report)?)?;

    println!(
        "done: total ${:.4} ({}) -> {}",
        meter.spent_nanodollars as f64 / 1e9,
        if args.offline {
            "offline synthetic pricing"
        } else {
            "live"
        },
        report_path.display()
    );
    for check in &report.offline_ground_truth {
        println!(
            "  truth-recovery k={} {}: setwise rho {:.3}, pairwise rho {}",
            check.k,
            check.attribute,
            check.rho_setwise_vs_truth,
            check
                .rho_pairwise_vs_truth
                .map_or("n/a".to_string(), |r| format!("{r:.3}")),
        );
    }
    for comparison in &report.comparisons {
        println!(
            "  k={} {}: rho {:.3} tau {:.3} top1 {} top3 {:.2}  ${:.5}/item vs pairwise ${:.5}/item",
            comparison.k,
            comparison.attribute,
            comparison.spearman_rho,
            comparison.kendall_tau,
            comparison.top1_agree,
            comparison.top3_overlap,
            comparison.setwise_dollars_per_item,
            comparison.pairwise_dollars_per_item,
        );
    }
    for arm in &report.setwise {
        if let Some(o) = &arm.order_sensitivity {
            if let Some(rate) = o.flip_rate {
                println!(
                    "  k={} {} order-sensitivity: flip rate {:.3} ({}/{} pairs, {} subsets repeated)",
                    arm.k,
                    arm.attribute,
                    rate,
                    o.direction_flips,
                    o.entity_pairs_compared,
                    o.subsets_with_repeats,
                );
            }
        }
        if let Some(h) = &arm.slot_histogram {
            println!(
                "  k={} {} slots first {:?} last {:?}{}",
                arm.k,
                arm.attribute,
                h.first_by_slot,
                h.last_by_slot,
                if arm.disconnected {
                    "  DISCONNECTED"
                } else {
                    ""
                }
            );
        }
    }
    Ok(())
}
