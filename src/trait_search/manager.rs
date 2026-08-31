use super::*;

impl TraitSearchManager {
    pub fn new(
        config: TraitSearchConfig,
        mut engines: HashMap<String, RatingEngine>,
    ) -> Result<Self> {
        if config.attributes.is_empty() {
            return Err(TraitSearchError::EmptyAttributes);
        }
        if config.n_entities == 0 {
            return Err(TraitSearchError::NonPositiveEntities);
        }

        let mut engine_map: HashMap<String, RatingEngine> = HashMap::new();
        let mut n_opt: Option<usize> = None;

        for attr in &config.attributes {
            let id = &attr.id;
            let engine = engines
                .remove(id)
                .ok_or_else(|| TraitSearchError::MissingEngine {
                    attribute_id: id.clone(),
                })?;

            let engine_n = engine.n;
            if let Some(n0) = n_opt {
                if engine_n != n0 {
                    return Err(TraitSearchError::EnginesSizeMismatch);
                }
            } else {
                n_opt = Some(engine_n);
            }
            engine_map.insert(id.clone(), engine);
        }

        let n = n_opt.ok_or(TraitSearchError::EnginesSizeMismatch)?;
        if n != config.n_entities {
            return Err(TraitSearchError::EntityCountMismatch {
                config_n: config.n_entities,
                engine_n: n,
            });
        }

        let entities = (0..n).map(GlobalEntityState::new).collect();

        Ok(Self {
            config,
            engines: engine_map,
            n,
            scales: HashMap::new(),
            z_scores: HashMap::new(),
            min_norm: HashMap::new(),
            percentiles: HashMap::new(),
            entities,
            sorted_indices: Vec::new(),
            band_indices: Vec::new(),
            boundary_index: None,
            state_valid: false,
            stop_streak: 0,
            has_degraded: false,
            explore_pruned: HashSet::new(),
            frustration: HashMap::new(),
        })
    }

    // ------------------------------------------------------------------
    // Public API
    // ------------------------------------------------------------------

    pub fn recompute_global_state(&mut self) -> Result<()> {
        self.solve_attributes()?;
        self.combine_attributes()?;
        self.rank_entities();
        if !self.band_indices.is_empty() && self.band_indices.len() <= MAX_REFINED_ACTIVE {
            let active = self.band_indices.clone();
            self.refine_active_variances(&active);
            self.rank_entities();
        }
        self.state_valid = true;
        Ok(())
    }

    pub fn invalidate(&mut self) {
        self.state_valid = false;
    }

    pub fn estimate_topk_error(&self) -> f64 {
        if !self.state_valid {
            return f64::INFINITY;
        }
        if self.band_indices.is_empty() {
            return 0.0;
        }

        let beta = beta_from_tolerated_error(self.config.topk.tolerated_error);
        let (lcb, ucb, _feasible) = self.compute_bounds(beta);
        let (incumbents, challengers) = self.frontier_sets(&lcb, &ucb);

        if incumbents.is_empty() || challengers.is_empty() {
            return 0.0;
        }

        let mut err = 0.0;
        for &i in &incumbents {
            for &j in &challengers {
                let delta = self.entities[i].u_mean - self.entities[j].u_mean;
                let var = self.global_diff_var_safe(i, j);
                err += inversion_prob(delta, var);
            }
        }
        err
    }

    pub fn propose_batch(
        &mut self,
        rater_id: &str,
        batch_size: usize,
        planner_mode: rating_engine::PlannerMode,
    ) -> Result<Vec<GlobalPlanProposal>> {
        // Clamp batch_size to prevent resource exhaustion
        let batch_size = batch_size.min(MAX_BATCH_SIZE);
        if batch_size == 0 {
            return Ok(Vec::new());
        }
        if !self.state_valid {
            self.recompute_global_state()?;
        }

        let band = &self.band_indices;
        if band.len() < 2 {
            return Ok(Vec::new());
        }

        let beta = beta_from_tolerated_error(self.config.topk.tolerated_error);
        let (lcb, ucb, _feasible) = self.compute_bounds(beta);
        let (incumbents, challengers) = self.frontier_sets(&lcb, &ucb);
        let critical_pair = self
            .critical_pair(&lcb, &ucb)
            .map(|(i, j)| if i < j { (i, j) } else { (j, i) });

        if incumbents.is_empty() || challengers.is_empty() {
            return Ok(Vec::new());
        }

        let mut band_candidates = self.build_frontier_candidates(&incumbents, &challengers);
        // Cross-boundary pairs decide membership, but they can over-anchor the
        // active set to one extreme item.  Add local rank-neighbor pairs inside
        // the uncertainty band so compressed/tied frontiers get direct
        // comparisons rather than only comparisons against the current top item.
        let neighbor_window = self.config.topk.band_size.max(1);
        for (pos, &i) in self.band_indices.iter().enumerate() {
            let end = (pos + neighbor_window + 1).min(self.band_indices.len());
            for &j in &self.band_indices[(pos + 1)..end] {
                let (a, b) = if i < j { (i, j) } else { (j, i) };
                band_candidates.push((a, b));
            }
        }
        if let Some((i_star, j_star)) = critical_pair {
            // Connectivity guardrail: ensure boundary items have minimal degree.
            let min_degree = 2;
            let mut anchor = self.sorted_indices.first().copied();
            if anchor == Some(i_star) || anchor == Some(j_star) {
                anchor = self.sorted_indices.get(1).copied();
            }
            if let Some(anchor_idx) = anchor {
                for attr in &self.config.attributes {
                    if let Some(engine) = self.engines.get(&attr.id) {
                        if !engine.has_min_degree(i_star, min_degree) {
                            let (a, b) = if i_star < anchor_idx {
                                (i_star, anchor_idx)
                            } else {
                                (anchor_idx, i_star)
                            };
                            band_candidates.push((a, b));
                        }
                        if !engine.has_min_degree(j_star, min_degree) {
                            let (a, b) = if j_star < anchor_idx {
                                (j_star, anchor_idx)
                            } else {
                                (anchor_idx, j_star)
                            };
                            band_candidates.push((a, b));
                        }
                    }
                }
            }
        }
        if let Some((i, j)) = critical_pair {
            band_candidates.push((i, j));
        }
        band_candidates.sort_unstable();
        band_candidates.dedup();
        if band_candidates.is_empty() {
            return Ok(Vec::new());
        }

        let mut pair_stats: HashMap<(usize, usize), f64> = HashMap::new();
        let use_effective = self.config.topk.effective_resistance_max_active > 0
            && self.band_indices.len() <= self.config.topk.effective_resistance_max_active;

        for &(i, j) in &band_candidates {
            let delta_mu = self.entities[i].u_mean - self.entities[j].u_mean;
            let base_var = if use_effective && Some((i, j)) == critical_pair {
                self.global_diff_var_effective(i, j)
                    .unwrap_or_else(|| self.global_diff_var_diag(i, j))
            } else {
                self.global_diff_var_diag(i, j)
            };
            let p_before = inversion_prob(delta_mu, base_var);
            pair_stats.insert((i, j), p_before);
        }

        let mut proposals: Vec<GlobalPlanProposal> = Vec::new();
        let mut critical_best: Option<GlobalPlanProposal> = None;

        let mut candidates = band_candidates.clone();
        if let Some((i, j)) = critical_pair {
            candidates.retain(|&(a, b)| !(a == i && b == j));
            candidates.insert(0, (i, j));
        }
        if candidates.len() > MAX_PLANNER_CANDIDATES {
            candidates.truncate(MAX_PLANNER_CANDIDATES);
        }

        for attr in &self.config.attributes {
            let attr_id = &attr.id;
            let engine =
                self.engines
                    .get(attr_id)
                    .ok_or_else(|| TraitSearchError::InternalError {
                        message: "engine map invariant violated".to_string(),
                    })?;

            let scale = self
                .scales
                .get(attr_id)
                .copied()
                .unwrap_or(SCALE_FLOOR)
                .max(SCALE_FLOOR);
            let uncertainty_weight = match engine.diag_cov() {
                Some(diag) => {
                    let mut sum = 0.0;
                    let mut count = 0usize;
                    for &idx in band {
                        if idx < diag.len() {
                            sum += diag[idx].max(0.0);
                            count += 1;
                        }
                    }
                    if count == 0 {
                        1.0
                    } else {
                        let avg_var = sum / (count as f64);
                        let denom = avg_var + scale * scale;
                        if denom > 0.0 {
                            (avg_var / denom).clamp(MIN_ATTR_UNCERTAINTY_WEIGHT, 1.0)
                        } else {
                            1.0
                        }
                    }
                }
                None => 1.0,
            };
            let normalized_weight = attr.weight.abs() / scale;
            let weight_factor = if normalized_weight == 0.0 {
                0.0
            } else {
                normalized_weight.powf(self.config.topk.weight_exponent) * uncertainty_weight
            };

            let proposals_attr =
                plan_edges_for_rater(engine, &candidates, rater_id, planner_mode, use_effective)
                    .map_err(|e| TraitSearchError::PlannerError {
                        message: e.to_string(),
                    })?;

            for PlanProposal {
                i,
                j,
                score,
                delta_info,
                delta_rank_risk,
                cost: _,
            } in proposals_attr
            {
                let (a, b) = if i <= j { (i, j) } else { (j, i) };
                let p_before = match pair_stats.get(&(a, b)) {
                    Some(v) => *v,
                    None => continue,
                };
                if p_before < MIN_PAIR_PROB {
                    continue;
                }

                let membership_weight = 0.5 * (self.entities[a].p_flip + self.entities[b].p_flip);
                let membership_weight = membership_weight.clamp(MIN_MEMBERSHIP_WEIGHT, 1.0);

                let weighted_score = weight_factor * score * p_before * membership_weight;
                if weighted_score <= 0.0 {
                    continue;
                }

                let proposal = GlobalPlanProposal {
                    attribute_id: attr_id.clone(),
                    i,
                    j,
                    global_score: weighted_score,
                    core_score: weight_factor * delta_rank_risk,
                    delta_info: weight_factor * delta_info,
                    delta_rank_risk: weight_factor * delta_rank_risk,
                };

                if Some((a, b)) == critical_pair {
                    let replace = match &critical_best {
                        Some(best) => proposal.global_score > best.global_score,
                        None => true,
                    };
                    if replace {
                        critical_best = Some(proposal.clone());
                    }
                }

                proposals.push(proposal);
            }
        }

        if proposals.is_empty() {
            if band_candidates.is_empty() {
                return Ok(Vec::new());
            }
            let mut attr_iter = self.config.attributes.iter().cycle();
            for &(i, j) in band_candidates.iter().take(batch_size) {
                let attr = attr_iter.next().ok_or(TraitSearchError::EmptyAttributes)?;
                proposals.push(GlobalPlanProposal {
                    attribute_id: attr.id.clone(),
                    i,
                    j,
                    global_score: 0.0,
                    core_score: 0.0,
                    delta_info: 0.0,
                    delta_rank_risk: 0.0,
                });
            }
        }

        proposals.sort_by(|a, b| {
            b.global_score
                .partial_cmp(&a.global_score)
                .unwrap_or(Ordering::Equal)
        });

        let mut deduped: Vec<GlobalPlanProposal> = Vec::with_capacity(batch_size);
        let mut seen: HashSet<(String, usize, usize)> = HashSet::new();

        if let Some(best) = critical_best {
            let (a, b) = if best.i <= best.j {
                (best.i, best.j)
            } else {
                (best.j, best.i)
            };
            let key = (best.attribute_id.clone(), a, b);
            if seen.insert(key) {
                deduped.push(best);
            }
        }

        for proposal in proposals.into_iter() {
            let (a, b) = if proposal.i <= proposal.j {
                (proposal.i, proposal.j)
            } else {
                (proposal.j, proposal.i)
            };
            let key = (proposal.attribute_id.clone(), a, b);

            if seen.insert(key) {
                deduped.push(proposal);
                if deduped.len() >= batch_size {
                    break;
                }
            }
        }

        // Forced exploration: if any feasible entity has total degree below
        // min_explore_degree, reserve up to half the batch for exploration proposals.
        // These go at the FRONT so they aren't crowded out by exploitation.
        let min_explore = self.config.topk.min_explore_degree;
        if min_explore > 0 {
            let explore_budget = (batch_size / 4).max(1);
            let (explore_proposals, pruned) =
                self.build_exploration_proposals(min_explore, explore_budget);
            self.explore_pruned.extend(pruned);
            if !explore_proposals.is_empty() {
                let mut merged = Vec::with_capacity(batch_size);
                let mut merged_seen: HashSet<(String, usize, usize)> = HashSet::new();
                // Exploration proposals first
                for ep in explore_proposals {
                    let (a, b) = if ep.i <= ep.j {
                        (ep.i, ep.j)
                    } else {
                        (ep.j, ep.i)
                    };
                    let key = (ep.attribute_id.clone(), a, b);
                    if merged_seen.insert(key) {
                        merged.push(ep);
                    }
                }
                // Then exploitation proposals
                for ep in deduped {
                    if merged.len() >= batch_size {
                        break;
                    }
                    let (a, b) = if ep.i <= ep.j {
                        (ep.i, ep.j)
                    } else {
                        (ep.j, ep.i)
                    };
                    let key = (ep.attribute_id.clone(), a, b);
                    if merged_seen.insert(key) {
                        merged.push(ep);
                    }
                }
                deduped = merged;
            }
        }

        Ok(deduped)
    }

    /// Build exploration proposals for entities with fewer than `min_degree` total
    /// observations. Pairs each under-observed entity against the best-measured
    /// anchor on a round-robin of attributes.
    fn build_exploration_proposals(
        &self,
        min_degree: usize,
        max_proposals: usize,
    ) -> (Vec<GlobalPlanProposal>, Vec<usize>) {
        let n = self.n;
        let n_attrs = self.config.attributes.len();
        if n_attrs == 0 {
            return (Vec::new(), Vec::new());
        }

        // Compute total degree for each entity across all attribute engines.
        // Uses has_min_degree as a probe: if entity has >= min_degree edges in this
        // attribute, credit min_degree; if >= 1, credit 1; else 0.
        let mut total_degree = vec![0usize; n];
        for attr in &self.config.attributes {
            if let Some(engine) = self.engines.get(&attr.id) {
                for (i, degree) in total_degree.iter_mut().enumerate() {
                    if engine.has_min_degree(i, min_degree) {
                        *degree += min_degree;
                    } else if engine.has_min_degree(i, 1) {
                        *degree += 1;
                    }
                }
            }
        }

        // Find entities needing exploration (total degree < min_degree * n_attrs)
        // Simplified: any entity with 0 total edges needs exploration most urgently
        let mut needy: Vec<usize> = Vec::new();
        let mut pruned: Vec<usize> = Vec::new();
        let k = self.config.topk.k;
        for (i, (&degree, entity)) in total_degree.iter().zip(self.entities.iter()).enumerate() {
            if entity.feasible && degree < min_degree {
                if let Some(eps) = self.config.topk.prune_p_topk_below {
                    // At least one observation, ranked below the boundary,
                    // and effectively no chance of crossing it: let it go.
                    if degree >= 1
                        && entity.rank.is_some_and(|r| r > k)
                        && 1.0 - entity.p_flip < eps
                    {
                        pruned.push(i);
                        continue;
                    }
                }
                needy.push(i);
            }
        }

        if needy.is_empty() {
            return (Vec::new(), pruned);
        }

        // Anchor DIVERSITY: pairing every under-observed entity against a
        // single top anchor builds a hub-and-spoke comparison graph — the
        // regret benchmark (tests/planner_regret.rs, issue #43) measured
        // that geometry losing to uniform random pair selection, and the
        // calibration battery independently flagged hub graphs as fragile
        // under IRLS+Huber. Instead, rotate anchors across the ranked list
        // (quantile stride), so exploration edges spread over the graph.
        if self.sorted_indices.is_empty() {
            return (Vec::new(), pruned);
        }
        let needy_set: std::collections::HashSet<usize> = needy.iter().copied().collect();
        let anchor_pool: Vec<usize> = self
            .sorted_indices
            .iter()
            .copied()
            .filter(|idx| !needy_set.contains(idx))
            .collect();
        // When everything is needy (cold start), fall back to chaining the
        // needy entities themselves: a path is still far better geometry
        // than a star.
        let fallback_chain = anchor_pool.is_empty();

        let mut proposals = Vec::new();
        let mut attr_cycle = self.config.attributes.iter().cycle();

        // Golden-ratio-ish stride spreads successive anchors across ranks.
        let stride = (anchor_pool.len().max(1) * 5 / 8).max(1);
        for (needy_pos, &entity_idx) in needy.iter().enumerate() {
            let anchor = if fallback_chain {
                // Chain: link to the next needy entity (wrapping), skipping
                // self-pairs.
                let next = needy[(needy_pos + 1) % needy.len()];
                if next == entity_idx {
                    continue;
                }
                next
            } else {
                anchor_pool[(needy_pos * stride) % anchor_pool.len()]
            };
            if entity_idx == anchor {
                continue;
            }
            // Propose one comparison per round-robin attribute
            let attr = match attr_cycle.next() {
                Some(a) => a,
                None => break,
            };
            let (a, b) = if entity_idx < anchor {
                (entity_idx, anchor)
            } else {
                (anchor, entity_idx)
            };
            proposals.push(GlobalPlanProposal {
                attribute_id: attr.id.clone(),
                i: a,
                j: b,
                global_score: f64::MAX, // highest priority
                core_score: 0.0,
                delta_info: 0.0,
                delta_rank_risk: 0.0,
            });

            if proposals.len() >= max_proposals {
                break;
            }
        }

        (proposals, pruned)
    }

    /// Entities excluded from further forced exploration by
    /// [`TopKConfig::prune_p_topk_below`] at any point during this run.
    pub fn explore_pruned_count(&self) -> usize {
        self.explore_pruned.len()
    }

    /// Curl fraction of one attribute's judgement field (0 = transitive,
    /// toward 1 = cyclic/noisy). None before the first solve.
    pub fn attribute_frustration(&self, attribute_id: &str) -> Option<f64> {
        self.frustration.get(attribute_id).copied()
    }

    pub fn ranked_indices(&self) -> Vec<usize> {
        self.sorted_indices.clone()
    }

    pub fn entity_state(&self, idx: usize) -> &GlobalEntityState {
        &self.entities[idx]
    }

    pub fn attribute_scores(&self, attr_id: &str) -> Option<&[f64]> {
        self.engines.get(attr_id).and_then(|engine| engine.scores())
    }

    pub fn attribute_std(&self, attr_id: &str) -> Option<Vec<f64>> {
        self.engines
            .get(attr_id)
            .and_then(|engine| engine.diag_cov())
            .map(|diag| {
                diag.iter()
                    .map(|&v| v.max(0.0).sqrt())
                    .collect::<Vec<f64>>()
            })
    }

    pub fn attribute_z_scores(&self, attr_id: &str) -> Option<&[f64]> {
        self.z_scores.get(attr_id).map(|v| v.as_slice())
    }

    pub fn attribute_min_norm(&self, attr_id: &str) -> Option<&[f64]> {
        self.min_norm.get(attr_id).map(|v| v.as_slice())
    }

    pub fn attribute_percentiles(&self, attr_id: &str) -> Option<&[f64]> {
        self.percentiles.get(attr_id).map(|v| v.as_slice())
    }

    /// Ensure derived units (z, min_norm, percentiles) are computed for an attribute.
    /// These are only needed for gate evaluation and response payloads, so compute lazily.
    pub fn ensure_attribute_units(&mut self, attr_id: &str) -> Result<()> {
        if self.z_scores.contains_key(attr_id) {
            return Ok(());
        }
        let scores = self
            .engines
            .get(attr_id)
            .and_then(|engine| engine.scores())
            .ok_or_else(|| TraitSearchError::InternalError {
                message: "scores not available; call solve() first".to_string(),
            })?;
        let (scale, z, min_norm, pct) = compute_attribute_units(scores);
        self.scales.insert(attr_id.to_string(), scale);
        self.z_scores.insert(attr_id.to_string(), z);
        self.min_norm.insert(attr_id.to_string(), min_norm);
        self.percentiles.insert(attr_id.to_string(), pct);
        Ok(())
    }

    /// Ensure derived units are computed for all attributes (used at response time).
    pub fn ensure_all_attribute_units(&mut self) -> Result<()> {
        let attr_ids: Vec<String> = self
            .config
            .attributes
            .iter()
            .map(|a| a.id.clone())
            .collect();
        for attr_id in attr_ids {
            self.ensure_attribute_units(&attr_id)?;
        }
        Ok(())
    }

    /// Add observations for a specific attribute and invalidate cached state.
    pub fn add_observations(&mut self, attr_id: &str, observations: &[Observation]) -> Result<()> {
        if observations.is_empty() {
            return Ok(());
        }
        let engine =
            self.engines
                .get_mut(attr_id)
                .ok_or_else(|| TraitSearchError::MissingEngine {
                    attribute_id: attr_id.to_string(),
                })?;
        engine.add_observations(observations);
        self.invalidate();
        Ok(())
    }

    /// Add a single observation (convenience wrapper).
    pub fn add_observation(&mut self, attr_id: &str, observation: Observation) -> Result<()> {
        let observations = [observation];
        self.add_observations(attr_id, &observations)
    }

    /// Rebuild an attribute's edge set from a complete observation log,
    /// replacing (not appending to) what was ingested incrementally. The
    /// end-of-run honest-σ refit uses this to re-weight evidence
    /// observations once the run's own noise estimate exists.
    pub fn reingest(&mut self, attr_id: &str, observations: &[Observation]) -> Result<()> {
        let engine =
            self.engines
                .get_mut(attr_id)
                .ok_or_else(|| TraitSearchError::MissingEngine {
                    attribute_id: attr_id.to_string(),
                })?;
        engine.ingest(observations);
        self.invalidate();
        Ok(())
    }
}
