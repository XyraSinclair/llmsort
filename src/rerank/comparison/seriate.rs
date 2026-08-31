use super::types::evidence_instrument_for_slug;
use super::*;
use crate::gateway::Message;
use crate::seriate::instrument::ratio_letter;

/// The seriate ratio-letter path: one single-token call whose answer-position
/// top-k logprobs are parsed into a judgement PMF. The point judgement
/// (direction, ratio, confidence) is DERIVED from the PMF so every existing
/// surface (traces, counterbalance flip stats, cache) keeps working, while
/// the PMF moments ride in `ComparisonUsage::evidence_moments` and enter the
/// solver with measured variance.
pub(super) async fn compare_pair_seriate(
    gateway: &dyn ChatGateway,
    cache: Option<&dyn PairwiseCache>,
    request: PairwiseComparisonRequest<'_>,
) -> Result<(PairwiseJudgement, ComparisonUsage), ComparisonError> {
    // A draw is deliberately fresh: the nonce bypasses the pairwise SQLite
    // cache in both directions. The provider PREFIX cache stays warm by
    // construction — the nonce line lands after every stable byte, and
    // `prompt_cache_key` below is derived from stable content only.
    let cache = if request.nonce.is_some() { None } else { cache };
    let mut rendered = request.spec.prompt_instance();
    if let Some(nonce) = &request.nonce {
        rendered.push_draw_token(nonce);
    }
    let rendered_prompt_digest = rendered.rendered_digest();
    let cache_key = cache.map(|_| request.spec.cache_key());
    if let (Some(cache), Some(ref key)) = (cache, &cache_key) {
        match cache.get(key).await {
            Ok(Some(hit)) => {
                if let Some(judgement) = cached_to_judgement(&hit, LADDER_RATIO_CAP) {
                    let usage = ComparisonUsage {
                        input_tokens: 0,
                        cache_read_tokens: None,
                        output_tokens: 0,
                        provider_cost_nanodollars: 0,
                        provider_cost_is_estimate: false,
                        cached: true,
                        served_model: None,
                        prompt_text: None,
                        rendered_prompt_digest: rendered_prompt_digest.clone(),
                        question_text: None,
                        raw_output: None,
                        output_logprobs: None,
                        pairwise_logprob_posterior: None,
                        evidence_moments: evidence_moments_from_cached(&hit),
                        ledger_draws: None,
                    };
                    return Ok((judgement, usage));
                }
            }
            Ok(None) => {}
            Err(err) => {
                if request.cache_only {
                    return Err(ComparisonError::Cache(err));
                }
                warn!(error = %err, "Cache read failed; falling back to live comparison");
            }
        }
    }
    if request.cache_only {
        return Err(ComparisonError::CacheMiss(
            "cache_only is enabled and no cached judgement was found".to_string(),
        ));
    }

    // Key on the shared prefix (see prompt_cache_key_from_parts): the
    // attribute-first template shares (system + attribute) across pairs;
    // the attr-last template shares (system + entity pair) across
    // attribute variants — the family sweep's cache economics (NORTH E10).
    let prompt_cache_key = if rendered.template_slug == RATIO_LETTER_ATTR_LAST_SLUG {
        super::super::prompt_cache_key_from_parts(
            &rendered.template_slug,
            &[request.spec.entity_a.text, request.spec.entity_b.text],
        )
    } else {
        super::super::prompt_cache_key_from_parts(
            &rendered.template_slug,
            &[request.spec.attribute.prompt],
        )
    };
    let messages = rendered.to_messages();
    let mut base_request = ChatRequest::new(
        ChatModel::parse(request.spec.model),
        messages,
        request.attribution.clone(),
    )
    // Single-letter answer; 16 is the observed provider floor (OpenAI
    // responses path rejects smaller — logprob reality map, 2026-07-04).
    .max_tokens(16);
    base_request.prompt_cache_key = Some(prompt_cache_key);
    // Route by the measured logprob matrix: clamp to the model's alternative
    // cap (over-cap on OpenRouter is a silent 200 with logprobs:null, not an
    // error) and pin reasoning off where the route requires it. Unknown
    // models keep the optimistic 20 and rely on the loud degradation below.
    let route = super::types::seriate_logprob_route(request.spec.model);

    let mut input_tokens_total = 0u32;
    let mut cache_read_tokens_total: Option<u32> = None;
    let mut output_tokens_total = 0u32;
    let mut provider_cost_total = 0i64;
    let mut provider_cost_is_estimate = false;
    let mut prompt_text = format!("{}\n---\n{}", rendered.system, rendered.user);

    // Two-phase protocol: a written analysis turn (verdict forbidden — a
    // visible verdict collapses the answer PMF, docs/LOGPROBS.md), appended
    // as an assistant message so the verdict call reads a one-token PMF
    // with the judge's own analysis in context. Same system + user bytes,
    // so the provider prefix cache serves the re-sent turn-1 prompt warm.
    if rendered.template_slug == RATIO_LETTER_2P_SLUG {
        // The analysis turn runs with reasoning DISABLED: the verdict's
        // structure comes from the written, verdict-forbidden analysis
        // text, and hidden reasoning on top of it is a measured noise
        // source, not a quality source (sigma-eps-knobs pack, 2026-08-31:
        // terra sigma_eps 0.260 -> 0.181 nats/call and every sort axis
        // improved across 3 paired seeds — order residual 0.238 -> 0.141,
        // flips 14/48 -> 6/48, -13% cost; luna unchanged within noise).
        let analysis_request = base_request
            .clone()
            .max_tokens(super::types::TWO_PHASE_ANALYSIS_MAX_TOKENS)
            .reasoning(ReasoningConfig::disabled());
        let analysis_response = gateway.chat(analysis_request).await?;
        input_tokens_total = input_tokens_total.saturating_add(analysis_response.input_tokens);
        cache_read_tokens_total = analysis_response.cache_read_tokens;
        output_tokens_total = output_tokens_total.saturating_add(analysis_response.output_tokens);
        provider_cost_total =
            provider_cost_total.saturating_add(analysis_response.cost_nanodollars);
        provider_cost_is_estimate |= analysis_response.cost_is_estimate;
        let analysis = analysis_response.content.trim().to_string();
        if analysis.is_empty() {
            warn!(
                model = request.spec.model,
                "two-phase analysis came back empty; degrading to single-phase verdict"
            );
        } else {
            prompt_text = format!(
                "{prompt_text}\n---[analysis]\n{analysis}\n---\n{}",
                ratio_letter::TWO_PHASE_ANSWER_PROMPT
            );
            base_request.messages.push(Message::assistant(analysis));
            base_request
                .messages
                .push(Message::user(ratio_letter::TWO_PHASE_ANSWER_PROMPT));
        }
    }

    if route.is_some_and(|r| r.pin_reasoning_off) {
        base_request.reasoning = Some(ReasoningConfig::disabled());
    }

    // Attempt 1: with logprobs. If the provider rejects the PARAMETER
    // (reasoning-class models 400 on it), degrade loudly to a sampled call.
    let with_logprobs = base_request
        .clone()
        .with_logprobs(route.map_or(PAIRWISE_LOGPROBS_TOP_N_DEFAULT, |r| r.top_n));
    let response = match gateway.chat(with_logprobs).await {
        Ok(response) => response,
        Err(err) if format!("{err}").to_ascii_lowercase().contains("logprob") => {
            warn!(model = request.spec.model, error = %err,
                "provider rejects logprobs; degrading to sampled mode");
            gateway.chat(base_request.clone()).await?
        }
        Err(err) => return Err(err.into()),
    };
    input_tokens_total = input_tokens_total.saturating_add(response.input_tokens);
    output_tokens_total = output_tokens_total.saturating_add(response.output_tokens);
    provider_cost_total = provider_cost_total.saturating_add(response.cost_nanodollars);
    provider_cost_is_estimate |= response.cost_is_estimate;

    if let Some(read) = response.cache_read_tokens {
        cache_read_tokens_total = Some(cache_read_tokens_total.unwrap_or(0).saturating_add(read));
    }
    let mut usage = ComparisonUsage {
        input_tokens: input_tokens_total,
        cache_read_tokens: cache_read_tokens_total,
        output_tokens: output_tokens_total,
        provider_cost_nanodollars: provider_cost_total,
        provider_cost_is_estimate,
        cached: false,
        served_model: response.served_model.clone(),
        prompt_text: Some(prompt_text),
        rendered_prompt_digest,
        question_text: Some(request.spec.attribute.prompt.to_string()),
        raw_output: Some(response.content.clone()),
        output_logprobs: fallback_stored_logprobs(response.output_logprobs.as_deref()),
        pairwise_logprob_posterior: None,
        evidence_moments: None,
        ledger_draws: None,
    };

    let parsed = match evidence_instrument_for_slug(&rendered.template_slug)
        .parse(&response.content, response.output_logprobs.as_deref())
    {
        Ok(parsed) => parsed,
        Err(err) => {
            warn!(error = %err, "ratio-letter parse failed; treating as refusal");
            let judgement = PairwiseJudgement::Refused;
            if let (Some(cache), Some(ref key)) = (cache, &cache_key) {
                let entry = judgement_to_cached(&judgement, &usage);
                let _ = cache.put(key, &entry).await;
            }
            return Ok((judgement, usage));
        }
    };

    if parsed.health.refused {
        let judgement = PairwiseJudgement::Refused;
        if let (Some(cache), Some(ref key)) = (cache, &cache_key) {
            let entry = judgement_to_cached(&judgement, &usage);
            let _ = cache.put(key, &entry).await;
        }
        return Ok((judgement, usage));
    }

    let Some((mean, var)) = parsed.evidence.log_ratio_moments() else {
        warn!("ratio-letter evidence has no informative mass; treating as refusal");
        let judgement = PairwiseJudgement::Refused;
        if let (Some(cache), Some(ref key)) = (cache, &cache_key) {
            let entry = judgement_to_cached(&judgement, &usage);
            let _ = cache.put(key, &entry).await;
        }
        return Ok((judgement, usage));
    };

    usage.evidence_moments = Some(EvidenceMoments {
        log_ratio_mean: mean,
        log_ratio_var: var,
        visible_mass: parsed.health.visible_mass,
        logprob_mode: parsed.mode == crate::seriate::AcquisitionMode::Logprob,
        e_lo: None,
        e_hi: None,
        conservation_gap: None,
    });

    // Point summary DERIVED from the PMF, for every point-shaped surface.
    let (p_a, _parity, p_b) = parsed
        .evidence
        .directional_summary()
        .unwrap_or((0.5, 0.0, 0.5));
    let higher_ranked = if mean >= 0.0 {
        HigherRanked::A
    } else {
        HigherRanked::B
    };
    let confidence = if mean >= 0.0 { p_a } else { p_b }.clamp(0.0, 1.0);
    let ratio = mean.abs().exp().clamp(1.0, 26.0);
    let judgement = PairwiseJudgement::Observation {
        higher_ranked,
        ratio,
        confidence,
    };

    if let (Some(cache), Some(ref key)) = (cache, &cache_key) {
        let entry = judgement_to_cached(&judgement, &usage);
        let _ = cache.put(key, &entry).await;
    }
    Ok((judgement, usage))
}
