use super::*;

/// Perform a pairwise comparison using the LLM.
pub async fn compare_pair(
    gateway: &dyn ChatGateway,
    cache: Option<&dyn PairwiseCache>,
    request: PairwiseComparisonRequest<'_>,
) -> Result<(PairwiseJudgement, ComparisonUsage), ComparisonError> {
    if request
        .spec
        .attribute
        .prompt_template_slug
        .is_some_and(is_evidence_slug)
    {
        return compare_pair_seriate(gateway, cache, request).await;
    }
    if request.spec.attribute.prompt_template_slug == Some(DECIMAL_LEDGER_SLUG) {
        if request.nonce.is_some() {
            return Err(ComparisonError::Parse(
                "nonce draws are unsupported on the decimal-ledger rail \
                 (it is its own multi-draw instrument)"
                    .to_string(),
            ));
        }
        return compare_pair_decimal_ledger(gateway, cache, request).await;
    }
    // A draw is deliberately fresh: the nonce bypasses the pairwise SQLite
    // cache in both directions (a nonce result must never pollute the
    // pair's cached judgement, and a cached judgement is not a draw).
    let cache = if request.nonce.is_some() { None } else { cache };
    let mut prompt_instance = request.spec.prompt_instance();
    if let Some(nonce) = &request.nonce {
        prompt_instance.push_draw_token(nonce);
    }
    let rendered_prompt_digest = prompt_instance.rendered_digest();
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

    // Key on the per-attribute system prefix (template + attribute), NOT the
    // entities: within one objective every pair shares that prefix, so
    // co-routing all pairs is what lets it hit (multi-recon 2026-08-06).
    let prompt_cache_key = super::super::prompt_cache_key_from_parts(
        &prompt_instance.template_slug,
        &[request.spec.attribute.prompt],
    );
    let mut chat_request = ChatRequest::new(
        ChatModel::parse(request.spec.model),
        prompt_instance.to_messages(),
        request.attribution,
    )
    .max_tokens(PAIRWISE_MAX_OUTPUT_TOKENS_DEFAULT);
    chat_request.prompt_cache_key = Some(prompt_cache_key);
    if should_use_json_mode(request.spec.model) {
        chat_request = chat_request.json();
    }
    chat_request = chat_request.max_tokens(pairwise_max_output_tokens(request.spec.model));
    if model_supports_logprobs(request.spec.model) {
        chat_request = chat_request.with_logprobs(pairwise_logprobs_top_n());
    }

    let wants_bucket_pmf = request.spec.attribute.prompt_template_slug
        == Some("canonical_bucket_v1")
        && chat_request.logprobs;
    let max_live_attempts = if wants_bucket_pmf {
        PAIRWISE_BUCKET_LOGPROB_MAX_ATTEMPTS
    } else {
        1
    };
    let prompt_text = format!(
        "{}\n---\n{}",
        prompt_instance.system.as_str(),
        prompt_instance.user.as_str()
    );

    let mut input_tokens_total = 0u32;
    let mut output_tokens_total = 0u32;
    let mut provider_cost_total = 0i64;
    let mut provider_cost_is_estimate = false;
    let mut cache_read_tokens_total: Option<u32> = None;

    for attempt_index in 0..max_live_attempts {
        let response = gateway.chat(chat_request.clone()).await?;
        input_tokens_total = input_tokens_total.saturating_add(response.input_tokens);
        if let Some(read) = response.cache_read_tokens {
            cache_read_tokens_total =
                Some(cache_read_tokens_total.unwrap_or(0).saturating_add(read));
        }
        output_tokens_total = output_tokens_total.saturating_add(response.output_tokens);
        provider_cost_total = provider_cost_total.saturating_add(response.cost_nanodollars);
        provider_cost_is_estimate |= response.cost_is_estimate;

        let mut usage = ComparisonUsage {
            input_tokens: input_tokens_total,
            cache_read_tokens: cache_read_tokens_total,
            output_tokens: output_tokens_total,
            provider_cost_nanodollars: provider_cost_total,
            provider_cost_is_estimate,
            cached: false,
            served_model: response.served_model.clone(),
            prompt_text: Some(prompt_text.clone()),
            rendered_prompt_digest: rendered_prompt_digest.clone(),
            question_text: Some(request.spec.attribute.prompt.to_string()),
            raw_output: Some(response.content.clone()),
            output_logprobs: None,
            pairwise_logprob_posterior: None,
            evidence_moments: None,
            ledger_draws: None,
        };

        match parse_pairwise_response(
            &response.content,
            request.spec.prompt_template().slug,
            response.output_logprobs.as_deref(),
        ) {
            Ok(judgement) => {
                if let PairwiseJudgement::Observation {
                    higher_ranked,
                    ratio,
                    ..
                } = &judgement
                {
                    let selected_side = match higher_ranked {
                        HigherRanked::A => PairwisePreferredSide::A,
                        HigherRanked::B => PairwisePreferredSide::B,
                    };
                    let raw_logprobs = response.output_logprobs.as_deref();
                    if request.spec.attribute.prompt_template_slug == Some("canonical_bucket_v1") {
                        usage.pairwise_logprob_posterior = raw_logprobs.and_then(|logprobs| {
                            pairwise_bucket_logprob_posterior(logprobs, selected_side, *ratio)
                        });
                        usage.output_logprobs = raw_logprobs
                            .and_then(|logprobs| {
                                compact_bucket_output_logprobs(logprobs, selected_side, *ratio)
                            })
                            .or_else(|| fallback_stored_logprobs(raw_logprobs));
                    } else {
                        usage.pairwise_logprob_posterior = raw_logprobs.and_then(|logprobs| {
                            pairwise_logprob_posterior(
                                logprobs,
                                selected_side,
                                *ratio,
                                RATIO_LADDER,
                            )
                        });
                        usage.output_logprobs = fallback_stored_logprobs(raw_logprobs);
                    }
                } else {
                    usage.output_logprobs =
                        fallback_stored_logprobs(response.output_logprobs.as_deref());
                }

                let should_retry_for_pmf = wants_bucket_pmf
                    && matches!(judgement, PairwiseJudgement::Observation { .. })
                    && usage.pairwise_logprob_posterior.is_none()
                    && attempt_index + 1 < max_live_attempts;
                if should_retry_for_pmf {
                    continue;
                }

                if let (Some(cache), Some(ref key)) = (cache, &cache_key) {
                    let entry = judgement_to_cached(&judgement, &usage);
                    let _ = cache.put(key, &entry).await;
                }
                return Ok((judgement, usage));
            }
            Err(ComparisonError::Parse(e)) => {
                usage.output_logprobs =
                    fallback_stored_logprobs(response.output_logprobs.as_deref());
                warn!(error = %e, "Failed to parse pairwise JSON response; treating as refusal");
                let judgement = PairwiseJudgement::Refused;
                if let (Some(cache), Some(ref key)) = (cache, &cache_key) {
                    let entry = judgement_to_cached(&judgement, &usage);
                    let _ = cache.put(key, &entry).await;
                }
                return Ok((judgement, usage));
            }
            Err(e) => return Err(e),
        }
    }

    unreachable!("live comparison loop always returns or errors")
}

/// Conservative estimate of input tokens for a single pairwise comparison prompt.
///
/// Used to reserve credits before executing the rerank. Overestimation is OK.
pub fn estimate_pairwise_input_tokens(
    attribute_name: &str,
    attribute_prompt: &str,
    prompt_template_slug: Option<&str>,
    entity_a_text: &str,
    entity_b_text: &str,
) -> u32 {
    let entity_a = EntityRef::with_context("A", entity_a_text);
    let entity_b = EntityRef::with_context("B", entity_b_text);
    let template = prompt_template_slug
        .and_then(prompt_by_slug)
        .unwrap_or(DEFAULT_PROMPT);
    let prompt_instance = template.render(attribute_name, attribute_prompt, entity_a, entity_b);
    let messages = prompt_instance.to_messages();

    // Count tokens in message content and add a small overhead per message
    // to account for role/formatting tokens.
    let content_tokens: usize = messages.iter().map(|m| count_tokens(&m.content)).sum();
    let overhead_tokens = 8usize.saturating_mul(messages.len());
    (content_tokens + overhead_tokens) as u32
}

// =============================================================================
// Mapping functions
// =============================================================================

/// Ratio cap for the ladder-shaped elicitation paths (the last rung of
/// [`RATIO_LADDER`]). The decimal-ledger path passes its own wider domain.
pub(super) const LADDER_RATIO_CAP: f64 = 26.0;

pub(super) fn cached_to_judgement(
    cached: &CachedJudgement,
    max_ratio: f64,
) -> Option<PairwiseJudgement> {
    if cached.refused {
        return Some(PairwiseJudgement::Refused);
    }
    let higher = cached.higher_ranked.as_deref()?;
    let ratio = cached.ratio?;
    let confidence = cached.confidence?;
    if !(1.0..=max_ratio).contains(&ratio) {
        return None;
    }
    let higher_ranked = match higher.to_uppercase().as_str() {
        "A" => HigherRanked::A,
        "B" => HigherRanked::B,
        _ => return None,
    };
    Some(PairwiseJudgement::Observation {
        higher_ranked,
        ratio,
        confidence,
    })
}

/// Whether to request logprobs for this model.
///
/// Logprobs are only useful for non-reasoning models that support them.
/// - Anthropic: no logprobs via OpenRouter
/// - Reasoning models: output tokens are post-reasoning, so logprob
///   distribution doesn't reflect the actual deliberation
/// - `:thinking` suffix: OpenRouter convention for reasoning variants
pub(super) fn model_supports_logprobs(model: &str) -> bool {
    // Anthropic never exposes logprobs via OpenRouter.
    if model.starts_with("anthropic/") {
        return false;
    }

    // `:thinking` suffix is OpenRouter's convention for reasoning variants.
    if model.contains(":thinking") {
        return false;
    }

    // Known reasoning model families by prefix/substring.
    let model_lower = model.to_lowercase();
    let is_reasoning = model_lower.starts_with("openai/o1")
        || model_lower.starts_with("openai/o3")
        || model_lower.starts_with("openai/o4")
        || model_lower.contains("deepseek-r1")
        || model_lower.contains("/qwq")
        || model_lower.contains("-thinking")
        || model_lower.contains("reasoning");

    if is_reasoning {
        return false;
    }

    // GPT-5.4 / GPT-5.6 families: logprobs fail UNLESS the request pins
    // reasoning effort "none" (re-census 2026-08-13 on Azure/OpenRouter:
    // 5.6-sol 400s at default effort but returns full logprobs at
    // effort=none; 5.4-mini now works at either, superseding the 2026-07-18
    // 502 finding). Callers that cannot pin effort keep the conservative
    // false here; the decimal-ledger instrument opts in via
    // `ledger_logprobs_require_effort_none` and pins the effort itself.
    if model_lower.starts_with("openai/gpt-5.4") || model_lower.starts_with("openai/gpt-5.6") {
        return false;
    }
    // GPT-5 base family (gpt-5, gpt-5-mini, gpt-5-chat-latest): mandatory
    // reasoning, "no path exists" per the docs/LOGPROBS.md census — the
    // provider 400s with "logprobs are not supported with reasoning models"
    // (measured live 2026-07-27; every wave-2 replication comparison failed).
    // Exact-family match: gpt-5.4/gpt-5.6 use a dot, so `gpt-5-` is safe.
    if model_lower == "openai/gpt-5" || model_lower.starts_with("openai/gpt-5-") {
        return false;
    }
    // Gemini 3.1 Pro Preview is reasoning-mandatory on OpenRouter and does not
    // advertise logprob/top_logprob support in live provider metadata. Treat
    // current and future Gemini Pro reasoning previews conservatively until a
    // non-reasoning endpoint explicitly exposes token logprobs.
    if model_lower.starts_with("google/gemini-3.1-pro")
        || model_lower.starts_with("google/gemini-3-pro")
    {
        return false;
    }

    true
}

/// Models whose logprobs are reachable ONLY with reasoning effort pinned to
/// "none" (re-census 2026-08-13: gpt-5.6-sol 400s at default effort, full
/// logprobs at effort=none; gpt-5.4 works at either — pinning is uniform and
/// harmless there). Effort is part of instrument identity (RESULTS.md
/// doctrine), so only the decimal-ledger path — whose census + validation
/// pack (notes/decimal-pmf-2026-08-10) was measured entirely at effort=none —
/// uses this; the ladder instrument stays as censused.
pub(super) fn ledger_logprobs_require_effort_none(model: &str) -> bool {
    let model_lower = model.to_lowercase();
    model_lower.starts_with("openai/gpt-5.4") || model_lower.starts_with("openai/gpt-5.6")
}

pub(super) fn should_use_json_mode(model: &str) -> bool {
    if std::env::var("CARDINAL_FORCE_JSON_MODE")
        .ok()
        .is_some_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
    {
        return true;
    }

    model.starts_with("openai/") || !model.contains('/')
}

/// The seriate ratio-letter path: one single-token call whose answer-position
/// top-k logprobs are parsed into a judgement PMF. The point judgement
/// (direction, ratio, confidence) is DERIVED from the PMF so every existing
/// surface (traces, counterbalance flip stats, cache) keeps working, while
/// the PMF moments ride in `ComparisonUsage::evidence_moments` and enter the
/// solver with measured variance.
pub(super) fn judgement_to_cached(
    judgement: &PairwiseJudgement,
    usage: &ComparisonUsage,
) -> CachedJudgement {
    match judgement {
        PairwiseJudgement::Refused => CachedJudgement {
            higher_ranked: None,
            ratio: None,
            confidence: None,
            refused: true,
            input_tokens: Some(usage.input_tokens),
            output_tokens: Some(usage.output_tokens),
            provider_cost_nanodollars: Some(usage.provider_cost_nanodollars),
            log_ratio_mean: None,
            log_ratio_var: None,
            visible_mass: None,
            logprob_mode: None,
            e_lo: None,
            e_hi: None,
            conservation_gap: None,
        },
        PairwiseJudgement::Observation {
            higher_ranked,
            ratio,
            confidence,
        } => CachedJudgement {
            higher_ranked: Some(match higher_ranked {
                HigherRanked::A => "A".to_string(),
                HigherRanked::B => "B".to_string(),
            }),
            ratio: Some(*ratio),
            confidence: Some(*confidence),
            refused: false,
            input_tokens: Some(usage.input_tokens),
            output_tokens: Some(usage.output_tokens),
            provider_cost_nanodollars: Some(usage.provider_cost_nanodollars),
            log_ratio_mean: usage.evidence_moments.map(|m| m.log_ratio_mean),
            log_ratio_var: usage.evidence_moments.map(|m| m.log_ratio_var),
            visible_mass: usage.evidence_moments.map(|m| m.visible_mass),
            logprob_mode: usage.evidence_moments.map(|m| m.logprob_mode),
            e_lo: usage.evidence_moments.and_then(|m| m.e_lo),
            e_hi: usage.evidence_moments.and_then(|m| m.e_hi),
            conservation_gap: usage.evidence_moments.and_then(|m| m.conservation_gap),
        },
    }
}

/// Reconstruct evidence moments from a cache hit (evidence-mode rows only).
pub(super) fn evidence_moments_from_cached(hit: &CachedJudgement) -> Option<EvidenceMoments> {
    let visible_mass = hit.visible_mass?;
    Some(EvidenceMoments {
        log_ratio_mean: hit.log_ratio_mean?,
        log_ratio_var: hit.log_ratio_var?,
        visible_mass,
        // Rows written since the `logprob_mode` column exists carry the
        // flag directly; for older rows infer it from visible mass (a
        // sampled/frequency fallback stores 0.0), so degraded judgements
        // stay visibly degraded on cache replay.
        logprob_mode: hit.logprob_mode.unwrap_or(visible_mass > 0.0),
        e_lo: hit.e_lo,
        e_hi: hit.e_hi,
        conservation_gap: hit.conservation_gap,
    })
}
