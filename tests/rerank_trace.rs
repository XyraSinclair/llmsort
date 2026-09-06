use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use llmsort::cache::{CachedJudgement, PairwiseCacheKey, SqlitePairwiseCache};
use llmsort::gateway::openrouter::OpenRouterAdapter;
use llmsort::gateway::{GatewayConfig, NoopUsageSink, ProviderGateway};
use llmsort::prompts::prompt_by_slug;
use llmsort::rating_engine::{Observation, RatingEngine};
use llmsort::rerank::{
    multi_rerank, MultiRerankAttributeSpec, MultiRerankEntity, MultiRerankMeta, MultiRerankRequest,
    MultiRerankTopKSpec, RerankExecution, RerankRunOptions, WarmStartData, WarmStartError,
    WarmStartProvider,
};
use llmsort::{
    Attribution, ComparisonEvent, ComparisonObserver, ComparisonTrace, ObserverError,
    PairwiseCache, TraceError, TraceSink,
};
use tempfile::tempdir;

#[derive(Default)]
struct VecTraceSink {
    events: Mutex<Vec<ComparisonTrace>>,
}

impl VecTraceSink {
    fn take(&self) -> Vec<ComparisonTrace> {
        std::mem::take(
            &mut self
                .events
                .lock()
                .expect("trace sink events mutex poisoned"),
        )
    }
}

impl TraceSink for VecTraceSink {
    fn record(&self, event: ComparisonTrace) -> Result<(), TraceError> {
        self.events
            .lock()
            .expect("trace sink events mutex poisoned")
            .push(event);
        Ok(())
    }
}

type ReplayPosterior = HashMap<String, (Vec<f64>, Vec<f64>)>;

fn replay_trace(
    meta: &MultiRerankMeta,
    rows: &[ComparisonTrace],
) -> Result<ReplayPosterior, String> {
    if meta.warm_start_observations > 0 {
        return Err("warm-start observations are not represented by trace rows".into());
    }
    if rows.len() != meta.comparisons_attempted {
        return Err("trace row count does not match rerank metadata".into());
    }
    if rows
        .windows(2)
        .any(|pair| pair[0].comparison_index >= pair[1].comparison_index)
    {
        return Err("trace rows are not in original comparison order".into());
    }
    let spec = meta
        .engine_spec
        .as_ref()
        .ok_or_else(|| "missing engine spec".to_string())?;
    let expected_id = spec.id().0;
    if rows.iter().any(|row| row.engine_spec_id != expected_id) {
        return Err("trace row engine spec mismatch".into());
    }

    let raters: HashMap<_, _> = spec.raters.iter().cloned().collect();
    if raters.len() != spec.raters.len() {
        return Err("duplicate rater id in engine spec".into());
    }

    let mut engines = HashMap::<String, RatingEngine>::new();
    let mut accepted_observations = 0usize;
    for row in rows {
        let engine = match engines.entry(row.attribute_id.clone()) {
            std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::hash_map::Entry::Vacant(entry) => entry.insert(
                RatingEngine::new(
                    spec.n,
                    spec.attribute.clone(),
                    raters.clone(),
                    Some(spec.config.clone()),
                )
                .map_err(str::to_owned)?,
            ),
        };
        match row.solver_observation.as_ref() {
            Some(observation) => {
                if row.refused || row.error.is_some() || row.higher_ranked.is_none() {
                    return Err("contradictory accepted trace row".into());
                }
                accepted_observations += 1;
                engine.add_observations(std::slice::from_ref(observation));
            }
            None if !row.refused && row.error.is_none() && row.higher_ranked.is_some() => {
                return Err("accepted trace row is missing solver observation".into());
            }
            None => {}
        }
    }
    if accepted_observations != meta.comparisons_used {
        return Err("accepted observation count does not match rerank metadata".into());
    }

    let mut replayed = HashMap::new();
    for (attribute_id, mut engine) in engines {
        let solved = engine.solve();
        replayed.insert(attribute_id, (solved.scores, solved.diag_cov));
    }
    Ok(replayed)
}

fn assert_live_matches_replay(
    req: &MultiRerankRequest,
    response: &llmsort::rerank::MultiRerankResponse,
    replayed: &ReplayPosterior,
) {
    for (entity_index, requested) in req.entities.iter().enumerate() {
        let live = response
            .entities
            .iter()
            .find(|entity| entity.id == requested.id)
            .expect("response must retain every request entity id");
        for attribute in &req.attributes {
            let live_score = &live.attribute_scores[&attribute.id];
            let (means, variances) = &replayed[&attribute.id];
            assert_eq!(
                live_score.latent_mean.to_bits(),
                means[entity_index].to_bits(),
                "latent mean differs for entity {} attribute {}",
                requested.id,
                attribute.id
            );
            assert_eq!(
                live_score.latent_std.to_bits(),
                variances[entity_index].max(0.0).sqrt().to_bits(),
                "latent std differs for entity {} attribute {}",
                requested.id,
                attribute.id
            );
        }
    }
}

#[derive(Clone)]
struct OneObservationWarmStart;

#[async_trait::async_trait]
impl WarmStartProvider for OneObservationWarmStart {
    async fn warm_start(
        &self,
        _req: &MultiRerankRequest,
        rater_id: &str,
    ) -> Result<WarmStartData, WarmStartError> {
        Ok(WarmStartData {
            observations_by_attribute: HashMap::from([(
                "clarity".to_string(),
                vec![Observation::new(0, 1, 2.0, 0.9, rater_id, 1.0)],
            )]),
        })
    }
}

#[derive(Default)]
struct VecObserver {
    events: Mutex<Vec<ComparisonEvent>>,
}

impl VecObserver {
    fn take(&self) -> Vec<ComparisonEvent> {
        std::mem::take(&mut self.events.lock().expect("observer events mutex poisoned"))
    }
}

#[async_trait::async_trait]
impl ComparisonObserver for VecObserver {
    async fn on_comparison(&self, event: ComparisonEvent) -> Result<(), ObserverError> {
        self.events
            .lock()
            .expect("observer events mutex poisoned")
            .push(event);
        Ok(())
    }
}

fn test_gateway() -> ProviderGateway<NoopUsageSink> {
    let openrouter = OpenRouterAdapter::new("test").unwrap();
    ProviderGateway::with_config(
        openrouter,
        std::sync::Arc::new(NoopUsageSink),
        GatewayConfig {
            max_retries: 0,
            retry_base_delay: Duration::from_millis(0),
        },
    )
}

fn canonical_v2_template_hash() -> String {
    let template = prompt_by_slug("canonical_v2").unwrap();
    template.template_hash()
}

fn make_request(model: &str) -> MultiRerankRequest {
    MultiRerankRequest {
        nonce_draws: None,
        entities: vec![
            MultiRerankEntity {
                id: "a".into(),
                text: "Entity A text".into(),
            },
            MultiRerankEntity {
                id: "b".into(),
                text: "Entity B text".into(),
            },
        ],
        attributes: vec![MultiRerankAttributeSpec {
            id: "clarity".into(),
            prompt: "clarity of explanation".into(),
            prompt_template_slug: Some("canonical_v2".into()),
            weight: 1.0,
        }],
        topk: MultiRerankTopKSpec {
            k: 1,
            weight_exponent: 1.0,
            tolerated_error: 0.0,
            band_size: 5,
            effective_resistance_max_active: 64,
            stop_sigma_inflate: 1.25,
            stop_min_consecutive: 1,
            min_explore_degree: 2,
            prune_p_topk_below: None,
        },
        gates: vec![],
        comparison_budget: Some(1),
        latency_budget_ms: None,
        max_cost_nanodollars: None,
        model: Some(model.into()),
        rater_id: None,
        comparison_concurrency: Some(1),
        max_pair_repeats: Some(1),
        randomize_presentation_order: true,
        counterbalance_pairs: false,
    }
}

#[tokio::test]
async fn rerank_records_trace_for_cached_comparison() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("cache.sqlite");
    let cache = SqlitePairwiseCache::new(&db_path).unwrap();

    let model = "openai/gpt-5-mini";
    let prompt_slug = "canonical_v2";
    let template_hash = canonical_v2_template_hash();

    // Pre-populate both A/B orders so we get a cache hit regardless of proposal ordering.
    let key_ab = PairwiseCacheKey::new(
        model,
        prompt_slug,
        &template_hash,
        "clarity",
        "clarity of explanation",
        "a",
        "Entity A text",
        "b",
        "Entity B text",
    );
    let key_ba = PairwiseCacheKey::new(
        model,
        prompt_slug,
        &template_hash,
        "clarity",
        "clarity of explanation",
        "b",
        "Entity B text",
        "a",
        "Entity A text",
    );

    // Entity "b" wins so response rank order differs from request index order.
    let value_ab = CachedJudgement {
        higher_ranked: Some("B".to_string()),
        ratio: Some(2.0),
        confidence: Some(0.9),
        refused: false,
        input_tokens: None,
        output_tokens: None,
        provider_cost_nanodollars: None,
        log_ratio_mean: None,
        log_ratio_var: None,
        visible_mass: None,
        logprob_mode: None,
        e_lo: None,
        e_hi: None,
        conservation_gap: None,
    };
    let value_ba = CachedJudgement {
        higher_ranked: Some("A".to_string()),
        ratio: Some(2.0),
        confidence: Some(0.9),
        refused: false,
        input_tokens: None,
        output_tokens: None,
        provider_cost_nanodollars: None,
        log_ratio_mean: None,
        log_ratio_var: None,
        visible_mass: None,
        logprob_mode: None,
        e_lo: None,
        e_hi: None,
        conservation_gap: None,
    };

    cache.put(&key_ab, &value_ab).await.unwrap();
    cache.put(&key_ba, &value_ba).await.unwrap();

    let gateway = test_gateway();
    let run_options = RerankRunOptions {
        rng_seed: Some(0),
        cache_only: true,
    };
    let req = make_request(model);

    let trace_sink = VecTraceSink::default();
    let resp = multi_rerank(
        req.clone(),
        RerankExecution::new(
            std::sync::Arc::new(gateway),
            Attribution::new("test::rerank_trace_cached"),
        )
        .cache(&cache)
        .run_options(run_options)
        .trace(&trace_sink),
    )
    .await
    .unwrap();
    assert_eq!(resp.meta.comparisons_attempted, 1);

    let events = trace_sink.take();
    assert_eq!(events.len(), 1);
    let event = &events[0];

    assert!(event.cached);
    assert_eq!(event.input_tokens, 0);
    assert_eq!(event.output_tokens, 0);
    assert_eq!(event.provider_cost_nanodollars, 0);
    assert!(event.error.is_none());
    assert!(event.higher_ranked.is_some());
    assert_eq!(event.ratio, Some(2.0));
    assert_eq!(event.confidence, Some(0.9));

    let entities: HashMap<&str, &str> = req
        .entities
        .iter()
        .map(|e| (e.id.as_str(), e.text.as_str()))
        .collect();

    let expected_key = PairwiseCacheKey::new(
        &event.model,
        &event.prompt_template_slug,
        &event.template_hash,
        &event.attribute_id,
        "clarity of explanation",
        &event.entity_a_id,
        entities[event.entity_a_id.as_str()],
        &event.entity_b_id,
        entities[event.entity_b_id.as_str()],
    );
    assert_eq!(event.cache_key_hash, expected_key.key_hash);
    assert_eq!(
        event.attribute_prompt_hash,
        expected_key.attribute_prompt_hash
    );
    let spec_id = resp
        .meta
        .engine_spec
        .as_ref()
        .expect("live rerank must expose its engine spec")
        .id()
        .0;
    assert_eq!(event.engine_spec_id, spec_id);
    assert!(event.solver_observation.is_some());
    let replayed = replay_trace(&resp.meta, &events).expect("cached point trace must replay");
    assert_live_matches_replay(&req, &resp, &replayed);

    let mut mismatched = events.clone();
    mismatched[0].engine_spec_id = "sha256:wrong-engine-spec".into();
    assert!(replay_trace(&resp.meta, &mismatched).is_err());

    let mut missing_observation = events.clone();
    missing_observation[0].solver_observation = None;
    assert!(replay_trace(&resp.meta, &missing_observation).is_err());
    assert!(replay_trace(&resp.meta, &[]).is_err());

    let mut doubled_meta_json = serde_json::to_value(&resp.meta).unwrap();
    let doubled_meta_object = doubled_meta_json.as_object_mut().unwrap();
    doubled_meta_object.insert("comparisons_attempted".into(), serde_json::json!(2));
    doubled_meta_object.insert("comparisons_used".into(), serde_json::json!(2));
    let doubled_meta: MultiRerankMeta = serde_json::from_value(doubled_meta_json).unwrap();
    let mut out_of_order = vec![events[0].clone(), events[0].clone()];
    out_of_order[0].comparison_index = 2;
    out_of_order[1].comparison_index = 1;
    assert!(replay_trace(&doubled_meta, &out_of_order).is_err());

    let mut legacy_meta_json = serde_json::to_value(&resp.meta).unwrap();
    let legacy_meta_object = legacy_meta_json.as_object_mut().unwrap();
    legacy_meta_object.remove("engine_spec");
    legacy_meta_object.remove("warm_start_observations");
    let legacy_meta: MultiRerankMeta = serde_json::from_value(legacy_meta_json).unwrap();
    assert!(legacy_meta.engine_spec.is_none());
    assert_eq!(legacy_meta.warm_start_observations, 0);

    let mut legacy_rows_json = serde_json::to_value(&events).unwrap();
    for row in legacy_rows_json.as_array_mut().unwrap() {
        let row = row.as_object_mut().unwrap();
        row.remove("engine_spec_id");
        row.remove("solver_observation");
    }
    let legacy_rows: Vec<ComparisonTrace> = serde_json::from_value(legacy_rows_json).unwrap();
    assert!(legacy_rows
        .iter()
        .all(|row| row.engine_spec_id.is_empty() && row.solver_observation.is_none()));
    assert!(replay_trace(&legacy_meta, &legacy_rows).is_err());

    let warm_start = OneObservationWarmStart;
    let warm_trace = VecTraceSink::default();
    let warm_response = multi_rerank(
        req.clone(),
        RerankExecution::new(
            std::sync::Arc::new(test_gateway()),
            Attribution::new("test::rerank_trace_warm_start"),
        )
        .cache(&cache)
        .warm_start(&warm_start)
        .run_options(RerankRunOptions {
            rng_seed: Some(0),
            cache_only: true,
        })
        .trace(&warm_trace),
    )
    .await
    .unwrap();
    assert_eq!(warm_response.meta.warm_start_observations, 1);
    assert!(replay_trace(&warm_response.meta, &warm_trace.take()).is_err());
}

#[tokio::test]
async fn cached_evidence_replays_bitwise_and_naive_confidence_reconstruction_diverges() {
    let dir = tempdir().unwrap();
    let cache = SqlitePairwiseCache::new(dir.path().join("cache.sqlite")).unwrap();
    let model = "openai/gpt-5-mini";
    let prompt_slug = "ratio_letter_v1";
    let template_hash = llmsort::seriate::instrument::Instrument::render(
        &llmsort::seriate::instrument::ratio_letter::RatioLetterInstrument,
        &llmsort::seriate::Attribute::new("fingerprint", "fingerprint"),
        &llmsort::seriate::Entity::new("A"),
        &llmsort::seriate::Entity::new("B"),
    )
    .template
    .0
     .0;
    let mean = 4.0f64.ln();

    let key_ab = PairwiseCacheKey::new(
        model,
        prompt_slug,
        &template_hash,
        "clarity",
        "clarity of explanation",
        "a",
        "Entity A text",
        "b",
        "Entity B text",
    );
    let key_ba = PairwiseCacheKey::new(
        model,
        prompt_slug,
        &template_hash,
        "clarity",
        "clarity of explanation",
        "b",
        "Entity B text",
        "a",
        "Entity A text",
    );
    let evidence = |higher_ranked: &str, log_ratio_mean: f64| CachedJudgement {
        higher_ranked: Some(higher_ranked.to_string()),
        ratio: Some(4.0),
        confidence: Some(0.2),
        refused: false,
        input_tokens: None,
        output_tokens: None,
        provider_cost_nanodollars: None,
        log_ratio_mean: Some(log_ratio_mean),
        log_ratio_var: Some(0.0),
        visible_mass: Some(1.0),
        logprob_mode: Some(true),
        e_lo: None,
        e_hi: None,
        conservation_gap: None,
    };
    cache.put(&key_ab, &evidence("A", mean)).await.unwrap();
    cache.put(&key_ba, &evidence("B", -mean)).await.unwrap();

    let mut req = make_request(model);
    req.attributes[0].prompt_template_slug = Some(prompt_slug.into());
    let trace_sink = VecTraceSink::default();
    let response = multi_rerank(
        req.clone(),
        RerankExecution::new(
            std::sync::Arc::new(test_gateway()),
            Attribution::new("test::rerank_trace_cached_evidence"),
        )
        .cache(&cache)
        .run_options(RerankRunOptions {
            rng_seed: Some(0),
            cache_only: true,
        })
        .trace(&trace_sink),
    )
    .await
    .unwrap();
    let rows = trace_sink.take();
    assert_eq!(rows.len(), 1);
    let accepted = &rows[0];
    assert!(accepted.cached);
    let observation = accepted
        .solver_observation
        .as_ref()
        .expect("cached PMF evidence must retain the accepted solver input");
    assert_eq!(
        observation.precision.map(f64::to_bits),
        Some(1000.0f64.to_bits())
    );
    assert_eq!(
        accepted.engine_spec_id,
        response.meta.engine_spec.as_ref().unwrap().id().0
    );

    let exact = replay_trace(&response.meta, &rows).expect("exact evidence trace must replay");
    assert_live_matches_replay(&req, &response, &exact);

    let mut laundered_rows = rows.clone();
    let laundered = laundered_rows[0].solver_observation.as_mut().unwrap();
    *laundered = Observation::new(
        laundered.i,
        laundered.j,
        rows[0].ratio.unwrap(),
        rows[0].confidence.unwrap(),
        laundered.rater_id.clone(),
        laundered.reps,
    );
    let naive = replay_trace(&response.meta, &laundered_rows)
        .expect("the planted negative control is structurally replayable");
    let exact = &exact["clarity"];
    let naive = &naive["clarity"];
    assert!(
        exact
            .0
            .iter()
            .zip(&naive.0)
            .any(|(left, right)| left.to_bits() != right.to_bits())
            || exact
                .1
                .iter()
                .zip(&naive.1)
                .any(|(left, right)| left.to_bits() != right.to_bits()),
        "reconstructing PMF evidence from raw ratio/confidence must not pass the replay oracle"
    );
}

#[tokio::test]
async fn seeded_swapped_trace_matches_presented_cache_key() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("cache.sqlite");
    let cache = SqlitePairwiseCache::new(&db_path).unwrap();

    let model = "openai/gpt-5-mini";
    let prompt_slug = "canonical_v2";
    let template_hash = canonical_v2_template_hash();
    let key_ba = PairwiseCacheKey::new(
        model,
        prompt_slug,
        &template_hash,
        "clarity",
        "clarity of explanation",
        "b",
        "Entity B text",
        "a",
        "Entity A text",
    );
    let value_ba = CachedJudgement {
        higher_ranked: Some("B".to_string()),
        ratio: Some(2.0),
        confidence: Some(0.9),
        refused: false,
        input_tokens: None,
        output_tokens: None,
        provider_cost_nanodollars: None,
        log_ratio_mean: None,
        log_ratio_var: None,
        visible_mass: None,
        logprob_mode: None,
        e_lo: None,
        e_hi: None,
        conservation_gap: None,
    };
    cache.put(&key_ba, &value_ba).await.unwrap();

    let mut observed_swapped_hit = false;
    for seed in 0..128 {
        let gateway = test_gateway();
        let observer_sink = VecObserver::default();
        let trace_sink = VecTraceSink::default();
        let result = multi_rerank(
            make_request(model),
            RerankExecution::new(
                std::sync::Arc::new(gateway),
                Attribution::new("test::rerank_trace_swapped"),
            )
            .cache(&cache)
            .run_options(RerankRunOptions {
                rng_seed: Some(seed),
                cache_only: true,
            })
            .trace(&trace_sink)
            .observer(&observer_sink),
        )
        .await;

        match result {
            Ok(resp) => {
                assert_eq!(resp.meta.comparisons_attempted, 1);
                let events = trace_sink.take();
                assert_eq!(events.len(), 1);
                let event = &events[0];
                assert!(event.cached);
                assert!(event.swapped);
                assert_eq!(event.entity_a_id, "b");
                assert_eq!(event.entity_b_id, "a");
                assert_eq!(event.entity_a_index, 1);
                assert_eq!(event.entity_b_index, 0);
                assert_eq!(event.higher_ranked.as_deref(), Some("B"));
                assert_eq!(event.cache_key_hash, key_ba.key_hash);
                let observer_events = observer_sink.take();
                assert_eq!(observer_events.len(), 1);
                let observer_event = &observer_events[0];
                assert_eq!(observer_event.entity_a_id, event.entity_a_id);
                assert_eq!(observer_event.entity_b_id, event.entity_b_id);
                assert_eq!(observer_event.entity_a_index, event.entity_a_index);
                assert_eq!(observer_event.entity_b_index, event.entity_b_index);
                observed_swapped_hit = true;
                break;
            }
            Err(err) => {
                assert!(
                    err.to_string().contains("cache_only")
                        || err.to_string().contains("Cache miss"),
                    "unexpected error for unswapped seed {seed}: {err}"
                );
            }
        }
    }

    assert!(
        observed_swapped_hit,
        "expected at least one deterministic seed to exercise swapped presentation"
    );
}

#[tokio::test]
async fn rerank_records_trace_on_cache_miss_in_cache_only_mode() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("cache.sqlite");
    let cache = SqlitePairwiseCache::new(&db_path).unwrap();

    let model = "openai/gpt-5-mini";
    let gateway = test_gateway();
    let run_options = RerankRunOptions {
        rng_seed: Some(0),
        cache_only: true,
    };
    let req = make_request(model);

    let trace_sink = VecTraceSink::default();
    let err = multi_rerank(
        req,
        RerankExecution::new(
            std::sync::Arc::new(gateway),
            Attribution::new("test::rerank_trace_cache_miss"),
        )
        .cache(&cache)
        .run_options(run_options)
        .trace(&trace_sink),
    )
    .await
    .unwrap_err();
    assert!(
        err.to_string().contains("cache_only") || err.to_string().contains("Cache miss"),
        "unexpected error: {err}"
    );

    let events = trace_sink.take();
    assert_eq!(events.len(), 1);
    let event = &events[0];
    assert!(!event.cached);
    assert!(event.error.as_deref().unwrap_or("").contains("cache_only"));
}
