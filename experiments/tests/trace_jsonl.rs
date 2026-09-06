use llmsort::rating_engine::Observation;
use llmsort::{ComparisonTrace, JsonlTraceSink, TraceSink};
use tempfile::tempdir;

fn make_trace(comparison_index: usize) -> ComparisonTrace {
    ComparisonTrace {
        evidence_moments: None,
        timestamp_ms: 0,
        comparison_index,
        attribute_id: "clarity".to_string(),
        attribute_index: 0,
        attribute_prompt_hash: "attr_hash".to_string(),
        prompt_template_slug: "canonical_v2".to_string(),
        template_hash: "template_hash".to_string(),
        rendered_prompt_digest: "rendered_digest".to_string(),
        engine_spec_id: "engine_spec".to_string(),
        entity_a_id: "a".to_string(),
        entity_b_id: "b".to_string(),
        entity_a_index: 0,
        entity_b_index: 1,
        entity_a_hash: "a_hash".to_string(),
        entity_b_hash: "b_hash".to_string(),
        cache_key_hash: "key_hash".to_string(),
        model: "openai/gpt-5-mini".to_string(),
        served_model: None,
        higher_ranked: Some("A".to_string()),
        ratio: Some(2.0),
        confidence: Some(0.9),
        solver_observation: (comparison_index == 1).then_some(Observation {
            i: usize::MAX - 1,
            j: usize::MAX,
            ratio: f64::from_bits(1.0f64.to_bits() + 1),
            confidence: -0.0,
            rater_id: "rater/µ".to_string(),
            reps: f64::MAX,
            precision: Some(f64::from_bits(1)),
        }),
        pairwise_logprob_posterior: None,
        output_logprob_token_count: None,
        pairwise_logprob_posterior_error: None,
        ledger_draws: None,
        refused: false,
        cached: true,
        swapped: false,
        input_tokens: 0,
        output_tokens: 0,
        provider_cost_nanodollars: 0,
        provider_cost_is_estimate: false,
        error: None,
    }
}

#[test]
fn jsonl_trace_sink_writes_events_and_flushes_on_join() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("trace.jsonl");

    let (sink, worker) = JsonlTraceSink::new(&path).unwrap();
    sink.record(make_trace(1)).unwrap();
    sink.record(make_trace(2)).unwrap();

    drop(sink);
    worker.join().unwrap();

    let raw = std::fs::read_to_string(&path).unwrap();
    let lines: Vec<&str> = raw.lines().collect();
    assert_eq!(lines.len(), 2);

    let first: ComparisonTrace = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(first.comparison_index, 1);
    assert_eq!(first.engine_spec_id, "engine_spec");
    let observation = first.solver_observation.unwrap();
    assert_eq!(observation.i, usize::MAX - 1);
    assert_eq!(observation.j, usize::MAX);
    assert_eq!(observation.rater_id, "rater/µ");
    assert_eq!(
        observation.ratio.to_bits(),
        f64::from_bits(1.0f64.to_bits() + 1).to_bits()
    );
    assert_eq!(observation.confidence.to_bits(), (-0.0f64).to_bits());
    assert_eq!(observation.reps.to_bits(), f64::MAX.to_bits());
    assert_eq!(
        observation.precision.map(f64::to_bits),
        Some(f64::from_bits(1).to_bits())
    );

    let second: ComparisonTrace = serde_json::from_str(lines[1]).unwrap();
    assert!(second.solver_observation.is_none());
}
