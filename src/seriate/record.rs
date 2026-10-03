//! The judgement record: seriate's immutable unit.
//!
//! Every number the system ever emits must have one of these as an ancestor.
//! A record binds WHAT was judged (attribute, presentation), HOW (instrument,
//! template hash, parser version, decode config, model), the RAW provider
//! capture it came from, the parsed evidence PMF, its health, and its cost.
//! Records are content-addressed: the id is a hash of the canonical
//! serialization of everything except the id itself, so identical claims
//! collide and tampering is detectable.

use crate::seriate::evidence::AnswerEvidence;
use crate::seriate::ontology::{AttributeId, CaptureId, JudgementId, Presentation, TemplateHash};
use serde::{Deserialize, Serialize};

/// Which elicitation instrument produced a judgement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstrumentKind {
    /// Direction + confidence, no magnitude.
    OrdinalPairwise,
    /// Single-letter ratio bucket (the ladder), logprob-capable.
    RatioLetterPairwise,
}

/// How the answer was acquired.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcquisitionMode {
    /// PMF read from answer-token top-logprobs.
    Logprob,
    /// PMF estimated from sampled completions.
    Sampled,
}

/// Versioned parser identity. Re-parsing an old capture under a new parser
/// yields a NEW judgement record pointing at the SAME capture.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParserVersion(pub String);

/// Decode configuration that shaped the provider call.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DecodeConfig {
    pub temperature: f64,
    pub max_tokens: u32,
    pub top_logprobs: Option<u8>,
}

/// Cost accounting for one judgement.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Cost {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub nanodollars: i64,
    pub is_estimate: bool,
}

/// Health facts for downstream weighting; never silently folded away.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EvidenceHealth {
    /// Probability mass visible at the answer position (1.0 for sampled).
    pub visible_mass: f64,
    /// Did the parser succeed on the primary token/JSON path?
    pub parsed_cleanly: bool,
    /// Provider refusal detected.
    pub refused: bool,
}

/// The immutable, content-addressed judgement record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JudgementRecord {
    pub id: JudgementId,
    pub instrument: InstrumentKind,
    pub mode: AcquisitionMode,
    pub attribute: AttributeId,
    pub presentation: Presentation,
    pub template: TemplateHash,
    pub parser: ParserVersion,
    pub model: String,
    pub decode: DecodeConfig,
    pub capture: CaptureId,
    pub evidence: AnswerEvidence,
    pub health: EvidenceHealth,
    pub cost: Cost,
    /// Unix millis at record creation.
    pub created_at_ms: u64,
}

/// Everything except the id: what the id is derived from.
#[derive(Serialize)]
struct JudgementBody<'a> {
    instrument: &'a InstrumentKind,
    mode: &'a AcquisitionMode,
    attribute: &'a AttributeId,
    presentation: &'a Presentation,
    template: &'a TemplateHash,
    parser: &'a ParserVersion,
    model: &'a str,
    decode: &'a DecodeConfig,
    capture: &'a CaptureId,
    evidence: &'a AnswerEvidence,
    health: &'a EvidenceHealth,
    cost: &'a Cost,
    created_at_ms: u64,
}

#[allow(clippy::too_many_arguments)]
impl JudgementRecord {
    pub fn new(
        instrument: InstrumentKind,
        mode: AcquisitionMode,
        attribute: AttributeId,
        presentation: Presentation,
        template: TemplateHash,
        parser: ParserVersion,
        model: String,
        decode: DecodeConfig,
        capture: CaptureId,
        evidence: AnswerEvidence,
        health: EvidenceHealth,
        cost: Cost,
        created_at_ms: u64,
    ) -> Self {
        let body = JudgementBody {
            instrument: &instrument,
            mode: &mode,
            attribute: &attribute,
            presentation: &presentation,
            template: &template,
            parser: &parser,
            model: &model,
            decode: &decode,
            capture: &capture,
            evidence: &evidence,
            health: &health,
            cost: &cost,
            created_at_ms,
        };
        let bytes = serde_json::to_vec(&body).expect("judgement body serializes");
        let id = JudgementId::derive(&bytes);
        Self {
            id,
            instrument,
            mode,
            attribute,
            presentation,
            template,
            parser,
            model,
            decode,
            capture,
            evidence,
            health,
            cost,
            created_at_ms,
        }
    }

    /// Recompute the id from current contents; must equal `self.id` for an
    /// untampered record.
    pub fn verify_id(&self) -> bool {
        let body = JudgementBody {
            instrument: &self.instrument,
            mode: &self.mode,
            attribute: &self.attribute,
            presentation: &self.presentation,
            template: &self.template,
            parser: &self.parser,
            model: &self.model,
            decode: &self.decode,
            capture: &self.capture,
            evidence: &self.evidence,
            health: &self.health,
            cost: &self.cost,
            created_at_ms: self.created_at_ms,
        };
        let bytes = serde_json::to_vec(&body).expect("judgement body serializes");
        JudgementId::derive(&bytes) == self.id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::seriate::atom::AnswerAtom;
    use crate::seriate::evidence::{evidence_from_resamples, AnswerEvidence};
    use crate::seriate::ontology::Entity;

    fn sample_record(evidence: AnswerEvidence) -> JudgementRecord {
        let a = Entity::new("first");
        let b = Entity::new("second");
        JudgementRecord::new(
            InstrumentKind::RatioLetterPairwise,
            AcquisitionMode::Sampled,
            crate::seriate::ontology::AttributeId::derive(b"attr"),
            Presentation {
                slot_a: a.id,
                slot_b: b.id,
            },
            TemplateHash::derive(b"template"),
            ParserVersion("ratio-letter/1".into()),
            "test/model".into(),
            DecodeConfig {
                temperature: 0.0,
                max_tokens: 8,
                top_logprobs: Some(20),
            },
            crate::seriate::ontology::CaptureId::derive(b"raw bytes"),
            evidence,
            EvidenceHealth {
                visible_mass: 1.0,
                parsed_cleanly: true,
                refused: false,
            },
            Cost::default(),
            1_700_000_000_000,
        )
    }

    #[test]
    fn id_is_deterministic_and_content_bound() {
        let ev = evidence_from_resamples(&[AnswerAtom::A(1)]).unwrap();
        let r1 = sample_record(ev.clone());
        let r2 = sample_record(ev);
        assert_eq!(r1.id, r2.id, "same content, same id");
        assert!(r1.verify_id());

        let ev2 = evidence_from_resamples(&[AnswerAtom::B(1)]).unwrap();
        let r3 = sample_record(ev2);
        assert_ne!(r1.id, r3.id, "different evidence, different id");
    }

    #[test]
    fn json_round_trip_preserves_id() {
        // Evidence with non-trivial floats (real exp() values).
        let ev = crate::seriate::evidence::evidence_from_logprobs(
            &[
                crate::seriate::evidence::AtomLogprob {
                    atom: AnswerAtom::A(3),
                    logprob: -0.2231435,
                },
                crate::seriate::evidence::AtomLogprob {
                    atom: AnswerAtom::Parity,
                    logprob: -2.2072749,
                },
            ],
            Some(0.95),
        )
        .unwrap();
        let r = sample_record(ev);
        assert!(r.verify_id());
        let json = serde_json::to_string(&r).unwrap();
        let back: JudgementRecord = serde_json::from_str(&json).unwrap();
        if !back.verify_id() {
            let a = serde_json::to_string(&r).unwrap();
            let b = serde_json::to_string(&back).unwrap();
            for (i, (ca, cb)) in a.chars().zip(b.chars()).enumerate() {
                if ca != cb {
                    panic!(
                        "first divergence at {i}: ...{}... vs ...{}...",
                        &a[i.saturating_sub(60)..(i + 60).min(a.len())],
                        &b[i.saturating_sub(60)..(i + 60).min(b.len())]
                    );
                }
            }
            panic!(
                "same serialization but verify failed: len {} vs {}",
                a.len(),
                b.len()
            );
        }
    }

    #[test]
    fn tampering_breaks_verification() {
        let ev = evidence_from_resamples(&[AnswerAtom::A(1)]).unwrap();
        let mut r = sample_record(ev);
        assert!(r.verify_id());
        r.model = "tampered/model".into();
        assert!(!r.verify_id());
    }
}
