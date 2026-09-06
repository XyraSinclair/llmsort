//! Replay E1 (setwise-cached-2026-08-15) through the additive-offset
//! calibration: fit the pivot-halo channel on the real elicited ratios and
//! measure what the correction buys on live evidence.
//!
//! Run: cargo run --locked -p llmsort-experiments --example e1_halo_recalibration
//!
//! The E1 pack's headline pathology: 795/870 elicited ratios < 1 (mean
//! ln r = −0.95 nats) — the model rates almost everything below reference
//! slot A. `bias_calibration` models this as one additive channel: for each
//! member-vs-pivot observation, m = (s_member − s_pivot) − γ_pivot + ε with
//! sign = −1 (the channel favours the pivot side j). Pivot rotation across
//! calls identifies γ separately from any item's score.
//!
//! Both arms here solve through the SAME path (`from_log_ratio_moments`,
//! unit variance) so the only delta is the fitted offset; the original run
//! entered `Observation::new` with stated confidence, so its RESULTS.md ρ
//! table differs slightly from the uncorrected column printed here. The
//! reference latents are the pack's own pairwise arm (report.json), same
//! items / model / seed.

use llmsort::bias_calibration::{solve_with_additive_offsets, BiasObservation};
use llmsort::rating_engine::{AttributeParams, Config, Observation, RaterParams, RatingEngine};
use serde_json::Value;
use std::collections::HashMap;

const PACK: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../research/artifacts/live/setwise-cached-2026-08-15"
);

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

fn kendall_tau_b(x: &[f64], y: &[f64]) -> f64 {
    let n = x.len();
    let (mut concordant, mut discordant, mut tie_x, mut tie_y) = (0i64, 0i64, 0i64, 0i64);
    for a in 0..n {
        for b in a + 1..n {
            let dx = x[a].total_cmp(&x[b]);
            let dy = y[a].total_cmp(&y[b]);
            match (dx.is_eq(), dy.is_eq()) {
                (true, true) => {
                    tie_x += 1;
                    tie_y += 1;
                }
                (true, false) => tie_x += 1,
                (false, true) => tie_y += 1,
                (false, false) => {
                    if dx == dy {
                        concordant += 1;
                    } else {
                        discordant += 1;
                    }
                }
            }
        }
    }
    let n0 = (n * (n - 1) / 2) as i64;
    let denom = (((n0 - tie_x) as f64) * ((n0 - tie_y) as f64)).sqrt();
    assert!(denom > 0.0);
    (concordant - discordant) as f64 / denom
}

fn main() {
    let report: Value = serde_json::from_str(
        &std::fs::read_to_string(format!("{PACK}/report.json")).expect("report.json"),
    )
    .expect("valid report JSON");

    // Entity index space: report.json's entity_ids order.
    let entity_ids: Vec<String> = report["entity_ids"]
        .as_array()
        .expect("entity_ids array")
        .iter()
        .map(|v| v.as_str().expect("entity id").to_owned())
        .collect();
    let n = entity_ids.len();
    let index: HashMap<&str, usize> = entity_ids
        .iter()
        .enumerate()
        .map(|(i, id)| (id.as_str(), i))
        .collect();

    // Pairwise reference latents per attribute, in entity-index order.
    let mut reference: HashMap<String, Vec<f64>> = HashMap::new();
    for arm in report["pairwise"].as_array().expect("pairwise array") {
        let attribute = arm["attribute"].as_str().expect("attribute").to_owned();
        let mut latent = vec![f64::NAN; n];
        for row in arm["latents"].as_array().expect("latents") {
            latent[index[row["id"].as_str().expect("latent id")]] =
                row["mean"].as_f64().expect("latent mean");
        }
        assert!(latent.iter().all(|v| v.is_finite()));
        reference.insert(attribute, latent);
    }

    // Elicited member-vs-pivot observations per (k, attribute) arm.
    let mut arms: HashMap<(u64, String), Vec<BiasObservation>> = HashMap::new();
    let trace = std::fs::read_to_string(format!("{PACK}/trace.jsonl")).expect("trace.jsonl");
    let (mut calls_ok, mut calls_skipped) = (0usize, 0usize);
    for line in trace.lines() {
        let row: Value = serde_json::from_str(line).expect("valid trace row");
        let Some(ratios) = row["parsed_ratios"].as_object() else {
            calls_skipped += 1;
            continue;
        };
        calls_ok += 1;
        let slots: Vec<&str> = row["slot_order_ids"]
            .as_array()
            .expect("slot_order_ids")
            .iter()
            .map(|v| v.as_str().expect("slot id"))
            .collect();
        let pivot = index[slots[0]];
        let arm = arms
            .entry((
                row["k"].as_u64().expect("k"),
                row["attribute"].as_str().expect("attribute").to_owned(),
            ))
            .or_default();
        for (position, member) in slots[1..].iter().enumerate() {
            let letter = char::from(b'B' + u8::try_from(position).expect("slot letter"));
            let r = ratios[&letter.to_string()].as_f64().expect("ratio");
            assert!(r.is_finite() && r > 0.0);
            arm.push(BiasObservation {
                i: index[member],
                j: pivot,
                log_ratio: r.ln(),
                // Rewritten per fit below: one global "pivot" channel, or one
                // channel per slot letter (nanojudge's per-slot shape).
                channel: Some(format!("slot_{letter}")),
                sign: -1.0,
            });
        }
    }
    println!(
        "E1 replay: {calls_ok} parsed calls ({calls_skipped} skipped), n={n} items, \
         channel=pivot (sign −1: halo favours the reference slot)."
    );
    println!("gamma>0 means the pivot slot reads that many nats hotter than it is.");

    let mut keys: Vec<_> = arms.keys().cloned().collect();
    keys.sort();
    for per_slot in [false, true] {
        println!(
            "\n--- {} ---",
            if per_slot {
                "per-slot channels (slot_B/C/D, nanojudge's shape)"
            } else {
                "one global pivot channel"
            }
        );
        println!(" k attribute          obs gamma(s)                it  rms_unc  rms_cor  rho_unc  rho_cor  tau_unc  tau_cor");
        for key in &keys {
            let obs: Vec<BiasObservation> = arms[key]
                .iter()
                .map(|o| BiasObservation {
                    channel: if per_slot {
                        o.channel.clone()
                    } else {
                        Some("pivot".to_owned())
                    },
                    ..o.clone()
                })
                .collect();
            let fit = solve_with_additive_offsets(n, &obs, 1.0).expect("calibrated solve");
            let gammas = fit
                .offsets
                .iter()
                .map(|(_, g)| format!("{g:+.3}"))
                .collect::<Vec<_>>()
                .join(" ");
            let naive = uncorrected_scores(n, &obs);
            let truth = &reference[&key.1];
            println!(
                "{:>2} {:<18} {:>4} {:<23} {:>2} {:>8.3} {:>8.3} {:>8.3} {:>8.3} {:>8.3} {:>8.3}",
                key.0,
                key.1,
                obs.len(),
                gammas,
                fit.iterations,
                fit.rms_residual_uncorrected,
                fit.rms_residual,
                spearman(&naive, truth),
                spearman(&fit.scores, truth),
                kendall_tau_b(&naive, truth),
                kendall_tau_b(&fit.scores, truth),
            );
        }
    }
}
