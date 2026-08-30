use super::*;

/// Default max output tokens for pairwise judgements.
///
/// Reasoning-capable models can spend a large hidden budget before they emit the
/// visible JSON answer. A small cap suppresses judgement quality and can yield
/// empty visible output on OpenRouter.
pub const PAIRWISE_MAX_OUTPUT_TOKENS_DEFAULT: u32 = 8192;
pub const PAIRWISE_MAX_OUTPUT_TOKENS_GPT5: u32 = PAIRWISE_MAX_OUTPUT_TOKENS_DEFAULT;
/// Measured on canonical_v2, 2026-08-29: gpt-5.4-mini 27 mean / 32 max;
/// gpt-5.6-terra (the default judge) 74 mean / 96 max. Sized to terra's
/// max; the cap still bounds the worst case. The PMF rail ignores this
/// (16-token single-letter answers).
pub const PAIRWISE_TYPICAL_OUTPUT_TOKENS: u32 = 96;
pub const PAIRWISE_LOGPROBS_TOP_N_DEFAULT: u32 = 20;
pub const PAIRWISE_BUCKET_LOGPROB_MAX_ATTEMPTS: usize = 3;

pub fn pairwise_max_output_tokens(model: &str) -> u32 {
    // Serve-side contexts can be smaller than the default budget (e.g. a local
    // vLLM judge with a tight KV pool rejects max_tokens > max_model_len).
    if let Some(cap) = std::env::var("CARDINAL_PAIRWISE_MAX_OUTPUT_TOKENS")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|value| *value >= 1)
    {
        return cap;
    }
    if model.starts_with("openai/gpt-5") {
        PAIRWISE_MAX_OUTPUT_TOKENS_GPT5
    } else {
        PAIRWISE_MAX_OUTPUT_TOKENS_DEFAULT
    }
}

/// The seriate single-token rail's measured logprob route for a model
/// (docs/LOGPROBS.md matrix: probed 2026-07-18/19, re-census 2026-08-13).
/// `None`: no measured path — the PMF rail is never the *default* there,
/// and an explicit evidence slug degrades loudly to sampled mode.
#[derive(Clone, Copy, Debug)]
pub struct SeriateLogprobRoute {
    /// Provider alternative-count cap. Requests must clamp to it: OpenAI
    /// 400s over-cap, but many OpenRouter hosts return 200 with
    /// `logprobs: null` — a silent loss of the whole PMF.
    pub top_n: u32,
    /// Pin `reasoning: disabled` on the call: the 5.5/5.6 families 400 on
    /// logprobs at any other effort, and on a reasoning-by-default model
    /// the 16-token single-letter budget burns as hidden reasoning.
    pub pin_reasoning_off: bool,
}

pub fn seriate_logprob_route(model: &str) -> Option<SeriateLogprobRoute> {
    let m = model.to_ascii_lowercase();
    if m.starts_with("openai/gpt-4.1") || m.starts_with("openai/gpt-4o") {
        return Some(SeriateLogprobRoute {
            top_n: 20,
            pin_reasoning_off: false,
        });
    }
    // 5.x families serve exactly 5 alternatives at reasoning effort "none"
    // (10/10 on 5.5 and 5.6-sol with the unlock, 0/1 without; 20/20 on
    // 5.4 either way). gpt-5.5-pro, the gpt-5 base family, and the
    // o-series have no path at all.
    let serves_five = ["gpt-5.1", "gpt-5.2", "gpt-5.4", "gpt-5.5", "gpt-5.6"]
        .iter()
        .any(|family| {
            m.strip_prefix("openai/")
                .is_some_and(|m| m.starts_with(family))
        });
    if serves_five && !m.starts_with("openai/gpt-5.5-pro") {
        return Some(SeriateLogprobRoute {
            top_n: 5,
            pin_reasoning_off: true,
        });
    }
    None
}

pub fn pairwise_logprobs_top_n() -> u32 {
    std::env::var("CARDINAL_PAIRWISE_LOGPROBS_TOP_N")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|value| (1..=50).contains(value))
        .unwrap_or(PAIRWISE_LOGPROBS_TOP_N_DEFAULT)
}

// =============================================================================
// JSON parsing
// =============================================================================

/// Error type for comparison operations.
#[derive(Debug, thiserror::Error)]
pub enum ComparisonError {
    #[error("Provider error: {0}")]
    Provider(#[from] ProviderError),
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("Cache error: {0}")]
    Cache(#[from] CacheError),
    #[error("Cache miss: {0}")]
    CacheMiss(String),
}

impl ComparisonError {
    pub(crate) fn next_non_retryable_streak(&self, current: usize) -> usize {
        let retryable = match self {
            Self::Provider(error) => error.is_retryable(),
            Self::Parse(_) | Self::Cache(_) | Self::CacheMiss(_) => false,
        };
        if retryable {
            0
        } else {
            current.saturating_add(1)
        }
    }
}

/// Usage info for a single LLM comparison call.
#[derive(Debug, Clone)]
pub struct ComparisonUsage {
    pub input_tokens: u32,
    /// Provider-reported cache-read (discounted) input tokens, when the
    /// provider surfaces them. The family sweep's economics live here.
    pub cache_read_tokens: Option<u32>,
    pub output_tokens: u32,
    pub provider_cost_nanodollars: i64,
    pub provider_cost_is_estimate: bool,
    pub cached: bool,
    /// Provider-reported served model for the live call, when surfaced
    /// (e.g. Claude Code modelUsage). None for cached judgements.
    pub served_model: Option<String>,
    pub prompt_text: Option<String>,
    /// Content identity of the exact system and user message bytes sent to the judge.
    pub rendered_prompt_digest: String,
    pub question_text: Option<String>,
    pub raw_output: Option<String>,
    pub output_logprobs: Option<Vec<TokenLogprob>>,
    /// PMF-derived log-ratio moments (ratio-letter path); the solver
    /// consumes these as explicit-precision observations when present.
    pub evidence_moments: Option<EvidenceMoments>,
    pub pairwise_logprob_posterior: Option<PairwiseLogprobPosterior>,
    /// Raw ledger draw trajectories + grammar version (decimal-ledger path
    /// only, live rows fused through the exact-atom ledger). This is the
    /// estimator-replay seam: `decimal_ledger::analyze` over these draws
    /// reproduces the judgement's moments and certificate bit-for-bit.
    pub ledger_draws: Option<decimal_ledger::LedgerDrawsRecord>,
}

#[derive(Debug, Clone, Copy)]
pub struct PairwiseComparisonEntity<'a> {
    pub id: &'a str,
    pub text: &'a str,
}

#[derive(Debug, Clone, Copy)]
pub struct PairwiseComparisonAttribute<'a> {
    pub id: &'a str,
    pub prompt: &'a str,
    pub prompt_template_slug: Option<&'a str>,
}

#[derive(Debug, Clone, Copy)]
pub struct PairwiseComparisonSpec<'a> {
    pub model: &'a str,
    pub attribute: PairwiseComparisonAttribute<'a>,
    pub entity_a: PairwiseComparisonEntity<'a>,
    pub entity_b: PairwiseComparisonEntity<'a>,
}

impl PairwiseComparisonSpec<'_> {
    #[must_use]
    pub fn prompt_template(self) -> PromptTemplate {
        self.attribute
            .prompt_template_slug
            .and_then(prompt_by_slug)
            .unwrap_or(DEFAULT_PROMPT)
    }

    #[must_use]
    pub fn prompt_instance(self) -> PromptInstance {
        if let Some(slug) = self
            .attribute
            .prompt_template_slug
            .filter(|slug| is_evidence_slug(slug))
        {
            let instrument = evidence_instrument_for_slug(slug);
            let attribute =
                crate::seriate::Attribute::new(self.attribute.id, self.attribute.prompt);
            let entity_a = crate::seriate::Entity::new(self.entity_a.text);
            let entity_b = crate::seriate::Entity::new(self.entity_b.text);
            let rendered = instrument.render(&attribute, &entity_a, &entity_b);
            return PromptInstance {
                template_slug: slug.to_string(),
                system: rendered.system,
                user: rendered.user,
            };
        }

        self.prompt_template().render(
            self.attribute.id,
            self.attribute.prompt,
            EntityRef::with_context("A", self.entity_a.text),
            EntityRef::with_context("B", self.entity_b.text),
        )
    }
    /// Content identity of the exact system and user message bytes this spec renders.
    #[must_use]
    pub fn rendered_prompt_digest(self) -> String {
        self.prompt_instance().rendered_digest()
    }

    #[must_use]
    pub fn cache_key(self) -> PairwiseCacheKey {
        // The ratio-letter path renders via seriate; its cache identity is
        // the seriate template hash, not a cardinal template.
        let (slug, template_hash) = if let Some(slug) = self
            .attribute
            .prompt_template_slug
            .filter(|slug| is_evidence_slug(slug))
        {
            (slug, seriate_template_fingerprint(slug).to_string())
        } else {
            let template = self.prompt_template();
            (template.slug, template.template_hash())
        };
        PairwiseCacheKey::from_parts(PairwiseCacheKeyParts {
            model: self.model,
            prompt_template: PairwiseCacheTemplate {
                slug,
                template_hash: &template_hash,
            },
            attribute: PairwiseCacheAttribute {
                id: self.attribute.id,
                prompt: self.attribute.prompt,
            },
            entity_a: PairwiseCacheEntity {
                id: self.entity_a.id,
                text: self.entity_a.text,
            },
            entity_b: PairwiseCacheEntity {
                id: self.entity_b.id,
                text: self.entity_b.text,
            },
        })
    }
}

#[derive(Debug, Clone)]
pub struct PairwiseComparisonRequest<'a> {
    pub spec: PairwiseComparisonSpec<'a>,
    pub cache_only: bool,
    pub attribution: Attribution,
}

/// Prompt-template slug for the seriate single-token ratio-letter
/// instrument: one completion position's top-k logprobs ARE the judgement
/// PMF. Rendering and parsing are delegated to `seriate` — cardinal never
/// duplicates the prompt text, so the two cannot drift.
pub const RATIO_LETTER_SLUG: &str = "ratio_letter_v1";

/// Attribute-LAST twin of [`RATIO_LETTER_SLUG`]: entities first, attribute
/// last, so the pair prefix is byte-stable across attribute variants and
/// provider prefix caches serve family sweeps ({A, A′, ¬A} on one judged
/// pair) at cached-input prices. Same alphabet, parser, and evidence
/// currency; distinct template hash and cache identity (NORTH E10).
pub const RATIO_LETTER_ATTR_LAST_SLUG: &str = "ratio_letter_attrlast_v1";

/// Prompt-template slug for the seriate single-token ORDINAL instrument:
/// a three-token alphabet (A / B / =) whose answer-position logprobs give
/// a calibrated direction PMF. The cheapest evidence instrument; direction
/// enters the solver at a fixed modest magnitude with PMF-carried
/// uncertainty.
pub const ORDINAL_LETTER_SLUG: &str = "ordinal_letter_v1";

/// True when the slug routes through the seriate evidence path.
pub fn is_evidence_slug(slug: &str) -> bool {
    slug == RATIO_LETTER_SLUG || slug == RATIO_LETTER_ATTR_LAST_SLUG || slug == ORDINAL_LETTER_SLUG
}

/// The one slug → seriate instrument map (rendering, parsing, and
/// fingerprinting all route through here so a new instrument is one arm).
pub(super) fn evidence_instrument_for_slug(
    slug: &str,
) -> Box<dyn crate::seriate::instrument::Instrument> {
    use crate::seriate::instrument::{ordinal, ratio_letter};
    match slug {
        ORDINAL_LETTER_SLUG => Box::new(ordinal::OrdinalInstrument),
        RATIO_LETTER_ATTR_LAST_SLUG => Box::new(ratio_letter::RatioLetterAttrLastInstrument),
        _ => Box::new(ratio_letter::RatioLetterInstrument),
    }
}

/// Prompt-template slug for the decimal-ledger evidence instrument
/// (research instrument): free-form decimal ratio elicited at temperature 1
/// across K redraws; per-draw exact chosen-token logprobs plus top-k
/// sidebands are fused into a credal exact-atom ledger whose (E\[Z\], var)
/// enter the solver as measured precision. Kernel in
/// [`super::decimal_ledger`]; evidence pack notes/decimal-pmf-2026-08-10.
pub const DECIMAL_LEDGER_SLUG: &str = "decimal_ledger_v1";

/// Redraws per decimal-ledger judgement. 8 sits on the measured
/// efficiency curve (SHOOTOUT.md: exact-mass harvesting beats frequency
/// MC once atoms accumulate; HARVEST.md closed envelopes to ~0.1 log
/// units in 25-40 draws on live providers, and 8 keeps per-judgement
/// cost within one order of the point path).
pub(crate) const DECIMAL_LEDGER_DRAWS: usize = 8;

/// PMF-derived log-ratio moments for one judgement, in PRESENTED
/// (A-over-B) coordinates. Carried alongside the point judgement so the
/// solver can weight by measured variance instead of stated confidence.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EvidenceMoments {
    /// Expected signed log-ratio (positive: presented slot A has more).
    pub log_ratio_mean: f64,
    /// Variance of the signed log-ratio under the judgement PMF.
    pub log_ratio_var: f64,
    /// Probability mass visible at the answer position.
    pub visible_mass: f64,
    /// True when the PMF came from logprobs; false when from a sampled
    /// point (loud degradation).
    pub logprob_mode: bool,
    /// Credal-envelope lower bound on the signed log-ratio (decimal-ledger
    /// evidence only). Persisted so the interval certificate survives the
    /// (mean, var) collapse: a post-solve audit can ask whether adversarial
    /// resolution of unattributed probability mass could flip a ranking
    /// (coherence review F5, 2026-08-11).
    pub e_lo: Option<f64>,
    /// Credal-envelope upper bound (see `e_lo`).
    pub e_hi: Option<f64>,
    /// Probability mass the token-layer enumeration failed to attribute
    /// (1 − Σ cells); doubles as a provider-jitter detector.
    pub conservation_gap: Option<f64>,
}

/// Stable template fingerprint for the ratio-letter path, derived from the
/// seriate instrument's own content-addressed template hash (so a change to
/// seriate's prompt text changes cardinal's cache identity automatically).
pub(super) fn seriate_template_fingerprint(slug: &str) -> &'static str {
    use std::sync::OnceLock;
    static RATIO: OnceLock<String> = OnceLock::new();
    static RATIO_ATTR_LAST: OnceLock<String> = OnceLock::new();
    static ORDINAL: OnceLock<String> = OnceLock::new();
    fn compute(slug: &str) -> String {
        let attribute = crate::seriate::Attribute::new("fingerprint", "fingerprint");
        let a = crate::seriate::Entity::new("A");
        let b = crate::seriate::Entity::new("B");
        evidence_instrument_for_slug(slug)
            .render(&attribute, &a, &b)
            .template
            .0
             .0
            .clone()
    }
    let cell = match slug {
        ORDINAL_LETTER_SLUG => &ORDINAL,
        RATIO_LETTER_ATTR_LAST_SLUG => &RATIO_ATTR_LAST,
        _ => &RATIO,
    };
    cell.get_or_init(|| compute(slug))
}
