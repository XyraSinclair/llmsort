//! Offline prototype: conditional rank slots → stick breaking → Luce edges.
//! Run: cargo run --locked -p llmsort-experiments --example stickbreak_kwise
//! Optional flags: --n 24 --rounds 3 --temperature 1.0.
//! sigma=0 rows are the ideal-PMF limit (exact PL conditionals reconstruct the
//! first-slot distribution): ideal information + graph coverage. sigma>0 rows
//! blur every score the judge reads by N(0, sigma) per call — the SAME
//! perceptual noise for both arms — measuring robustness of the fold and the
//! df weights under imperfect PMFs. Edge counts include repeated pairs.

use clap::Parser;
use llmsort::rating_engine::{AttributeParams, Config, Observation, RaterParams, RatingEngine};
use rand::{distributions::WeightedIndex, prelude::*};
use std::collections::HashMap;

#[derive(Parser)]
struct Args {
    #[arg(long, default_value_t = 24)]
    n: usize,
    #[arg(long, default_value_t = 3)]
    rounds: usize,
    #[arg(long, default_value_t = 1.0)]
    temperature: f64,
}

struct Slot {
    winner: usize, // lineup-local index
    pmf: Vec<f64>, // full conditional distribution; placed members have zero mass
}

/// One Box–Muller standard-normal draw.
fn gauss(rng: &mut StdRng) -> f64 {
    (-2.0 * (1.0 - rng.gen::<f64>()).ln()).sqrt() * (std::f64::consts::TAU * rng.gen::<f64>()).cos()
}

/// The judge's per-call perception: true scores blurred by N(0, sigma) per
/// item read. Both arms use this same perceptual model, so the noise
/// semantics are identical across instruments.
fn perceive(truth: &[f64], sigma: f64, rng: &mut StdRng) -> Vec<f64> {
    truth.iter().map(|&s| s + sigma * gauss(rng)).collect()
}

fn judge(lineup: &[usize], truth: &[f64], temperature: f64, rng: &mut StdRng) -> Vec<Slot> {
    let mut unplaced = vec![true; lineup.len()];
    (0..lineup.len() - 1)
        .map(|_| {
            let max = lineup
                .iter()
                .enumerate()
                .filter(|(i, _)| unplaced[*i])
                .map(|(_, &item)| truth[item])
                .fold(f64::NEG_INFINITY, f64::max);
            let mut pmf: Vec<f64> = lineup
                .iter()
                .enumerate()
                .map(|(i, &item)| {
                    if unplaced[i] {
                        ((truth[item] - max) / temperature).exp()
                    } else {
                        0.0
                    }
                })
                .collect();
            let mass: f64 = pmf.iter().sum();
            pmf.iter_mut().for_each(|p| *p /= mass);
            let winner = WeightedIndex::new(&pmf)
                .expect("valid conditional PMF")
                .sample(rng);
            unplaced[winner] = false;
            Slot { winner, pmf }
        })
        .collect()
}

fn reconstruct(k: usize, slots: &[Slot]) -> Vec<f64> {
    assert_eq!(slots.len(), k - 1);
    let mut unplaced = vec![true; k];
    let mut q = vec![0.0; k];
    let mut residual = 1.0;
    for slot in slots {
        assert!(unplaced[slot.winner]);
        let mass: f64 = slot
            .pmf
            .iter()
            .enumerate()
            .filter(|(i, _)| unplaced[*i])
            .map(|(_, p)| p)
            .sum();
        assert!(mass.is_finite() && mass > 0.0);
        q[slot.winner] = residual * slot.pmf[slot.winner] / mass;
        residual -= q[slot.winner];
        unplaced[slot.winner] = false;
    }
    q[unplaced.iter().position(|&u| u).expect("last member")] = residual;
    assert!(q.iter().all(|p| p.is_finite() && *p >= 0.0));
    assert!((q.iter().sum::<f64>() - 1.0).abs() < 1e-12);
    q
}

fn fold(lineup: &[usize], q: &[f64]) -> Vec<Observation> {
    let k = lineup.len();
    let pairs: Vec<_> = (0..k)
        .flat_map(|a| (a + 1..k).map(move |b| (a, b)))
        .filter(|&(a, b)| q[a] + q[b] > 0.0)
        .collect();
    let variance = pairs.len() as f64 / (k - 1) as f64;
    pairs
        .into_iter()
        .map(|(a, b)| {
            // Luce P(a > b) = q[a] / (q[a] + q[b]); its logit is this log ratio.
            Observation::from_log_ratio_moments(
                lineup[a],
                lineup[b],
                q[a].max(1e-9).ln() - q[b].max(1e-9).ln(),
                variance,
                "judge",
                1.0,
            )
        })
        .collect()
}

fn solve(n: usize, observations: &[Observation]) -> Vec<f64> {
    let raters = HashMap::from([("judge".to_owned(), RaterParams::default())]);
    let mut engine = RatingEngine::new(
        n,
        AttributeParams::default(),
        raters,
        Some(Config::default()),
    )
    .expect("valid engine");
    engine.ingest(observations);
    engine.solve().scores
}

fn ranks(values: &[f64]) -> Vec<f64> {
    assert!(values.iter().all(|v| v.is_finite()));
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

fn rho(scores: &[f64], truth: &[f64]) -> f64 {
    let center = (truth.len() - 1) as f64 / 2.0;
    let (mut xy, mut xx, mut yy) = (0.0, 0.0, 0.0);
    for (x, y) in ranks(scores).into_iter().zip(ranks(truth)) {
        let (x, y) = (x - center, y - center);
        xy += x * y;
        xx += x * x;
        yy += y * y;
    }
    assert!(xx > 0.0 && yy > 0.0, "Spearman requires nonconstant scores");
    xy / (xx * yy).sqrt()
}

fn main() {
    let args = Args::parse();
    assert!((7..=5000).contains(&args.n) && args.rounds > 0);
    assert!(args.temperature.is_finite() && args.temperature > 0.0);
    println!(
        "Synthetic PL judge: n={}, rounds={}, T={}, seeds=42..46; edges count observations.",
        args.n, args.rounds, args.temperature
    );
    println!("sigma = per-call perceptual noise std on each read score (0 = ideal PMFs).");
    println!("sigma  k calls edges stickbreak_rho pairwise_rho");
    for sigma in [0.0, 0.5, 1.0] {
        for k in [3, 5, 7] {
            let calls = args.rounds * args.n.div_ceil(k - 1);
            let (mut stick_rho, mut pair_rho) = (0.0, 0.0);
            let mut edge_count = 0;
            for seed in 42..47 {
                let mut rng = StdRng::seed_from_u64(seed);
                // Box–Muller: Normal(0, variance 1.5), not standard deviation 1.5.
                let truth: Vec<f64> = (0..args.n)
                    .map(|_| {
                        (-2.0 * (1.0 - rng.gen::<f64>()).ln()).sqrt()
                            * (std::f64::consts::TAU * rng.gen::<f64>()).cos()
                            * 1.5_f64.sqrt()
                    })
                    .collect();
                let mut observations = Vec::new();
                let mut order: Vec<_> = (0..args.n).collect();
                for _ in 0..args.rounds {
                    order.shuffle(&mut rng);
                    for start in (0..args.n).step_by(k - 1) {
                        let lineup: Vec<_> = (0..k).map(|j| order[(start + j) % args.n]).collect();
                        let seen = perceive(&truth, sigma, &mut rng);
                        let slots = judge(&lineup, &seen, args.temperature, &mut rng);
                        observations.extend(fold(&lineup, &reconstruct(k, &slots)));
                    }
                }
                edge_count = observations.len();
                stick_rho += rho(&solve(args.n, &observations), &truth);
                // Separate deterministic stream: baseline pairs do not depend on slot draws.
                let mut pair_rng = StdRng::seed_from_u64(seed ^ 0x7061_6972);
                let baseline: Vec<_> = (0..calls)
                    .map(|_| {
                        let a = pair_rng.gen_range(0..args.n);
                        let b = (a + pair_rng.gen_range(1..args.n)) % args.n;
                        let seen = perceive(&truth, sigma, &mut pair_rng);
                        let p = 1.0 / (1.0 + ((seen[b] - seen[a]) / args.temperature).exp());
                        let log_odds = p.ln() - (-p).ln_1p();
                        Observation::from_log_ratio_moments(a, b, log_odds, 1.0, "judge", 1.0)
                    })
                    .collect();
                pair_rho += rho(&solve(args.n, &baseline), &truth);
            }
            println!(
                "{sigma:>5.1} {k:>2} {calls:>5} {edge_count:>5} {:>14.6} {:>12.6}",
                stick_rho / 5.0,
                pair_rho / 5.0
            );
        }
    }
}
