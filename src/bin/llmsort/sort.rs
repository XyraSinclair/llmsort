use super::*;

pub(super) async fn run(command: Commands) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        Commands::Sort {
            file,
            by,
            model,
            policy,
            policy_config,
            budget,
            max_dollars,
            max_seconds,
            top_k,
            format,
            scores,
            reverse,
            two_sided,
            also_by,
            no_counterbalance,
            setwise,
            k,
            template,
            elaborate,
            prune_below,
            seed,
            concurrency,
            cache_only,
            no_cache,
            cache,
            trace,
            quiet,
            estimate,
        } => {
            if cache_only && no_cache {
                return Err("--cache-only and --no-cache are mutually exclusive".into());
            }
            let raw = read_sort_input(file.as_deref())?;
            let documents = parse_sort_items(&raw)?;
            if documents.is_empty() {
                return Err("no items to sort: input is empty".into());
            }

            if setwise {
                if policy.is_some()
                    || policy_config.is_some()
                    || budget.is_some()
                    || max_dollars.is_some()
                    || max_seconds.is_some()
                    || two_sided
                    || !also_by.is_empty()
                    || no_counterbalance
                    || template.is_some()
                    || prune_below.is_some()
                    || cache_only
                    || no_cache
                    || cache.is_some()
                    || trace.is_some()
                    || estimate
                {
                    return Err("--setwise supports --model, --k, --top-k, --seed, \
                                --concurrency, --format, --scores, --reverse, \
                                --elaborate, and --quiet only; the pairwise path owns \
                                budgets, policies, caches, probes, and traces"
                        .into());
                }
                if top_k.is_some_and(|t| t == 0 || t * 2 >= documents.len()) {
                    return Err("--setwise --top-k needs 0 < K < n/2; for larger K the \
                                funnel buys nothing - use the pairwise path directly"
                        .into());
                }
                let gateway = Arc::new(provider_gateway(false)?);
                let criterion = if elaborate {
                    let rubric = llmsort::rerank::elaborate_criterion(
                        gateway.as_ref(),
                        model.as_deref(),
                        &by,
                        Attribution::new("llmsort::sort::elaborate"),
                    )
                    .await?;
                    if !quiet {
                        eprintln!(
                            "elaborated criterion ({}, ${:.4}):
{}
",
                            rubric.model_used,
                            rubric.provider_cost_nanodollars as f64 / 1e9,
                            rubric.elaborated
                        );
                    }
                    rubric.elaborated
                } else {
                    by.clone()
                };
                let opts = llmsort::rerank::SetwiseOptions {
                    model: model.clone(),
                    k,
                    seed,
                    concurrency,
                    ..Default::default()
                };
                let documents_by_id: std::collections::HashMap<
                    String,
                    llmsort::rerank::RerankDocument,
                > = documents
                    .iter()
                    .map(|d| (d.id.clone(), d.clone()))
                    .collect();
                let mut sorted = llmsort::rerank::sort_documents_setwise(
                    documents,
                    &criterion,
                    gateway.clone(),
                    opts,
                )
                .await
                .map_err(|e| e.to_string())?;

                // The funnel (E14): screen -> certified pairwise refine of the
                // top-3K slice. The refined head replaces the screen's head;
                // the tail keeps its screen order. Latent stats stay
                // stage-native (the log-ratio scales differ across the
                // splice); ranks are renumbered over the spliced list.
                let mut refined_meta: Option<(usize, i64)> = None;
                if let Some(target_k) = top_k {
                    let m = (3 * target_k).min(sorted.items.len());
                    let slice_docs: Vec<llmsort::rerank::RerankDocument> = sorted.items[..m]
                        .iter()
                        .map(|i| documents_by_id[&i.id].clone())
                        .collect();
                    let execution = llmsort::rerank::RerankExecution::new(
                        gateway.clone(),
                        Attribution::new("llmsort::sort::funnel"),
                    )
                    .run_options(RerankRunOptions {
                        rng_seed: seed,
                        ..RerankRunOptions::default()
                    });
                    let refined = llmsort::rerank::sort_documents(
                        slice_docs,
                        &criterion,
                        execution,
                        llmsort::rerank::SortOptions {
                            model: model.clone(),
                            top_k: Some(target_k),
                            ..Default::default()
                        },
                    )
                    .await?;
                    refined_meta = Some((
                        refined.meta.comparisons_used,
                        refined.meta.provider_cost_nanodollars,
                    ));
                    let tail: Vec<_> = sorted.items.split_off(m);
                    sorted.items = refined.items;
                    sorted.items.extend(tail);
                    for (idx, item) in sorted.items.iter_mut().enumerate() {
                        item.rank = idx + 1;
                    }
                }
                if reverse {
                    sorted.items.reverse();
                }
                let stdout = io::stdout();
                let mut out = stdout.lock();
                render_setwise(&mut out, &sorted, format, scores)?;
                if !quiet {
                    let cost_usd = sorted.cost_nanodollars as f64 / 1e9;
                    let gauge = match &sorted.gauge {
                        Some(g) => match g.flip_rate {
                            Some(f) if f < 0.20 => format!(
                                " · gauge: flip {f:.2} — trust (measured: flip < 0.20 ⇒ ρ ≥ 0.64 vs pairwise)"
                            ),
                            Some(f) if f < 0.25 => {
                                format!(" · gauge: flip {f:.2} — CAUTION (0.20–0.25)")
                            }
                            Some(f) => format!(
                                " · gauge: flip {f:.2} — DO NOT TRUST: shrink --k, or use the pairwise path"
                            ),
                            None => " · gauge: no repeated pairs measured".to_string(),
                        },
                        None => String::new(),
                    };
                    let components = if sorted.components > 1 {
                        format!(
                            " · DISCONNECTED: {} components — ranks are only defined within components",
                            sorted.components
                        )
                    } else {
                        String::new()
                    };
                    let funnel = match refined_meta {
                        Some((cmp, nano)) => format!(
                            " · funnel: top-{} certified over a {}-item slice, {} comparisons, ${:.4} (top-k stability across seeds is 0.3-0.7 at this budget - PROGRAM.md E14)",
                            top_k.expect("refined implies top_k"),
                            (3 * top_k.expect("checked")).min(sorted.items.len()),
                            cmp,
                            nano as f64 / 1e9,
                        ),
                        None => String::new(),
                    };
                    let degraded = {
                        let mut s = String::new();
                        if let Some(e) = &sorted.first_error {
                            s.push_str(&format!(" · first error: {e}"));
                        }
                        if let Some(sample) = sorted.malformed_samples.first() {
                            s.push_str(&format!(" · malformed sample: {sample:?}"));
                        }
                        s
                    };
                    eprintln!(
                        "sorted {} items by \"{by}\" · setwise k={k} · {} calls ({} ok, {} malformed, {} errored) · ${cost_usd:.4} · {}{gauge}{components}{funnel}{degraded}",
                        sorted.items.len(),
                        sorted.calls,
                        sorted.calls_ok,
                        sorted.calls_malformed,
                        sorted.calls_errored,
                        sorted.model_used,
                    );
                }
                return Ok(());
            }

            let max_cost_nanodollars = max_dollars.map(dollars_to_nanodollars).transpose()?;
            let latency_budget_ms = max_seconds.map(seconds_to_milliseconds).transpose()?;
            let template_used = template.clone().unwrap_or_else(|| {
                llmsort::rerank::default_template_slug(model.as_deref()).to_string()
            });

            if estimate {
                let opts = llmsort::rerank::SortOptions {
                    model: model.clone(),
                    comparison_budget: budget,
                    max_cost_nanodollars,
                    latency_budget_ms,
                    top_k,
                    counterbalance: !no_counterbalance,
                    two_sided,
                    also_by: also_by.clone(),
                    prune_p_topk_below: prune_below,
                    prompt_template_slug: template.clone(),
                    ..Default::default()
                };
                let simple = llmsort::rerank::sort::sort_request(documents.clone(), &by, &opts);
                let multi = llmsort::rerank::simple::to_multi_request(&simple);
                let charge = llmsort::rerank::estimate_max_rerank_charge(&multi);
                let cost_cap = max_dollars
                    .map(|dollars| format!(" · capped at ${dollars:.2} by --max-dollars"))
                    .unwrap_or_default();
                println!(
                    "estimate: {} comparisons · ~{} input + ~{} output tokens each · ~${:.4} typical · ${:.2} hard max (provider output cap){cost_cap}",
                    charge.comparison_budget,
                    charge.input_tokens_per_comparison,
                    charge.typical_output_tokens_per_comparison,
                    charge.provider_cost_typical_nanodollars as f64 / 1e9,
                    charge.provider_cost_max_nanodollars as f64 / 1e9,
                );
                eprintln!(
                    "estimate only — no network, no cache; actual runs stop earlier on certified top-k or cache hits"
                );
                return Ok(());
            }

            let gateway = provider_gateway(cache_only)?;

            let cache_store = if no_cache {
                None
            } else {
                let cache_path = cache.unwrap_or_else(SqlitePairwiseCache::default_path);
                Some(SqlitePairwiseCache::new(cache_path)?)
            };
            let policy_obj = load_policy(policy, policy_config)?;

            let (trace_sink, trace_worker) = if let Some(path) = trace {
                let (sink, worker) = JsonlTraceSink::new(path)?;
                (Some(sink), Some(worker))
            } else {
                (None, None)
            };
            let trace_ref = trace_sink.as_ref().map(|sink| sink as &dyn TraceSink);

            let gateway = Arc::new(gateway);
            let mut execution = llmsort::rerank::RerankExecution::new(
                gateway.clone(),
                Attribution::new("llmsort::sort"),
            )
            .run_options(RerankRunOptions {
                rng_seed: seed,
                cache_only,
            });
            if let Some(store) = cache_store.as_ref() {
                execution = execution.cache(store);
            }
            if let Some(policy) = policy_obj {
                execution = execution.model_policy(policy);
            }
            if let Some(trace) = trace_ref {
                execution = execution.trace(trace);
            }

            let opts = llmsort::rerank::SortOptions {
                model: model.clone(),
                comparison_budget: budget,
                max_cost_nanodollars,
                latency_budget_ms,
                top_k,
                counterbalance: !no_counterbalance,
                two_sided,
                also_by,
                prune_p_topk_below: prune_below,
                prompt_template_slug: template,
                comparison_concurrency: concurrency,
                ..Default::default()
            };
            let criterion = if elaborate {
                let rubric = llmsort::rerank::elaborate_criterion(
                    gateway.as_ref(),
                    model.as_deref(),
                    &by,
                    Attribution::new("llmsort::sort::elaborate"),
                )
                .await?;
                if !quiet {
                    eprintln!(
                        "elaborated criterion ({}, ${:.4}):
{}
",
                        rubric.model_used,
                        rubric.provider_cost_nanodollars as f64 / 1e9,
                        rubric.elaborated
                    );
                }
                rubric.elaborated
            } else {
                by.clone()
            };
            let mut sorted =
                llmsort::rerank::sort_documents(documents, &criterion, execution, opts).await?;

            drop(trace_sink);
            if let Some(worker) = trace_worker {
                worker.join()?;
            }

            // A sort where every comparison failed or was refused is not a
            // sort; refuse to emit uninformative output on stdout.
            if sorted.meta.comparisons_attempted > 0 && sorted.meta.comparisons_used == 0 {
                let first_error = sorted
                    .meta
                    .first_error
                    .as_deref()
                    .map(|error| format!("; first error: {error}"))
                    .unwrap_or_default();
                return Err(format!(
                    "all {} comparison attempts failed ({} refused){first_error}; output would be \
                     uninformative. Re-run with --trace <path> to see per-comparison \
                     errors (bad model slug and invalid API key are the usual causes).",
                    sorted.meta.comparisons_attempted, sorted.meta.comparisons_refused,
                )
                .into());
            }

            if reverse {
                sorted.items.reverse();
            }
            let stdout = io::stdout();
            let mut out = stdout.lock();
            render_sorted(&mut out, &sorted, format, scores)?;

            if !quiet {
                let meta = &sorted.meta;
                let cost_usd = meta.provider_cost_nanodollars as f64 / 1e9;
                let estimate = if meta.provider_cost_is_estimate {
                    "~"
                } else {
                    ""
                };
                let evidence = if meta.evidence_judgements > 0 {
                    let residual = meta
                        .evidence_order_residual_mean_abs
                        .map(|r| format!(", order-residual {r:.3} nats"))
                        .unwrap_or_default();
                    format!(
                        " · evidence: {}/{} logprob-mode, visible {:.2}{residual}",
                        meta.logprob_mode_judgements,
                        meta.evidence_judgements,
                        meta.evidence_visible_mass_mean.unwrap_or(0.0)
                    )
                } else {
                    String::new()
                };
                let frustration = meta
                    .judgement_frustration_mean
                    .map(|f| format!(" · frustration {f:.3}"))
                    .unwrap_or_default();
                let flips = if meta.pairs_counterbalanced > 0 {
                    format!(
                        " · order flips: {}/{}",
                        meta.position_flips, meta.pairs_counterbalanced
                    )
                } else {
                    String::new()
                };
                eprintln!(
                    "sorted {} items by \"{by}\" · {template_used} · {} comparisons ({} cached, {} refused) · {estimate}${cost_usd:.4}{flips}{evidence}{frustration} · stop: {}",
                    sorted.items.len(),
                    meta.comparisons_used,
                    meta.comparisons_cached,
                    meta.comparisons_refused,
                    serde_json::to_value(meta.stop_reason)?.as_str().unwrap_or("unknown"),
                );
                if meta.comparisons_attempted > 0 {
                    let unresolved = adjacent_ranks_within_one_sigma(&sorted.items);
                    if unresolved == 0 {
                        eprintln!("resolution: every adjacent rank is separated by more than 1σ");
                    } else {
                        eprintln!(
                            "resolution: {unresolved} of {} adjacent ranks are within 1σ of each other — the order is coarse at this budget; raise --budget or set --top-k to focus it",
                            sorted.items.len().saturating_sub(1),
                        );
                    }
                }
                // Error budget, experimentalist-style: statistical and
                // systematic components side by side, each in its native
                // unit — never silently pooled.
                {
                    let stat = if sorted.items.is_empty() {
                        None
                    } else {
                        Some(
                            sorted.items.iter().map(|i| i.latent_std).sum::<f64>()
                                / sorted.items.len() as f64,
                        )
                    };
                    let mut parts: Vec<String> = Vec::new();
                    if let Some(stat) = stat {
                        let honest = if meta.evidence_sigma_w.is_some() {
                            " incl sigma_w"
                        } else {
                            ""
                        };
                        parts.push(format!("stat ±{stat:.3} (posterior{honest}, mean)"));
                    }
                    if let Some(sigma_w) = meta.evidence_sigma_w {
                        parts.push(format!("noise sigma_w {sigma_w:.3} nats/call"));
                    }
                    if let Some(residual) = meta.evidence_order_residual_mean_abs {
                        parts.push(format!("syst order {residual:.3} nats/pair"));
                    }
                    if let Some(hcr) = meta.judgement_frustration_mean {
                        parts.push(format!("syst cyclic {:.1}% of energy", hcr * 100.0));
                    }
                    if meta.topk_error > 0.0 {
                        parts.push(format!(
                            "rank risk {:.3} (top-k flip probability)",
                            meta.topk_error
                        ));
                    }
                    if parts.len() > 1 {
                        eprintln!("error budget: {}", parts.join(" · "));
                    }
                }
                for probe in &sorted.probes {
                    let kind = match probe.kind {
                        llmsort::rerank::SortProbeKind::Opposite => "opposite",
                        llmsort::rerank::SortProbeKind::Paraphrase => "paraphrase",
                    };
                    match probe.consistency {
                        Some(c) => {
                            let verdict = if c >= 0.7 {
                                "consistent"
                            } else if c >= 0.3 {
                                "shaky"
                            } else {
                                "INCOHERENT for this judge"
                            };
                            eprintln!(
                                "probe [{kind}] \"{}\": consistency {c:+.2} — {verdict}",
                                probe.prompt
                            );
                        }
                        None => eprintln!(
                            "probe [{kind}] \"{}\": not enough shared scores to assess",
                            probe.prompt
                        ),
                    }
                }
            }
        }
        _ => unreachable!(),
    }

    Ok(())
}

fn dollars_to_nanodollars(dollars: f64) -> Result<i64, &'static str> {
    if !dollars.is_finite() || dollars <= 0.0 {
        return Err("--max-dollars must be finite and greater than 0");
    }
    let nanodollars = (dollars * 1e9).floor();
    if nanodollars < 1.0 {
        return Err("--max-dollars must be at least $0.000000001");
    }
    if nanodollars > i64::MAX as f64 {
        return Err("--max-dollars is too large");
    }
    Ok(nanodollars as i64)
}

fn seconds_to_milliseconds(seconds: f64) -> Result<u64, &'static str> {
    if !seconds.is_finite() || seconds <= 0.0 {
        return Err("--max-seconds must be finite and greater than 0");
    }
    let milliseconds = (seconds * 1e3).ceil();
    if milliseconds < 1.0 {
        return Err("--max-seconds must be at least 0.001");
    }
    if milliseconds > u64::MAX as f64 {
        return Err("--max-seconds is too large");
    }
    Ok(milliseconds as u64)
}
