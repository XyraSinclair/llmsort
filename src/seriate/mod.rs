//! # seriate (vendored)
//!
//! Provenanced single-token elicitation instruments, folded back from the
//! standalone `seriate` crate (2026-08-11, seriate @ `ba32ca0`; decision
//! record in `notes/seriate-fold-2026-08-11.md`). The unit is the
//! structured, provenanced judgement: an immutable, content-addressed
//! evidence record traceable to raw provider bytes.
//!
//! Invariants:
//! 1. Nothing fabricates a number without a judgement-record ancestor.
//! 2. Ordinal evidence suffices; ratio magnitudes are an upgrade.
//! 3. Logprobs are harnessed where real and degradation is loud where not:
//!    every PMF carries its [`evidence::PmfCompleteness`].
//!
//! Vendored closure: `ontology`, `atom`, `evidence`, `record`, the
//! [`instrument::Instrument`] trait with the `ratio_letter` and `ordinal`
//! instruments, consuming [`crate::gateway::TokenLogprob`] directly (the
//! standalone crate's transport shim was unified away). Its CLI, gateway,
//! sqlite evidence log, posterior compiler, and k-wise/scalar instruments
//! were culled with the repo
//! (history preserved at <https://github.com/XyraSinclair/seriate>).

pub mod atom;
pub mod evidence;
pub mod instrument;
pub mod ontology;
pub mod record;

pub use atom::{interpolate_ratio, AnswerAtom, Side, RATIO_LADDER};
pub use evidence::{
    evidence_from_logprobs, evidence_from_resamples, AnswerEvidence, AtomLogprob, AtomProb,
    EvidenceError, PmfCompleteness,
};
pub use ontology::{
    Attribute, AttributeId, CaptureId, ContentId, Entity, EntityId, JudgementId, PairKey,
    Presentation, TemplateHash,
};
pub use record::{
    AcquisitionMode, Cost, DecodeConfig, EvidenceHealth, InstrumentKind, JudgementRecord,
    ParserVersion,
};
