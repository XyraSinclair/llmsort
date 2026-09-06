use llmsort::rating_engine::{
    plan_edges_for_rater, AttributeParams, Config, Observation, PlannerMode, RaterParams,
    RatingEngine,
};
use nalgebra::{DMatrix, DVector};
use rand::{rngs::StdRng, Rng, SeedableRng};
use std::{collections::HashMap, time::Instant};

fn main() {
    let n = 1501;
    let mut rng = StdRng::seed_from_u64(20260906);
    let truth: Vec<f64> = (0..n).map(|_| rng.gen_range(-1.0..1.0)).collect();
    let mut observations = Vec::new();
    for i in 1..n {
        for t in 0..6 {
            let j = if t == 0 { i - 1 } else { rng.gen_range(0..i) };
            let mu = truth[i] - truth[j] + rng.gen_range(-0.1..0.1);
            let mut ob = Observation::new(i, j, mu.exp(), 1.0, "r", 1.0);
            ob.precision = Some(rng.gen_range(0.2..3.0));
            observations.push(ob);
        }
    }
    // Exercise the real IRLS loop but keep weights fixed so the independent
    // dense reconstruction is precisely the same weighted normal equation.
    let cfg = Config {
        huber_k: 1e6,
        ..Config::default()
    };
    let mut engine = RatingEngine::new(
        n,
        AttributeParams::default(),
        HashMap::from([("r".into(), RaterParams::default())]),
        Some(cfg.clone()),
    )
    .unwrap();
    engine.ingest(&observations);
    let start = Instant::now();
    let summary = engine.solve();
    let elapsed = start.elapsed().as_secs_f64();
    assert!(!summary.degraded);
    let dim = n - 1;
    let mut a = DMatrix::<f64>::zeros(dim, dim);
    let mut rhs = DVector::<f64>::zeros(dim);
    for edge in &engine.edges {
        let pi = edge.i.checked_sub(1);
        let pj = edge.j.checked_sub(1);
        let w = edge.lam;
        if let Some(i) = pi {
            a[(i, i)] += w;
            rhs[i] += w * edge.mu;
        }
        if let Some(j) = pj {
            a[(j, j)] += w;
            rhs[j] -= w * edge.mu;
        }
        if let (Some(i), Some(j)) = (pi, pj) {
            a[(i, j)] -= w;
            a[(j, i)] -= w;
        }
    }
    for i in 0..dim {
        a[(i, i)] += cfg.ridge_lambda;
    }
    let chol = a.cholesky().unwrap();
    let x = chol.solve(&rhs);
    let mut dense = vec![0.0];
    dense.extend(x.iter().copied());
    let dense_mean = dense.iter().sum::<f64>() / n as f64;
    let cg_mean = summary.scores.iter().sum::<f64>() / n as f64;
    let max_diff = dense
        .iter()
        .zip(&summary.scores)
        .map(|(a, b)| ((a - dense_mean) - (b - cg_mean)).abs())
        .fold(0.0, f64::max);
    println!("kdim={dim} edges={} max_absolute_centered_score_difference={max_diff:.12e} engine_wall_seconds={elapsed:.6}", engine.edges.len());
    assert!(max_diff < 1e-6);
    let mut b = DVector::zeros(dim);
    b[700] = 1.0;
    b[1400] = -1.0;
    let dense_var = b.dot(&chol.solve(&b));
    let cg_var = engine.diff_var_for(701, 1401).unwrap();
    println!(
        "difference_variance_error={:.12e} targeted_marginals={:?} planner_proposals={}",
        (dense_var - cg_var).abs(),
        engine.marginal_vars_for(&[701, 1401]).unwrap(),
        plan_edges_for_rater(&engine, &[(701, 1401)], "r", PlannerMode::Cardinal, true)
            .unwrap()
            .len()
    );
    assert!((dense_var - cg_var).abs() < 1e-6);
}
