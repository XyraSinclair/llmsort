//! Additive-offset calibration: presentation biases fitted jointly with scores.
//!
//! The additive sibling of [`crate::gain_calibration`]. That module treats each
//! wording as an instrument with an unknown multiplicative *gain*; this one
//! treats each presentation channel as an instrument with an unknown additive
//! *offset* in log-ratio space:
//!
//! ```text
//! m_obs = (s_i − s_j) + sign · γ_channel + ε
//! ```
//!
//! where `sign` says which side of the pair the channel favours in this
//! particular observation (+1 toward `i`, −1 toward `j`), and `γ_channel` is
//! one fitted offset per named channel (nats). Two live pathologies are
//! instances of this model:
//!
//! - **Order bias**: channel = the judge (or judge×template), sign = +1 when
//!   `i` was presented first. Counterbalancing cancels this at 2× calls;
//!   fitting γ removes it at 1× and *measures* it, which is what lets a run
//!   shed counterbalancing adaptively once γ is pinned near zero.
//! - **Pivot halo** (PROGRAM.md E1: 795/870 setwise ratios < 1 against the
//!   pivot, mean −0.95 nats): channel = the pivot role, sign = +1 when `i`
//!   is the pivot of the call. Pivot rotation across calls is what makes γ
//!   identifiable separately from the pivot item's score.
//!
//! The offset is deliberately NOT a column in the core IRLS solve: the engine
//! fuses observations into per-pair sufficient statistics before solving, and
//! the packet layer pins that fusion bitwise. An in-matrix bias column would
//! break both. Instead — exactly like the gain module — the bilinear fit
//! alternates two closed-form steps above the engine: scores given offsets
//! (subtract `sign·γ`, ordinary robust solve), then offsets given scores
//! (per-channel MAP mean of the signed residuals under a Gaussian prior
//! `N(0, prior_tau2)`). Each step cannot increase the penalized residual, so
//! the alternation converges; the prior keeps a channel finite even when its
//! sign never varies (an un-rotated pivot, a never-flipped order).
//!
//! Prior scale: measured order residuals run 0.04–0.28 nats and the worst
//! observed halo −0.95 nats (E1/E10 packs), so the default `prior_tau2 = 1.0`
//! (std 1 nat) is weak against every observed effect while still pinning
//! unidentified channels to 0.
//!
//! Verified by direct execution (2026-09-06, planted recovery, n = 8, noise
//! σ = 0.15): planted order bias +0.5 and pivot halo −0.9 recovered as
//! +0.513 / −0.900; corrected RMS 0.139 (the noise floor) vs 0.652
//! uncorrected; score correlation with truth 0.9994; 7 alternation rounds.
//!
//! Provenance note: the fitted-in-the-likelihood shape is validated by
//! nanojudge (`laplace_bt.rs`, per-judge per-slot β with a Gaussian prior,
//! mined 2026-09-05 — `research/notes/nanojudge-mining-2026-09-05/`); the
//! alternation-above-the-engine placement is this repo's own architecture.

use std::collections::HashMap;

use serde::Serialize;

use crate::rating_engine::{AttributeParams, Config, Observation, RaterParams, RatingEngine};

/// One observation with an optional additive-offset tag: signed log-ratio
/// toward `i`, plus which presentation channel (if any) pushed on it and in
/// which direction.
#[derive(Debug, Clone)]
pub struct BiasObservation {
    pub i: usize,
    pub j: usize,
    /// Signed log-ratio toward `i`, as elicited (uncorrected).
    pub log_ratio: f64,
    /// Named offset channel this observation is exposed to, or `None` for a
    /// plain observation (e.g. one half of a counterbalanced pair after
    /// averaging).
    pub channel: Option<String>,
    /// Direction of the channel's push: +1.0 when the channel favours `i`
    /// (i presented first / i is the pivot), −1.0 when it favours `j`.
    /// Ignored when `channel` is `None`. Must be finite.
    pub sign: f64,
}

/// Result of [`solve_with_additive_offsets`].
#[derive(Debug, Serialize)]
pub struct BiasCalibratedSolve {
    /// Latent scores with every fitted offset subtracted out.
    pub scores: Vec<f64>,
    /// Fitted additive offset per channel (nats, sorted by channel name).
    /// Positive means the channel inflates the favoured side by that many
    /// nats; e.g. an order-bias channel at +0.3 says the first-presented
    /// item reads 0.3 nats hotter than it is.
    pub offsets: Vec<(String, f64)>,
    /// Alternation rounds until every offset moved < 1e-6.
    pub iterations: usize,
    /// Root-mean-square residual of the final fit (nats).
    pub rms_residual: f64,
    /// RMS residual of a naive solve that ignores offsets — the price of
    /// NOT correcting, on the same data.
    pub rms_residual_uncorrected: f64,
}

fn solve_scores(n: usize, obs: &[(usize, usize, f64)]) -> Option<Vec<f64>> {
    let mut raters = HashMap::new();
    raters.insert("bias".to_string(), RaterParams::default());
    let mut engine = RatingEngine::new(
        n,
        AttributeParams::default(),
        raters,
        Some(Config::default()),
    )
    .ok()?;
    let observations: Vec<Observation> = obs
        .iter()
        .map(|&(i, j, m)| Observation::from_log_ratio_moments(i, j, m, 1.0, "bias", 1.0))
        .collect();
    engine.ingest(&observations);
    Some(engine.solve().scores)
}

fn signed_offset(o: &BiasObservation, offsets: &HashMap<String, f64>) -> f64 {
    match &o.channel {
        Some(c) => o.sign * offsets.get(c).copied().unwrap_or(0.0),
        None => 0.0,
    }
}

fn rms(obs: &[BiasObservation], scores: &[f64], offsets: &HashMap<String, f64>) -> f64 {
    let sum: f64 = obs
        .iter()
        .map(|o| {
            let pred = (scores[o.i] - scores[o.j]) + signed_offset(o, offsets);
            (o.log_ratio - pred).powi(2)
        })
        .sum();
    (sum / obs.len().max(1) as f64).sqrt()
}

/// Fit scores and per-channel additive offsets jointly.
///
/// `prior_tau2` is the Gaussian prior variance on each offset (nats²); it
/// must be finite and positive. Returns `None` on empty input, invalid
/// `prior_tau2`, a non-finite `log_ratio`/`sign`, or an index out of range.
pub fn solve_with_additive_offsets(
    n: usize,
    obs: &[BiasObservation],
    prior_tau2: f64,
) -> Option<BiasCalibratedSolve> {
    if obs.is_empty() || !(prior_tau2.is_finite() && prior_tau2 > 0.0) {
        return None;
    }
    for o in obs {
        if o.i >= n || o.j >= n || !o.log_ratio.is_finite() || !o.sign.is_finite() {
            return None;
        }
    }
    let mut channels: Vec<String> = obs.iter().filter_map(|o| o.channel.clone()).collect();
    channels.sort();
    channels.dedup();

    let mut offsets: HashMap<String, f64> = channels.iter().map(|c| (c.clone(), 0.0)).collect();

    // Uncorrected baseline: one solve pretending every offset is 0.
    let naive: Vec<(usize, usize, f64)> = obs.iter().map(|o| (o.i, o.j, o.log_ratio)).collect();
    let naive_scores = solve_scores(n, &naive)?;
    let zero_offsets: HashMap<String, f64> = channels.iter().map(|c| (c.clone(), 0.0)).collect();
    let rms_residual_uncorrected = rms(obs, &naive_scores, &zero_offsets);

    let prior_precision = 1.0 / prior_tau2;
    let mut scores = naive_scores;
    let mut iterations = 0usize;
    for round in 0..50 {
        iterations = round + 1;
        // Scores given offsets: subtract each observation's signed offset and
        // re-solve on the corrected log-ratios.
        let corrected: Vec<(usize, usize, f64)> = obs
            .iter()
            .map(|o| (o.i, o.j, o.log_ratio - signed_offset(o, &offsets)))
            .collect();
        scores = solve_scores(n, &corrected)?;

        // Offsets given scores: per-channel MAP mean of the signed residuals,
        //   γ_c = Σ sign·(m − d) / (Σ sign² + 1/τ²),   d = s_i − s_j,
        // the closed-form ridge regression of the residual on the sign column.
        let mut moved = 0.0f64;
        for c in &channels {
            let (mut num, mut den) = (0.0f64, prior_precision);
            for o in obs.iter().filter(|o| o.channel.as_deref() == Some(c)) {
                let d = scores[o.i] - scores[o.j];
                num += o.sign * (o.log_ratio - d);
                den += o.sign * o.sign;
            }
            let new_offset = num / den;
            moved = moved.max((new_offset - offsets[c]).abs());
            offsets.insert(c.clone(), new_offset);
        }
        if moved < 1e-6 {
            break;
        }
    }

    let rms_residual = rms(obs, &scores, &offsets);
    let mut offsets_out: Vec<(String, f64)> =
        channels.iter().map(|c| (c.clone(), offsets[c])).collect();
    offsets_out.sort_by(|a, b| a.0.cmp(&b.0));

    Some(BiasCalibratedSolve {
        scores,
        offsets: offsets_out,
        iterations,
        rms_residual,
        rms_residual_uncorrected,
    })
}
