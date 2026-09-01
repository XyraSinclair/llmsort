use super::*;

/// Resolve a judge item argument: literal text, or `@path` file contents.
pub(super) fn read_item_arg(raw: &str) -> Result<String, Box<dyn std::error::Error>> {
    if let Some(path) = raw.strip_prefix('@') {
        std::fs::read_to_string(path).map_err(|err| format!("failed to read {path}: {err}").into())
    } else {
        Ok(raw.to_string())
    }
}

/// Read raw sort input from a file or stdin (`-` or omitted).
pub(super) fn read_sort_input(
    file: Option<&std::path::Path>,
) -> Result<String, Box<dyn std::error::Error>> {
    match file {
        Some(path) if path.as_os_str() != "-" => std::fs::read_to_string(path)
            .map_err(|err| format!("failed to read {}: {err}", path.display()).into()),
        _ => {
            let mut raw = String::new();
            io::Read::read_to_string(&mut io::stdin(), &mut raw)?;
            Ok(raw)
        }
    }
}

/// Parse sort input: newline-delimited plain text, or a JSON array of strings
/// or `{"id", "text"}` objects when the first non-whitespace byte is `[`.
pub(super) fn parse_sort_items(
    raw: &str,
) -> Result<Vec<llmsort::rerank::RerankDocument>, Box<dyn std::error::Error>> {
    use llmsort::rerank::RerankDocument;

    if raw.trim_start().starts_with('[') {
        let value: serde_json::Value = serde_json::from_str(raw)
            .map_err(|err| format!("input looks like JSON but failed to parse: {err}"))?;
        let arr = value
            .as_array()
            .ok_or("JSON input must be an array of strings or {id, text} objects")?;
        let mut documents = Vec::with_capacity(arr.len());
        for (idx, elem) in arr.iter().enumerate() {
            if let Some(text) = elem.as_str() {
                documents.push(RerankDocument {
                    id: format!("item-{idx:04}"),
                    text: text.to_string(),
                });
            } else if let Some(obj) = elem.as_object() {
                let text = obj
                    .get("text")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| format!("JSON element {idx} needs a string \"text\" field"))?;
                let id = obj
                    .get("id")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("item-{idx:04}"));
                documents.push(RerankDocument {
                    id,
                    text: text.to_string(),
                });
            } else {
                return Err(format!(
                    "JSON element {idx} must be a string or an object with a \"text\" field"
                )
                .into());
            }
        }
        Ok(documents)
    } else {
        Ok(raw
            .lines()
            .map(|line| line.strip_suffix('\r').unwrap_or(line))
            .filter(|line| !line.trim().is_empty())
            .enumerate()
            .map(|(idx, line)| RerankDocument {
                id: format!("item-{idx:04}"),
                text: line.to_string(),
            })
            .collect())
    }
}

/// Adjacent rank pairs whose gap is inside `scale` joint posterior sigmas.
/// `scale = 1.0` is the resolution question as-run; `scale = 0.5` asks the
/// same question of a hypothetical ~4x budget (posterior sigma halves).
pub(super) fn adjacent_ranks_within_sigma(
    items: &[llmsort::rerank::SortedItem],
    scale: f64,
) -> usize {
    items
        .windows(2)
        .filter(|pair| {
            let gap = (pair[0].latent_mean - pair[1].latent_mean).abs();
            let joint_std = pair[0].latent_std.hypot(pair[1].latent_std);
            gap < joint_std * scale
        })
        .count()
}

/// Standard normal CDF (Abramowitz-Stegun 7.1.26 erf, |error| < 1.5e-7 —
/// display precision; the solver's own CDF is crate-private).
fn normal_cdf(z: f64) -> f64 {
    let x = z / std::f64::consts::SQRT_2;
    let t = 1.0 / (1.0 + 0.3275911 * x.abs());
    let y = 1.0
        - (((((1.061405429 * t - 1.453152027) * t) + 1.421413741) * t - 0.284496736) * t
            + 0.254829592)
            * t
            * (-x * x).exp();
    let erf = if x < 0.0 { -y } else { y };
    0.5 * (1.0 + erf)
}

/// Expected agreement between THIS run and an independent rerun, the number
/// users mean by "is it consistent": each run estimates every pair's order
/// with error ~N(0, s_i^2 + s_j^2) (independence across items is the
/// conservative reading — shared comparisons only correlate estimates
/// toward agreement), so a pair agrees across runs with p^2 + (1-p)^2,
/// p = Phi(|gap| / sigma_diff). Plug-in on the posterior; returns
/// (mean over all pairs, mean over adjacent pairs), None when the
/// posterior carries no uncertainty information.
pub(super) fn rerun_agreement(items: &[llmsort::rerank::SortedItem]) -> Option<(f64, f64)> {
    if items.len() < 2 || !items.iter().any(|i| i.latent_std > 0.0) {
        return None;
    }
    let agree = |a: &llmsort::rerank::SortedItem, b: &llmsort::rerank::SortedItem| -> f64 {
        let gap = (a.latent_mean - b.latent_mean).abs();
        let sigma_diff = a.latent_std.hypot(b.latent_std);
        if sigma_diff <= 0.0 {
            return 1.0; // measured-exact pair: any rerun reproduces it
        }
        let p = normal_cdf(gap / sigma_diff);
        p * p + (1.0 - p) * (1.0 - p)
    };
    let mut sum_all = 0.0;
    let mut n_all = 0usize;
    for (idx, a) in items.iter().enumerate() {
        for b in &items[idx + 1..] {
            sum_all += agree(a, b);
            n_all += 1;
        }
    }
    let sum_adj: f64 = items.windows(2).map(|w| agree(&w[0], &w[1])).sum();
    Some((sum_all / n_all as f64, sum_adj / (items.len() - 1) as f64))
}

/// Render sorted output in the requested format.
pub(super) fn render_sorted(
    out: &mut impl Write,
    sorted: &llmsort::rerank::SortedTexts,
    format: SortFormatArg,
    scores: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if matches!(format, SortFormatArg::Json) {
        serde_json::to_writer_pretty(&mut *out, sorted)?;
        writeln!(out)?;
        return Ok(());
    }
    render_items(out, &sorted.items, format, scores)
}

/// Setwise result rendering: same item shape, setwise accounting in Json.
pub(super) fn render_setwise(
    out: &mut impl Write,
    sorted: &llmsort::rerank::SetwiseSorted,
    format: SortFormatArg,
    scores: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if matches!(format, SortFormatArg::Json) {
        serde_json::to_writer_pretty(&mut *out, sorted)?;
        writeln!(out)?;
        return Ok(());
    }
    render_items(out, &sorted.items, format, scores)
}

fn render_items(
    out: &mut impl Write,
    items: &[llmsort::rerank::SortedItem],
    format: SortFormatArg,
    scores: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    match format {
        SortFormatArg::Text => {
            for item in items {
                if scores {
                    writeln!(
                        out,
                        "{:.3}\u{b1}{:.3}\t{}",
                        item.latent_mean, item.latent_std, item.text
                    )?;
                } else {
                    writeln!(out, "{}", item.text)?;
                }
            }
        }
        SortFormatArg::Json => unreachable!("handled by the callers"),
        SortFormatArg::Jsonl => {
            for item in items {
                serde_json::to_writer(&mut *out, item)?;
                writeln!(out)?;
            }
        }
        SortFormatArg::Csv => {
            writeln!(
                out,
                "rank,id,latent_mean,latent_std,z_score,percentile,text"
            )?;
            for item in items {
                writeln!(
                    out,
                    "{},{},{:.6},{:.6},{:.6},{:.6},{}",
                    item.rank,
                    csv_field(&item.id),
                    item.latent_mean,
                    item.latent_std,
                    item.z_score,
                    item.percentile,
                    csv_field(&item.text),
                )?;
            }
        }
    }
    Ok(())
}

/// Quote a CSV field when it contains a comma, quote, or newline.
pub(super) fn csv_field(raw: &str) -> String {
    if raw.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", raw.replace('"', "\"\""))
    } else {
        raw.to_string()
    }
}

pub(super) fn provider_gateway(
    cache_only: bool,
) -> Result<ProviderGateway<NoopUsageSink>, Box<dyn std::error::Error>> {
    if cache_only {
        let adapter = llmsort::gateway::openrouter::OpenRouterAdapter::with_config(
            "cache-only",
            "http://127.0.0.1:9",
            std::time::Duration::from_secs(1),
            None,
            None,
        )?;
        return Ok(ProviderGateway::with_config(
            adapter,
            Arc::new(NoopUsageSink),
            llmsort::gateway::GatewayConfig::default(),
        ));
    }

    if std::env::var("OPENROUTER_API_KEY").is_err() {
        return Err("OPENROUTER_API_KEY is not set. Create a key at \
             https://openrouter.ai/keys and `export OPENROUTER_API_KEY=...`, \
             or use --cache-only to replay cached judgements."
            .into());
    }

    Ok(ProviderGateway::from_env(Arc::new(NoopUsageSink))?)
}

pub(super) fn require_openrouter_key() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var("OPENROUTER_API_KEY").is_err() {
        return Err("OPENROUTER_API_KEY is not set. Create a key at \
             https://openrouter.ai/keys and `export OPENROUTER_API_KEY=...`."
            .into());
    }
    Ok(())
}

pub(super) fn load_policy(
    policy: Option<String>,
    policy_config: Option<PathBuf>,
) -> Result<Option<Arc<dyn ModelPolicy>>, Box<dyn std::error::Error>> {
    if let Some(path) = policy_config {
        return Ok(Some(load_policy_from_path(path)?));
    }
    if let Some(name) = policy {
        let registry = PolicyRegistry::default();
        let available = registry.list().join(", ");
        let policy = registry
            .get(&name)
            .ok_or_else(|| format!("unknown policy '{name}'; available policies: {available}"))?;
        return Ok(Some(policy));
    }
    Ok(None)
}

pub(super) fn read_json<T: serde::de::DeserializeOwned>(
    path: &PathBuf,
) -> Result<T, Box<dyn std::error::Error>> {
    let raw = std::fs::read_to_string(path)
        .map_err(|err| format!("failed to read JSON from {}: {err}", path.display()))?;
    serde_json::from_str(&raw)
        .map_err(|err| format!("failed to parse JSON in {}: {err}", path.display()).into())
}

pub(super) fn write_json<T: serde::Serialize>(path: &PathBuf, value: &T) -> Result<(), io::Error> {
    let json = serde_json::to_string_pretty(value).map_err(io::Error::other)?;
    std::fs::write(path, json)
}

pub(super) fn parse_report_top_n(raw: &str) -> Result<usize, String> {
    let value = raw
        .parse::<usize>()
        .map_err(|err| format!("invalid integer '{raw}': {err}"))?;
    if value >= 1 {
        Ok(value)
    } else {
        Err(format!("value must be at least 1, got {raw}"))
    }
}
