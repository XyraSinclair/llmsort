use super::diagnostics::probe_seed;
use super::*;

// ---------------------------------------------------------------------
//  Utilities
// ---------------------------------------------------------------------

pub(super) fn median(mut v: Vec<f64>) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
    let n = v.len();
    if n % 2 == 1 {
        v[n / 2]
    } else {
        0.5 * (v[n / 2 - 1] + v[n / 2])
    }
}

pub(super) fn mad(x: &[f64]) -> f64 {
    if x.is_empty() {
        return 0.0;
    }
    let m = median(x.to_vec());
    let devs: Vec<f64> = x.iter().map(|v| (v - m).abs()).collect();
    1.4826 * median(devs)
}

pub(super) fn weighted_median(x: &[f64], w: &[f64]) -> f64 {
    let n = x.len();
    if n == 0 || w.len() != n {
        return 0.0;
    }
    let mut idx: Vec<usize> = (0..n).collect();
    idx.sort_by(|&i, &j| x[i].partial_cmp(&x[j]).unwrap_or(Ordering::Equal));

    let mut x_sorted = Vec::with_capacity(n);
    let mut w_sorted = Vec::with_capacity(n);
    for i in idx {
        x_sorted.push(x[i]);
        w_sorted.push(w[i].max(0.0));
    }

    let w_sum: f64 = w_sorted.iter().sum();
    if w_sum <= 0.0 {
        return median(x_sorted);
    }

    let cutoff = 0.5 * w_sum;
    let mut cum = 0.0;
    for (xi, wi) in x_sorted.iter().zip(w_sorted.iter()) {
        cum += *wi;
        if cum >= cutoff {
            return *xi;
        }
    }
    *x_sorted
        .last()
        .expect("x_sorted is non-empty (caller provides non-empty x)")
}

// ---------------------------------------------------------------------
//  Graph topology and gauge
// ---------------------------------------------------------------------

pub(super) fn compute_components(n: usize, edges: &[Edge]) -> Vec<usize> {
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    for e in edges {
        let i = e.i;
        let j = e.j;
        if i == j || i >= n || j >= n {
            continue;
        }
        adj[i].push(j);
        adj[j].push(i);
    }

    let mut labels = vec![usize::MAX; n];
    let mut comp_id = 0;
    for start in 0..n {
        if labels[start] != usize::MAX {
            continue;
        }
        let mut stack = vec![start];
        labels[start] = comp_id;
        while let Some(u) = stack.pop() {
            for &v in &adj[u] {
                if labels[v] == usize::MAX {
                    labels[v] = comp_id;
                    stack.push(v);
                }
            }
        }
        comp_id += 1;
    }
    labels
}

/// Pin one node per connected component (min index) and return:
/// - keep_idx: non-pinned nodes
/// - labels: component labels for each node
pub(super) fn pin_nodes(n: usize, edges: &[Edge]) -> (Vec<usize>, Vec<usize>) {
    if edges.is_empty() {
        // Match Python behavior: no edges → keep all nodes free, each its own component.
        let labels: Vec<usize> = (0..n).collect();
        let keep_idx: Vec<usize> = (0..n).collect();
        return (keep_idx, labels);
    }

    let labels = compute_components(n, edges);
    let mut keep_mask = vec![true; n];

    let max_label = labels.iter().copied().max().unwrap_or(0);
    for c in 0..=max_label {
        let mut min_node: Option<usize> = None;
        for (node, &lab) in labels.iter().enumerate() {
            if lab == c {
                min_node = Some(match min_node {
                    None => node,
                    Some(m) => m.min(node),
                });
            }
        }
        if let Some(pin) = min_node {
            keep_mask[pin] = false;
        }
    }

    let keep_idx: Vec<usize> = (0..n).filter(|&i| keep_mask[i]).collect();
    (keep_idx, labels)
}

/// Shift scores so that min(score) = 0 for each connected component.
pub(super) fn normalize_per_component(scores: &[f64], labels: &[usize]) -> Vec<f64> {
    let n = scores.len();
    if n == 0 {
        return Vec::new();
    }
    let mut out = scores.to_vec();
    let max_label = labels.iter().copied().max().unwrap_or(0);
    for c in 0..=max_label {
        let mut min_score: Option<f64> = None;
        for (i, &lab) in labels.iter().enumerate() {
            if lab == c {
                let s = out[i];
                min_score = Some(match min_score {
                    None => s,
                    Some(m) => m.min(s),
                });
            }
        }
        if let Some(m) = min_score {
            for (i, &lab) in labels.iter().enumerate() {
                if lab == c {
                    out[i] -= m;
                }
            }
        }
    }
    out
}

pub(super) fn build_pos_map(n: usize, keep_idx: &[usize]) -> Vec<Option<usize>> {
    let mut pos = vec![None; n];
    for (p, &node) in keep_idx.iter().enumerate() {
        if node < n {
            pos[node] = Some(p);
        }
    }
    pos
}

/// Retained factorization or matrix-free system for covariance and planner solves.
#[derive(Debug, Clone)]
pub(super) enum LinearSolver {
    Dense(Cholesky<f64, nalgebra::Dyn>),
    Cg(CgSystem),
}

impl LinearSolver {
    pub(super) fn dim(&self) -> usize {
        match self {
            Self::Dense(chol) => chol.l().nrows(),
            Self::Cg(system) => system.diag.len(),
        }
    }

    pub(super) fn solve(&self, rhs: &DVector<f64>) -> Option<DVector<f64>> {
        match self {
            Self::Dense(chol) => Some(chol.solve(rhs)),
            Self::Cg(system) => {
                let (x, converged) = system.solve(rhs);
                converged.then_some(x)
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct CgSystem {
    // Reduced endpoints retain the dense assembly's position map and edge order.
    edges: Vec<(Option<usize>, Option<usize>, f64)>,
    diag: DVector<f64>,
    ridge: f64,
}

impl CgSystem {
    fn apply(&self, x: &DVector<f64>) -> DVector<f64> {
        let mut out = DVector::zeros(x.len());
        for &(pi, pj, w) in &self.edges {
            let delta = w * (pi.map_or(0.0, |i| x[i]) - pj.map_or(0.0, |j| x[j]));
            if let Some(i) = pi {
                out[i] += delta;
            }
            if let Some(j) = pj {
                out[j] -= delta;
            }
        }
        for i in 0..x.len() {
            out[i] += self.ridge * x[i];
        }
        out
    }

    /// Jacobi-PCG, with a true residual check before accepting convergence.
    /// On breakdown/exhaustion retain the iterate with the smallest residual.
    fn solve(&self, rhs: &DVector<f64>) -> (DVector<f64>, bool) {
        let mut x = DVector::zeros(rhs.len());
        let mut best = x.clone();
        let mut best_norm = rhs.norm();
        if best_norm == 0.0 {
            return (x, true);
        }
        if !best_norm.is_finite() || self.diag.iter().any(|d| !d.is_finite() || *d <= 0.0) {
            return (best, false);
        }
        let tolerance = 1e-10 * best_norm;
        let mut residual = rhs.clone();
        let mut z = residual.component_div(&self.diag);
        let mut direction = z.clone();
        let mut rz = residual.dot(&z);
        for _ in 0..10 * rhs.len() {
            let applied = self.apply(&direction);
            let curvature = direction.dot(&applied);
            if !curvature.is_finite() || curvature <= 0.0 || !rz.is_finite() || rz <= 0.0 {
                break;
            }
            let alpha = rz / curvature;
            x.axpy(alpha, &direction, 1.0);
            residual.axpy(-alpha, &applied, 1.0);
            let mut norm = residual.norm();
            let restart = norm <= tolerance;
            if norm < best_norm || restart {
                let true_residual = rhs - self.apply(&x);
                norm = true_residual.norm();
                if norm < best_norm {
                    best_norm = norm;
                    best = x.clone();
                }
                if norm <= tolerance {
                    return (x, true);
                }
                if restart {
                    residual = true_residual;
                }
            }
            z = residual.component_div(&self.diag);
            let next_rz = residual.dot(&z);
            if restart {
                direction = z.clone();
            } else {
                direction *= next_rz / rz;
                direction += &z;
            }
            rz = next_rz;
        }
        (best, false)
    }
}

pub(super) struct LinearSolveResult {
    s_full: Vec<f64>,
    diag_fallback: Vec<f64>,
    chol: Option<LinearSolver>,
    degraded: bool,
}

// ---------------------------------------------------------------------
//  IRLS with Huber loss
// ---------------------------------------------------------------------

/// Solve (B^T W B) s = B^T W mu in free coordinates (keep_idx).
/// Returns the solution, diagonal of L, and a reusable solver.
pub(super) fn solve_weighted_least_squares(
    n: usize,
    edges: &[Edge],
    mu: &[f64],
    lam_eff: &[f64],
    keep_idx: &[usize],
    cfg: &Config,
) -> LinearSolveResult {
    let m = edges.len();
    let kdim = keep_idx.len();

    if kdim == 0 {
        return LinearSolveResult {
            s_full: vec![0.0; n],
            diag_fallback: Vec::new(),
            chol: None,
            degraded: false,
        };
    }
    if m == 0 {
        return LinearSolveResult {
            s_full: vec![0.0; n],
            diag_fallback: vec![0.0; kdim],
            chol: None,
            degraded: false,
        };
    }

    let pos = build_pos_map(n, keep_idx);

    let build_system = |ridge_lambda: f64| -> (DMatrix<f64>, DVector<f64>, Vec<f64>) {
        let mut l_red = DMatrix::<f64>::zeros(kdim, kdim);
        let mut rhs_red = DVector::<f64>::zeros(kdim);

        for (k, e) in edges.iter().enumerate() {
            let i = e.i;
            let j = e.j;
            let w = lam_eff[k];
            if w <= 0.0 {
                continue;
            }
            let mu_k = mu[k];

            let pi_opt = if i < n { pos[i] } else { None };
            let pj_opt = if j < n { pos[j] } else { None };

            if let Some(pi) = pi_opt {
                l_red[(pi, pi)] += w;
                rhs_red[pi] += w * mu_k;
            }
            if let Some(pj) = pj_opt {
                l_red[(pj, pj)] += w;
                rhs_red[pj] -= w * mu_k;
            }
            if let (Some(pi), Some(pj)) = (pi_opt, pj_opt) {
                l_red[(pi, pj)] -= w;
                l_red[(pj, pi)] -= w;
            }
        }

        if ridge_lambda > 0.0 {
            for d in 0..kdim {
                l_red[(d, d)] += ridge_lambda;
            }
        }

        let mut diag_fallback = Vec::with_capacity(kdim);
        for d in 0..kdim {
            diag_fallback.push(l_red[(d, d)]);
        }

        (l_red, rhs_red, diag_fallback)
    };

    let base_ridge = cfg.ridge_lambda.max(0.0);
    let mut ridge_candidates = Vec::new();
    ridge_candidates.push(base_ridge);

    let mut ridge = if base_ridge > 0.0 { base_ridge } else { 1e-9 };
    for _ in 0..4 {
        ridge *= 10.0;
        ridge_candidates.push(ridge);
    }

    if kdim > DENSE_SOLVE_MAX_DIM {
        let mut system = CgSystem {
            edges: Vec::with_capacity(m),
            diag: DVector::zeros(kdim),
            ridge: 0.0,
        };
        let mut rhs = DVector::zeros(kdim);
        for (k, edge) in edges.iter().enumerate() {
            let w = lam_eff[k];
            if w <= 0.0 {
                continue;
            }
            let pi = pos.get(edge.i).copied().flatten();
            let pj = pos.get(edge.j).copied().flatten();
            system.edges.push((pi, pj, w));
            if let Some(i) = pi {
                system.diag[i] += w;
                rhs[i] += w * mu[k];
            }
            if let Some(j) = pj {
                system.diag[j] += w;
                rhs[j] -= w * mu[k];
            }
            if let (Some(i), Some(j)) = (pi, pj) {
                if i == j {
                    system.diag[i] -= w;
                    system.diag[j] -= w;
                }
            }
        }
        let diagonal = system.diag.clone();
        let mut x = DVector::zeros(kdim);
        let mut converged = false;
        for ridge in ridge_candidates {
            system.ridge = ridge;
            system.diag = diagonal.map(|d| d + ridge);
            (x, converged) = system.solve(&rhs);
            if converged {
                break;
            }
        }
        let mut s_full = vec![0.0; n];
        for (p, &node) in keep_idx.iter().enumerate() {
            s_full[node] = x[p];
        }
        return LinearSolveResult {
            s_full,
            diag_fallback: system.diag.as_slice().to_vec(),
            degraded: !converged || system.ridge > base_ridge + cfg.tiny,
            chol: converged.then_some(LinearSolver::Cg(system)),
        };
    }

    let mut diag_fallback = Vec::new();
    let mut chol: Option<Cholesky<f64, nalgebra::Dyn>> = None;
    let mut x = DVector::<f64>::zeros(kdim);
    let mut used_ridge = base_ridge;
    let want_eig_fallback = kdim <= EXACT_DIAG_MAX_DIM;
    let mut last_l_red: Option<DMatrix<f64>> = None;
    let mut last_rhs_red: Option<DVector<f64>> = None;

    for ridge_lambda in ridge_candidates.iter() {
        let (l_red, rhs_red, diag) = build_system(*ridge_lambda);
        diag_fallback = diag;
        if want_eig_fallback {
            last_l_red = Some(l_red.clone());
            last_rhs_red = Some(rhs_red.clone());
        }
        let attempt = Cholesky::new(l_red);
        if let Some(c) = attempt {
            x = c.solve(&rhs_red);
            chol = Some(c);
            used_ridge = *ridge_lambda;
            break;
        }
        used_ridge = *ridge_lambda;
    }

    if chol.is_none() && want_eig_fallback {
        if let (Some(l_red), Some(rhs_red)) = (last_l_red, last_rhs_red) {
            let eig = SymmetricEigen::new(l_red);
            let mut inv = eig.eigenvalues.clone();
            for i in 0..inv.len() {
                let denom = if inv[i].abs() <= cfg.tiny {
                    cfg.tiny
                } else {
                    inv[i].abs()
                };
                inv[i] = 1.0 / denom;
            }
            let vt_rhs = eig.eigenvectors.transpose() * rhs_red;
            let scaled = inv.component_mul(&vt_rhs);
            x = &eig.eigenvectors * scaled;

            let mut diag_inv = Vec::with_capacity(kdim);
            for i in 0..kdim {
                let mut acc = 0.0;
                for k in 0..kdim {
                    let v = eig.eigenvectors[(i, k)];
                    acc += v * v * inv[k];
                }
                diag_inv.push(acc.max(0.0));
            }
            diag_fallback = diag_inv;
        }
    }

    let mut s_full = vec![0.0; n];
    for (p, &node) in keep_idx.iter().enumerate() {
        s_full[node] = x[p];
    }

    let degraded = chol.is_none() || used_ridge > base_ridge + cfg.tiny;

    LinearSolveResult {
        s_full,
        diag_fallback,
        chol: chol.map(LinearSolver::Dense),
        degraded,
    }
}

/// Hutchinson estimation of diag(L^-1) in reduced coordinates.
/// Reuses either backend; None signals a failed iterative probe.
pub(super) fn hutchinson_diag(
    diag_fallback: &[f64],
    precomputed_chol: Option<&LinearSolver>,
    probes: usize,
    cfg: &Config,
    rng: &mut StdRng,
) -> Option<Vec<f64>> {
    let n = diag_fallback.len();
    if n == 0 {
        return Some(Vec::new());
    }

    let chol = match precomputed_chol {
        Some(c) => c,
        None => {
            // Fallback: diagonal approximation when Cholesky fails (ill-conditioned or
            // weakly connected Laplacian). This is conservative but less accurate.
            let mut diag = Vec::with_capacity(n);
            for &d in diag_fallback {
                let denom = if d.abs() <= cfg.tiny {
                    cfg.tiny
                } else {
                    d.abs()
                };
                diag.push((1.0 / denom).max(0.0));
            }
            return Some(diag);
        }
    };

    if n <= EXACT_DIAG_MAX_DIM {
        let mut diag = Vec::with_capacity(n);
        for i in 0..n {
            let mut e = DVector::<f64>::zeros(n);
            e[i] = 1.0;
            let x = chol.solve(&e)?;
            diag.push(x[i].max(0.0));
        }
        return Some(diag);
    }

    let probes = probes.max(1);
    let mut acc = DVector::<f64>::zeros(n);

    for _ in 0..probes {
        let z = DVector::from_iterator(
            n,
            (0..n).map(|_| if rng.gen_bool(0.5) { 1.0 } else { -1.0 }),
        );
        let x = chol.solve(&z)?;
        acc += z.component_mul(&x);
    }

    let inv_probes = 1.0 / (probes as f64);
    Some((0..n).map(|i| (acc[i] * inv_probes).max(0.0)).collect())
}

/// Robust IRLS loop with Huber loss.
pub(super) fn solve_irls_huber(
    n: usize,
    edges: &[Edge],
    cfg: &Config,
    keep_idx: &[usize],
) -> IrlsHuberSolveResult {
    let m = edges.len();
    if m == 0 {
        return (
            vec![0.0; n],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            None,
            false,
        );
    }

    let mu: Vec<f64> = edges.iter().map(|e| e.mu).collect();
    let lam_raw: Vec<f64> = edges.iter().map(|e| e.lam).collect();
    let mut lam_eff = lam_raw.clone();

    let mut residuals = vec![0.0; m];

    let mut last_obj: Option<f64> = None;
    let mut cg_degraded = false;

    for _ in 0..cfg.irls_max_iters {
        let solve = solve_weighted_least_squares(n, edges, &mu, &lam_eff, keep_idx, cfg);
        cg_degraded |= keep_idx.len() > DENSE_SOLVE_MAX_DIM && solve.degraded;
        let s_candidate = solve.s_full;

        for (k, e) in edges.iter().enumerate() {
            residuals[k] = mu[k] - (s_candidate[e.i] - s_candidate[e.j]);
        }
        if residuals.is_empty() {
            break;
        }

        let scale_mad = mad(&residuals);
        let max_abs_residual = residuals.iter().map(|r| r.abs()).fold(0.0, f64::max);
        // A MAD that is vanishingly small RELATIVE to the residual range is a
        // degenerate scale estimate — it means most residuals are tied up to
        // floating-point noise (e.g. duplicate anchor observations), not that
        // every larger residual is an outlier. Treating such a MAD as a real
        // scale sets delta = huber_k * (fp noise) and Huber-clips every edge,
        // crushing the whole fit toward zero. Fall back to the max-abs scale
        // exactly as for MAD == 0. (Regression:
        // tests/property_solver.rs::huber_mad_scale_collapses_on_near_tied_residuals)
        const MAD_RELATIVE_DEGENERACY_FLOOR: f64 = 1e-8;
        let scale = if scale_mad <= cfg.tiny
            || scale_mad <= MAD_RELATIVE_DEGENERACY_FLOOR * max_abs_residual
        {
            max_abs_residual
        } else {
            scale_mad
        };
        if scale <= cfg.tiny {
            // Perfect fit (no residual spread) — keep weights unchanged.
            break;
        }
        let delta = cfg.huber_k * scale;

        let mut z = vec![1.0; m];
        for k in 0..m {
            let abs_r = residuals[k].abs();
            if abs_r > delta {
                z[k] = delta / (abs_r + cfg.tiny);
            }
        }

        let lam_eff_new: Vec<f64> = lam_raw
            .iter()
            .zip(z.iter())
            .map(|(lr, zz)| lr * zz)
            .collect();

        let obj: f64 = lam_eff_new
            .iter()
            .zip(residuals.iter())
            .map(|(lam, r)| lam * r * r)
            .sum();

        if let Some(prev) = last_obj {
            if (prev - obj).abs() <= cfg.irls_tol * prev.max(1.0) {
                lam_eff = lam_eff_new;
                break;
            }
        }

        lam_eff = lam_eff_new;
        last_obj = Some(obj);
    }

    // Final solve with converged weights - reuse the solver for hutchinson_diag
    let final_solve = solve_weighted_least_squares(n, edges, &mu, &lam_eff, keep_idx, cfg);
    let s_full = final_solve.s_full;

    for (k, e) in edges.iter().enumerate() {
        residuals[k] = mu[k] - (s_full[e.i] - s_full[e.j]);
    }

    let seed = probe_seed(cfg.rng_seed, edges, &lam_eff);
    let mut probe_rng = StdRng::seed_from_u64(seed);
    let diag_red = hutchinson_diag(
        &final_solve.diag_fallback,
        final_solve.chol.as_ref(),
        cfg.hutch_probes,
        cfg,
        &mut probe_rng,
    );

    let degraded = cg_degraded || final_solve.degraded || diag_red.is_none();
    let diag_red = diag_red.unwrap_or_else(|| {
        hutchinson_diag(
            &final_solve.diag_fallback,
            None,
            cfg.hutch_probes,
            cfg,
            &mut probe_rng,
        )
        .expect("diagonal approximation requires no iterative solve")
    });

    (
        s_full,
        residuals,
        lam_eff,
        diag_red,
        final_solve.chol,
        degraded,
    )
}
