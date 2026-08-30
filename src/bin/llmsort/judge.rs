use super::*;

pub(super) async fn run(command: Commands) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        Commands::Judge {
            item_a,
            item_b,
            by,
            model,
            template,
            show_prompt,
            json,
            no_cache,
            cache,
            spin,
            sweep,
            orbit,
            wordings,
            draws,
            temperature,
            consortium,
            packets_out,
        } => {
            let text_a = read_item_arg(&item_a)?;
            let text_b = read_item_arg(&item_b)?;
            let model = model
                .as_deref()
                .unwrap_or(llmsort::rerank::DEFAULT_MODEL)
                .to_string();

            if let Some(consortium) = consortium {
                let models: Vec<String> = consortium
                    .split(',')
                    .map(|m| m.trim().to_string())
                    .filter(|m| !m.is_empty())
                    .collect();
                require_openrouter_key()?;
                let gateway = ProviderGateway::from_env(Arc::new(NoopUsageSink))?;
                let cache_store = if no_cache {
                    None
                } else {
                    let cache_path = cache.unwrap_or_else(SqlitePairwiseCache::default_path);
                    Some(SqlitePairwiseCache::new(cache_path)?)
                };
                let cache_ref = cache_store
                    .as_ref()
                    .map(|c| c as &dyn llmsort::cache::PairwiseCache);
                // Stable entity labels: packets accrete across runs by
                // id + content hash, so @path items keep their file stem
                // and literals get a content-derived label.
                let label = |arg: &str, text: &str| -> String {
                    arg.strip_prefix('@')
                        .and_then(|p| {
                            std::path::Path::new(p)
                                .file_stem()
                                .map(|s| s.to_string_lossy().into_owned())
                        })
                        .unwrap_or_else(|| {
                            llmsort::packet::entity_text_hash(text)[..12].to_string()
                        })
                };
                let id_a = label(&item_a, &text_a);
                let id_b = label(&item_b, &text_b);
                let created = chrono::Utc::now().to_rfc3339();
                let report = llmsort::rerank::consortium_verdict(
                    &gateway,
                    cache_ref,
                    &models,
                    &by,
                    (&id_a, &text_a),
                    (&id_b, &text_b),
                    &template,
                    &created,
                    Attribution::new("llmsort::judge::consortium"),
                )
                .await?;
                let written = if let Some(dir) = packets_out.as_ref() {
                    std::fs::create_dir_all(dir)?;
                    let mut paths = Vec::new();
                    for packet in &report.packets {
                        let path = dir.join(format!(
                            "packet-{}-{}.json",
                            packet.judge.replace('/', "-"),
                            &packet.id().0[..12],
                        ));
                        std::fs::write(&path, serde_json::to_string_pretty(packet)?)?;
                        paths.push(path);
                    }
                    paths
                } else {
                    Vec::new()
                };
                if json {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                } else {
                    println!(
                        "{:<34} {:>8} {:>9}  top bias",
                        "judge", "belief", "coherence"
                    );
                    for j in &report.judges {
                        match (j.belief, j.coherence, &j.top_bias) {
                            (Some(b), Some(c), Some((name, coef))) => {
                                println!("{:<34} {b:+8.3} {c:9.3}  {name} {coef:+.3}", j.model)
                            }
                            _ => println!(
                                "{:<34} orbit incomplete ({} refusals) — excluded",
                                j.model, j.refusals
                            ),
                        }
                    }
                    match (report.belief, report.ratio) {
                        (Some(b), Some(r)) => {
                            let toward = if b >= 0.0 { &id_a } else { &id_b };
                            println!(
                                "belief (fused, toward {toward}): {:+.3} nats · ratio {r:.2}×",
                                b
                            );
                            let spread = report
                                .judge_spread_nats
                                .map(|s| format!("{s:.3}"))
                                .unwrap_or_else(|| "n/a (1 judge)".into());
                            let bias = report
                                .orbit_bias_rms
                                .map(|s| format!("{s:.3}"))
                                .unwrap_or_else(|| "n/a".into());
                            let unanimity = match report.direction_unanimous {
                                Some(true) => format!(
                                    "unanimous ({}/{})",
                                    report.usable_judges, report.usable_judges
                                ),
                                Some(false) => "SPLIT".into(),
                                None => "n/a".into(),
                            };
                            println!(
                                "error budget: syst orbit-bias rms {bias} · syst judge spread \
                                 {spread} (nats) · direction {unanimity}"
                            );
                        }
                        _ => println!("no usable judge completed its orbit — no verdict"),
                    }
                    if let Some(matrix) = &report.residual_correlation {
                        let rows: Vec<String> = matrix
                            .iter()
                            .map(|row| {
                                row.iter()
                                    .map(|v| format!("{v:+.2}"))
                                    .collect::<Vec<_>>()
                                    .join(" ")
                            })
                            .collect();
                        println!(
                            "shared-bias correlation (orbit residuals, n = 8 cells): [{}]",
                            rows.join(" | ")
                        );
                    }
                    for path in &written {
                        println!("packet: {}", path.display());
                    }
                }
                eprintln!(
                    "{} judges ({} usable) · {} comparisons ({} cached) · ${:.4}",
                    report.judges.len(),
                    report.usable_judges,
                    report.comparisons,
                    report.comparisons_cached,
                    report.cost_nanodollars as f64 / 1e9,
                );
                return Ok(());
            }

            if let Some(k) = draws {
                require_openrouter_key()?;
                let gateway = ProviderGateway::from_env(Arc::new(NoopUsageSink))?;
                let report = llmsort::rerank::nonce_draws(
                    &gateway,
                    &model,
                    &template,
                    &by,
                    ("A", &text_a),
                    ("B", &text_b),
                    k,
                    temperature,
                    7,
                    Attribution::new("llmsort::judge::draws"),
                )
                .await?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                } else {
                    for (i, d) in report.draws.iter().enumerate() {
                        match d {
                            Some(m) => println!("draw {i}: {m:+.3} nats"),
                            None => println!("draw {i}: refused"),
                        }
                    }
                    match (report.mean, report.sigma_w) {
                        (Some(m), Some(s)) => println!(
                            "mean {m:+.3} nats · sigma_w {s:.3} (n = {})",
                            report.comparisons - report.refusals
                        ),
                        (Some(m), None) => println!("mean {m:+.3} nats (single usable draw)"),
                        _ => println!("no usable draws"),
                    }
                    println!(
                        "cache: {} of {} input tokens billed as cached",
                        report.cache_read_tokens_total, report.input_tokens_total
                    );
                }
                eprintln!(
                    "{} draws ({} refused) · ${:.4}",
                    report.comparisons,
                    report.refusals,
                    report.cost_nanodollars as f64 / 1e9,
                );
                return Ok(());
            }

            if orbit {
                require_openrouter_key()?;
                let gateway = ProviderGateway::from_env(Arc::new(NoopUsageSink))?;
                let cache_store = if no_cache {
                    None
                } else {
                    let cache_path = cache.unwrap_or_else(SqlitePairwiseCache::default_path);
                    Some(SqlitePairwiseCache::new(cache_path)?)
                };
                let cache_ref = cache_store
                    .as_ref()
                    .map(|c| c as &dyn llmsort::cache::PairwiseCache);
                let report = llmsort::rerank::orbit_transform(
                    &gateway,
                    cache_ref,
                    &model,
                    &by,
                    ("A", &text_a),
                    ("B", &text_b),
                    &template,
                    Attribution::new("llmsort::judge::orbit"),
                )
                .await?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                } else if report.refusals > 0 {
                    println!(
                        "orbit incomplete: {} refusals in 8 variants — no transform",
                        report.refusals
                    );
                } else {
                    let total: f64 = report.energies.iter().sum();
                    for (idx, name) in llmsort::rerank::CHARACTERS.iter().enumerate() {
                        println!(
                            "{name:<26} {:+.3} nats  ({:.1}% of energy)",
                            report.coefficients[idx],
                            100.0 * report.energies[idx] / total.max(1e-12)
                        );
                    }
                    if let Some(c) = report.coherence {
                        println!("coherence (invariant fraction): {c:.3}");
                    }
                    println!("parseval residual: {:.2e}", report.parseval_residual);
                }
                eprintln!(
                    "{} comparisons ({} cached) · ${:.4}",
                    report.comparisons,
                    report.comparisons_cached,
                    report.cost_nanodollars as f64 / 1e9,
                );
                return Ok(());
            }

            if wordings {
                require_openrouter_key()?;
                let gateway = ProviderGateway::from_env(Arc::new(NoopUsageSink))?;
                let cache_store = if no_cache {
                    None
                } else {
                    let cache_path = cache.unwrap_or_else(SqlitePairwiseCache::default_path);
                    Some(SqlitePairwiseCache::new(cache_path)?)
                };
                let cache_ref = cache_store
                    .as_ref()
                    .map(|c| c as &dyn llmsort::cache::PairwiseCache);
                let report = llmsort::rerank::wording_invariance(
                    &gateway,
                    cache_ref,
                    &model,
                    &by,
                    ("A", &text_a),
                    ("B", &text_b),
                    Attribution::new("llmsort::judge::wordings"),
                )
                .await?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                } else {
                    for r in &report.readings {
                        match r.mean_log_ratio {
                            Some(m) => println!("{:<14} {m:+.3} nats", r.template),
                            None => println!("{:<14} refused", r.template),
                        }
                    }
                    match report.sign_consistent {
                        Some(true) => {
                            println!("inversion: OK — the judge can mirror its own scale")
                        }
                        Some(false) => println!(
                            "inversion: FAILS — asking \"which has less\" flips the belief"
                        ),
                        None => println!("inversion: undetermined"),
                    }
                    if let Some(d) = report.max_disagreement_nats {
                        println!(
                            "max wording disagreement: {d:.3} nats{}",
                            if d > 0.5 {
                                " — numerical framing bias"
                            } else {
                                ""
                            }
                        );
                    }
                }
                eprintln!(
                    "{} comparisons ({} cached) · ${:.4}",
                    report.comparisons,
                    report.comparisons_cached,
                    report.cost_nanodollars as f64 / 1e9,
                );
                return Ok(());
            }

            if sweep {
                require_openrouter_key()?;
                let gateway = ProviderGateway::from_env(Arc::new(NoopUsageSink))?;
                let cache_store = if no_cache {
                    None
                } else {
                    let cache_path = cache.unwrap_or_else(SqlitePairwiseCache::default_path);
                    Some(SqlitePairwiseCache::new(cache_path)?)
                };
                let cache_ref = cache_store
                    .as_ref()
                    .map(|c| c as &dyn llmsort::cache::PairwiseCache);
                let report = llmsort::rerank::spin_sweep(
                    &gateway,
                    cache_ref,
                    &model,
                    &template,
                    &by,
                    ("A", &text_a),
                    ("B", &text_b),
                    Attribution::new("llmsort::judge::sweep"),
                )
                .await?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                } else {
                    for r in &report.readings {
                        match r.mean_log_ratio {
                            Some(m) => println!("field {:+}: {m:+.3} nats", r.field),
                            None => println!("field {:+}: refused", r.field),
                        }
                    }
                    match (report.chi_slope, report.linearity_r2) {
                        (Some(chi), Some(r2)) => {
                            let even = report
                                .even_response_mean
                                .map(|e| format!(" · even component {e:+.3} nats"))
                                .unwrap_or_default();
                            println!(
                                "response: odd slope {chi:+.3} nats/step · linear R² {r2:.3}{even}"
                            );
                        }
                        _ => println!("response: unmeasurable (refusals)"),
                    }
                    match report.belief_survives_sweep {
                        Some(true) => println!("sign(m) constant over the sweep: yes"),
                        Some(false) => println!("sign(m) constant over the sweep: no"),
                        None => {
                            println!("sign(m) constant over the sweep: undetermined (m(0) = 0)")
                        }
                    }
                }
                eprintln!(
                    "{} comparisons ({} cached) · ${:.4}",
                    report.comparisons,
                    report.comparisons_cached,
                    report.cost_nanodollars as f64 / 1e9,
                );
                return Ok(());
            }

            if spin {
                require_openrouter_key()?;
                let gateway = ProviderGateway::from_env(Arc::new(NoopUsageSink))?;
                let cache_store = if no_cache {
                    None
                } else {
                    let cache_path = cache.unwrap_or_else(SqlitePairwiseCache::default_path);
                    Some(SqlitePairwiseCache::new(cache_path)?)
                };
                let cache_ref = cache_store
                    .as_ref()
                    .map(|c| c as &dyn llmsort::cache::PairwiseCache);
                let report = llmsort::rerank::spin_probe(
                    &gateway,
                    cache_ref,
                    &model,
                    &template,
                    &by,
                    ("A", &text_a),
                    ("B", &text_b),
                    Attribution::new("llmsort::judge::spin"),
                )
                .await?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                } else {
                    for reading in &report.readings {
                        let label = match reading.framing {
                            llmsort::rerank::SpinFraming::Neutral => "neutral   ",
                            llmsort::rerank::SpinFraming::ProFirst => "pro-A spin",
                            llmsort::rerank::SpinFraming::ProSecond => "pro-B spin",
                        };
                        match reading.mean_log_ratio {
                            Some(m) => {
                                let winner = if m >= 0.0 { "A" } else { "B" };
                                let order = if reading.flipped_by_order {
                                    " · ORDER-FLIPPED"
                                } else {
                                    ""
                                };
                                println!("{label}: {winner} wins · {:+.3} nats{order}", m);
                            }
                            None => println!("{label}: refused"),
                        }
                    }
                    match report.susceptibility_nats {
                        Some(chi) => println!("susceptibility (secant): {chi:+.3} nats/spin"),
                        None => println!("susceptibility: unmeasurable (refusals)"),
                    }
                    match report.belief_survives_spin {
                        Some(true) => println!("sign(m) constant across framings: yes"),
                        Some(false) => println!("sign(m) constant across framings: no"),
                        None => println!("sign(m) constant across framings: undetermined"),
                    }
                }
                eprintln!(
                    "{} comparisons ({} cached) · ${:.4}",
                    report.comparisons,
                    report.comparisons_cached,
                    report.cost_nanodollars as f64 / 1e9,
                );
                return Ok(());
            }

            let spec = llmsort::rerank::PairwiseComparisonSpec {
                model: &model,
                attribute: llmsort::rerank::PairwiseComparisonAttribute {
                    id: "judge",
                    prompt: &by,
                    prompt_template_slug: Some(&template),
                },
                entity_a: llmsort::rerank::PairwiseComparisonEntity {
                    id: "A",
                    text: &text_a,
                },
                entity_b: llmsort::rerank::PairwiseComparisonEntity {
                    id: "B",
                    text: &text_b,
                },
            };

            if show_prompt {
                let rendered = spec.prompt_instance();
                eprintln!(
                    "--- system ---
{}
--- user ---
{}
---",
                    rendered.system, rendered.user
                );
            }

            require_openrouter_key()?;
            let gateway = ProviderGateway::from_env(Arc::new(NoopUsageSink))?;
            let cache_store = if no_cache {
                None
            } else {
                let cache_path = cache.unwrap_or_else(SqlitePairwiseCache::default_path);
                Some(SqlitePairwiseCache::new(cache_path)?)
            };
            let cache_ref = cache_store
                .as_ref()
                .map(|c| c as &dyn llmsort::cache::PairwiseCache);

            let (judgement, usage) = llmsort::rerank::compare_pair(
                &gateway,
                cache_ref,
                llmsort::rerank::PairwiseComparisonRequest {
                    spec,
                    cache_only: false,
                    attribution: Attribution::new("llmsort::judge"),
                },
            )
            .await?;

            let cost_usd = usage.provider_cost_nanodollars as f64 / 1e9;
            match judgement {
                llmsort::rerank::PairwiseJudgement::Observation {
                    higher_ranked,
                    ratio,
                    confidence,
                } => {
                    let winner = match higher_ranked {
                        llmsort::rerank::HigherRanked::A => "A",
                        llmsort::rerank::HigherRanked::B => "B",
                    };
                    if json {
                        println!(
                            "{}",
                            serde_json::json!({
                                "higher_ranked": winner,
                                "ratio": ratio,
                                "confidence": confidence,
                                "refused": false,
                                "model": model,
                                "input_tokens": usage.input_tokens,
                                "output_tokens": usage.output_tokens,
                                "cost_nanodollars": usage.provider_cost_nanodollars,
                                "cached": usage.cached,
                            })
                        );
                    } else {
                        let cached = if usage.cached { " · cached" } else { "" };
                        println!(
                            "{winner} wins · ratio {ratio} · confidence {confidence:.2} · ${cost_usd:.4}{cached}"
                        );
                    }
                }
                llmsort::rerank::PairwiseJudgement::Refused => {
                    if json {
                        println!(
                            "{}",
                            serde_json::json!({
                                "refused": true,
                                "model": model,
                                "cost_nanodollars": usage.provider_cost_nanodollars,
                                "cached": usage.cached,
                            })
                        );
                    } else {
                        println!("REFUSED · ${cost_usd:.4}");
                    }
                }
            }
        }
        _ => unreachable!(),
    }

    Ok(())
}
