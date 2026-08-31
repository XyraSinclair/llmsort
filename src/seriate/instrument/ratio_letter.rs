//! The flagship instrument: one letter, the full 52-letter answer alphabet,
//! logprob-native. Slot A vs slot B on an attribute; the answer is EXACTLY
//! ONE letter, requested as the FIRST completion token, so a single
//! completion-token position carries the model's whole prior over the
//! judgement (see [`crate::seriate::atom`]).
//!
//! Salvaged (redesigned) from the diamond2 `cardinal-ratio-json-letters-v1`
//! prompt: same letter-case answer alphabet and ratio ladder, dropped the
//! JSON wrapper and confidence field entirely — with the answer as the bare
//! first token, single-token logprobs already ARE the confidence.

use crate::gateway::TokenLogprob;
use crate::seriate::atom::{AnswerAtom, RATIO_LADDER};
use crate::seriate::evidence::{evidence_from_logprobs, evidence_from_resamples, AtomLogprob};
use crate::seriate::ontology::{Attribute, Entity};
use crate::seriate::record::{AcquisitionMode, EvidenceHealth, InstrumentKind, ParserVersion};

use super::{
    candidate_mass, evidence_all_off_alphabet, find_answer_position, merge_candidates,
    template_hash, trim_token, Instrument, InstrumentError, ParseOutcome, RenderedPrompt,
};

/// Version string stamped on every record this parser produces.
pub const PARSER_VERSION: &str = "ratio-letter/1";

/// The refusal token the prompt asks the model to use when it cannot judge
/// the attribute at all.
pub const REFUSAL_TOKEN: &str = "!";

const SLOT_A_PLACEHOLDER: &str = "\u{2603}SLOT_A\u{2603}";
const SLOT_B_PLACEHOLDER: &str = "\u{2603}SLOT_B\u{2603}";

/// Single-letter ratio-bucket pairwise instrument.
#[derive(Clone, Copy, Debug, Default)]
pub struct RatioLetterInstrument;

fn system_prompt() -> String {
    let mut s = String::from(
        "You are comparing two entities, presented as entity A and entity B, on exactly one \
         attribute. Use the attribute text as the only criterion; the task is comparative, not \
         absolute.\n\n\
         Decide which entity has more of the attribute and roughly how many times more, then \
         answer with EXACTLY ONE character: the letter naming your judgement. That character must \
         be the FIRST thing you output — no punctuation, no explanation, no JSON, nothing before \
         or after it.\n\n\
         Letter meaning:\n\
         - A or a: the entities are indistinguishable on the attribute (parity). Reserve this for \
         genuine indistinguishability; if you can identify any winner at all, use at least B/b.\n\
         - Uppercase B..Z: entity A has more, magnitude given by the ladder below.\n\
         - Lowercase b..z: entity B has more, same magnitude ladder.\n\
         Same letter, different case means the same magnitude with the opposite winner.\n\n\
         Ratio ladder (winner's amount \u{00f7} loser's amount):\n",
    );
    for (i, ratio) in RATIO_LADDER.iter().enumerate() {
        let upper = (b'B' + i as u8) as char;
        let lower = (b'b' + i as u8) as char;
        s.push_str(&format!("{upper}/{lower}  {ratio:.3}\n"));
    }
    s.push_str(&format!(
        "\nIf the attribute is not applicable or the comparison is genuinely undecidable for \
         these two entities, answer with a single '{REFUSAL_TOKEN}' instead of a letter.\n"
    ));
    s
}

fn render_user(attribute: &Attribute, entity_a: &str, entity_b: &str) -> String {
    format!(
        "<attribute>\n<name>{}</name>\n<text>\n{}\n</text>\n</attribute>\n\n\
         <entity_A>\n{}\n</entity_A>\n\n<entity_B>\n{}\n</entity_B>",
        attribute.name, attribute.text, entity_a, entity_b
    )
}

fn render_user_attr_last(attribute: &Attribute, entity_a: &str, entity_b: &str) -> String {
    format!(
        "<entity_A>\n{}\n</entity_A>\n\n<entity_B>\n{}\n</entity_B>\n\n\
         <attribute>\n<name>{}</name>\n<text>\n{}\n</text>\n</attribute>",
        entity_a, entity_b, attribute.name, attribute.text
    )
}

/// Attribute-LAST variant of [`RatioLetterInstrument`]: entities render
/// first, the attribute arrives last, so the (system + entity pair) token
/// prefix is byte-stable across attribute variants and provider prefix
/// caches serve a family sweep — {A, A′, ¬A} over one judged pair — at
/// cached-input prices (NORTH E10's mechanical gate: the attribute-first
/// order shares zero prefix across variants). Same alphabet, same parser,
/// same evidence; only the template hash and slug differ.
#[derive(Clone, Copy, Debug, Default)]
pub struct RatioLetterAttrLastInstrument;

impl Instrument for RatioLetterAttrLastInstrument {
    fn kind(&self) -> InstrumentKind {
        InstrumentKind::RatioLetterPairwise
    }

    fn parser_version(&self) -> ParserVersion {
        ParserVersion(PARSER_VERSION.to_string())
    }

    fn render(&self, attribute: &Attribute, slot_a: &Entity, slot_b: &Entity) -> RenderedPrompt {
        let system = system_prompt();
        let user = render_user_attr_last(attribute, &slot_a.body, &slot_b.body);
        let skeleton = render_user_attr_last(attribute, SLOT_A_PLACEHOLDER, SLOT_B_PLACEHOLDER);
        RenderedPrompt {
            template: template_hash(&system, &skeleton),
            system,
            user,
            response_format_json: false,
            answer_alphabet: ratio_letter_alphabet(),
        }
    }

    fn parse(
        &self,
        content: &str,
        answer_logprobs: Option<&[TokenLogprob]>,
    ) -> Result<ParseOutcome, InstrumentError> {
        RatioLetterInstrument.parse(content, answer_logprobs)
    }
}

/// The fixed second-turn instruction of the two-phase instrument. Part of
/// the template identity (folded into the template hash below).
pub const TWO_PHASE_ANSWER_PROMPT: &str =
    "Answer now: exactly one character — the ladder letter (or '!') — as the first and only \
     thing you output.";

fn two_phase_system_prompt() -> String {
    let mut s = system_prompt();
    s.push_str(
        "\nThis judgement happens in two turns. Turn 1: give a brief concrete analysis \
         (2-4 sentences) weighing the two entities on the attribute — cite what in each entity \
         drives your judgement. Do NOT state a letter, a ratio, or any final verdict in turn 1. \
         Turn 2: when asked to answer, output exactly one character as specified above.\n",
    );
    s
}

/// Two-phase variant of [`RatioLetterInstrument`] for reasoning-class
/// judges: turn 1 elicits a WRITTEN analysis with the verdict token
/// forbidden (a visible verdict collapses the turn-2 PMF — measured,
/// docs/LOGPROBS.md) and reasoning off (the consistency lives in the
/// analysis text; hidden reasoning behind it is measured verdict
/// jitter — sigma-eps-knobs pack, 2026-08-31), turn 2 re-sends the
/// conversation with the analysis as an assistant message and reads the
/// single ladder letter at reasoning-off with answer logprobs. The
/// posterior still arrives as one token position's PMF. `render` returns the turn-1
/// prompt (the deterministic part); the executing path appends the
/// analysis and [`TWO_PHASE_ANSWER_PROMPT`].
#[derive(Clone, Copy, Debug, Default)]
pub struct RatioLetterTwoPhaseInstrument;

impl Instrument for RatioLetterTwoPhaseInstrument {
    fn kind(&self) -> InstrumentKind {
        InstrumentKind::RatioLetterPairwise
    }

    fn parser_version(&self) -> ParserVersion {
        ParserVersion(PARSER_VERSION.to_string())
    }

    fn render(&self, attribute: &Attribute, slot_a: &Entity, slot_b: &Entity) -> RenderedPrompt {
        let system = two_phase_system_prompt();
        let user = render_user(attribute, &slot_a.body, &slot_b.body);
        // The fixed turn-2 instruction is part of the instrument's identity:
        // fold it into the skeleton so changing it changes the hash.
        let skeleton = format!(
            "{}\u{1f}{TWO_PHASE_ANSWER_PROMPT}",
            render_user(attribute, SLOT_A_PLACEHOLDER, SLOT_B_PLACEHOLDER)
        );
        RenderedPrompt {
            template: template_hash(&system, &skeleton),
            system,
            user,
            response_format_json: false,
            answer_alphabet: ratio_letter_alphabet(),
        }
    }

    fn parse(
        &self,
        content: &str,
        answer_logprobs: Option<&[TokenLogprob]>,
    ) -> Result<ParseOutcome, InstrumentError> {
        RatioLetterInstrument.parse(content, answer_logprobs)
    }
}

/// The 52 single-character tokens whose logprobs constitute the answer PMF
/// (the refusal token `!` is intentionally excluded: it is not part of the
/// informative alphabet, only a health signal).
pub fn ratio_letter_alphabet() -> Vec<String> {
    let mut out = Vec::with_capacity(52);
    for b in b'A'..=b'Z' {
        out.push((b as char).to_string());
    }
    for b in b'a'..=b'z' {
        out.push((b as char).to_string());
    }
    out
}

fn is_ratio_token(trimmed: &str) -> bool {
    single_char_atom(trimmed).is_some()
}

fn is_refusal_token(trimmed: &str) -> bool {
    trimmed == REFUSAL_TOKEN
}

fn single_char_atom(trimmed: &str) -> Option<AnswerAtom> {
    let mut chars = trimmed.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => AnswerAtom::from_letter(c),
        _ => None,
    }
}

fn parse_logprob(positions: &[TokenLogprob]) -> Result<ParseOutcome, InstrumentError> {
    let position = find_answer_position(positions, |t| is_ratio_token(t) || is_refusal_token(t))
        .ok_or(InstrumentError::Unparseable)?;
    let chosen = trim_token(&position.token);
    let refused = is_refusal_token(chosen);
    let parsed_cleanly = is_ratio_token(chosen);

    let candidates = merge_candidates(position);
    let atom_logprobs: Vec<AtomLogprob> = candidates
        .iter()
        .filter_map(|(tok, logprob)| {
            single_char_atom(trim_token(tok)).map(|atom| AtomLogprob {
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
    let refused = trimmed.starts_with(REFUSAL_TOKEN);
    let first = trimmed.chars().next().expect("non-empty checked above");
    let atom = AnswerAtom::from_letter(first).unwrap_or(AnswerAtom::OffAlphabet);
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

impl Instrument for RatioLetterInstrument {
    fn kind(&self) -> InstrumentKind {
        InstrumentKind::RatioLetterPairwise
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
            answer_alphabet: ratio_letter_alphabet(),
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
    use crate::seriate::evidence::PmfCompleteness;

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
    fn render_is_deterministic_and_hides_slots_from_the_template_hash() {
        let inst = RatioLetterInstrument;
        let a1 = Entity::new("first entity body");
        let b1 = Entity::new("second entity body");
        let a2 = Entity::new("a totally different entity");
        let b2 = Entity::new("yet another different entity");
        let r1 = inst.render(&attr(), &a1, &b1);
        let r2 = inst.render(&attr(), &a1, &b1);
        assert_eq!(r1, r2, "render is a pure function of its inputs");

        let r3 = inst.render(&attr(), &a2, &b2);
        assert_eq!(
            r1.template, r3.template,
            "different entities, same attribute -> same template hash"
        );
        assert_ne!(r1.user, r3.user, "user text does differ per entity");
        assert_eq!(r1.answer_alphabet.len(), 52);
        assert!(!r1.response_format_json);
    }

    #[test]
    fn different_attribute_changes_the_template_hash() {
        let inst = RatioLetterInstrument;
        let a = Entity::new("x");
        let b = Entity::new("y");
        let r1 = inst.render(&attr(), &a, &b);
        let other = Attribute::new("clarity", "how clear the writing is");
        let r2 = inst.render(&other, &a, &b);
        assert_ne!(r1.template, r2.template);
    }

    #[test]
    fn logprob_parse_splits_mass_honestly_with_junk_present() {
        // Chosen token "B" (A wins bucket 1) at 60%, alt "c" (B wins bucket
        // 2) at 20%, alt "###" junk at 10%: 90% visible, 10% never shown.
        let position = tl("B", 0.6, &[("B", 0.6), ("c", 0.2), ("###", 0.1)]);
        let outcome = RatioLetterInstrument.parse("B", Some(&[position])).unwrap();
        assert!((outcome.evidence.p(AnswerAtom::A(1)) - 0.6).abs() < 1e-9);
        assert!((outcome.evidence.p(AnswerAtom::B(2)) - 0.2).abs() < 1e-9);
        assert!((outcome.evidence.p(AnswerAtom::OffAlphabet) - 0.1).abs() < 1e-9);
        assert!((outcome.evidence.p(AnswerAtom::Abstain) - 0.1).abs() < 1e-9);
        assert!(outcome.health.parsed_cleanly);
        assert!(!outcome.health.refused);
        assert_eq!(outcome.mode, AcquisitionMode::Logprob);
        match outcome.evidence.completeness {
            PmfCompleteness::Truncated { .. } => {}
            other => panic!("expected Truncated, got {other:?}"),
        }
    }

    #[test]
    fn logprob_parse_skips_a_leading_filler_position() {
        let filler = tl(" ", 0.999, &[]);
        let answer = tl("g", 0.9, &[("g", 0.9), ("h", 0.05)]);
        let outcome = RatioLetterInstrument
            .parse("g", Some(&[filler, answer]))
            .unwrap();
        assert!(
            outcome.evidence.p(AnswerAtom::B(6)) > 0.0,
            "lowercase g = B wins bucket 6"
        );
    }

    #[test]
    fn logprob_parse_handles_refusal_with_no_informative_alternatives() {
        let position = tl("!", 0.95, &[("!", 0.95), ("###", 0.05)]);
        let outcome = RatioLetterInstrument.parse("!", Some(&[position])).unwrap();
        assert!(outcome.health.refused);
        assert!(!outcome.health.parsed_cleanly);
        assert_eq!(outcome.evidence.off_alphabet_mass(), 1.0);
    }

    #[test]
    fn logprob_parse_errors_when_no_position_matches_at_all() {
        let position = tl("hello", 0.9, &[("world", 0.05)]);
        let err = RatioLetterInstrument
            .parse("hello world", Some(&[position]))
            .unwrap_err();
        assert_eq!(err, InstrumentError::Unparseable);
    }

    #[test]
    fn sampled_fallback_uses_first_char_of_trimmed_content() {
        let outcome = RatioLetterInstrument.parse("  H extra junk", None).unwrap();
        assert_eq!(
            outcome.evidence.p(AnswerAtom::A(7)),
            1.0,
            "uppercase H = A wins bucket 7"
        );
        assert_eq!(outcome.mode, AcquisitionMode::Sampled);
        assert!(outcome.health.parsed_cleanly);
        assert_eq!(outcome.health.visible_mass, 1.0);
    }

    #[test]
    fn sampled_fallback_detects_refusal_and_junk() {
        let refusal = RatioLetterInstrument.parse(" ! ", None).unwrap();
        assert!(refusal.health.refused);

        let junk = RatioLetterInstrument.parse("###", None).unwrap();
        assert!(!junk.health.parsed_cleanly);
        assert_eq!(junk.evidence.off_alphabet_mass(), 1.0);
    }

    #[test]
    fn sampled_fallback_errors_on_empty_content() {
        let err = RatioLetterInstrument.parse("   ", None).unwrap_err();
        assert_eq!(err, InstrumentError::EmptyContent);
    }

    #[test]
    fn empty_logprob_slice_falls_back_to_sampled_path() {
        let outcome = RatioLetterInstrument.parse("b", Some(&[])).unwrap();
        assert_eq!(outcome.mode, AcquisitionMode::Sampled);
    }
}
