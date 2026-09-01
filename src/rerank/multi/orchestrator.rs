use std::collections::{HashMap, HashSet};
use std::sync::atomic::Ordering as AtomicOrdering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::stream::{self, StreamExt};
use rand::{rngs::StdRng, Rng, SeedableRng};

use crate::rating_engine::{AttributeParams, Observation, PlannerMode, RaterParams, RatingEngine};
use crate::trait_search::TraitSearchManager;

use super::super::comparison::{
    compare_pair, PairwiseComparisonAttribute, PairwiseComparisonEntity, PairwiseComparisonRequest,
    PairwiseComparisonSpec,
};
use super::super::hooks::ComparisonEvent;
use super::super::model_policy::ModelPolicyContext;
use super::super::trace::{now_epoch_ms, ComparisonTrace};
use super::super::types::{HigherRanked, MultiRerankRequest, PairwiseJudgement, RerankStopReason};
use super::execution::{build_engine_config, build_trait_search_config, RerankExecution};
use super::request::{
    default_comparison_budget, materialize_template_defaults, validate_multi_rerank_request,
    MultiRerankError, DEFAULT_COMPARISON_CONCURRENCY, DEFAULT_MODEL, EVIDENCE_VAR_FLOOR,
};
use super::response::{build_response, BuiltResponse, ResponseContext};
use super::task::{CompareTask, TraceFields};

const CONSECUTIVE_FAILURE_LIMIT: usize = 5;
/// Run a multi-attribute reranking session.
///
/// If a cache is provided, cached pairwise judgements are reused and new
/// judgements are written back to the cache.
pub(crate) async fn multi_rerank_with_failures(
    mut req: MultiRerankRequest,
    execution: RerankExecution<'_>,
) -> Result<BuiltResponse, MultiRerankError> {
    materialize_template_defaults(&mut req);
    validate_multi_rerank_request(&req)?;

    let n_entities = req.entities.len();
    let n_attributes = req.attributes.len();

    let comparison_budget = req
        .comparison_budget
        .unwrap_or_else(|| default_comparison_budget(n_entities, n_attributes));
    let latency_budget = req.latency_budget_ms.map(Duration::from_millis);
    let cache_only = execution.run_options.cache_only;
    if cache_only && execution.cache.is_none() {
        return Err(MultiRerankError::InvalidRequest(
            "cache_only requires a cache instance".into(),
        ));
    }

    let base_model = req.model.as_deref().unwrap_or(DEFAULT_MODEL);
    let rater_id = req.rater_id.as_deref().unwrap_or(base_model);
    let comparison_concurrency = req
        .comparison_concurrency
        .unwrap_or(DEFAULT_COMPARISON_CONCURRENCY);
    let max_pair_repeats = req.max_pair_repeats;

    let (config, topk_cfg) = build_trait_search_config(&req);

    let mut engines: HashMap<String, RatingEngine> = HashMap::new();
    let mut raters: HashMap<String, RaterParams> = HashMap::new();
    raters.insert(rater_id.to_string(), RaterParams::default());

    let engine_cfg = build_engine_config(&execution.run_options, &topk_cfg);

    for attr in &req.attributes {
        let engine = RatingEngine::new(
            n_entities,
            AttributeParams::default(),
            raters.clone(),
            Some(engine_cfg.clone()),
        )
        .map_err(|e| MultiRerankError::RatingEngine(e.to_string()))?;
        engines.insert(attr.id.clone(), engine);
    }
    let engine_spec = engines
        .values()
        .next()
        .expect("validated rerank request has at least one attribute")
        .spec();
    if engines.values().any(|engine| engine.spec() != engine_spec) {
        return Err(MultiRerankError::RatingEngine(
            "per-attribute engine specifications diverged".to_string(),
        ));
    }
    let engine_spec_id = engine_spec.id().0;

    let mut manager = TraitSearchManager::new(config, engines)?;

    let start_time = Instant::now();

    let mut pair_repeats: HashMap<(usize, usize, usize), f64> = HashMap::new();

    let mut comparisons_attempted: usize = 0;
    let mut comparisons_failed: usize = 0;
    let mut first_error: Option<String> = None;
    let mut consecutive_failures: usize = 0;
    let mut comparisons_used: usize = 0;
    let mut comparisons_refused: usize = 0;
    let mut comparisons_cached: usize = 0;

    let mut attribute_attempted: Vec<usize> = vec![0; n_attributes];
    let mut attribute_used: Vec<usize> = vec![0; n_attributes];

    let mut provider_input_tokens: u32 = 0;
    let mut provider_output_tokens: u32 = 0;
    let mut provider_cost_nanodollars: i64 = 0;
    let mut provider_cost_is_estimate = false;

    let attr_id_to_index: HashMap<&str, usize> = req
        .attributes
        .iter()
        .enumerate()
        .map(|(idx, a)| (a.id.as_str(), idx))
        .collect();
    let mut warm_start_observations = 0usize;
    // Complete per-attribute observation log, mirroring everything ingested
    // incrementally. The end-of-run honest-σ refit re-ingests from here with
    // context noise folded into each evidence observation's variance.
    let mut observation_log: HashMap<String, Vec<Observation>> = HashMap::new();

    if let Some(provider) = execution.warm_start {
        match provider.warm_start(&req, rater_id).await {
            Ok(data) => {
                let mut total_loaded = 0usize;

                for (attribute_id, observations) in data.observations_by_attribute {
                    if observations.is_empty() {
                        continue;
                    }

                    if let Err(e) = manager.add_observations(&attribute_id, &observations) {
                        tracing::warn!(
                            attribute_id = %attribute_id,
                            error = %e,
                            "Warm-start failed: add observations"
                        );
                        continue;
                    }

                    total_loaded = total_loaded.saturating_add(observations.len());
                    observation_log
                        .entry(attribute_id.clone())
                        .or_default()
                        .extend_from_slice(&observations);

                    let Some(&attr_idx) = attr_id_to_index.get(attribute_id.as_str()) else {
                        continue;
                    };

                    for obs in &observations {
                        let (a, b) = (obs.i.min(obs.j), obs.i.max(obs.j));
                        let key = (attr_idx, a, b);
                        let reps = if obs.reps.is_finite() && obs.reps > 0.0 {
                            obs.reps
                        } else {
                            0.0
                        };
                        *pair_repeats.entry(key).or_insert(0.0) += reps;
                    }
                }

                if total_loaded > 0 {
                    manager.invalidate();
                }
                warm_start_observations = total_loaded;
            }
            Err(e) => {
                tracing::warn!(error = %e, "Warm-start provider failed");
            }
        }
    }

    let mut refused_pairs: HashSet<(usize, usize, usize)> = HashSet::new();
    let mut models_used: HashSet<String> = HashSet::new();
    // Counterbalancing diagnostics: first decisive direction observed per
    // (attribute, pair) in each presentation order; a pair counts once.
    let mut counterbalance_dirs: HashMap<(usize, usize, usize), [Option<HigherRanked>; 2]> =
        HashMap::new();
    let mut counterbalance_done: HashSet<(usize, usize, usize)> = HashSet::new();
    let mut pairs_counterbalanced: usize = 0;
    let mut position_flips: usize = 0;
    // Evidence-mode accounting (ratio-letter path).
    let mut evidence_judgements: usize = 0;
    let mut logprob_mode_judgements: usize = 0;
    let mut visible_mass_sum: f64 = 0.0;
    // PMF-level counterbalance residuals: for a pair asked in both orders,
    // an unbiased judge's presented-coordinate means sum to zero; the sum
    // measures position bias in log-ratio units, per pair — strictly
    // richer than binary direction flips.
    let mut evidence_order_means: HashMap<(usize, usize, usize), [Option<f64>; 2]> = HashMap::new();
    let mut evidence_order_residual_sum_abs: f64 = 0.0;
    let mut evidence_order_residual_pairs: usize = 0;
    let mut presentation_rng = execution
        .run_options
        .rng_seed
        .map(|seed| StdRng::seed_from_u64(seed ^ 0xC0A7_5EED_5EED_5EED));

    let stop_reason = 'rerank: loop {
        if let Some(flag) = execution.cancel_flag {
            if flag.load(AtomicOrdering::Relaxed) {
                break 'rerank RerankStopReason::Cancelled;
            }
        }

        manager.recompute_global_state()?;
        let current_error = manager.estimate_topk_error();

        if manager.certified_stop() {
            break 'rerank RerankStopReason::CertifiedStop;
        }
        if current_error <= topk_cfg.tolerated_error {
            break 'rerank RerankStopReason::ToleratedErrorMet;
        }
        if comparisons_attempted >= comparison_budget {
            break 'rerank RerankStopReason::BudgetExhausted;
        }
        if let Some(limit) = latency_budget {
            if start_time.elapsed() >= limit {
                break 'rerank RerankStopReason::LatencyBudgetExceeded;
            }
        }
        let remaining_budget = comparison_budget.saturating_sub(comparisons_attempted);
        if remaining_budget == 0 {
            break 'rerank RerankStopReason::BudgetExhausted;
        }
        let Some(batch_size) = super::request::cost_capped_batch_size(
            &req,
            provider_cost_nanodollars,
            comparisons_attempted,
            remaining_budget,
        ) else {
            break 'rerank RerankStopReason::CostBudgetExhausted;
        };
        if req.counterbalance_pairs && batch_size < 2 {
            // A counterbalanced pair needs two calls; one slot cannot start one.
            break 'rerank RerankStopReason::BudgetExhausted;
        }
        let proposal_request_size = (batch_size.saturating_mul(3)).max(batch_size);
        let proposals =
            manager.propose_batch(rater_id, proposal_request_size, PlannerMode::Hybrid)?;

        if proposals.is_empty() {
            break 'rerank RerankStopReason::NoProposals;
        }

        let mut batch_seen: HashSet<(usize, usize, usize)> = HashSet::new();
        let mut tasks: Vec<CompareTask> = Vec::with_capacity(batch_size);

        for proposal in proposals {
            let attr_id = proposal.attribute_id.as_str();
            let Some(&attr_idx) = attr_id_to_index.get(attr_id) else {
                continue;
            };

            let i = proposal.i;
            let j = proposal.j;
            if i >= req.entities.len() || j >= req.entities.len() {
                continue;
            }

            let (a, b) = if i <= j { (i, j) } else { (j, i) };
            let key = (attr_idx, a, b);

            if refused_pairs.contains(&key) {
                continue;
            }
            if !batch_seen.insert(key) {
                continue;
            }
            if let Some(max) = max_pair_repeats {
                if pair_repeats.get(&key).copied().unwrap_or(0.0) >= max as f64 {
                    continue;
                }
            }

            if req.counterbalance_pairs {
                // Both presentation orders, deterministically. Two calls per
                // pair; presentation randomization is subsumed.
                if tasks.len() + 2 > batch_size {
                    break;
                }
                for swapped in [false, true] {
                    tasks.push(CompareTask {
                        key,
                        attr_idx,
                        i,
                        j,
                        swapped,
                    });
                }
            } else {
                let swapped = if req.randomize_presentation_order {
                    if let Some(rng) = presentation_rng.as_mut() {
                        rng.gen_bool(0.5)
                    } else {
                        rand::thread_rng().gen_bool(0.5)
                    }
                } else {
                    false
                };
                tasks.push(CompareTask {
                    key,
                    attr_idx,
                    i,
                    j,
                    swapped,
                });
            }

            if tasks.len() >= batch_size {
                break;
            }
        }

        if tasks.is_empty() {
            break 'rerank RerankStopReason::NoNewPairs;
        }

        let mut score_cache: HashMap<String, Vec<f64>> = HashMap::new();
        let mut std_cache: HashMap<String, Vec<f64>> = HashMap::new();
        if execution.model_policy.is_some() {
            let mut attrs_in_batch: HashSet<&str> = HashSet::new();
            for task in &tasks {
                attrs_in_batch.insert(req.attributes[task.attr_idx].id.as_str());
            }
            for attr_id in attrs_in_batch {
                if let Some(scores) = manager.attribute_scores(attr_id) {
                    score_cache.insert(attr_id.to_string(), scores.to_vec());
                }
                if let Some(stds) = manager.attribute_std(attr_id) {
                    std_cache.insert(attr_id.to_string(), stds);
                }
            }
        }
        let score_cache = Arc::new(score_cache);
        let std_cache = Arc::new(std_cache);
        let base_model = base_model.to_string();
        let comparisons_attempted_snapshot = comparisons_attempted;
        let comparisons_used_snapshot = comparisons_used;

        let batch_results = stream::iter(tasks.into_iter().map(|task| {
            let gateway = execution.gateway.clone();
            let attribution = execution.attribution.clone();
            let policy = execution.model_policy.clone();
            let attr = &req.attributes[task.attr_idx];
            // When swapped, present entity j as "A" and entity i as "B"
            // to counteract position bias.
            let (entity_a, entity_b) = if task.swapped {
                (&req.entities[task.j], &req.entities[task.i])
            } else {
                (&req.entities[task.i], &req.entities[task.j])
            };
            let score_cache = score_cache.clone();
            let std_cache = std_cache.clone();
            let context = ModelPolicyContext {
                global_topk_error: current_error,
                comparisons_attempted: comparisons_attempted_snapshot,
                comparisons_used: comparisons_used_snapshot,
                attribute_comparisons_attempted: attribute_attempted[task.attr_idx],
                attribute_comparisons_used: attribute_used[task.attr_idx],
                attribute_id: &attr.id,
                i: task.i,
                j: task.j,
                attribute_scores: score_cache.get(&attr.id).map(|v| v.as_slice()),
                attribute_stds: std_cache.get(&attr.id).map(|v| v.as_slice()),
            };
            let selected_model = if let Some(policy) = policy.as_ref() {
                policy.select_model(&context)
            } else {
                base_model.clone()
            };
            async move {
                let comparison = PairwiseComparisonRequest {
                    spec: PairwiseComparisonSpec {
                        model: &selected_model,
                        attribute: PairwiseComparisonAttribute {
                            id: &attr.id,
                            prompt: &attr.prompt,
                            prompt_template_slug: attr.prompt_template_slug.as_deref(),
                        },
                        entity_a: PairwiseComparisonEntity {
                            id: &entity_a.id,
                            text: &entity_a.text,
                        },
                        entity_b: PairwiseComparisonEntity {
                            id: &entity_b.id,
                            text: &entity_b.text,
                        },
                    },
                    cache_only,
                    attribution,
                    nonce: None,
                };
                let judgement = compare_pair(gateway.as_ref(), execution.cache, comparison).await;
                (task, judgement, selected_model)
            }
        }))
        .buffer_unordered(comparison_concurrency)
        .collect::<Vec<_>>()
        .await;

        for (task, judgement, selected_model) in batch_results {
            comparisons_attempted += 1;
            let comparison_index = comparisons_attempted;
            attribute_attempted[task.attr_idx] =
                attribute_attempted[task.attr_idx].saturating_add(1);

            let attr = &req.attributes[task.attr_idx];
            let (trace_entity_a_index, trace_entity_b_index) = if task.swapped {
                (task.j, task.i)
            } else {
                (task.i, task.j)
            };
            let trace_entity_a = &req.entities[trace_entity_a_index];
            let trace_entity_b = &req.entities[trace_entity_b_index];

            models_used.insert(selected_model.clone());
            let attr_id = attr.id.as_str();

            let trace_fields = if execution.trace.is_some() {
                let comparison = PairwiseComparisonSpec {
                    model: &selected_model,
                    attribute: PairwiseComparisonAttribute {
                        id: &attr.id,
                        prompt: &attr.prompt,
                        prompt_template_slug: attr.prompt_template_slug.as_deref(),
                    },
                    entity_a: PairwiseComparisonEntity {
                        id: &trace_entity_a.id,
                        text: &trace_entity_a.text,
                    },
                    entity_b: PairwiseComparisonEntity {
                        id: &trace_entity_b.id,
                        text: &trace_entity_b.text,
                    },
                };
                let cache_key = comparison.cache_key();
                Some(TraceFields {
                    attribute_prompt_hash: cache_key.attribute_prompt_hash,
                    prompt_template_slug: cache_key.prompt_template_slug.clone(),
                    template_hash: cache_key.template_hash.clone(),
                    rendered_prompt_digest: comparison.rendered_prompt_digest(),
                    entity_a_hash: cache_key.entity_a_hash,
                    entity_b_hash: cache_key.entity_b_hash,
                    cache_key_hash: cache_key.key_hash,
                })
            } else {
                None
            };

            let build_trace = |cached: bool,
                               input_tokens: u32,
                               output_tokens: u32,
                               provider_cost_nanodollars: i64,
                               provider_cost_is_estimate: bool,
                               rendered_prompt_digest: Option<&str>,
                               error: Option<String>| {
                let fields = trace_fields
                    .as_ref()
                    .expect("trace_fields set when trace active");
                ComparisonTrace {
                    timestamp_ms: now_epoch_ms(),
                    comparison_index,
                    attribute_id: attr.id.clone(),
                    attribute_index: task.attr_idx,
                    attribute_prompt_hash: fields.attribute_prompt_hash.clone(),
                    prompt_template_slug: fields.prompt_template_slug.clone(),
                    template_hash: fields.template_hash.clone(),
                    rendered_prompt_digest: rendered_prompt_digest
                        .unwrap_or(&fields.rendered_prompt_digest)
                        .to_string(),
                    engine_spec_id: engine_spec_id.clone(),
                    entity_a_id: trace_entity_a.id.clone(),
                    entity_b_id: trace_entity_b.id.clone(),
                    entity_a_index: trace_entity_a_index,
                    entity_b_index: trace_entity_b_index,
                    entity_a_hash: fields.entity_a_hash.clone(),
                    entity_b_hash: fields.entity_b_hash.clone(),
                    cache_key_hash: fields.cache_key_hash.clone(),
                    model: selected_model.clone(),
                    served_model: None,
                    higher_ranked: None,
                    ratio: None,
                    confidence: None,
                    solver_observation: None,
                    pairwise_logprob_posterior: None,
                    output_logprob_token_count: None,
                    pairwise_logprob_posterior_error: None,
                    ledger_draws: None,
                    refused: false,
                    cached,
                    swapped: task.swapped,
                    input_tokens,
                    output_tokens,
                    provider_cost_nanodollars,
                    provider_cost_is_estimate,
                    error,
                }
            };

            match judgement {
                Ok((PairwiseJudgement::Refused, usage)) => {
                    consecutive_failures = 0;
                    if usage.cached {
                        comparisons_cached += 1;
                    }
                    provider_input_tokens =
                        provider_input_tokens.saturating_add(usage.input_tokens);
                    provider_output_tokens =
                        provider_output_tokens.saturating_add(usage.output_tokens);
                    provider_cost_nanodollars =
                        provider_cost_nanodollars.saturating_add(usage.provider_cost_nanodollars);
                    provider_cost_is_estimate |= usage.provider_cost_is_estimate;
                    comparisons_refused += 1;
                    refused_pairs.insert(task.key);

                    if let Some(trace) = execution.trace {
                        let mut event = build_trace(
                            usage.cached,
                            usage.input_tokens,
                            usage.output_tokens,
                            usage.provider_cost_nanodollars,
                            usage.provider_cost_is_estimate,
                            Some(&usage.rendered_prompt_digest),
                            None,
                        );
                        event.refused = true;
                        event.served_model = usage.served_model.clone();
                        event.output_logprob_token_count =
                            usage.output_logprobs.as_ref().map(Vec::len);
                        if !usage.cached && event.output_logprob_token_count.is_none() {
                            event.pairwise_logprob_posterior_error =
                                Some("provider_returned_no_output_logprobs".to_string());
                        }
                        trace.record(event)?;
                    }

                    if let Some(observer) = execution.observer {
                        let event = ComparisonEvent {
                            attribute_id: attr.id.clone(),
                            attribute_index: task.attr_idx,
                            entity_a_id: trace_entity_a.id.clone(),
                            entity_b_id: trace_entity_b.id.clone(),
                            entity_a_index: trace_entity_a_index,
                            entity_b_index: trace_entity_b_index,
                            model: selected_model.clone(),
                            judgement: PairwiseJudgement::Refused,
                            usage,
                        };
                        if let Err(e) = observer.on_comparison(event).await {
                            tracing::warn!(error = %e, "Comparison observer failed");
                        }
                    }
                }
                Ok((
                    PairwiseJudgement::Observation {
                        higher_ranked,
                        ratio,
                        confidence,
                    },
                    usage,
                )) => {
                    consecutive_failures = 0;
                    if usage.cached {
                        comparisons_cached += 1;
                    }
                    provider_input_tokens =
                        provider_input_tokens.saturating_add(usage.input_tokens);
                    provider_output_tokens =
                        provider_output_tokens.saturating_add(usage.output_tokens);
                    provider_cost_nanodollars =
                        provider_cost_nanodollars.saturating_add(usage.provider_cost_nanodollars);
                    provider_cost_is_estimate |= usage.provider_cost_is_estimate;
                    // When presentation was swapped, "A" in the LLM
                    // response actually refers to entity j, not i.
                    let effective = if task.swapped {
                        match higher_ranked {
                            HigherRanked::A => HigherRanked::B,
                            HigherRanked::B => HigherRanked::A,
                        }
                    } else {
                        higher_ranked
                    };
                    if req.counterbalance_pairs
                        && ratio > 1.0
                        && !counterbalance_done.contains(&task.key)
                    {
                        let entry = counterbalance_dirs.entry(task.key).or_insert([None, None]);
                        entry[task.swapped as usize] = Some(effective);
                        if let [Some(unswapped_dir), Some(swapped_dir)] = *entry {
                            pairs_counterbalanced += 1;
                            if unswapped_dir != swapped_dir {
                                position_flips += 1;
                            }
                            counterbalance_done.insert(task.key);
                        }
                    }
                    let (obs_i, obs_j) = match effective {
                        HigherRanked::A => (task.i, task.j),
                        HigherRanked::B => (task.j, task.i),
                    };
                    // PMF-derived moments (ratio-letter path): the solver
                    // gets the measured mean and variance directly, with
                    // stated confidence out of the loop. Moments arrive in
                    // PRESENTED coordinates; a swapped presentation flips
                    // the sign (reflection is exact for the letter algebra).
                    let obs = if let Some(moments) = usage.evidence_moments {
                        evidence_judgements += 1;
                        if moments.logprob_mode {
                            logprob_mode_judgements += 1;
                        }
                        visible_mass_sum += moments.visible_mass;
                        if req.counterbalance_pairs {
                            let entry =
                                evidence_order_means.entry(task.key).or_insert([None, None]);
                            let slot = task.swapped as usize;
                            if entry[slot].is_none() {
                                entry[slot] = Some(moments.log_ratio_mean);
                                if let [Some(unswapped), Some(swapped)] = *entry {
                                    // Both presented-coordinate means; an
                                    // unbiased judge gives sum == 0.
                                    evidence_order_residual_sum_abs += (unswapped + swapped).abs();
                                    evidence_order_residual_pairs += 1;
                                }
                            }
                        }
                        let mean_ij = if task.swapped {
                            -moments.log_ratio_mean
                        } else {
                            moments.log_ratio_mean
                        };
                        Observation::from_log_ratio_moments(
                            task.i,
                            task.j,
                            mean_ij,
                            moments.log_ratio_var.max(EVIDENCE_VAR_FLOOR),
                            rater_id,
                            1.0,
                        )
                    } else {
                        // Point-path order residual: same diagnostic as the
                        // PMF path, from the presented-coordinate signed
                        // log-ratio. An unbiased judge's two orders sum to
                        // zero; the residual is position bias in nats.
                        if req.counterbalance_pairs {
                            let toward_presented_a = match effective {
                                HigherRanked::A => 1.0,
                                HigherRanked::B => -1.0,
                            } * if task.swapped { -1.0 } else { 1.0 };
                            let presented_m = toward_presented_a * ratio.max(1.0).ln();
                            let entry =
                                evidence_order_means.entry(task.key).or_insert([None, None]);
                            let slot = task.swapped as usize;
                            if entry[slot].is_none() {
                                entry[slot] = Some(presented_m);
                                if let [Some(unswapped), Some(swapped)] = *entry {
                                    evidence_order_residual_sum_abs += (unswapped + swapped).abs();
                                    evidence_order_residual_pairs += 1;
                                }
                            }
                        }
                        Observation::new(obs_i, obs_j, ratio, confidence, rater_id, 1.0)
                    };
                    let trace_observation = execution.trace.is_some().then(|| obs.clone());
                    let logged_observation = obs.clone();
                    let solver_error = manager
                        .add_observation(attr_id, obs)
                        .err()
                        .map(|error| error.to_string());
                    if let Some(error) = &solver_error {
                        tracing::warn!(
                            attribute_id = %attr_id,
                            error,
                            "Failed to add observation"
                        );
                    } else {
                        comparisons_used += 1;
                        attribute_used[task.attr_idx] =
                            attribute_used[task.attr_idx].saturating_add(1);
                        observation_log
                            .entry(attr_id.to_string())
                            .or_default()
                            .push(logged_observation);
                    }
                    *pair_repeats.entry(task.key).or_insert(0.0) += 1.0;

                    if let Some(trace) = execution.trace {
                        let mut event = build_trace(
                            usage.cached,
                            usage.input_tokens,
                            usage.output_tokens,
                            usage.provider_cost_nanodollars,
                            usage.provider_cost_is_estimate,
                            Some(&usage.rendered_prompt_digest),
                            None,
                        );
                        event.served_model = usage.served_model.clone();
                        event.higher_ranked = Some(match higher_ranked {
                            HigherRanked::A => "A".to_string(),
                            HigherRanked::B => "B".to_string(),
                        });
                        event.ratio = Some(ratio);
                        event.confidence = Some(confidence);
                        if solver_error.is_none() {
                            event.solver_observation = trace_observation;
                        } else {
                            event.error = solver_error
                                .as_ref()
                                .map(|error| format!("solver rejected observation: {error}"));
                        }
                        event.pairwise_logprob_posterior = usage.pairwise_logprob_posterior.clone();
                        event.ledger_draws = usage.ledger_draws.clone();
                        event.output_logprob_token_count =
                            usage.output_logprobs.as_ref().map(Vec::len);
                        if !usage.cached && event.pairwise_logprob_posterior.is_none() {
                            event.pairwise_logprob_posterior_error = match event
                                .output_logprob_token_count
                            {
                                Some(count) => Some(format!(
                                    "posterior_parse_failed_from_{count}_output_logprob_tokens"
                                )),
                                None => Some("provider_returned_no_output_logprobs".to_string()),
                            };
                        }
                        trace.record(event)?;
                    }

                    if let Some(observer) = execution.observer {
                        let event = ComparisonEvent {
                            attribute_id: attr.id.clone(),
                            attribute_index: task.attr_idx,
                            entity_a_id: trace_entity_a.id.clone(),
                            entity_b_id: trace_entity_b.id.clone(),
                            entity_a_index: trace_entity_a_index,
                            entity_b_index: trace_entity_b_index,
                            model: selected_model.clone(),
                            judgement: PairwiseJudgement::Observation {
                                higher_ranked,
                                ratio,
                                confidence,
                            },
                            usage,
                        };
                        if let Err(e) = observer.on_comparison(event).await {
                            tracing::warn!(error = %e, "Comparison observer failed");
                        }
                    }
                }
                Err(e) => {
                    comparisons_failed = comparisons_failed.saturating_add(1);
                    first_error.get_or_insert_with(|| e.to_string());
                    consecutive_failures = e.next_non_retryable_streak(consecutive_failures);
                    if let Some(trace) = execution.trace {
                        let event = build_trace(false, 0, 0, 0, false, None, Some(e.to_string()));
                        trace.record(event)?;
                    }
                    if cache_only {
                        return Err(MultiRerankError::Comparison(e));
                    }
                    tracing::warn!(
                        attribute_id = %attr_id,
                        i = task.i,
                        j = task.j,
                        error = %e,
                        "Comparison failed"
                    );
                }
            }
        }

        if consecutive_failures >= CONSECUTIVE_FAILURE_LIMIT {
            break 'rerank RerankStopReason::ConsecutiveFailures;
        }
    };

    // Honest-σ refit. PMF-internal variance understates true per-observation
    // uncertainty: the phase-1 analysis is stochastic even at temperature 0,
    // and that within-call noise never shows up inside a single verdict PMF
    // (slot-hetero pack, 2026-08-30: σε = 0.215 nats/call on the terra 2p
    // rail, ~1× signal scale, while the PMF-weighted posterior reported
    // ±0.050). The run's own counterbalance residual is a self-calibrating
    // estimate: with slot bias ≈ 0 (measured), m_fwd + m_rev ~ N(0, 2σε²)
    // per pair at one draw each, so σε = mean|m_fwd + m_rev| · √π / 2
    // (validated: 0.245 predicted vs 0.215 measured directly). Where real
    // slot bias exists the residual includes it and the refit over-widens —
    // conservative, never overconfident. Evidence observations get
    // var + σ_w²; point observations keep unit precision (their weighting
    // never claimed calibration). Without counterbalancing there is no
    // estimator: σ_w stays None and nothing is inflated. In a
    // mixed-instrument run the residual pools evidence and point pairs
    // (matching `evidence_order_residual_mean_abs`'s declared "ANY
    // instrument" semantics), so σ_w is then partly estimated from
    // point-path residuals while applied only to evidence observations —
    // homogeneous pools, the normal case, are unaffected.
    let evidence_sigma_w = (evidence_order_residual_pairs > 0 && evidence_judgements > 0)
        .then(|| {
            (evidence_order_residual_sum_abs / evidence_order_residual_pairs as f64)
                * std::f64::consts::PI.sqrt()
                / 2.0
        })
        .filter(|sigma| sigma.is_finite() && *sigma > 0.0);
    // Companion to sigma_w: the RMS total per-observation sigma. The ratio
    // sigma_w / obs_sigma_rms is the aleatoric share of each observation's
    // model variance — the part that actually resamples on an independent
    // rerun (the PMF component is the judge's reproducible expressed
    // spread; measured on luna seeds 11-15: posterior-based rerun-agreement
    // predictions ran 54-69% while measured cross-run agreement held at
    // 74%, and empirical rerun sigma was ~0.3x posterior sigma — the
    // consistency surface needs this split to be honest).
    let evidence_obs_sigma_rms = evidence_sigma_w.and_then(|sigma_w| {
        let (sum_var, n) = observation_log
            .values()
            .flatten()
            .filter_map(|ob| ob.precision)
            .filter(|p| p.is_finite() && *p > 0.0)
            .fold((0.0f64, 0usize), |(s, n), p| (s + 1.0 / p, n + 1));
        (n > 0).then(|| (sum_var / n as f64 + sigma_w * sigma_w).sqrt())
    });
    if let Some(sigma_w) = evidence_sigma_w {
        for (attribute_id, observations) in &observation_log {
            if !observations.iter().any(|ob| ob.precision.is_some()) {
                continue; // pure point-path attribute: nothing to widen
            }
            let inflated: Vec<Observation> = observations
                .iter()
                .map(|ob| match ob.precision {
                    Some(p) if p.is_finite() && p > 0.0 => {
                        let mut ob = ob.clone();
                        ob.precision = Some(1.0 / (1.0 / p + sigma_w * sigma_w));
                        ob
                    }
                    _ => ob.clone(),
                })
                .collect();
            manager
                .reingest(attribute_id, &inflated)
                .map_err(|e| MultiRerankError::RatingEngine(e.to_string()))?;
        }
    }

    build_response(
        &req,
        &mut manager,
        ResponseContext {
            topk_cfg: &topk_cfg,
            comparisons_attempted,
            comparisons_failed,
            first_error,
            comparisons_used,
            comparisons_refused,
            comparisons_cached,
            comparison_budget,
            start_time,
            base_model,
            rater_id,
            engine_spec,
            warm_start_observations,
            provider_input_tokens,
            provider_output_tokens,
            provider_cost_nanodollars,
            provider_cost_is_estimate,
            models_used,
            pairs_counterbalanced,
            position_flips,
            evidence_judgements,
            logprob_mode_judgements,
            visible_mass_sum,
            evidence_order_residual_sum_abs,
            evidence_order_residual_pairs,
            evidence_sigma_w,
            evidence_obs_sigma_rms,
            stop_reason,
        },
    )
}
