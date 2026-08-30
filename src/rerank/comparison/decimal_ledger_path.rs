use super::*;

/// The decimal-ledger evidence path: K temperature-1 redraws of a free-form
/// decimal ratio whose per-draw exact chosen-token logprobs (plus top-k
/// sidebands) are fused into a credal exact-atom ledger
/// ([`super::super::decimal_ledger`]). The judgement PMF's (E\[Z\], var) ride in
/// `ComparisonUsage::evidence_moments` and enter the solver as measured
/// precision; the point judgement is DERIVED from the ledger so every
/// point-shaped surface (traces, counterbalance stats, cache) keeps working.
///
/// Degradation ladder, loud at every step:
/// 1. provider rejects the logprobs PARAMETER → sampled draws, frequency-MC
///    moments (`logprob_mode: false`);
/// 2. logprobs present but fewer than 2 trajectories parse (non-o200k digit
///    grouping, malformed JSON) → same frequency-MC fallback over the
///    text-parsed ratios;
/// 3. fewer than 2 usable draws of any kind → `Refused`.
///
/// Transport errors mid-loop propagate (matching the seriate path); the
/// judgement cache stores only fused outcomes, so a failed loop caches
/// nothing.
pub(super) async fn compare_pair_decimal_ledger(
    gateway: &dyn ChatGateway,
    cache: Option<&dyn PairwiseCache>,
    request: PairwiseComparisonRequest<'_>,
) -> Result<(PairwiseJudgement, ComparisonUsage), ComparisonError> {
    let rendered = request.spec.prompt_instance();
    let rendered_prompt_digest = rendered.rendered_digest();
    let cache_key = cache.map(|_| request.spec.cache_key());
    if let (Some(cache), Some(ref key)) = (cache, &cache_key) {
        match cache.get(key).await {
            Ok(Some(hit)) => {
                // The decimal instrument's domain reaches 999.9; validating
                // cached rows against the 26.0 ladder cap would turn every
                // strong judgement into a permanent cache miss.
                if let Some(judgement) = cached_to_judgement(&hit, decimal_ledger::DOMAIN_HI) {
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

    // Per-attribute system-prefix key; see compare_pair for the rationale.
    let prompt_cache_key = super::super::prompt_cache_key_from_parts(
        &rendered.template_slug,
        &[request.spec.attribute.prompt],
    );
    let mut base_request = ChatRequest::new(
        ChatModel::parse(request.spec.model),
        rendered.to_messages(),
        request.attribution.clone(),
    )
    // Resampling IS the instrument: temperature 1 makes each redraw an
    // independent measurement of the judgement PMF (census 2026-08-10).
    .temperature(1.0)
    .max_tokens(PAIRWISE_MAX_OUTPUT_TOKENS_DEFAULT);
    base_request.prompt_cache_key = Some(prompt_cache_key);
    if should_use_json_mode(request.spec.model) {
        base_request = base_request.json();
    }
    // GPT-5.x: logprobs are reachable only at reasoning effort "none";
    // effort=none is also this instrument's measured identity (the whole
    // decimal-pmf census/validation pack ran at effort none).
    let pin_effort_none = ledger_logprobs_require_effort_none(request.spec.model);
    if pin_effort_none {
        base_request = base_request.reasoning(ReasoningConfig {
            enabled: None,
            effort: Some(ReasoningEffort::None),
            max_tokens: None,
            exclude: None,
        });
    }

    let mut input_tokens_total = 0u32;
    let mut cache_read_tokens_total: Option<u32> = None;
    let mut output_tokens_total = 0u32;
    let mut provider_cost_total = 0i64;
    let mut provider_cost_is_estimate = false;
    let mut served_model = None;
    let mut first_content: Option<String> = None;
    let mut first_logprobs = None;

    // Seed from the house logprob census (Anthropic and reasoning-variant
    // models never surface logprobs; their rejections often don't say
    // "logprob", so the substring sniff below would miss them and the
    // judgement would error instead of degrading). The sniff remains as
    // the backstop for models the census wrongly believes support them.
    let mut logprobs_supported = model_supports_logprobs(request.spec.model) || pin_effort_none;
    if !logprobs_supported {
        warn!(
            model = request.spec.model,
            "decimal ledger: model census says no logprobs; using sampled draws (frequency-MC moments)"
        );
    }
    let mut trajectories = Vec::new();
    let mut text_obs: Vec<(HigherRanked, f64)> = Vec::new();
    let mut refusals = 0usize;

    for draw in 0..DECIMAL_LEDGER_DRAWS {
        let attempt = if logprobs_supported {
            base_request.clone().with_logprobs(20)
        } else {
            base_request.clone()
        };
        let response = match gateway.chat(attempt).await {
            Ok(response) => response,
            Err(err)
                if logprobs_supported
                    && format!("{err}").to_ascii_lowercase().contains("logprob") =>
            {
                warn!(model = request.spec.model, error = %err,
                    "provider rejects logprobs; decimal ledger degrading to sampled draws");
                logprobs_supported = false;
                gateway.chat(base_request.clone()).await?
            }
            Err(err) => return Err(err.into()),
        };
        input_tokens_total = input_tokens_total.saturating_add(response.input_tokens);
        if let Some(read) = response.cache_read_tokens {
            cache_read_tokens_total =
                Some(cache_read_tokens_total.unwrap_or(0).saturating_add(read));
        }
        output_tokens_total = output_tokens_total.saturating_add(response.output_tokens);
        provider_cost_total = provider_cost_total.saturating_add(response.cost_nanodollars);
        provider_cost_is_estimate |= response.cost_is_estimate;
        if served_model.is_none() {
            served_model = response.served_model.clone();
        }
        if draw == 0 {
            first_content = Some(response.content.clone());
            first_logprobs = fallback_stored_logprobs(response.output_logprobs.as_deref());
        }

        match parse_decimal_ledger_text(&response.content) {
            DecimalDrawText::Refused => {
                refusals += 1;
                continue;
            }
            DecimalDrawText::Observation {
                higher_ranked,
                ratio,
            } => {
                text_obs.push((higher_ranked, ratio));
            }
            DecimalDrawText::Unparseable => {}
        }
        if let Some(trajectory) = response
            .output_logprobs
            .as_deref()
            .and_then(decimal_ledger::extract_trajectory)
        {
            trajectories.push(trajectory);
        }
    }

    let prompt_text = format!("{}\n---\n{}", rendered.system, rendered.user);
    let mut usage = ComparisonUsage {
        input_tokens: input_tokens_total,
        cache_read_tokens: cache_read_tokens_total,
        output_tokens: output_tokens_total,
        provider_cost_nanodollars: provider_cost_total,
        provider_cost_is_estimate,
        cached: false,
        served_model,
        prompt_text: Some(prompt_text),
        rendered_prompt_digest,
        question_text: Some(request.spec.attribute.prompt.to_string()),
        raw_output: first_content,
        output_logprobs: first_logprobs,
        pairwise_logprob_posterior: None,
        evidence_moments: None,
        ledger_draws: None,
    };

    // Fusion ladder: exact-atom ledger, then frequency MC, then refusal.
    let fused = if trajectories.len() >= 2 {
        decimal_ledger::analyze(&trajectories).map(|outcome| {
            let confidence = if outcome.mean >= 0.0 {
                outcome.p_dir_a
            } else {
                1.0 - outcome.p_dir_a
            };
            (
                outcome.mean,
                outcome.var,
                outcome.enumerated_mass,
                true,
                confidence,
                Some((outcome.e_lo, outcome.e_hi, outcome.conservation_gap)),
            )
        })
    } else {
        None
    };
    let fused = fused.or_else(|| {
        if text_obs.len() < 2 {
            return None;
        }
        if trajectories.len() < 2 && logprobs_supported {
            warn!(
                model = request.spec.model,
                parsed = trajectories.len(),
                draws = DECIMAL_LEDGER_DRAWS,
                "decimal ledger: too few token trajectories parsed; \
                 degrading to frequency-MC moments"
            );
        }
        let z: Vec<f64> = text_obs
            .iter()
            .map(|(higher, ratio)| {
                let signed = match higher {
                    HigherRanked::A => 1.0,
                    HigherRanked::B => -1.0,
                };
                signed * ratio.clamp(1.0, decimal_ledger::DOMAIN_HI).ln()
            })
            .collect();
        let n = z.len() as f64;
        let mean = z.iter().sum::<f64>() / n;
        // Variance of the MEAN across draws (sample variance / n).
        let var = z.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1.0) / n;
        let agree = z.iter().filter(|v| (**v >= 0.0) == (mean >= 0.0)).count() as f64 / n;
        Some((mean, var, 0.0, false, agree, None))
    });

    let Some((mean, var, visible_mass, logprob_mode, confidence, certificate)) = fused else {
        // Cache the refusal only when the model actually refused a majority
        // of draws. A transient parse-failure burst must not become a sticky
        // $0 `Refused` served from cache forever (integration review,
        // 2026-08-11); uncached, the next run simply retries live.
        let genuine_refusal = refusals * 2 >= DECIMAL_LEDGER_DRAWS;
        if !genuine_refusal {
            warn!(
                model = request.spec.model,
                refusals,
                draws = DECIMAL_LEDGER_DRAWS,
                "decimal ledger: no usable draws; treating as refusal (not cached)"
            );
        }
        let judgement = PairwiseJudgement::Refused;
        if genuine_refusal {
            if let (Some(cache), Some(ref key)) = (cache, &cache_key) {
                let entry = judgement_to_cached(&judgement, &usage);
                let _ = cache.put(key, &entry).await;
            }
        }
        return Ok((judgement, usage));
    };

    usage.evidence_moments = Some(EvidenceMoments {
        log_ratio_mean: mean,
        log_ratio_var: var,
        visible_mass,
        logprob_mode,
        e_lo: certificate.map(|(lo, _, _)| lo),
        e_hi: certificate.map(|(_, hi, _)| hi),
        conservation_gap: certificate.map(|(_, _, gap)| gap),
    });
    // Estimator-replay seam: persist the raw draws exactly when the ledger
    // produced this judgement's evidence (logprob_mode). MC-fallback rows
    // had < 2 usable trajectories — nothing analyze() could replay.
    if logprob_mode {
        usage.ledger_draws = Some(decimal_ledger::LedgerDrawsRecord {
            grammar_version: decimal_ledger::GRAMMAR_VERSION.to_string(),
            draws: trajectories,
        });
    }

    let higher_ranked = if mean >= 0.0 {
        HigherRanked::A
    } else {
        HigherRanked::B
    };
    let ratio = mean.abs().exp().clamp(1.0, decimal_ledger::DOMAIN_HI);
    let judgement = PairwiseJudgement::Observation {
        higher_ranked,
        ratio,
        confidence: confidence.clamp(0.0, 1.0),
    };

    if let (Some(cache), Some(ref key)) = (cache, &cache_key) {
        let entry = judgement_to_cached(&judgement, &usage);
        let _ = cache.put(key, &entry).await;
    }
    Ok((judgement, usage))
}

/// Minimal text-layer parse of one decimal-ledger draw (refusal detection
/// plus the MC-fallback observation). Token-layer truth comes from
/// [`decimal_ledger::extract_trajectory`]; this parse never overrides it.
pub(super) enum DecimalDrawText {
    Refused,
    Observation {
        higher_ranked: HigherRanked,
        ratio: f64,
    },
    Unparseable,
}

pub(super) fn parse_decimal_ledger_text(content: &str) -> DecimalDrawText {
    let Some(start) = content.find('{') else {
        return DecimalDrawText::Unparseable;
    };
    let Some(end) = content.rfind('}') else {
        return DecimalDrawText::Unparseable;
    };
    if start > end {
        // A '}' before the first '{' (prose like "oops :} then {…") would
        // otherwise panic the slice below on raw model output
        // (falsifier BUG-1, 2026-08-11).
        return DecimalDrawText::Unparseable;
    }
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&content[start..=end]) else {
        return DecimalDrawText::Unparseable;
    };
    if value.get("refused").and_then(serde_json::Value::as_bool) == Some(true) {
        return DecimalDrawText::Refused;
    }
    let higher_ranked = match value
        .get("higher_ranked")
        .and_then(serde_json::Value::as_str)
    {
        Some("A") => HigherRanked::A,
        Some("B") => HigherRanked::B,
        _ => return DecimalDrawText::Unparseable,
    };
    let ratio = match value.get("ratio") {
        Some(serde_json::Value::String(s)) => s.trim().parse::<f64>().ok(),
        Some(serde_json::Value::Number(n)) => n.as_f64(),
        _ => None,
    };
    match ratio {
        Some(ratio) if ratio.is_finite() && ratio > 0.0 => DecimalDrawText::Observation {
            higher_ranked,
            ratio,
        },
        _ => DecimalDrawText::Unparseable,
    }
}
