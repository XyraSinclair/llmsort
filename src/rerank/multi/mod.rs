//! Multi-attribute reranking / trait search orchestrator.
//!
//! Wires together:
//! - TraitSearchManager (multi-attribute top-k uncertainty logic)
//! - RatingEngine (per-attribute IRLS solver)
//! - Pairwise LLM comparisons on a ratio ladder with confidence
//!
//! Core loop:
//! 1. Solve per-attribute rating engines and build global utility + uncertainty.
//! 2. Estimate top-k error via TraitSearchManager::estimate_topk_error().
//! 3. If error > tolerated_error and budgets remain, call propose_batch()
//!    to select highest-value comparisons.
//! 4. For each proposed (attribute_id, i, j):
//!    - Call LLM with evaluator prompt.
//!    - Parse JSON `{higher_ranked, ratio, confidence}` or `{refused:true}`.
//!    - Map to (ln_ratio, variance) and feed into the corresponding engine.
//! 5. Repeat until top-k error ≤ tolerated_error or budget/latency hit.

mod execution;
mod orchestrator;
mod request;
mod response;
mod task {
    use std::collections::{hash_map::DefaultHasher, HashMap, HashSet};
    use std::hash::{Hash, Hasher};

    use rand::{rngs::StdRng, Rng};

    use crate::trait_search::GlobalPlanProposal;

    use super::super::comparison::PairwiseComparisonSpec;
    use super::super::trace::RenderedPromptBytes;
    use super::super::types::MultiRerankRequest;

    type PairKey = (usize, usize, usize);

    #[derive(Clone, Copy)]
    pub(super) struct CompareTask {
        pub(super) key: PairKey,
        pub(super) attr_idx: usize,
        pub(super) i: usize,
        pub(super) j: usize,
        /// When true, entity j is presented as "A" and entity i as "B"
        /// to counteract position bias.
        pub(super) swapped: bool,
    }

    pub(super) struct TaskPlanner<'a, 'rng> {
        req: &'a MultiRerankRequest,
        attr_id_to_index: &'a HashMap<&'a str, usize>,
        refused_pairs: &'a HashSet<PairKey>,
        pair_repeats: &'a HashMap<PairKey, f64>,
        batch_size: usize,
        presentation_rng: &'rng mut Option<StdRng>,
        tasks: Vec<CompareTask>,
    }

    impl<'a, 'rng> TaskPlanner<'a, 'rng> {
        pub(super) fn new(
            req: &'a MultiRerankRequest,
            attr_id_to_index: &'a HashMap<&'a str, usize>,
            refused_pairs: &'a HashSet<PairKey>,
            pair_repeats: &'a HashMap<PairKey, f64>,
            batch_size: usize,
            presentation_rng: &'rng mut Option<StdRng>,
        ) -> Self {
            Self {
                req,
                attr_id_to_index,
                refused_pairs,
                pair_repeats,
                batch_size,
                presentation_rng,
                tasks: Vec::with_capacity(batch_size),
            }
        }

        pub(super) fn plan(
            mut self,
            proposals: Vec<GlobalPlanProposal>,
            ranked: &[usize],
            nonce_draws: usize,
            comparisons_attempted: usize,
        ) -> (Vec<CompareTask>, Vec<(CompareTask, Option<String>)>) {
            let mut batch_seen = HashSet::new();
            for proposal in proposals {
                let Some(&attr_idx) = self.attr_id_to_index.get(proposal.attribute_id.as_str())
                else {
                    continue;
                };
                let (i, j) = (proposal.i, proposal.j);
                if i >= self.req.entities.len() || j >= self.req.entities.len() {
                    continue;
                }
                let (a, b) = if i <= j { (i, j) } else { (j, i) };
                let key = (attr_idx, a, b);
                if self.refused_pairs.contains(&key)
                    || !batch_seen.insert(key)
                    || self.at_repeat_cap(key)
                {
                    continue;
                }
                if self.push_pair(key, attr_idx, i, j) {
                    break;
                }
            }

            if self.tasks.is_empty() {
                let mut coverage_seen = HashSet::new();
                'coverage: for stride in 1..ranked.len().max(1) {
                    for pos in 0..ranked.len().saturating_sub(stride) {
                        let (i, j) = (ranked[pos], ranked[pos + stride]);
                        let (a, b) = if i <= j { (i, j) } else { (j, i) };
                        for attr_idx in 0..self.req.attributes.len() {
                            let key = (attr_idx, a, b);
                            if self.refused_pairs.contains(&key)
                                || !coverage_seen.insert(key)
                                || self.at_repeat_cap(key)
                            {
                                continue;
                            }
                            if self.push_pair(key, attr_idx, i, j) {
                                break 'coverage;
                            }
                        }
                    }
                }
            }

            self.tasks.sort_by_key(|task| {
                let first = if task.swapped { task.j } else { task.i };
                (task.attr_idx, first)
            });
            let drawn_tasks = if nonce_draws > 1 {
                self.tasks
                    .iter()
                    .flat_map(|task| {
                        (0..nonce_draws).map(|draw| {
                            let mut hasher = DefaultHasher::new();
                            (
                                task.attr_idx,
                                task.i,
                                task.j,
                                task.swapped,
                                draw,
                                comparisons_attempted,
                            )
                                .hash(&mut hasher);
                            (*task, Some(format!("{:016x}", hasher.finish())))
                        })
                    })
                    .collect()
            } else {
                self.tasks.iter().map(|task| (*task, None)).collect()
            };
            (self.tasks, drawn_tasks)
        }

        fn at_repeat_cap(&self, key: PairKey) -> bool {
            self.req.max_pair_repeats.is_some_and(|max| {
                self.pair_repeats.get(&key).copied().unwrap_or(0.0) >= max as f64
            })
        }

        /// Add one pair in the configured presentation order(s). Returns true
        /// when the batch is full or cannot fit a counterbalanced pair.
        fn push_pair(&mut self, key: PairKey, attr_idx: usize, i: usize, j: usize) -> bool {
            if self.req.counterbalance_pairs {
                if self.tasks.len() + 2 > self.batch_size {
                    return true;
                }
                self.tasks.extend([false, true].map(|swapped| CompareTask {
                    key,
                    attr_idx,
                    i,
                    j,
                    swapped,
                }));
            } else {
                let swapped = self.req.randomize_presentation_order
                    && match self.presentation_rng.as_mut() {
                        Some(rng) => rng.gen_bool(0.5),
                        None => rand::thread_rng().gen_bool(0.5),
                    };
                self.tasks.push(CompareTask {
                    key,
                    attr_idx,
                    i,
                    j,
                    swapped,
                });
            }
            self.tasks.len() >= self.batch_size
        }
    }

    #[derive(Clone)]
    pub(super) struct TraceFields {
        pub(super) attribute_prompt_hash: String,
        pub(super) prompt_template_slug: String,
        pub(super) template_hash: String,
        pub(super) rendered_prompt_digest: String,
        /// Exact bytes behind `rendered_prompt_digest` (recomputability: the
        /// store retains the bytes, not just their hash).
        pub(super) rendered_prompt: RenderedPromptBytes,
        pub(super) entity_a_hash: String,
        pub(super) entity_b_hash: String,
        pub(super) cache_key_hash: String,
    }

    impl TraceFields {
        pub(super) fn new(comparison: PairwiseComparisonSpec<'_>) -> Self {
            let cache_key = comparison.cache_key();
            let prompt = comparison.prompt_instance();
            Self {
                attribute_prompt_hash: cache_key.attribute_prompt_hash,
                prompt_template_slug: cache_key.prompt_template_slug,
                template_hash: cache_key.template_hash,
                rendered_prompt_digest: prompt.rendered_digest(),
                rendered_prompt: RenderedPromptBytes {
                    system: prompt.system,
                    user: prompt.user,
                },
                entity_a_hash: cache_key.entity_a_hash,
                entity_b_hash: cache_key.entity_b_hash,
                cache_key_hash: cache_key.key_hash,
            }
        }
    }
}

pub use execution::{
    build_engine_config, build_trait_search_config, JudgementRunInstrumentation, RerankExecution,
};
pub(crate) use orchestrator::multi_rerank_with_failures;
pub use request::{
    default_template_slug, estimate_max_rerank_charge, validate_multi_rerank_request,
    MultiRerankError, RerankChargeEstimate, DEFAULT_MODEL, EVIDENCE_VAR_FLOOR,
};

/// Run a multi-attribute reranking session.
pub async fn multi_rerank(
    request: super::types::MultiRerankRequest,
    execution: RerankExecution<'_>,
) -> Result<super::types::MultiRerankResponse, MultiRerankError> {
    Ok(multi_rerank_with_failures(request, execution)
        .await?
        .response)
}

#[cfg(test)]
mod tests;
