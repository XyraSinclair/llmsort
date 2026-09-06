//! The stickbreak promotion gate: fit per-slot presentation channels over
//! the live stick-breaking edges and re-measure agreement.
//!
//! Run: cargo run --locked -p llmsort-experiments --example stickbreak_slot_bias
//!
//! The arbiter run (stickbreak-order-arbiter-2026-09-06) showed stickbreak
//! near-perfectly repeatable yet in stable disagreement with a 2.3×-cover
//! pairwise reference, with first-pick histograms pulling hard toward slot
//! B. Model: the item presented in slot p reads β_p nats hot, so a
//! stick-breaking edge between lineup slots a and b is
//! `m = (s_a − s_b) + β_{slot(a)} − β_{slot(b)} + ε` — two channels with
//! opposite signs on one observation (the multi-channel `bias_calibration`
//! shape). β is identifiable up to a constant (every edge carries one +1
//! and one −1); the Gaussian prior pins the gauge softly and betas are
//! reported mean-centered.
//!
//! Edges are rebuilt from the pack's persisted `slot_pmfs` exactly as the
//! live fold built them (stick-breaking over renormalized conditional PMFs,
//! q clamped at 1e-9 before logs). Reference: the pack's own pairwise arm.

use llmsort::bias_calibration::{solve_with_additive_offsets, BiasObservation};
use llmsort::rating_engine::{AttributeParams, Config, Observation, RaterParams, RatingEngine};
use serde_json::Value;
use std::collections::HashMap;

const PACK: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../research/artifacts/live/stickbreak-order-arbiter-2026-09-06"
);
const SLOT_LETTERS: [char; 8] = ['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H'];

fn uncorrected_scores(n: usize, obs: &[BiasObservation]) -> Vec<f64> {
    let raters = HashMap::from([("judge".to_owned(), RaterParams::default())]);
    let mut engine = RatingEngine::new(
        n,
        AttributeParams::default(),
        raters,
        Some(Config::default()),
    )
    .expect("valid engine");
    let observations: Vec<Observation> = obs
        .iter()
        .map(|o| Observation::from_log_ratio_moments(o.i, o.j, o.log_ratio, 1.0, "judge", 1.0))
        .collect();
    engine.ingest(&observations);
    engine.solve().scores
}

fn ranks(values: &[f64]) -> Vec<f64> {
    let mut order: Vec<_> = (0..values.len()).collect();
    order.sort_by(|&a, &b| values[a].total_cmp(&values[b]));
    let mut ranks = vec![0.0; values.len()];
    let mut start = 0;
    while start < order.len() {
        let mut end = start + 1;
        while end < order.len() && values[order[end]] == values[order[start]] {
            end += 1;
        }
        for &i in &order[start..end] {
            ranks[i] = (start + end - 1) as f64 / 2.0;
        }
        start = end;
    }
    ranks
}

fn spearman(x: &[f64], y: &[f64]) -> f64 {
    let (rx, ry) = (ranks(x), ranks(y));
    let center = (x.len() - 1) as f64 / 2.0;
    let (mut xy, mut xx, mut yy) = (0.0, 0.0, 0.0);
    for (a, b) in rx.into_iter().zip(ry) {
        let (a, b) = (a - center, b - center);
        xy += a * b;
        xx += a * a;
        yy += b * b;
    }
    assert!(xx > 0.0 && yy > 0.0);
    xy / (xx * yy).sqrt()
}

/// Rebuild the winner distribution q over lineup slot positions from the
/// persisted per-rank renormalized PMFs, mirroring the live fold.
fn rebuild_q(slots: &[usize], pmfs: &[HashMap<String, f64>], kk: usize) -> Vec<f64> {
    assert_eq!(pmfs.len(), kk - 1);
    let mut q = vec![0.0; kk];
    let mut residual = 1.0f64;
    for (r, &chosen) in slots[..kk - 1].iter().enumerate() {
        let letter = SLOT_LETTERS[chosen].to_string();
        let p = pmfs[r][&letter];
        q[chosen] = residual * p;
        residual -= q[chosen];
    }
    q[slots[kk - 1]] = residual.max(0.0);
    q
}

fn fit_and_report(
    label: &str,
    n: usize,
    obs: &[BiasObservation],
    truth: &[f64],
) {
    let fit = solve_with_additive_offsets(n, obs, 1.0).expect("calibrated solve");
    let mean_beta: f64 =
        fit.offsets.iter().map(|(_, g)| g).sum::<f64>() / fit.offsets.len() as f64;
    let betas: Vec<String> = fit
        .offsets
        .iter()
        .map(|(c, g)| format!("{c}={:+.3}", g - mean_beta))
        .collect();
    let naive = uncorrected_scores(n, obs);
    println!(
        "{label} obs {:>4}  {}  it {:>2}  rms {:.3}->{:.3}  rho {:+.3}->{:+.3}",
        obs.len(),
        betas.join(" "),
        fit.iterations,
        fit.rms_residual_uncorrected,
        fit.rms_residual,
        spearman(&naive, truth),
        spearman(&fit.scores, truth),
    );
}

fn main() {
    let report: Value = serde_json::from_str(
        &std::fs::read_to_string(format!("{PACK}/report.json")).expect("report.json"),
    )
    .expect("valid report JSON");
    let entity_ids: Vec<String> = report["entity_ids"]
        .as_array()
        .expect("entity_ids")
        .iter()
        .map(|v| v.as_str().expect("id").to_owned())
        .collect();
    let n = entity_ids.len();
    let index: HashMap<&str, usize> = entity_ids
        .iter()
        .enumerate()
        .map(|(i, id)| (id.as_str(), i))
        .collect();
    let mut reference: HashMap<String, Vec<f64>> = HashMap::new();
    for arm in report["pairwise"].as_array().expect("pairwise") {
        let mut latent = vec![f64::NAN; n];
        for row in arm["latents"].as_array().expect("latents") {
            latent[index[row["id"].as_str().expect("id")]] =
                row["mean"].as_f64().expect("mean");
        }
        assert!(latent.iter().all(|v| v.is_finite()));
        reference.insert(arm["attribute"].as_str().expect("attr").to_owned(), latent);
    }

    // Slot-channelled edges per (k, attribute) arm.
    let mut arms: HashMap<(u64, String), Vec<BiasObservation>> = HashMap::new();
    let trace = std::fs::read_to_string(format!("{PACK}/trace.jsonl")).expect("trace.jsonl");
    for line in trace.lines() {
        let row: Value = serde_json::from_str(line).expect("trace row");
        let (Some(slots_v), Some(pmfs_v)) =
            (row["parsed_slots"].as_array(), row["slot_pmfs"].as_array())
        else {
            continue;
        };
        let lineup: Vec<usize> = row["slot_order_ids"]
            .as_array()
            .expect("slot_order_ids")
            .iter()
            .map(|v| index[v.as_str().expect("id")])
            .collect();
        let kk = lineup.len();
        let slots: Vec<usize> = slots_v
            .iter()
            .map(|v| usize::try_from(v.as_u64().expect("slot")).expect("fits"))
            .collect();
        let pmfs: Vec<HashMap<String, f64>> = pmfs_v
            .iter()
            .map(|m| {
                m.as_object()
                    .expect("pmf map")
                    .iter()
                    .map(|(k, v)| (k.clone(), v.as_f64().expect("mass")))
                    .collect()
            })
            .collect();
        let q = rebuild_q(&slots, &pmfs, kk);
        let arm = arms
            .entry((
                row["k"].as_u64().expect("k"),
                row["attribute"].as_str().expect("attribute").to_owned(),
            ))
            .or_default();
        for a in 0..kk {
            for b in a + 1..kk {
                // Winsorize at the ratio ladder's ceiling (26×): beyond it a
                // near-deterministic PMF's ln-q tail (down to the 1e-9
                // clamp, ±20 nats) carries no calibrated magnitude, and
                // the offset step is least-squares — unclamped tails, not
                // slot effects, would dominate the fit.
                let m = (q[a].max(1e-9).ln() - q[b].max(1e-9).ln())
                    .clamp(-(26.0f64.ln()), 26.0f64.ln());
                arm.push(BiasObservation {
                    i: lineup[a],
                    j: lineup[b],
                    log_ratio: m,
                    channels: vec![
                        (SLOT_LETTERS[a].to_string(), 1.0),
                        (SLOT_LETTERS[b].to_string(), -1.0),
                    ],
                });
            }
        }
    }

    println!(
        "Slot-bias gate over {PACK}: beta_p = nats an item reads hot in slot p (mean-centered);"
    );
    println!("rho = Spearman vs the pack's pairwise latents, uncorrected -> slot-corrected.");
    println!("\n--- per (k, attribute) arm ---");
    let mut keys: Vec<_> = arms.keys().cloned().collect();
    keys.sort();
    for key in &keys {
        fit_and_report(
            &format!("k={} {:<18}", key.0, key.1),
            n,
            &arms[key],
            &reference[&key.1],
        );
    }
    println!("\n--- pooled across k per attribute (shared slot channels) ---");
    let mut attrs: Vec<String> = keys.iter().map(|(_, a)| a.clone()).collect();
    attrs.sort();
    attrs.dedup();
    for attr in &attrs {
        let pooled: Vec<BiasObservation> = keys
            .iter()
            .filter(|(_, a)| a == attr)
            .flat_map(|key| arms[key].iter().cloned())
            .collect();
        fit_and_report(&format!("all {attr:<18}"), n, &pooled, &reference[attr]);
    }
}
