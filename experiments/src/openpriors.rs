//! # The OpenPriors collaborative ledger — typed contract (draft)
//!
//! OpenPriors widens from "our ratio ledger" to the open platform for
//! externalized LLM beliefs: anyone runs structured judgements through it,
//! under one absolute rule — full disclosure of how every number was made.
//! This module is the type-level derivation of that platform, extending the
//! locked five-noun ontology (`docs/WHAT_WHY_HOW.md`: attribute → magnitude
//! → instrument → evidence → scaling) with the sixth noun collaboration
//! requires: the **account**.
//!
//! ## The four invariants
//!
//! 1. **Names are aliases; content hashes are identity.** Entities,
//!    attribute prompts, instruments, and records are addressed by hashes of
//!    their canonical bytes. Prompt drift cannot hide; identical instruments
//!    registered by different accounts share one corpus.
//! 2. **Every published number is a declared pure function of append-only
//!    records.** A [`Record`] stores the verbatim completion; its
//!    [`Evidence`] equals `interpret(instrument, bindings, raw)` — a
//!    function shipped here, so anyone can recompute it. Fits recompute from
//!    records, never from summaries. A judgement you cannot audit is an
//!    opinion.
//! 3. **Instruments are permissionless data; currencies and fitters are
//!    platform-versioned code.** A contributor uploads an [`Instrument`] —
//!    a typed signature, verbatim templates, and a declared
//!    [`Interpretation`] drawn from a small closed combinator set. The
//!    [`Currency`] enum and the solvers that fuse each currency are the
//!    platform's; extending *them* is a rare, reviewed event. New paradigm ⇒
//!    usually a new instrument (free); occasionally a new combinator or
//!    currency (platform release).
//! 4. **Trust attaches to accounts and is a view, never a delete.** Records
//!    are attributed and immutable; [`Standing`] is a fold over
//!    [`TrustEvent`]s. Weak instruments are allowed and *measured* (the
//!    pairwiseratio.org coherence pattern, generalized per instrument×model);
//!    misrepresenting method or results is the one capital offense and flags
//!    the account's entire ledger — visibly quarantined, never destroyed.
//!
//! ## Two collaboration lanes
//!
//! - **Hosted**: the platform executes the provider call (BYO key or prepaid
//!   wallet). Provenance is platform-attested — the strongest tier.
//! - **External**: the platform issues a rendered schedule whose digest
//!   binds template bytes, seed, axis, and every entity id+text (the
//!   `cardinald` `/v1/schedule` mechanism, already live for `claude-code`);
//!   the contributor's harness answers and submits digest-bound results.
//!   Provenance is account-attested — worth exactly the account's standing.
//!
//! ## Today's objects in these types
//!
//! - `canonical_v2` → `Pair` / `Json` / [`Interpretation::RatioJson`] →
//!   [`Evidence::LogRatio`].
//! - `ratio_letter_v1` (and the two-phase read) → `Pair` / `SingleToken` /
//!   [`Interpretation::RatioLadder`] → `LogRatio`; the logprob PMF over the
//!   alphabet is the engine's evidence path and stays engine-side.
//! - `ordinal_letter_v1` → `Pair` / `SingleToken` /
//!   [`Interpretation::OrdinalLetter`] → [`Evidence::Ordinal`] (fused by the
//!   censored-likelihood path).
//! - The openpriors-forecaster Forecast Record → `Single` / `Json` /
//!   [`Interpretation::ProbabilityJson`] → [`Evidence::Probability`].
//! - The `harness` allowlist string → [`Attestation::External`] plus
//!   account standing.
//!
//! ## Deliberately not here (yet)
//!
//! PMF→(mean, variance) fusion, the honest-σ machinery, and all fitters
//! (engine rooms); the ClickHouse landing schema; wallet/billing; the HTTP
//! surface. This module is the contract those parts share. It graduates out
//! of `experiments/` when the registry implementation earns it.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

// ---------------------------------------------------------------------------
// Identity
// ---------------------------------------------------------------------------

/// SHA-256 hex over the canonical serde_json bytes of the addressed value.
///
/// Canonicalization rides the repo's identity law (packet discipline):
/// struct field order is fixed by the type, maps are `BTreeMap`, floats
/// round-trip. A content address must never drift across versions.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContentHash(pub String);

/// A registered contributor account (`acct_<32 hex>`). Trust lives here.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AccountId(pub String);

/// A judged text. The id is an alias; the text hash is the identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityRef {
    pub entity_id: String,
    pub text_hash: ContentHash,
}

/// An attribute under a lens. The prompt hash is the identity; a changed
/// wording is a different attribute row, surfaced as prompt variance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttributeRef {
    pub lens: String,
    pub axis_key: String,
    pub prompt_hash: ContentHash,
}

/// A forecastable proposition: hash of (question, resolution criteria).
/// Settlement appends records; it never edits them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PropositionRef {
    pub proposition_hash: ContentHash,
}

// ---------------------------------------------------------------------------
// Instruments: permissionless, content-addressed data
// ---------------------------------------------------------------------------

/// How many entities one call presents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Arity {
    Single,
    Pair,
    Set { k: u8 },
}

/// The raw output space a call must produce.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputSpace {
    /// The verdict is the FIRST completion token, drawn from this alphabet.
    /// Single-token instruments are the logprob-readable class: one token
    /// position's top-k IS the posterior (the PMF rail).
    SingleToken { alphabet: Vec<String> },
    /// A completion validated against this JSON Schema.
    Json { schema: serde_json::Value },
}

/// The typed elicitation contract: what goes in, what must come out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signature {
    pub arity: Arity,
    pub output: OutputSpace,
}

/// Reasoning pin for a turn. Measured to matter (sigma-eps-knobs pack:
/// hidden reasoning on the analysis turn was pure verdict noise).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReasoningPin {
    Off,
    EffortNone,
    Unpinned,
}

/// Decoding constraints for one turn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Decode {
    pub max_tokens: u32,
    /// Forbid the verdict alphabet in this turn (the two-phase pattern:
    /// analysis first, verdict token read in a later turn).
    pub forbid_verdict: bool,
    pub reasoning: ReasoningPin,
    /// Read logprobs at this turn's first completion token.
    pub read_logprobs: bool,
}

/// One rendered turn. Templates are verbatim bytes with typed slots:
/// `{entity_a}`, `{entity_b}`, `{attribute}`, `{analysis}` (prior turn's
/// completion). The rendered bytes — not the template — are what a record's
/// prompt hash addresses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Turn {
    pub system: Option<String>,
    pub user: String,
    pub decode: Decode,
}

/// The verbatim prompt program: one or more turns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplateFamily {
    pub turns: Vec<Turn>,
}

/// Which side of a presented pair a ladder rung asserts has more.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    FirstHigher,
    SecondHigher,
    Equal,
}

/// One rung of a finite ratio ladder: a direction and a magnitude ≥ 1.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LadderRung {
    pub direction: Direction,
    pub ratio: f64,
}

/// The declared pure function from raw output to evidence — **data, not
/// code**. This closed combinator set is what makes third-party rubric
/// systems auditable: the platform (and anyone else) recomputes evidence
/// from the verbatim completion. Contributors compose freely within it;
/// extending the set itself is a platform release.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Interpretation {
    /// Verdict token → (direction, ratio) on a finite ladder (pair arity).
    RatioLadder { rungs: BTreeMap<String, LadderRung> },
    /// Verdict token → direction only (pair arity, censored magnitude).
    OrdinalLetter {
        first_higher: Vec<String>,
        second_higher: Vec<String>,
    },
    /// JSON completion → which side (`"A"`/`"B"`) and how many times more.
    RatioJson {
        higher_pointer: String,
        ratio_pointer: String,
    },
    /// JSON completion → a numeric quantity for the presented entity.
    QuantityJson { pointer: String, unit: String },
    /// JSON completion → a probability in [0, 1] for a proposition.
    ProbabilityJson { pointer: String },
}

/// An uploadable judgement paradigm. Identity is
/// [`Instrument::content_hash`] over exactly these semantic fields —
/// registration metadata (name, owner) lives in
/// [`InstrumentRegistration`], so two accounts registering the same
/// instrument share one corpus.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Instrument {
    pub signature: Signature,
    pub template: TemplateFamily,
    pub interpretation: Interpretation,
}

/// The currency an instrument's evidence feeds. Closed, platform-versioned:
/// each variant has exactly one fuser.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Currency {
    /// (E[log-ratio], honest variance) — the cardinal fuser (IRLS/Huber).
    LogRatio,
    /// Direction-only pair reads — the censored-likelihood path.
    Ordinal,
    /// Absolute numeric readings with units.
    Quantity,
    /// Probabilities over resolvable propositions.
    Probability,
}

impl Instrument {
    /// The one identity of this instrument.
    pub fn content_hash(&self) -> ContentHash {
        let bytes = serde_json::to_vec(self).expect("instrument serializes");
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        ContentHash(format!("{:x}", hasher.finalize()))
    }

    /// Which fuser this instrument's evidence feeds — total over the
    /// combinator set by construction.
    pub fn currency(&self) -> Currency {
        match self.interpretation {
            Interpretation::RatioLadder { .. } | Interpretation::RatioJson { .. } => {
                Currency::LogRatio
            }
            Interpretation::OrdinalLetter { .. } => Currency::Ordinal,
            Interpretation::QuantityJson { .. } => Currency::Quantity,
            Interpretation::ProbabilityJson { .. } => Currency::Probability,
        }
    }
}

/// Registration metadata: an account's name for a content-addressed
/// instrument. Aliases, never identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentRegistration {
    pub instrument: ContentHash,
    pub name: String,
    pub owner: AccountId,
    /// RFC 3339.
    pub registered_at: String,
}

// ---------------------------------------------------------------------------
// Records: the append-only atoms
// ---------------------------------------------------------------------------

/// Full disclosure of the judging model. `params` carries every sampling
/// parameter the provider accepted; an undisclosed parameter is a refusable
/// record, not a shrug.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelSpec {
    pub slug: String,
    pub temperature: f64,
    pub seed: Option<u64>,
    pub params: BTreeMap<String, serde_json::Value>,
}

/// Who vouches that this record's exchange actually happened as stated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Attestation {
    /// The platform executed the provider call itself (BYO key or wallet).
    Hosted,
    /// An external harness answered a platform-issued schedule. The digest
    /// binds results to the exact rendering (template bytes, seed, axis,
    /// every entity id+text) — results cannot land under altered inputs.
    External {
        harness: String,
        harness_version: String,
        schedule_digest: ContentHash,
    },
}

/// What was bound into the template slots, in presentation order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bindings {
    /// Slot order = presentation order; counterbalancing is a schedule-level
    /// concern, so `interpret` never needs a `swapped` flag.
    pub entities: Vec<EntityRef>,
    pub attribute: Option<AttributeRef>,
    pub proposition: Option<PropositionRef>,
}

/// The verbatim provider response. Logprob PMFs ride the engine's evidence
/// path; here only the sampled text is contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawOutput {
    pub completion: String,
    pub refused: bool,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
}

/// One provider call, attributed and immutable. `evidence` is redundant by
/// construction — it MUST equal `interpret(instrument, bindings, raw)`, and
/// verifiers recompute it (invariant 2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Record {
    pub instrument: ContentHash,
    pub model: ModelSpec,
    pub bindings: Bindings,
    /// Hash of the rendered prompt bytes; the store retains the bytes.
    pub rendered_prompt_hash: ContentHash,
    pub raw: RawOutput,
    pub evidence: Vec<Evidence>,
    pub account: AccountId,
    pub attestation: Attestation,
    pub cost_nanodollars: u64,
    /// RFC 3339.
    pub at: String,
}

// ---------------------------------------------------------------------------
// Evidence: the closed currency set
// ---------------------------------------------------------------------------

/// The commensurable output of every instrument. Entity refs are in
/// presentation order (`a` = first presented).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Evidence {
    LogRatio {
        a: EntityRef,
        b: EntityRef,
        attribute: AttributeRef,
        /// Positive means `a` has more.
        mean: f64,
        /// Honest variance: unit precision for sampled point reads; measured
        /// precision only via the engine's PMF path.
        variance: f64,
    },
    Ordinal {
        higher: EntityRef,
        lower: EntityRef,
        attribute: AttributeRef,
    },
    Quantity {
        entity: EntityRef,
        attribute: AttributeRef,
        value: f64,
        /// `None` is loud: a fuser may refuse or model it, never assume it.
        variance: Option<f64>,
        unit: String,
    },
    Probability {
        proposition: PropositionRef,
        p: f64,
    },
}

// ---------------------------------------------------------------------------
// Trust: a fold over events, never a rewrite
// ---------------------------------------------------------------------------

/// Appended to an account's history; records themselves never change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrustEvent {
    /// The capital offense: the account misrepresented models, parameters,
    /// prompts, or results. Flags the account's entire ledger.
    DisclosureViolation {
        finding: String,
        records: Vec<ContentHash>,
        at: String,
    },
    /// A measured coherence result for (instrument, model) — the
    /// pairwiseratio.org pattern generalized. Informative, never fatal.
    CoherenceReading {
        instrument: ContentHash,
        model: String,
        /// The evidence pack backing the reading.
        pack: String,
        at: String,
    },
}

/// Derived standing — a view over the append-only event stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Standing {
    pub account: AccountId,
    /// True iff any [`TrustEvent::DisclosureViolation`] exists.
    pub flagged: bool,
    pub events: Vec<TrustEvent>,
}

impl Standing {
    pub fn fold(account: AccountId, events: Vec<TrustEvent>) -> Self {
        let flagged = events
            .iter()
            .any(|event| matches!(event, TrustEvent::DisclosureViolation { .. }));
        Self {
            account,
            flagged,
            events,
        }
    }
}

// ---------------------------------------------------------------------------
// interpret: invariant 2, executable
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum InterpretError {
    #[error("verdict token {0:?} is not in the instrument's ladder or letter sets")]
    UnknownVerdict(String),
    #[error("completion is empty where a verdict token was required")]
    EmptyVerdict,
    #[error("completion is not valid JSON: {0}")]
    BadJson(String),
    #[error("JSON pointer {0:?} missing or wrong type")]
    BadPointer(String),
    #[error("value out of range at {0:?}: {1}")]
    OutOfRange(String, f64),
    #[error("bindings do not satisfy the instrument arity/slots: {0}")]
    BadBindings(&'static str),
}

/// Recompute a record's evidence from its verbatim raw output. Pure; total
/// over the combinator set; the function every verifier runs. A refused
/// call yields no evidence.
pub fn interpret(
    instrument: &Instrument,
    bindings: &Bindings,
    raw: &RawOutput,
) -> Result<Vec<Evidence>, InterpretError> {
    if raw.refused {
        return Ok(Vec::new());
    }
    match &instrument.interpretation {
        Interpretation::RatioLadder { rungs } => {
            let (a, b, attribute) = pair_bindings(bindings)?;
            let rung = rungs
                .get(verdict_token(raw)?)
                .ok_or_else(|| InterpretError::UnknownVerdict(verdict_owned(raw)))?;
            Ok(vec![log_ratio(a, b, attribute, rung)])
        }
        Interpretation::OrdinalLetter {
            first_higher,
            second_higher,
        } => {
            let (a, b, attribute) = pair_bindings(bindings)?;
            let token = verdict_token(raw)?;
            let (higher, lower) = if first_higher.iter().any(|t| t == token) {
                (a, b)
            } else if second_higher.iter().any(|t| t == token) {
                (b, a)
            } else {
                return Err(InterpretError::UnknownVerdict(token.to_string()));
            };
            Ok(vec![Evidence::Ordinal {
                higher: higher.clone(),
                lower: lower.clone(),
                attribute: attribute.clone(),
            }])
        }
        Interpretation::RatioJson {
            higher_pointer,
            ratio_pointer,
        } => {
            let (a, b, attribute) = pair_bindings(bindings)?;
            let value = parse_json(raw)?;
            let higher = string_at(&value, higher_pointer)?;
            let ratio = number_at(&value, ratio_pointer)?;
            if ratio < 1.0 {
                return Err(InterpretError::OutOfRange(ratio_pointer.clone(), ratio));
            }
            let direction = match higher.as_str() {
                "A" => Direction::FirstHigher,
                "B" => Direction::SecondHigher,
                other => return Err(InterpretError::UnknownVerdict(other.to_string())),
            };
            Ok(vec![log_ratio(
                a,
                b,
                attribute,
                &LadderRung { direction, ratio },
            )])
        }
        Interpretation::QuantityJson { pointer, unit } => {
            let entity = single_binding(bindings)?;
            let attribute = bindings
                .attribute
                .as_ref()
                .ok_or(InterpretError::BadBindings(
                    "quantity requires an attribute",
                ))?;
            let value = number_at(&parse_json(raw)?, pointer)?;
            Ok(vec![Evidence::Quantity {
                entity: entity.clone(),
                attribute: attribute.clone(),
                value,
                variance: None,
                unit: unit.clone(),
            }])
        }
        Interpretation::ProbabilityJson { pointer } => {
            let proposition = bindings
                .proposition
                .as_ref()
                .ok_or(InterpretError::BadBindings(
                    "probability requires a proposition",
                ))?;
            let p = number_at(&parse_json(raw)?, pointer)?;
            if !(0.0..=1.0).contains(&p) {
                return Err(InterpretError::OutOfRange(pointer.clone(), p));
            }
            Ok(vec![Evidence::Probability {
                proposition: proposition.clone(),
                p,
            }])
        }
    }
}

fn pair_bindings(
    bindings: &Bindings,
) -> Result<(&EntityRef, &EntityRef, &AttributeRef), InterpretError> {
    let [a, b] = bindings.entities.as_slice() else {
        return Err(InterpretError::BadBindings(
            "pair arity requires exactly two entities",
        ));
    };
    let attribute = bindings
        .attribute
        .as_ref()
        .ok_or(InterpretError::BadBindings(
            "pair reads require an attribute",
        ))?;
    Ok((a, b, attribute))
}

fn single_binding(bindings: &Bindings) -> Result<&EntityRef, InterpretError> {
    let [entity] = bindings.entities.as_slice() else {
        return Err(InterpretError::BadBindings(
            "single arity requires exactly one entity",
        ));
    };
    Ok(entity)
}

fn verdict_token(raw: &RawOutput) -> Result<&str, InterpretError> {
    let token = raw.completion.trim();
    let first = token
        .split_whitespace()
        .next()
        .ok_or(InterpretError::EmptyVerdict)?;
    Ok(first)
}

fn verdict_owned(raw: &RawOutput) -> String {
    verdict_token(raw).map(str::to_string).unwrap_or_default()
}

fn parse_json(raw: &RawOutput) -> Result<serde_json::Value, InterpretError> {
    serde_json::from_str(&raw.completion).map_err(|e| InterpretError::BadJson(e.to_string()))
}

fn string_at(value: &serde_json::Value, pointer: &str) -> Result<String, InterpretError> {
    value
        .pointer(pointer)
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| InterpretError::BadPointer(pointer.to_string()))
}

fn number_at(value: &serde_json::Value, pointer: &str) -> Result<f64, InterpretError> {
    value
        .pointer(pointer)
        .and_then(serde_json::Value::as_f64)
        .ok_or_else(|| InterpretError::BadPointer(pointer.to_string()))
}

fn log_ratio(
    a: &EntityRef,
    b: &EntityRef,
    attribute: &AttributeRef,
    rung: &LadderRung,
) -> Evidence {
    let magnitude = rung.ratio.max(1.0).ln();
    let mean = match rung.direction {
        Direction::FirstHigher => magnitude,
        Direction::SecondHigher => -magnitude,
        Direction::Equal => 0.0,
    };
    Evidence::LogRatio {
        a: a.clone(),
        b: b.clone(),
        attribute: attribute.clone(),
        mean,
        // Point observations carry unit precision (docs/MODEL.md); measured
        // precision enters only via the engine's PMF evidence path.
        variance: 1.0,
    }
}
