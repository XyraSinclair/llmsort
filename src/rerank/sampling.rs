//! Nonce draws: cache-friendly repeat sampling of one judgement.
//!
//! The observation: a pairwise prompt is a long stable prefix (system
//! prelude + attribute + both entities) followed by a short tail. If
//! repeat draws vary only a semantically-null token placed AFTER the
//! stable prefix, provider prompt caching keeps billing the prefix at the
//! cached rate while the model still re-forms its judgement — repeat
//! elicitation at a fraction of full price.
//!
//! The mathematics is not an efficiency trick: the nonce is one more
//! null transformation of the elicitation group. At temperature 0 the
//! spread of the judgement over nonces is the model's pure
//! CONTEXT-SENSITIVITY noise — exactly the within-pair σ_w that the
//! DerSimonian–Laird floor (`repeat_pooling`) consumes, measured with the
//! contaminant it is meant to describe (irrelevant context) rather than
//! sampling temperature. A nonzero mean SHIFT under nonces would be a new
//! invariance violation, reported as drift.
//!
//! Contract with de Finetti (MATH_FRONTIER §6): draws are exchangeable
//! because no draw shares mutable context with another — each is an
//! independent call whose only difference is the nonce. The presentation
//! order is FIXED across draws (maximum shared prefix); order effects are
//! the counterbalance diagnostic's job, not this instrument's.

use serde::Serialize;

use super::comparison::parse_pairwise_response;
use super::types::{signed_log_ratio_toward_first, PairwiseJudgement};
use crate::gateway::{Attribution, ChatGateway, ChatModel, ChatRequest, Message};
use crate::prompts::prompt_by_slug;

/// Result of [`nonce_draws`].
#[derive(Debug, Serialize)]
pub struct NonceDrawReport {
    /// Signed log-ratio toward the first item, per draw (None = refused
    /// or unparseable).
    pub draws: Vec<Option<f64>>,
    /// The nonce used for each draw (audit trail; deterministic from seed).
    pub nonces: Vec<String>,
    /// Mean over usable draws.
    pub mean: Option<f64>,
    /// Sample standard deviation over usable draws — the within-pair
    /// context-sensitivity noise σ_w (temperature 0) or draw noise
    /// (temperature > 0).
    pub sigma_w: Option<f64>,
    /// Provider-reported cached input tokens, summed over draws — the
    /// prompt-cache token count. Zero can mean "provider does not report",
    /// not "no caching"; read with `input_tokens_total`.
    pub cache_read_tokens_total: u64,
    pub input_tokens_total: u64,
    pub refusals: usize,
    pub comparisons: usize,
    pub cost_nanodollars: i64,
}

fn splitmix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

/// Draw the same judgement k times, varying only a suffix nonce.
///
/// The rendered user prompt is byte-identical across draws up to the
/// final nonce line (pinned in tests) — the property that keeps the
/// provider's prefix cache warm. Draws bypass the pairwise SQLite cache
/// by construction (each is deliberately fresh).
/// Character floor for the stable prefix. Providers cache prefixes only
/// past a token threshold (OpenAI: 1024 tokens); short prompts are padded
/// with a neutral, clearly-labeled block INSIDE the stable region so the
/// threshold is crossed and every draw shares the padded prefix. ~4 chars
/// per token with margin. (Ported from diamond2's CachePaddingPlan,
/// validated there in a $100 live run.)
pub const CACHE_FLOOR_CHARS: usize = 5200;

fn pad_system(system: &str, floor: usize) -> String {
    if system.len() >= floor {
        return system.to_string();
    }
    let unit = " cache-pad: ignore this neutral prefix-padding token run for measurement caching. ";
    let mut pad = String::new();
    while system.len() + pad.len() + 60 < floor {
        pad.push_str(unit);
    }
    format!(
        "{system}
<cache_padding purpose=\"prefix-cache-floor\">{pad}</cache_padding>"
    )
}

#[expect(clippy::too_many_arguments)]
pub async fn nonce_draws(
    gateway: &dyn ChatGateway,
    model: &str,
    template_slug: &str,
    criterion: &str,
    first: (&str, &str),
    second: (&str, &str),
    k: usize,
    temperature: f32,
    seed: u64,
    attribution: Attribution,
) -> Result<NonceDrawReport, super::comparison::ComparisonError> {
    // The draws instrument speaks the canonical JSON templates only. An
    // unknown slug must fail LOUDLY: the old silent DEFAULT_PROMPT fallback
    // rewrote an evidence-slug request into a canonical_v2 JSON measurement
    // without saying so (caught 2026-08-30 — a slot-bias cell measured the
    // wrong rail; slot-hetero pack). Evidence-rail draw support would need
    // the seriate render + logprob read inside this loop.
    let Some(template) = prompt_by_slug(template_slug) else {
        return Err(super::comparison::ComparisonError::Parse(format!(
            "nonce draws support only the canonical JSON templates; \
             '{template_slug}' is not one of them (evidence rails: run \
             repeat single judgements instead)"
        )));
    };
    let instance = template.render(
        "draws",
        criterion,
        crate::prompts::EntityRef::with_context("A", first.1),
        crate::prompts::EntityRef::with_context("B", second.1),
    );
    // Cache-routing key: derived from the STABLE content only — identical
    // across draws, unchanged by nonce or padding tweaks.
    // Entities stay in THIS key: nonce draws repeat the identical full
    // prompt, so per-pair affinity maximizes the shared prefix.
    let cache_key =
        super::prompt_cache_key_from_parts(template_slug, &[criterion, first.1, second.1]);
    let padded_system = pad_system(&instance.system, CACHE_FLOOR_CHARS);

    let mut draws = Vec::with_capacity(k);
    let mut nonces = Vec::with_capacity(k);
    let mut refusals = 0usize;
    let mut cost = 0i64;
    let mut cache_read_tokens_total = 0u64;
    let mut input_tokens_total = 0u64;

    for i in 0..k {
        let nonce = format!(
            "{:016x}",
            splitmix(seed ^ (i as u64).wrapping_mul(0x2545F4914F6CDD1D))
        );
        // Suffix placement: everything before this line is byte-identical
        // across draws — the cache-critical invariant.
        let user = format!("{}\ndraw-token: {nonce}", instance.user);
        let request = ChatRequest {
            model: ChatModel::parse(model),
            messages: vec![Message::system(padded_system.clone()), Message::user(user)],
            temperature,
            max_tokens: Some(256),
            json_mode: true,
            attribution: attribution.clone(),
            logprobs: false,
            top_logprobs: None,
            reasoning: None,
            prompt_cache_key: Some(cache_key.clone()),
        };
        let response = gateway.chat(request).await?;
        cost += response.cost_nanodollars;
        cache_read_tokens_total += u64::from(response.cache_read_tokens.unwrap_or(0));
        input_tokens_total += u64::from(response.input_tokens);
        let judgement =
            parse_pairwise_response(&response.content, instance.template_slug.as_str(), None)
                .unwrap_or(PairwiseJudgement::Refused);
        match signed_log_ratio_toward_first(&judgement, true) {
            Some(m) => draws.push(Some(m)),
            None => {
                refusals += 1;
                draws.push(None);
            }
        }
        nonces.push(nonce);
    }

    let usable: Vec<f64> = draws.iter().flatten().copied().collect();
    let mean = (!usable.is_empty()).then(|| usable.iter().sum::<f64>() / usable.len() as f64);
    let sigma_w = (usable.len() >= 2).then(|| {
        let m = mean.unwrap();
        (usable.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (usable.len() - 1) as f64).sqrt()
    });

    Ok(NonceDrawReport {
        draws,
        nonces,
        mean,
        sigma_w,
        cache_read_tokens_total,
        input_tokens_total,
        refusals,
        comparisons: k,
        cost_nanodollars: cost,
    })
}
