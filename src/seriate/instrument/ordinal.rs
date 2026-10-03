//! Direction-only pairwise instrument: which entity has more of the
//! attribute, with no attempt at magnitude. Three-token answer alphabet
//! (`A`, `B`, `=`), single token, logprob-capable — the cheap, robust
//! fallback when ratio-ladder resolution isn't needed or isn't trusted.
//!
//! Direction evidence is lowered into the same additive log-ratio space the
//! ratio-letter instrument uses by pinning every directional judgement to a
//! FIXED, modest magnitude: bucket 7 of [`crate::seriate::atom::RATIO_LADDER`] (ratio ≈ 2.78, ≈
//! `e`). This is a deliberate approximation: the PMF still carries full
//! DIRECTION uncertainty (`P(A wins)` / `P(parity)` / `P(B wins)`), it just
//! collapses MAGNITUDE uncertainty within a direction to a point. Ordinal
//! evidence is therefore always weaker per-observation than a genuine
//! ratio-letter reading, but it costs the same one token and never asks the
//! model for a magnitude it may not have a stable prior over.

use crate::gateway::TokenLogprob;
use crate::seriate::atom::AnswerAtom;
use crate::seriate::evidence::{evidence_from_logprobs, evidence_from_resamples, AtomLogprob};
use crate::seriate::ontology::{Attribute, Entity};
use crate::seriate::record::{AcquisitionMode, EvidenceHealth, InstrumentKind, ParserVersion};

use super::{
    candidate_mass, evidence_all_off_alphabet, find_answer_position, merge_candidates,
    template_hash, trim_token, Instrument, InstrumentError, ParseOutcome, RenderedPrompt,
};

/// Version string stamped on every record this parser produces.
pub const PARSER_VERSION: &str = "ordinal/1";

/// The refusal token the prompt asks the model to use when it cannot judge
/// direction at all.
pub const REFUSAL_TOKEN: &str = "!";

/// 1-based ladder bucket every directional ordinal judgement is pinned to.
/// Bucket 7 -> `RATIO_LADDER[6]` ≈ 2.78 (≈ Euler's number): modest, not
/// extreme, so a direction-only reading never masquerades as a strong
/// magnitude claim.
pub const FIXED_BUCKET: u8 = 7;

const TOKEN_A: &str = "A";
const TOKEN_B: &str = "B";
const TOKEN_EQUAL: &str = "=";

const SLOT_A_PLACEHOLDER: &str = "\u{2603}SLOT_A\u{2603}";
const SLOT_B_PLACEHOLDER: &str = "\u{2603}SLOT_B\u{2603}";

/// Direction-only pairwise instrument.
#[derive(Clone, Copy, Debug, Default)]
pub struct OrdinalInstrument;

fn system_prompt() -> String {
    format!(
        "You are comparing two entities, presented as entity A and entity B, on exactly one \
         attribute. Use the attribute text as the only criterion.\n\n\
         Decide only the DIRECTION of the difference — do not attempt to judge magnitude. Answer \
         with EXACTLY ONE character, the first thing you output:\n\
         - '{TOKEN_A}' if entity A has more of the attribute.\n\
         - '{TOKEN_B}' if entity B has more of the attribute.\n\
         - '{TOKEN_EQUAL}' if they are indistinguishable on the attribute.\n\n\
         If the attribute is not applicable or the comparison is genuinely undecidable, answer \
         with a single '{REFUSAL_TOKEN}' instead.\n"
    )
}

fn render_user(attribute: &Attribute, entity_a: &str, entity_b: &str) -> String {
    format!(
        "<attribute>\n<name>{}</name>\n<text>\n{}\n</text>\n</attribute>\n\n\
         <entity_A>\n{}\n</entity_A>\n\n<entity_B>\n{}\n</entity_B>",
        attribute.name, attribute.text, entity_a, entity_b
    )
}

/// The three tokens whose logprobs constitute the answer PMF.
fn ordinal_alphabet() -> Vec<String> {
    vec![
        TOKEN_A.to_string(),
        TOKEN_B.to_string(),
        TOKEN_EQUAL.to_string(),
    ]
}

fn direction_atom(trimmed: &str) -> Option<AnswerAtom> {
    match trimmed {
        TOKEN_A => Some(AnswerAtom::A(FIXED_BUCKET)),
        TOKEN_B => Some(AnswerAtom::B(FIXED_BUCKET)),
        TOKEN_EQUAL => Some(AnswerAtom::Parity),
        _ => None,
    }
}

fn is_refusal_token(trimmed: &str) -> bool {
    trimmed == REFUSAL_TOKEN
}

fn parse_logprob(positions: &[TokenLogprob]) -> Result<ParseOutcome, InstrumentError> {
    let position = find_answer_position(positions, |t| {
        direction_atom(t).is_some() || is_refusal_token(t)
    })
    .ok_or(InstrumentError::Unparseable)?;
    let chosen = trim_token(&position.token);
    let refused = is_refusal_token(chosen);
    let parsed_cleanly = direction_atom(chosen).is_some();

    let candidates = merge_candidates(position);
    let atom_logprobs: Vec<AtomLogprob> = candidates
        .iter()
        .filter_map(|(tok, logprob)| {
            direction_atom(trim_token(tok)).map(|atom| AtomLogprob {
                atom,
                logprob: *logprob,
            })
        })
        .collect();
    let visible_mass = candidate_mass(&candidates);

    let evidence = if atom_logprobs.is_empty() {
        evidence_all_off_alphabet(visible_mass)?
    } else {
        evidence_from_logprobs(&atom_logprobs, Some(visible_mass))?
    };
    Ok(ParseOutcome {
        evidence,
        health: EvidenceHealth {
            visible_mass,
            parsed_cleanly,
            refused,
        },
        mode: AcquisitionMode::Logprob,
    })
}

fn parse_sampled(content: &str) -> Result<ParseOutcome, InstrumentError> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Err(InstrumentError::EmptyContent);
    }
    let refused = is_refusal_token(trimmed);
    // Mirror the logprob path's tolerance for a trailing filler by looking
    // only at the token-shaped prefix (first non-whitespace run).
    let first_token = trimmed.split_whitespace().next().unwrap_or(trimmed);
    let atom = direction_atom(first_token).unwrap_or(AnswerAtom::OffAlphabet);
    let evidence = evidence_from_resamples(&[atom])?;
    Ok(ParseOutcome {
        evidence,
        health: EvidenceHealth {
            visible_mass: 1.0,
            parsed_cleanly: atom.is_informative(),
            refused,
        },
        mode: AcquisitionMode::Sampled,
    })
}

impl Instrument for OrdinalInstrument {
    fn kind(&self) -> InstrumentKind {
        InstrumentKind::OrdinalPairwise
    }

    fn parser_version(&self) -> ParserVersion {
        ParserVersion(PARSER_VERSION.to_string())
    }

    fn render(&self, attribute: &Attribute, slot_a: &Entity, slot_b: &Entity) -> RenderedPrompt {
        let system = system_prompt();
        let user = render_user(attribute, &slot_a.body, &slot_b.body);
        let skeleton = render_user(attribute, SLOT_A_PLACEHOLDER, SLOT_B_PLACEHOLDER);
        RenderedPrompt {
            template: template_hash(&system, &skeleton),
            system,
            user,
            response_format_json: false,
            answer_alphabet: ordinal_alphabet(),
        }
    }

    fn parse(
        &self,
        content: &str,
        answer_logprobs: Option<&[TokenLogprob]>,
    ) -> Result<ParseOutcome, InstrumentError> {
        match answer_logprobs {
            Some(positions) if !positions.is_empty() => parse_logprob(positions),
            _ => parse_sampled(content),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::seriate::atom::RATIO_LADDER;

    fn fixed_bucket_ratio() -> f64 {
        RATIO_LADDER[(FIXED_BUCKET - 1) as usize]
    }

    fn attr() -> Attribute {
        Attribute::new("rawness", "how raw and unguarded the writing is")
    }

    fn tl(token: &str, p: f64, alts: &[(&str, f64)]) -> TokenLogprob {
        TokenLogprob {
            token: token.to_string(),
            logprob: p.ln(),
            top_alternatives: alts
                .iter()
                .map(|(t, p)| crate::gateway::TokenAlternative {
                    token: t.to_string(),
                    logprob: p.ln(),
                })
                .collect(),
        }
    }

    #[test]
    fn fixed_bucket_maps_to_the_documented_ratio() {
        assert!((fixed_bucket_ratio() - 2.78).abs() < 1e-9);
    }

    #[test]
    fn render_is_deterministic_and_hides_slots_from_the_template_hash() {
        let inst = OrdinalInstrument;
        let a1 = Entity::new("first");
        let b1 = Entity::new("second");
        let a2 = Entity::new("wildly different");
        let b2 = Entity::new("also different");
        let r1 = inst.render(&attr(), &a1, &b1);
        assert_eq!(r1, inst.render(&attr(), &a1, &b1));
        let r3 = inst.render(&attr(), &a2, &b2);
        assert_eq!(r1.template, r3.template);
        assert_eq!(r1.answer_alphabet, vec!["A", "B", "="]);
    }

    #[test]
    fn logprob_parse_maps_direction_to_the_fixed_bucket() {
        let position = tl("A", 0.7, &[("A", 0.7), ("=", 0.2), ("B", 0.1)]);
        let outcome = OrdinalInstrument.parse("A", Some(&[position])).unwrap();
        assert!((outcome.evidence.p(AnswerAtom::A(FIXED_BUCKET)) - 0.7).abs() < 1e-9);
        assert!((outcome.evidence.p(AnswerAtom::Parity) - 0.2).abs() < 1e-9);
        assert!((outcome.evidence.p(AnswerAtom::B(FIXED_BUCKET)) - 0.1).abs() < 1e-9);
        assert!(outcome.health.parsed_cleanly);
        assert_eq!(outcome.mode, AcquisitionMode::Logprob);
    }

    #[test]
    fn logprob_parse_honors_visible_mass_split() {
        let position = tl("=", 0.5, &[("=", 0.5), ("A", 0.3)]);
        let outcome = OrdinalInstrument.parse("=", Some(&[position])).unwrap();
        assert!((outcome.health.visible_mass - 0.8).abs() < 1e-9);
        assert!((outcome.evidence.abstain_mass() - 0.2).abs() < 1e-9);
    }

    #[test]
    fn logprob_parse_handles_refusal_with_no_informative_alternatives() {
        let position = tl("!", 1.0, &[("!", 1.0)]);
        let outcome = OrdinalInstrument.parse("!", Some(&[position])).unwrap();
        assert!(outcome.health.refused);
        assert!(!outcome.health.parsed_cleanly);
        assert_eq!(outcome.evidence.off_alphabet_mass(), 1.0);
    }

    #[test]
    fn logprob_parse_errors_when_nothing_matches() {
        let position = tl("hello", 0.9, &[]);
        let err = OrdinalInstrument
            .parse("hello", Some(&[position]))
            .unwrap_err();
        assert_eq!(err, InstrumentError::Unparseable);
    }

    #[test]
    fn sampled_fallback_reads_the_first_token() {
        let outcome = OrdinalInstrument.parse(" B trailing junk", None).unwrap();
        assert_eq!(outcome.evidence.p(AnswerAtom::B(FIXED_BUCKET)), 1.0);
        assert_eq!(outcome.mode, AcquisitionMode::Sampled);
    }

    #[test]
    fn sampled_fallback_detects_refusal_and_junk() {
        assert!(OrdinalInstrument.parse("!", None).unwrap().health.refused);
        let junk = OrdinalInstrument.parse("banana", None).unwrap();
        assert!(!junk.health.parsed_cleanly);
        assert_eq!(junk.evidence.off_alphabet_mass(), 1.0);
    }

    #[test]
    fn sampled_fallback_errors_on_empty_content() {
        assert_eq!(
            OrdinalInstrument.parse("", None).unwrap_err(),
            InstrumentError::EmptyContent
        );
    }
}
