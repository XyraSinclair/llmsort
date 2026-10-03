//! Evidence: normalized PMFs over the answer space, with honest accounting
//! of how much probability mass was actually visible.
//!
//! Every judgement in seriate is a distribution, not a point. Logprob mode
//! yields the model's prior directly (top-k logprobs at the answer token);
//! sampled mode yields an empirical PMF. `PmfCompleteness` travels with every
//! PMF so downstream weighting can distinguish "the model's full prior" from
//! "the top-5 shadow of it".

use crate::seriate::atom::AnswerAtom;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// How complete the PMF is relative to the model's true answer distribution.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum PmfCompleteness {
    /// Every unit of probability mass is accounted for by a parsed atom.
    Complete,
    /// Provider showed only part of the distribution; the remainder is
    /// carried on the `Abstain` atom.
    Truncated {
        shown_mass: f64,
        unresolved_mass: f64,
    },
    /// PMF is an empirical frequency over `samples` independent draws.
    Empirical { samples: u32 },
}

/// One (atom, probability) support point.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct AtomProb {
    pub atom: AnswerAtom,
    pub p: f64,
}

/// One (atom, logprob) pair as parsed from provider top-logprobs.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct AtomLogprob {
    pub atom: AnswerAtom,
    pub logprob: f64,
}

/// Errors constructing or combining evidence.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum EvidenceError {
    #[error("evidence support is empty")]
    Empty,
    #[error("non-finite probability or logprob")]
    NonFinite,
    #[error("negative probability")]
    Negative,
    #[error("support has zero total mass")]
    ZeroMass,
    #[error("visible mass out of range or inconsistent with shown support")]
    InvalidMass,
}

/// A normalized PMF over the answer space, plus completeness provenance.
///
/// Invariants (enforced at construction, tested):
/// - support is deduplicated (same atom merged), sorted, strictly positive;
/// - probabilities sum to 1 (up to fp);
/// - completeness always present.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AnswerEvidence {
    support: Vec<AtomProb>,
    pub completeness: PmfCompleteness,
}

impl AnswerEvidence {
    /// Normalize arbitrary non-negative weights over atoms into a PMF.
    pub fn new(
        support: impl IntoIterator<Item = AtomProb>,
        completeness: PmfCompleteness,
    ) -> Result<Self, EvidenceError> {
        let mut merged = BTreeMap::<AnswerAtom, f64>::new();
        for AtomProb { atom, p } in support {
            if !p.is_finite() {
                return Err(EvidenceError::NonFinite);
            }
            if p < 0.0 {
                return Err(EvidenceError::Negative);
            }
            *merged.entry(atom).or_default() += p;
        }
        if merged.is_empty() {
            return Err(EvidenceError::Empty);
        }
        let z: f64 = merged.values().sum();
        if z <= 0.0 {
            return Err(EvidenceError::ZeroMass);
        }
        Ok(Self {
            support: merged
                .into_iter()
                .filter(|(_, p)| *p > 0.0)
                .map(|(atom, p)| AtomProb { atom, p: p / z })
                .collect(),
            completeness,
        })
    }

    pub fn support(&self) -> &[AtomProb] {
        &self.support
    }

    /// Probability of a specific atom (0 when absent from support).
    pub fn p(&self, atom: AnswerAtom) -> f64 {
        self.support
            .iter()
            .find(|x| x.atom == atom)
            .map(|x| x.p)
            .unwrap_or(0.0)
    }

    /// Mass on tokens that were visible but parsed to no atom.
    pub fn off_alphabet_mass(&self) -> f64 {
        self.p(AnswerAtom::OffAlphabet)
    }

    /// Mass the provider never made visible.
    pub fn abstain_mass(&self) -> f64 {
        self.p(AnswerAtom::Abstain)
    }

    /// Total mass on informative atoms (everything except escapes).
    pub fn informative_mass(&self) -> f64 {
        self.support
            .iter()
            .filter(|x| x.atom.is_informative())
            .map(|x| x.p)
            .sum()
    }

    /// P(slot A judged higher), P(parity), P(slot B judged higher),
    /// renormalized over informative mass. `None` if no informative mass.
    pub fn directional_summary(&self) -> Option<(f64, f64, f64)> {
        let z = self.informative_mass();
        if z <= 0.0 {
            return None;
        }
        let mut a = 0.0;
        let mut parity = 0.0;
        let mut b = 0.0;
        for AtomProb { atom, p } in &self.support {
            match atom {
                AnswerAtom::A(_) => a += p,
                AnswerAtom::B(_) => b += p,
                AnswerAtom::Parity => parity += p,
                _ => {}
            }
        }
        Some((a / z, parity / z, b / z))
    }

    /// Expected signed log-ratio over informative mass, with the variance of
    /// the same. `None` if no informative mass. This is the moment pair the
    /// compiler consumes as a weighted observation.
    pub fn log_ratio_moments(&self) -> Option<(f64, f64)> {
        let z = self.informative_mass();
        if z <= 0.0 {
            return None;
        }
        let mut mean = 0.0;
        for AtomProb { atom, p } in &self.support {
            if let Some(lr) = atom.signed_log_ratio() {
                mean += (p / z) * lr;
            }
        }
        let mut var = 0.0;
        for AtomProb { atom, p } in &self.support {
            if let Some(lr) = atom.signed_log_ratio() {
                var += (p / z) * (lr - mean).powi(2);
            }
        }
        Some((mean, var))
    }

    /// The same evidence with presented slots exchanged (exact reflection).
    pub fn reflected(&self) -> Self {
        Self {
            support: {
                let mut s: Vec<AtomProb> = self
                    .support
                    .iter()
                    .map(|x| AtomProb {
                        atom: x.atom.reflected(),
                        p: x.p,
                    })
                    .collect();
                s.sort_by_key(|x| x.atom);
                s
            },
            completeness: self.completeness,
        }
    }
}

/// Build evidence from parsed answer-token top-logprobs.
///
/// `visible_mass` is the total probability the provider made visible at the
/// answer position (sum over ALL top-k tokens, parseable or not). Mass that
/// was visible but parsed to no atom becomes `OffAlphabet`; mass never shown
/// becomes `Abstain`, and completeness records the split.
pub fn evidence_from_logprobs(
    atom_logprobs: &[AtomLogprob],
    visible_mass: Option<f64>,
) -> Result<AnswerEvidence, EvidenceError> {
    if atom_logprobs.is_empty() {
        return Err(EvidenceError::Empty);
    }
    let mut support = Vec::with_capacity(atom_logprobs.len() + 2);
    let mut shown: f64 = 0.0;
    for AtomLogprob { atom, logprob } in atom_logprobs {
        if !logprob.is_finite() {
            return Err(EvidenceError::NonFinite);
        }
        let p = logprob.exp();
        if !p.is_finite() {
            return Err(EvidenceError::NonFinite);
        }
        shown += p;
        support.push(AtomProb { atom: *atom, p });
    }
    let mut visible = visible_mass.unwrap_or(shown);
    // Real provider quirk: the chosen token's logprob is sometimes rounded to
    // exactly 0.0 while alternatives stay finite, pushing summed mass a hair
    // over 1.0. Forgive small overflow by clamping; reject real nonsense.
    const PROVIDER_ROUNDING_SLOP: f64 = 1e-4;
    if visible.is_finite() && visible > 1.0 && visible <= 1.0 + PROVIDER_ROUNDING_SLOP {
        visible = 1.0;
    }
    if shown.is_finite() && shown > visible && shown <= visible + PROVIDER_ROUNDING_SLOP {
        shown = visible;
    }
    if !visible.is_finite() || !(0.0..=1.0).contains(&visible) || shown > visible {
        return Err(EvidenceError::InvalidMass);
    }
    let off_alphabet = (visible - shown).max(0.0);
    if off_alphabet > 0.0 {
        support.push(AtomProb {
            atom: AnswerAtom::OffAlphabet,
            p: off_alphabet,
        });
    }
    let unresolved = (1.0 - visible).max(0.0);
    if unresolved > 0.0 {
        support.push(AtomProb {
            atom: AnswerAtom::Abstain,
            p: unresolved,
        });
    }
    AnswerEvidence::new(
        support,
        if unresolved > 0.0 {
            PmfCompleteness::Truncated {
                shown_mass: (1.0 - unresolved).clamp(0.0, 1.0),
                unresolved_mass: unresolved,
            }
        } else {
            PmfCompleteness::Complete
        },
    )
}

/// Build empirical evidence from repeated sampled answers.
pub fn evidence_from_resamples(samples: &[AnswerAtom]) -> Result<AnswerEvidence, EvidenceError> {
    let mut counts = BTreeMap::<AnswerAtom, f64>::new();
    for atom in samples {
        *counts.entry(*atom).or_default() += 1.0;
    }
    AnswerEvidence::new(
        counts.into_iter().map(|(atom, p)| AtomProb { atom, p }),
        PmfCompleteness::Empirical {
            samples: samples.len() as u32,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::seriate::atom::RATIO_LADDER;

    fn lp(atom: AnswerAtom, p: f64) -> AtomLogprob {
        AtomLogprob {
            atom,
            logprob: p.ln(),
        }
    }

    #[test]
    fn construction_normalizes_dedups_and_sorts() {
        let ev = AnswerEvidence::new(
            [
                AtomProb {
                    atom: AnswerAtom::A(3),
                    p: 2.0,
                },
                AtomProb {
                    atom: AnswerAtom::B(3),
                    p: 1.0,
                },
                AtomProb {
                    atom: AnswerAtom::A(3),
                    p: 1.0,
                },
            ],
            PmfCompleteness::Complete,
        )
        .unwrap();
        assert_eq!(ev.support().len(), 2);
        let total: f64 = ev.support().iter().map(|x| x.p).sum();
        assert!((total - 1.0).abs() < 1e-12);
        assert!((ev.p(AnswerAtom::A(3)) - 0.75).abs() < 1e-12);
        assert!((ev.p(AnswerAtom::B(3)) - 0.25).abs() < 1e-12);
    }

    #[test]
    fn construction_rejects_bad_mass() {
        assert_eq!(
            AnswerEvidence::new([], PmfCompleteness::Complete).unwrap_err(),
            EvidenceError::Empty
        );
        assert_eq!(
            AnswerEvidence::new(
                [AtomProb {
                    atom: AnswerAtom::Parity,
                    p: -0.1
                }],
                PmfCompleteness::Complete
            )
            .unwrap_err(),
            EvidenceError::Negative
        );
        assert_eq!(
            AnswerEvidence::new(
                [AtomProb {
                    atom: AnswerAtom::Parity,
                    p: 0.0
                }],
                PmfCompleteness::Complete
            )
            .unwrap_err(),
            EvidenceError::ZeroMass
        );
        assert_eq!(
            AnswerEvidence::new(
                [AtomProb {
                    atom: AnswerAtom::Parity,
                    p: f64::NAN
                }],
                PmfCompleteness::Complete
            )
            .unwrap_err(),
            EvidenceError::NonFinite
        );
    }

    #[test]
    fn logprob_evidence_accounts_for_every_unit_of_mass() {
        // 60% on B (A wins small), 20% on c (B wins bucket 2), provider says
        // 90% was visible: 10% off-alphabet, 10% never shown.
        let ev = evidence_from_logprobs(
            &[lp(AnswerAtom::A(1), 0.6), lp(AnswerAtom::B(2), 0.2)],
            Some(0.9),
        )
        .unwrap();
        assert!((ev.p(AnswerAtom::A(1)) - 0.6).abs() < 1e-9);
        assert!((ev.p(AnswerAtom::B(2)) - 0.2).abs() < 1e-9);
        assert!((ev.p(AnswerAtom::OffAlphabet) - 0.1).abs() < 1e-9);
        assert!((ev.p(AnswerAtom::Abstain) - 0.1).abs() < 1e-9);
        match ev.completeness {
            PmfCompleteness::Truncated {
                shown_mass,
                unresolved_mass,
            } => {
                assert!((shown_mass - 0.9).abs() < 1e-9);
                assert!((unresolved_mass - 0.1).abs() < 1e-9);
            }
            other => panic!("expected Truncated, got {other:?}"),
        }
    }

    #[test]
    fn logprob_evidence_complete_when_all_mass_visible() {
        let ev = evidence_from_logprobs(
            &[lp(AnswerAtom::A(1), 0.7), lp(AnswerAtom::Parity, 0.3)],
            Some(1.0),
        )
        .unwrap();
        assert_eq!(ev.completeness, PmfCompleteness::Complete);
        assert_eq!(ev.p(AnswerAtom::Abstain), 0.0);
    }

    #[test]
    fn visible_mass_clamps_small_provider_rounding_overflow() {
        // Chosen token rounded to logprob 0.0 (p=1.0) with a finite alt:
        // summed mass 1.0000001 must clamp, not error.
        let ev = evidence_from_logprobs(
            &[
                AtomLogprob {
                    atom: AnswerAtom::A(1),
                    logprob: 0.0,
                },
                AtomLogprob {
                    atom: AnswerAtom::B(1),
                    logprob: (1e-7f64).ln(),
                },
            ],
            None,
        )
        .unwrap();
        assert_eq!(ev.completeness, PmfCompleteness::Complete);
        // Genuine nonsense still rejected.
        assert!(evidence_from_logprobs(
            &[AtomLogprob {
                atom: AnswerAtom::A(1),
                logprob: 0.5
            }],
            None
        )
        .is_err());
    }

    #[test]
    fn logprob_evidence_rejects_inconsistent_visible_mass() {
        // Shown 0.8 but visible claimed 0.5: impossible.
        let err = evidence_from_logprobs(&[lp(AnswerAtom::A(1), 0.8)], Some(0.5)).unwrap_err();
        assert_eq!(err, EvidenceError::InvalidMass);
    }

    #[test]
    fn truncation_is_monotone_under_deeper_top_k() {
        // Seeing MORE of the same distribution never increases abstain mass.
        let full = [
            lp(AnswerAtom::A(2), 0.5),
            lp(AnswerAtom::A(1), 0.25),
            lp(AnswerAtom::Parity, 0.15),
        ];
        let shallow = evidence_from_logprobs(&full[..1], Some(0.5)).unwrap();
        let deeper = evidence_from_logprobs(&full[..2], Some(0.75)).unwrap();
        let deepest = evidence_from_logprobs(&full, Some(0.9)).unwrap();
        let abstain = |e: &AnswerEvidence| e.p(AnswerAtom::Abstain);
        assert!(abstain(&shallow) > abstain(&deeper));
        assert!(abstain(&deeper) > abstain(&deepest));
    }

    #[test]
    fn empirical_evidence_is_frequency() {
        let ev = evidence_from_resamples(&[
            AnswerAtom::A(1),
            AnswerAtom::A(1),
            AnswerAtom::B(1),
            AnswerAtom::Parity,
        ])
        .unwrap();
        assert!((ev.p(AnswerAtom::A(1)) - 0.5).abs() < 1e-12);
        assert_eq!(ev.completeness, PmfCompleteness::Empirical { samples: 4 });
    }

    #[test]
    fn reflection_is_an_involution_preserving_mass() {
        let ev = evidence_from_logprobs(
            &[
                lp(AnswerAtom::A(3), 0.5),
                lp(AnswerAtom::B(1), 0.3),
                lp(AnswerAtom::Parity, 0.1),
            ],
            Some(0.95),
        )
        .unwrap();
        let r = ev.reflected();
        assert!((r.p(AnswerAtom::B(3)) - ev.p(AnswerAtom::A(3))).abs() < 1e-12);
        assert!((r.p(AnswerAtom::A(1)) - ev.p(AnswerAtom::B(1))).abs() < 1e-12);
        assert_eq!(r.reflected(), ev);
        let (mean, _) = ev.log_ratio_moments().unwrap();
        let (rmean, _) = r.log_ratio_moments().unwrap();
        assert!((mean + rmean).abs() < 1e-12, "moments antisymmetric");
    }

    #[test]
    fn moments_match_hand_computation() {
        // 70% A-bucket-1 (ln 1.06), 30% parity (0).
        let ev = evidence_from_logprobs(
            &[lp(AnswerAtom::A(1), 0.7), lp(AnswerAtom::Parity, 0.3)],
            Some(1.0),
        )
        .unwrap();
        let (mean, var) = ev.log_ratio_moments().unwrap();
        let l = RATIO_LADDER[0].ln();
        assert!((mean - 0.7 * l).abs() < 1e-12);
        let expect_var = 0.7 * (l - 0.7 * l).powi(2) + 0.3 * (0.7 * l).powi(2);
        assert!((var - expect_var).abs() < 1e-12);
    }

    #[test]
    fn escape_mass_is_excluded_from_moments_but_not_ignored() {
        let ev = evidence_from_logprobs(&[lp(AnswerAtom::A(1), 0.5)], Some(0.5)).unwrap();
        // Half the mass is Abstain; moments renormalize over informative mass
        // only, and informative_mass reports the discount.
        assert!((ev.informative_mass() - 0.5).abs() < 1e-9);
        let (mean, _) = ev.log_ratio_moments().unwrap();
        assert!((mean - RATIO_LADDER[0].ln()).abs() < 1e-9);
    }
}
