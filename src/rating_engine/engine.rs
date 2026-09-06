use super::diagnostics::{compute_hcr, compute_loo, compute_pcr_lite};
use super::math::{build_pos_map, normalize_per_component, pin_nodes, solve_irls_huber};
use super::ranking::{
    compute_calibration_evidence, compute_rank_stability, pair_prob_and_flip, pair_rank_weight,
    RankCache,
};
use super::*;

// ---------------------------------------------------------------------
//  Engine
// ---------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct RatingEngine {
    pub n: usize,
    pub attr: AttributeParams,
    pub raters: HashMap<String, RaterParams>,
    pub cfg: Config,

    pub edges: Vec<Edge>,
    edge_index: HashMap<(usize, usize), usize>,

    // Cached topology
    keep_idx: Vec<usize>,
    labels: Vec<usize>,
    topology_dirty: bool,

    // Cached solve results
    last_scores: Option<Vec<f64>>,
    last_diag_cov: Option<Vec<f64>>,
    last_residuals: Option<Vec<f64>>,
    last_lam_eff: Option<Vec<f64>>,
    last_chol: Option<math::LinearSolver>,
}

impl RatingEngine {
    pub fn new(
        n: usize,
        attr: AttributeParams,
        raters: HashMap<String, RaterParams>,
        cfg: Option<Config>,
    ) -> Result<Self, &'static str> {
        if n == 0 {
            return Err("Item count must be positive");
        }
        if n > MAX_ITEMS {
            return Err("Item count exceeds maximum allowed (5,000)");
        }
        let cfg = cfg.unwrap_or_default();
        Ok(Self {
            n,
            attr,
            raters,
            cfg,
            edges: Vec::new(),
            edge_index: HashMap::new(),
            keep_idx: (0..n).collect(),
            labels: (0..n).collect(),
            topology_dirty: true,
            last_scores: None,
            last_diag_cov: None,
            last_residuals: None,
            last_lam_eff: None,
            last_chol: None,
        })
    }

    /// Snapshot the complete constructor input that determines solver policy.
    #[must_use]
    pub fn spec(&self) -> EngineSpec {
        let mut raters: Vec<_> = self
            .raters
            .iter()
            .map(|(id, params)| (id.clone(), params.clone()))
            .collect();
        raters.sort_by(|left, right| left.0.cmp(&right.0));
        EngineSpec {
            n: self.n,
            attribute: self.attr.clone(),
            raters,
            config: self.cfg.clone(),
        }
    }

    pub fn scores(&self) -> Option<&[f64]> {
        self.last_scores.as_deref()
    }

    pub fn diag_cov(&self) -> Option<&[f64]> {
        self.last_diag_cov.as_deref()
    }

    /// Targeted marginal variances for a subset of indices (same order as input).
    /// Uses the current reduced Laplacian solve state when available.
    pub fn marginal_vars_for(&self, indices: &[usize]) -> Option<Vec<f64>> {
        if indices.is_empty() {
            return Some(Vec::new());
        }

        let chol = self.last_chol.as_ref()?;
        let diag_cov = self.last_diag_cov.as_ref()?;
        let pos = build_pos_map(self.n, &self.keep_idx);

        let rdim = chol.dim();
        let mut out = Vec::with_capacity(indices.len());
        for &idx in indices {
            let p = match pos.get(idx).copied().flatten() {
                Some(v) => v,
                None => {
                    // Entity not in the reduced system (zero observations).
                    // Use the diag_cov fallback which rating_engine already set
                    // to the component-max for isolated entities.
                    let fallback = if idx < diag_cov.len() {
                        diag_cov[idx].max(0.0)
                    } else {
                        0.0
                    };
                    out.push(fallback);
                    continue;
                }
            };
            if p >= rdim {
                let fallback = if idx < diag_cov.len() {
                    diag_cov[idx].max(0.0)
                } else {
                    0.0
                };
                out.push(fallback);
                continue;
            }
            let mut b = DVector::<f64>::zeros(rdim);
            b[p] = 1.0;
            let x = chol.solve(&b)?;
            out.push(x[p].max(0.0));
        }

        Some(out)
    }

    /// Variance of score difference s_i - s_j using the reduced Laplacian.
    pub fn diff_var_for(&self, i: usize, j: usize) -> Option<f64> {
        let chol = self.last_chol.as_ref()?;
        let diag_cov = self.last_diag_cov.as_ref()?;
        if i >= self.labels.len() || j >= self.labels.len() {
            return None;
        }
        if self.labels[i] != self.labels[j] {
            return Some((diag_cov[i] + diag_cov[j]).max(0.0));
        }
        let pos = build_pos_map(self.n, &self.keep_idx);
        let pi = pos.get(i).copied().flatten();
        let pj = pos.get(j).copied().flatten();

        let rdim = chol.dim();
        if pi.is_none() && pj.is_none() {
            return Some((diag_cov[i] + diag_cov[j]).max(0.0));
        }
        if let Some(pi) = pi {
            if pi >= rdim {
                return Some((diag_cov[i] + diag_cov[j]).max(0.0));
            }
        }
        if let Some(pj) = pj {
            if pj >= rdim {
                return Some((diag_cov[i] + diag_cov[j]).max(0.0));
            }
        }
        let mut b = DVector::<f64>::zeros(rdim);
        if let Some(pi) = pi {
            b[pi] = 1.0;
        }
        if let Some(pj) = pj {
            b[pj] = -1.0;
        }
        let x = chol.solve(&b)?;
        Some(b.dot(&x).max(0.0))
    }

    /// Whether two nodes are in the same connected component (based on current edges).
    pub fn same_component(&self, i: usize, j: usize) -> bool {
        if self.topology_dirty {
            return false;
        }
        if i >= self.labels.len() || j >= self.labels.len() {
            return false;
        }
        self.labels[i] == self.labels[j]
    }

    pub fn has_min_degree(&self, idx: usize, min_degree: usize) -> bool {
        if min_degree == 0 {
            return true;
        }
        let mut deg = 0usize;
        for edge in &self.edges {
            if edge.i == idx || edge.j == idx {
                deg += 1;
                if deg >= min_degree {
                    return true;
                }
            }
        }
        false
    }

    fn mark_dirty_after_edges_change(&mut self, topology_changed: bool) {
        if topology_changed {
            self.topology_dirty = true;
        }
        self.last_scores = None;
        self.last_diag_cov = None;
        self.last_residuals = None;
        self.last_lam_eff = None;
        self.last_chol = None;
    }

    fn fuse_bulk(&mut self, observations: &[Observation]) {
        let t = self.attr.temperature.max(self.cfg.tiny);
        let mut buckets: FuseBuckets = FuseBuckets::new();

        for ob in observations {
            let i = ob.i;
            let j = ob.j;
            if i == j {
                continue;
            }
            if i >= self.n || j >= self.n {
                continue;
            }

            let (u, v, sign) = if i < j { (i, j, 1.0) } else { (j, i, -1.0) };

            if !ob.ratio.is_finite() || ob.ratio <= 0.0 {
                continue;
            }
            let ratio = ob.ratio.max(self.cfg.tiny);
            let mut log_r = sign * ratio.ln();
            let max_log = self.cfg.max_log_ratio;
            log_r = log_r.clamp(-max_log, max_log);

            let beta_r = match self.raters.get(&ob.rater_id) {
                Some(r) => r.beta.max(self.cfg.tiny),
                None => continue, // skip unknown raters
            };
            let reps = ob.reps.clamp(0.0, MAX_REPS);

            // Explicit measured precision takes precedence. Point observations
            // receive unit precision: stated confidence is not calibrated.
            let per_judgement_weight = match ob.precision {
                Some(p) if p.is_finite() && p > 0.0 => p,
                Some(_) => continue,
                None => 1.0,
            };
            let lam = (beta_r * per_judgement_weight * reps) / t;
            if !lam.is_finite() || lam <= 0.0 {
                continue;
            }

            buckets
                .entry((u, v))
                .or_default()
                .push((log_r, lam, ob.rater_id.clone()));
        }

        let mut edges = Vec::new();
        let mut edge_index = HashMap::new();

        for ((i, j), lst) in buckets.into_iter() {
            if lst.is_empty() {
                continue;
            }
            let mut num = 0.0;
            let mut lam_total = 0.0;
            let mut contribs: HashMap<String, f64> = HashMap::new();
            for (mu_obs, lam, rid) in lst.into_iter() {
                lam_total += lam;
                num += mu_obs * lam;
                *contribs.entry(rid).or_insert(0.0) += lam;
            }
            if lam_total <= 0.0 {
                continue;
            }
            let mu = num / lam_total;
            let idx = edges.len();
            edges.push(Edge {
                i,
                j,
                mu,
                lam: lam_total,
                contributors: contribs,
            });
            edge_index.insert((i, j), idx);
        }

        self.edges = edges;
        self.edge_index = edge_index;
    }

    /// Replace edge set by fusing all observations (bulk ingest/reset).
    /// Use `add_observations` for incremental updates.
    pub fn ingest(&mut self, observations: &[Observation]) {
        self.fuse_bulk(observations);
        self.mark_dirty_after_edges_change(true);
    }

    pub fn add_observations(&mut self, observations: &[Observation]) {
        let t = self.attr.temperature.max(self.cfg.tiny);
        let mut new_edge_added = false;

        for ob in observations {
            let i = ob.i;
            let j = ob.j;
            if i == j {
                continue;
            }
            if i >= self.n || j >= self.n {
                continue;
            }

            let (u, v, sign) = if i < j { (i, j, 1.0) } else { (j, i, -1.0) };

            if !ob.ratio.is_finite() || ob.ratio <= 0.0 {
                continue;
            }
            let ratio = ob.ratio.max(self.cfg.tiny);
            let mut log_r = sign * ratio.ln();
            let max_log = self.cfg.max_log_ratio;
            log_r = log_r.clamp(-max_log, max_log);

            let beta_r = match self.raters.get(&ob.rater_id) {
                Some(r) => r.beta.max(self.cfg.tiny),
                None => continue, // skip unknown raters
            };
            let reps = ob.reps.clamp(0.0, MAX_REPS);
            // Explicit measured precision takes precedence. Point observations
            // receive unit precision: stated confidence is not calibrated.
            let per_judgement_weight = match ob.precision {
                Some(p) if p.is_finite() && p > 0.0 => p,
                Some(_) => continue,
                None => 1.0,
            };
            let lam_new = (beta_r * per_judgement_weight * reps) / t;
            if !lam_new.is_finite() || lam_new <= 0.0 {
                continue;
            }

            let key = (u, v);
            if let Some(idx) = self.edge_index.get(&key).copied() {
                let e = &mut self.edges[idx];
                let lam_prev = e.lam;
                let lam_tot = lam_prev + lam_new;
                if lam_tot <= 0.0 {
                    continue;
                }
                let mu_prev = e.mu;
                let mu_new = (mu_prev * lam_prev + log_r * lam_new) / lam_tot;
                e.mu = mu_new;
                e.lam = lam_tot;
                *e.contributors.entry(ob.rater_id.clone()).or_insert(0.0) += lam_new;
            } else {
                let mut contributors = HashMap::new();
                contributors.insert(ob.rater_id.clone(), lam_new);
                let e = Edge {
                    i: u,
                    j: v,
                    mu: log_r,
                    lam: lam_new,
                    contributors,
                };
                self.edges.push(e);
                let idx = self.edges.len() - 1;
                self.edge_index.insert(key, idx);
                new_edge_added = true;
            }
        }

        self.mark_dirty_after_edges_change(new_edge_added);
    }

    fn ensure_topology(&mut self) {
        if self.topology_dirty {
            let (keep_idx, labels) = pin_nodes(self.n, &self.edges);
            self.keep_idx = keep_idx;
            self.labels = labels;
            self.topology_dirty = false;
        }
    }

    pub fn solve(&mut self) -> SolveSummary {
        self.ensure_topology();
        let keep_idx = self.keep_idx.clone();

        let (s, residuals, lam_eff, diag_red, chol, degraded) =
            solve_irls_huber(self.n, &self.edges, &self.cfg, &keep_idx);

        let mut diag_cov = vec![0.0; self.n];
        if !diag_red.is_empty() && diag_red.len() == keep_idx.len() {
            for (pos, &node) in keep_idx.iter().enumerate() {
                diag_cov[node] = diag_red[pos].max(0.0);
            }
        }
        if self.edges.is_empty() {
            diag_cov.fill(1.0);
        }
        if !self.edges.is_empty() {
            let mut keep_mask = vec![false; self.n];
            for &node in &keep_idx {
                if node < self.n {
                    keep_mask[node] = true;
                }
            }
            let components = if self.labels.is_empty() {
                self.n
            } else {
                self.labels.iter().copied().max().unwrap_or(0) + 1
            };
            let mut comp_max: Vec<f64> = vec![0.0; components];
            for i in 0..self.n {
                if !keep_mask[i] {
                    continue;
                }
                if i >= self.labels.len() {
                    continue;
                }
                let c = self.labels[i];
                let v = diag_cov[i];
                if v.is_finite() {
                    comp_max[c] = comp_max[c].max(v);
                }
            }
            let mut global_max = comp_max.iter().copied().fold(0.0, f64::max);
            if global_max <= 0.0 {
                global_max = 1.0;
            }
            for i in 0..self.n {
                if keep_mask[i] {
                    continue;
                }
                if i >= self.labels.len() {
                    continue;
                }
                let c = self.labels[i];
                let fallback = if c < comp_max.len() && comp_max[c] > 0.0 {
                    comp_max[c]
                } else {
                    global_max
                };
                diag_cov[i] = fallback;
            }
        }

        let mu: Vec<f64> = self.edges.iter().map(|e| e.mu).collect();
        let scores_norm = normalize_per_component(&s, &self.labels);

        let hcr = compute_hcr(&mu, &residuals, &lam_eff, &self.cfg);
        let endpoints: Vec<(usize, usize)> = self.edges.iter().map(|e| (e.i, e.j)).collect();
        let hodge = compute_hodge_split(&endpoints, &mu, &residuals, &lam_eff, self.n, &self.cfg);
        let spectral = spectral_diagnostics(&endpoints, &lam_eff, self.n, EXACT_DIAG_MAX_DIM);
        let lam_raw: Vec<f64> = self.edges.iter().map(|e| e.lam).collect();
        let loo = spectral
            .as_ref()
            .map(|s| compute_loo(&residuals, &lam_raw, &s.edge_leverage));
        let pcr = compute_pcr_lite(&mu, &residuals, &lam_eff, &self.cfg);

        let (expected_rev, max_flip, rank_risk) =
            compute_rank_stability(&scores_norm, &diag_cov, &self.cfg);

        let cal_evidence =
            compute_calibration_evidence(&residuals, &self.edges, &lam_eff, &self.cfg);

        let m = self.edges.len();
        let components = if self.labels.is_empty() {
            self.n
        } else {
            self.labels.iter().copied().max().unwrap_or(0) + 1
        };
        let cycle_dim = (m as isize - self.n as isize + components as isize).max(0) as usize;

        let total_info: f64 = lam_eff.iter().sum();

        self.last_scores = Some(scores_norm.clone());
        self.last_diag_cov = Some(diag_cov.clone());
        self.last_residuals = Some(residuals.clone());
        self.last_lam_eff = Some(lam_eff.clone());
        self.last_chol = chol;

        SolveSummary {
            scores: scores_norm,
            residuals,
            diag_cov,
            hcr,
            hodge,
            spectral,
            loo,
            pcr,
            total_info,
            expected_rank_reversals: expected_rev,
            max_pair_reversal_prob: max_flip,
            rank_risk,
            components,
            cycle_dim,
            calibration_evidence: cal_evidence,
            degraded,
        }
    }

    pub fn pair_probability(&self, i: usize, j: usize) -> Result<(f64, f64), &'static str> {
        match (&self.last_scores, &self.last_diag_cov) {
            (Some(scores), Some(diag_cov)) => {
                Ok(pair_prob_and_flip(scores, diag_cov, i, j, &self.cfg))
            }
            _ => Err("No solve() results available"),
        }
    }

    pub fn rank_stability(&self) -> Result<(f64, f64, f64), &'static str> {
        match (&self.last_scores, &self.last_diag_cov) {
            (Some(scores), Some(diag_cov)) => {
                Ok(compute_rank_stability(scores, diag_cov, &self.cfg))
            }
            _ => Err("No solve() results available"),
        }
    }
}

// ---------------------------------------------------------------------
//  Planner
// ---------------------------------------------------------------------

/// Compute effective resistance using precomputed Cholesky and position map.
/// Avoids O(N³) Cholesky per pair when called in a loop.
fn effective_resistance_with_chol(
    diag_cov: &[f64],
    labels: &[usize],
    i: usize,
    j: usize,
    chol: &math::LinearSolver,
    pos: &[Option<usize>],
) -> Option<f64> {
    if labels[i] != labels[j] {
        return Some((diag_cov[i] + diag_cov[j]).max(0.0));
    }

    let pi = pos.get(i).copied().flatten();
    let pj = pos.get(j).copied().flatten();

    if pi.is_none() && pj.is_none() {
        return Some((diag_cov[i] + diag_cov[j]).max(0.0));
    }

    let rdim = chol.dim();
    let mut b = DVector::<f64>::zeros(rdim);
    if let Some(p) = pi {
        b[p] += 1.0;
    }
    if let Some(p) = pj {
        b[p] -= 1.0;
    }

    let x = chol.solve(&b)?;
    let r = b.dot(&x);
    Some(r.max(0.0))
}

#[derive(Debug, Clone, Copy)]
pub enum PlannerMode {
    Cardinal,
    Ordinal,
    Hybrid,
}

/// Windowed candidate generation for the planner: each item paired with its
/// `window` upward neighbours in the current score order, so the candidate
/// set is O(n·window) instead of O(n²) and planner spend concentrates where
/// rank flips are live. This is matchmaking policy, deliberately OUTSIDE the
/// engine's content identity: `plan_edges_for_rater` already takes its
/// candidates from the caller, so no Config field and no `EngineSpec`
/// identity churn (provenance: nanojudge's OPPONENT_WINDOW_SIZE windowed
/// pairing, mined 2026-09-05 — `research/notes/nanojudge-mining-2026-09-05/`;
/// their window rides the matchmaker, not the likelihood, and so does ours).
///
/// Each unordered pair appears once. `window >= n−1` reproduces the full
/// candidate set. Errors before `solve()` (no score order to window over)
/// and on `window == 0` (an empty plan is a caller bug, not a plan).
pub fn windowed_candidates(
    engine: &RatingEngine,
    window: usize,
) -> Result<Vec<(usize, usize)>, &'static str> {
    if window == 0 {
        return Err("window must be at least 1");
    }
    let scores = match &engine.last_scores {
        Some(s) => s,
        None => return Err("Engine has no solve() state; call solve() first."),
    };
    let mut order: Vec<usize> = (0..engine.n).collect();
    order.sort_by(|&a, &b| scores[a].total_cmp(&scores[b]).then(a.cmp(&b)));
    let mut out = Vec::new();
    for (pos, &i) in order.iter().enumerate() {
        for &j in order[pos + 1..].iter().take(window) {
            out.push((i, j));
        }
    }
    Ok(out)
}

/// Plans which entity pairs to compare next for the given rater.
///
/// PlannerMode controls optimization objective: Cardinal maximizes information gain,
/// Ordinal minimizes rank uncertainty, Hybrid blends both (weighted by `lambda_risk`).
/// When `use_effective_resistance` is true, uses full graph effective resistance (slower, more accurate);
/// otherwise uses diagonal covariance approximation (faster).
/// Returns proposals sorted by `score` (descending): utility-per-cost of observing each pair.
/// Candidates spanning disconnected components are handled via component-aware fallbacks
/// (effective resistance reduces to diagonal covariance across components).
pub fn plan_edges_for_rater(
    engine: &RatingEngine,
    candidates: &[(usize, usize)],
    rater_id: &str,
    mode: PlannerMode,
    use_effective_resistance: bool,
) -> Result<Vec<PlanProposal>, &'static str> {
    if candidates.len() > MAX_CANDIDATES {
        return Err("Candidate count exceeds maximum allowed (50,000)");
    }

    let scores = match &engine.last_scores {
        Some(s) => s,
        None => return Err("Engine has no solve() state; call solve() first."),
    };
    let diag_cov = match &engine.last_diag_cov {
        Some(c) => c,
        None => return Err("Engine has no solve() state; call solve() first."),
    };

    let cfg = &engine.cfg;
    let n = engine.n;

    let r = match engine.raters.get(rater_id) {
        Some(r) => r,
        None => return Err("Unknown rater_id"),
    };

    let beta_r = r.beta.max(cfg.tiny);
    let cost = r.cost_per_edge.max(cfg.tiny);
    let t = engine.attr.temperature.max(cfg.tiny);

    let lam_new = beta_r / t;

    // Pre-compute Cholesky and position map once (O(N³)), not per-candidate.
    let mut er_pos: Option<Vec<Option<usize>>> = None;
    let mut er_chol: Option<&math::LinearSolver> = None;
    if use_effective_resistance
        && engine.last_chol.is_some()
        && engine.last_diag_cov.is_some()
        && !engine.keep_idx.is_empty()
    {
        er_pos = Some(build_pos_map(engine.n, &engine.keep_idx));
        er_chol = engine.last_chol.as_ref();
    }

    let mut proposals = Vec::new();
    let mut cache = RankCache::default();

    for &(i_raw, j_raw) in candidates {
        let i = i_raw;
        let j = j_raw;
        if i == j || i >= n || j >= n {
            continue;
        }

        let r_ij = if let (Some(chol), Some(pos)) = (er_chol, er_pos.as_ref()) {
            effective_resistance_with_chol(diag_cov, &engine.labels, i, j, chol, pos)
                .ok_or("Effective resistance solve did not converge")?
        } else {
            (diag_cov[i] + diag_cov[j]).max(0.0)
        };

        let delta_info = 0.5 * (1.0 + lam_new * r_ij).ln();

        let diff = scores[i] - scores[j];
        let var_before = r_ij.max(cfg.tiny);

        let z_before = diff / var_before.sqrt();
        let p_gt_before = normal_cdf(z_before);
        let mut p_flip_before = if diff < 0.0 {
            p_gt_before
        } else {
            1.0 - p_gt_before
        };
        if diff == 0.0 {
            p_flip_before = 0.5;
        }
        p_flip_before = p_flip_before.clamp(0.0, 1.0);

        let var_after = var_before / (1.0 + lam_new * var_before).max(cfg.tiny);
        let z_after = diff / var_after.max(cfg.tiny).sqrt();
        let p_gt_after = normal_cdf(z_after);
        let mut p_flip_after = if diff < 0.0 {
            p_gt_after
        } else {
            1.0 - p_gt_after
        };
        if diff == 0.0 {
            p_flip_after = 0.5;
        }
        p_flip_after = p_flip_after.clamp(0.0, 1.0);

        let w_ij = pair_rank_weight(scores, i, j, cfg, &mut cache);
        let delta_rank_risk = w_ij * (p_flip_before - p_flip_after);

        let score = match mode {
            PlannerMode::Cardinal => delta_info / cost,
            PlannerMode::Ordinal => delta_rank_risk / cost,
            PlannerMode::Hybrid => (delta_info + cfg.lambda_risk * delta_rank_risk) / cost,
        };

        proposals.push(PlanProposal {
            i,
            j,
            score,
            delta_info,
            delta_rank_risk,
            cost,
        });
    }

    proposals.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| {
                let min_a = a.i.min(a.j);
                let min_b = b.i.min(b.j);
                min_a.cmp(&min_b)
            })
            .then_with(|| {
                let max_a = a.i.max(a.j);
                let max_b = b.i.max(b.j);
                max_a.cmp(&max_b)
            })
    });

    Ok(proposals)
}
