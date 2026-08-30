use super::*;

pub(super) async fn run(command: Commands) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        Commands::Explain {
            file,
            candidate,
            propose,
            model,
            budget,
            format_json,
            no_cache,
            cache,
            seed,
        } => {
            let raw = read_sort_input(file.as_deref())?;
            let documents = parse_sort_items(&raw)?;
            if documents.len() < 3 {
                return Err(
                    "explain requires at least 3 items (in your believed order, best first)".into(),
                );
            }
            require_openrouter_key()?;
            let gateway = Arc::new(ProviderGateway::from_env(Arc::new(NoopUsageSink))?);

            let mut candidates = candidate;
            if let Some(count) = propose {
                let (proposed, usage) = llmsort::rerank::propose_candidates(
                    gateway.as_ref(),
                    model.as_deref().unwrap_or(llmsort::rerank::DEFAULT_MODEL),
                    &documents,
                    count,
                    Attribution::new("llmsort::explain::propose"),
                )
                .await?;
                eprintln!(
                    "proposed {} candidate attributes (${:.4}):",
                    proposed.len(),
                    usage.cost_nanodollars as f64 / 1e9
                );
                for c in &proposed {
                    eprintln!("  - {c}");
                }
                candidates.extend(proposed);
            }
            if candidates.is_empty() {
                return Err(
                    "no candidate attributes: pass --candidate \"<attribute>\" (repeatable) and/or --propose <n>"
                        .into(),
                );
            }

            let cache_store = if no_cache {
                None
            } else {
                let cache_path = cache.unwrap_or_else(SqlitePairwiseCache::default_path);
                Some(SqlitePairwiseCache::new(cache_path)?)
            };
            let mut execution = llmsort::rerank::RerankExecution::new(
                gateway.clone(),
                Attribution::new("llmsort::explain"),
            )
            .run_options(RerankRunOptions {
                rng_seed: seed,
                cache_only: false,
            });
            if let Some(store) = cache_store.as_ref() {
                execution = execution.cache(store);
            }

            let explanation = llmsort::rerank::explain_ranking(
                documents,
                candidates,
                execution,
                llmsort::rerank::ExplainOptions {
                    model,
                    comparison_budget: budget,
                    ..Default::default()
                },
            )
            .await?;

            if format_json {
                println!("{}", serde_json::to_string_pretty(&explanation)?);
            } else {
                println!("attribute                                    | alone ρ | weight");
                println!("---------------------------------------------|---------|-------");
                for attr in &explanation.attributes {
                    let rho = attr
                        .spearman_alone
                        .map(|r| format!("{r:+.2}"))
                        .unwrap_or_else(|| "  n/a".into());
                    let prompt: String = attr.prompt.chars().take(44).collect();
                    println!("{prompt:<45}| {rho:>7} | {:.2}", attr.fitted_weight);
                }
                match explanation.combined_spearman {
                    Some(c) => println!(
                        "
weighted combination reconstructs your ranking at ρ = {c:+.2}"
                    ),
                    None => println!(
                        "
no combination of these attributes reconstructs your ranking"
                    ),
                }
            }
            let meta = &explanation.meta;
            eprintln!(
                "{} comparisons ({} cached, {} refused) · ${:.4} · order flips: {}/{}",
                meta.comparisons_used,
                meta.comparisons_cached,
                meta.comparisons_refused,
                meta.provider_cost_nanodollars as f64 / 1e9,
                meta.position_flips,
                meta.pairs_counterbalanced,
            );
        }
        Commands::CacheExport { db, out } => {
            let path = db.unwrap_or_else(SqlitePairwiseCache::default_path);
            let cache = SqlitePairwiseCache::new(path)?;
            cache.export_jsonl(out).await?;
        }
        Commands::CachePrune {
            db,
            max_age_days,
            max_rows,
        } => {
            if max_age_days.is_none() && max_rows.is_none() {
                return Err("cache-prune requires --max-age-days and/or --max-rows".into());
            }
            if matches!(max_rows, Some(0)) {
                return Err("--max-rows must be >= 1".into());
            }
            let path = db.unwrap_or_else(SqlitePairwiseCache::default_path);
            let cache = SqlitePairwiseCache::new(path)?;
            let _lock = cache.lock_exclusive()?;
            let stats = cache.prune(max_age_days, max_rows).await?;
            println!(
                "pruned {} rows; {} rows remain",
                stats.deleted, stats.remaining
            );
        }
        Commands::Policy { command } => match command {
            PolicyCommands::List => {
                let registry = PolicyRegistry::default();
                for name in registry.list() {
                    println!("{name}");
                }
            }
            PolicyCommands::Load { config } => {
                let policy = load_policy_from_path(config)?;
                let description = policy.describe().unwrap_or_else(|| "unknown".to_string());
                println!("{description}");
            }
        },
        Commands::Report {
            request,
            response,
            out,
            format,
            top_n,
            include_infeasible,
            no_attr_scores,
            rng_seed,
            policy,
            cache_only,
        } => {
            let req: MultiRerankRequest = read_json(&request)?;
            let resp: MultiRerankResponse = read_json(&response)?;
            validate_multi_rerank_request(&req)?;
            validate_report_inputs(&req, &resp)?;
            let opts = RerankReportOptions {
                top_n,
                include_infeasible,
                include_attribute_scores: !no_attr_scores,
                rng_seed,
                model_policy: policy,
                cache_only,
            };
            let report = build_report(&req, &resp, &opts);
            match format {
                ReportFormatArg::Json => {
                    let json = serde_json::to_string_pretty(&report)?;
                    std::fs::write(out, json)?;
                }
                ReportFormatArg::Md | ReportFormatArg::Markdown => {
                    let markdown = render_report_markdown(&report);
                    std::fs::write(out, markdown)?;
                }
            }
        }
        Commands::Validate { request } => {
            let req: MultiRerankRequest = read_json(&request)?;
            validate_multi_rerank_request(&req)?;
            println!("valid request: {}", request.display());
        }
        Commands::Rerank {
            request,
            out,
            cache,
            lock_cache,
            cache_only,
            policy,
            policy_config,
            rng_seed,
            report,
            trace,
        } => {
            let req: MultiRerankRequest = read_json(&request)?;
            validate_multi_rerank_request(&req)?;
            let cache_path = cache.unwrap_or_else(SqlitePairwiseCache::default_path);
            let cache = SqlitePairwiseCache::new(cache_path)?;
            let _lock = if lock_cache {
                Some(cache.lock_exclusive()?)
            } else {
                None
            };

            let policy_obj = load_policy(policy, policy_config)?;
            let options = RerankRunOptions {
                rng_seed,
                cache_only,
            };
            let gateway = provider_gateway(cache_only)?;

            let (trace_sink, trace_worker) = if let Some(path) = trace {
                let (sink, worker) = JsonlTraceSink::new(path)?;
                (Some(sink), Some(worker))
            } else {
                (None, None)
            };
            let trace_ref = trace_sink.as_ref().map(|sink| sink as &dyn TraceSink);

            let mut execution = llmsort::rerank::RerankExecution::new(
                Arc::new(gateway),
                Attribution::new("llmsort::rerank"),
            )
            .cache(&cache)
            .run_options(options);
            if let Some(policy) = policy_obj.clone() {
                execution = execution.model_policy(policy);
            }
            if let Some(trace) = trace_ref {
                execution = execution.trace(trace);
            }

            let resp = llmsort::rerank::multi_rerank(req.clone(), execution).await?;

            write_json(&out, &resp)?;

            drop(trace_sink);
            if let Some(worker) = trace_worker {
                worker.join()?;
            }

            if let Some(report_path) = report {
                let opts = RerankReportOptions {
                    top_n: 10,
                    include_infeasible: false,
                    include_attribute_scores: true,
                    rng_seed,
                    model_policy: policy_obj.and_then(|policy| policy.describe()),
                    cache_only,
                };
                let report = build_report(&req, &resp, &opts);
                let markdown = render_report_markdown(&report);
                std::fs::write(report_path, markdown)?;
            }
        }
        _ => unreachable!(),
    }

    Ok(())
}
