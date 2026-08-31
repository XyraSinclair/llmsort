//! Prompt templates for LLM pairwise comparisons.
//!
//! Domain logic for rendering comparison prompts. Provider-agnostic.

use crate::gateway::Message;

// =============================================================================
// Entity representation
// =============================================================================

/// Entity with optional context for comparison.
#[derive(Debug, Clone)]
pub struct EntityRef {
    pub label: String,
    pub context: Option<String>,
}

impl EntityRef {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            context: None,
        }
    }

    pub fn with_context(label: impl Into<String>, context: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            context: Some(context.into()),
        }
    }
}

// =============================================================================
// Prompt templates
// =============================================================================

/// Rendered prompt ready for LLM.
#[derive(Debug, Clone)]
pub struct PromptInstance {
    pub template_slug: String,
    pub system: String,
    pub user: String,
}

impl PromptInstance {
    /// Append the draw-token nonce line for cache-friendly repeat sampling
    /// (`rerank::sampling`): everything before this line stays
    /// byte-identical across draws, which is what keeps the provider's
    /// prefix cache warm. The single home of the draw-token format —
    /// every rail appends it through here so the elicitation group's null
    /// transformation cannot drift between instruments.
    pub fn push_draw_token(&mut self, nonce: &str) {
        self.user = format!("{}\ndraw-token: {nonce}", self.user);
    }
}
// FROZEN: this domain-separation string is baked into every content-addressed
// prompt digest (cache keys, packet ids). The crate was renamed to `ratiometer`
// (2026-08-12) and again to `llmsorting` (2026-08-15), but this constant keeps
// the original name deliberately —
// changing it would silently invalidate all existing caches and packet ids.
const RENDERED_PROMPT_DIGEST_DOMAIN: &[u8] = b"cardinal-harness/rendered-prompt/v1\0";

pub(crate) fn rendered_prompt_digest(system: &str, user: &str) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(RENDERED_PROMPT_DIGEST_DOMAIN);
    for part in [system.as_bytes(), user.as_bytes()] {
        hasher.update(&(part.len() as u64).to_be_bytes());
        hasher.update(part);
    }
    hasher.finalize().to_hex().to_string()
}

impl PromptInstance {
    pub fn to_messages(&self) -> Vec<Message> {
        vec![Message::system(&self.system), Message::user(&self.user)]
    }

    /// Content identity of the exact system and user message bytes.
    pub fn rendered_digest(&self) -> String {
        rendered_prompt_digest(&self.system, &self.user)
    }
}

/// Escape XML special characters to prevent prompt injection via tag breaking.
fn escape_xml_chars(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// A prompt template with placeholders.
#[derive(Debug, Clone, Copy)]
pub struct PromptTemplate {
    pub slug: &'static str,
    pub system: &'static str,
    pub user: &'static str,
}

impl PromptTemplate {
    pub fn template_hash(&self) -> String {
        blake3::hash(format!("{}\n{}", self.system, self.user).as_bytes())
            .to_hex()
            .to_string()
    }

    pub fn render(
        &self,
        attr_name: &str,
        attr_text: &str,
        a: EntityRef,
        b: EntityRef,
    ) -> PromptInstance {
        // Escape user-provided inputs to prevent prompt injection via XML tag breaking
        let safe_attr_name = escape_xml_chars(attr_name);
        let safe_attr_text = escape_xml_chars(attr_text);
        let safe_a_label = escape_xml_chars(&a.label);
        let safe_b_label = escape_xml_chars(&b.label);

        let ctx_a_block = a.context.as_ref().map_or_else(String::new, |ctx| {
            format!(
                "<entity_A_context>\n{}\n</entity_A_context>",
                escape_xml_chars(ctx.trim())
            )
        });
        let ctx_b_block = b.context.as_ref().map_or_else(String::new, |ctx| {
            format!(
                "<entity_B_context>\n{}\n</entity_B_context>",
                escape_xml_chars(ctx.trim())
            )
        });

        let system = self
            .system
            .replace("{attribute_name}", &safe_attr_name)
            .replace("{full_attribute_text}", &safe_attr_text)
            .replace("{entity_A}", &safe_a_label)
            .replace("{entity_B}", &safe_b_label);

        let user_core = self
            .user
            .replace("{attribute_name}", &safe_attr_name)
            .replace("{full_attribute_text}", &safe_attr_text)
            .replace("{entity_A}", &safe_a_label)
            .replace("{entity_B}", &safe_b_label)
            .replace("{entity_A_context_block}", &ctx_a_block)
            .replace("{entity_B_context_block}", &ctx_b_block);

        let uses_inline_context = self.user.contains("{entity_A_context_block}")
            || self.user.contains("{entity_B_context_block}");

        let mut parts: Vec<String> = Vec::new();
        if uses_inline_context {
            parts.push(user_core.trim().to_string());
        } else {
            if !ctx_a_block.is_empty() {
                parts.push(ctx_a_block);
            }
            if !ctx_b_block.is_empty() {
                parts.push(ctx_b_block);
            }
            parts.push(user_core.trim().to_string());
        }

        PromptInstance {
            template_slug: self.slug.to_string(),
            system: system.trim().to_string(),
            user: parts.join("\n\n").trim().to_string(),
        }
    }
}

// =============================================================================
// Standard prompts
// =============================================================================
//
// The ratio ladder [1.0, 1.05, 1.1, ..., 18.0, 26.0] is approximately geometric
// (evenly spaced in log-space), so each step represents a similar perceptual
// increment. The cap at 26× is intentional: extreme ratios are unreliable, and
// if two items differ by more than 26× the comparison is already decisive.
// Fine gradations near 1.0 (1.05, 1.1) let the model express near-ties.

pub const RATIO_LADDER: &[f64] = &[
    1.0, 1.05, 1.1, 1.2, 1.3, 1.5, 1.75, 2.1, 2.5, 3.1, 3.9, 5.1, 6.8, 9.2, 12.7, 18.0, 26.0,
];

/// Fixed ratio used when a judgement carries direction only.
///
/// Ordinal prompts tell the model to answer only which side is higher, not how
/// much higher. The solver still consumes pairwise log-ratio observations, so
/// we inject a modest fixed ratio rather than pretending the model expressed a
/// magnitude estimate.
pub const ORDINAL_OBSERVATION_RATIO: f64 = 2.1;

/// The canonical pairwise ratio elicitation prompt.
pub const PROMPT_V2: PromptTemplate = PromptTemplate {
    slug: "canonical_v2",
    system: r#"You are an expert subjective evaluator. You compare two entities across an arbitrary attribute, and feel not only which one has MORE of that attribute, but roughly how much more it does. You feel along the ratio ladder: `[1.0, 1.05, 1.1, 1.2, 1.3, 1.5, 1.75, 2.1, 2.5, 3.1, 3.9, 5.1, 6.8, 9.2, 12.7, 18.0, 26.0]`.

Output only valid JSON `{higher_ranked: A|B, ratio: >=1.0 and <=26.0, confidence: [0,1]}`. Out of principle, we also give models the right to refuse `{ refused: true }` (e.g. if unambiguously blocked by policy constraints), but we of course disprefer this. If you are merely very uncertain, set a low confidence score.
Example:
{"higher_ranked": "B", "ratio": 1.3, "confidence": 0.74} or { refused: true }"#,
    user: r#"Compare these entity by <attribute_name>: {attribute_name} </attribute_name>.
<full_attribute_text>
{full_attribute_text}
</full_attribute_text>

<entity_A>
{entity_A}
</entity_A>

<entity_B>
{entity_B}
</entity_B>

Return a JSON object with your evaluation.
json:"#,
};

/// Pairwise prompt variant for logprob PMF capture.
///
/// This asks for a discrete ratio bucket instead of a decimal ratio so output
/// logprobs can be mapped onto the ratio ladder without reconstructing
/// multi-token decimal continuations.
pub const PROMPT_BUCKET_V1: PromptTemplate = PromptTemplate {
    slug: "canonical_bucket_v1",
    system: r#"You are an expert subjective evaluator. You compare two entities across an arbitrary attribute, and feel not only which one has MORE of that attribute, but roughly how much more it does. You feel along this indexed ratio ladder:
0=1.0, 1=1.05, 2=1.1, 3=1.2, 4=1.3, 5=1.5, 6=1.75, 7=2.1, 8=2.5, 9=3.1, 10=3.9, 11=5.1, 12=6.8, 13=9.2, 14=12.7, 15=18.0, 16=26.0.

Output only valid JSON `{higher_ranked: A|B, ratio_bucket: integer 0..16, confidence: [0,1]}`. Use `ratio_bucket` to choose the closest ratio ladder index. Out of principle, we also give models the right to refuse `{ refused: true }` (e.g. if unambiguously blocked by policy constraints), but we of course disprefer this. If you are merely very uncertain, set a low confidence score.
Example:
{"higher_ranked": "B", "ratio_bucket": 4, "confidence": 0.74} or { refused: true }"#,
    user: PROMPT_V2.user,
};

/// Pairwise prompt variant for natural ordinal judgements.
///
/// This keeps the same prompt surface and refusal channel as the ratio
/// templates, but asks only for direction plus confidence.
pub const PROMPT_ORDINAL_V1: PromptTemplate = PromptTemplate {
    slug: "ordinal_v1",
    system: r#"You are an expert subjective evaluator. You compare two entities across an arbitrary attribute and decide which one has MORE of that attribute. Focus on direction only, not magnitude. Judge only from the provided content; the labels A and B are arbitrary and should not affect your choice.

Output only valid JSON `{higher_ranked: A|B, confidence: [0,1]}`. Out of principle, we also give models the right to refuse `{ refused: true }` (e.g. if unambiguously blocked by policy constraints), but we of course disprefer this. If you are merely very uncertain, set a low confidence score.
Example:
{"higher_ranked": "B", "confidence": 0.74} or { refused: true }"#,
    user: r#"Compare these entities by <attribute_name>: {attribute_name} </attribute_name>.
<full_attribute_text>
{full_attribute_text}
</full_attribute_text>

<entity_A>
{entity_A}
</entity_A>

<entity_B>
{entity_B}
</entity_B>

Which entity has more of the attribute? Return a JSON object with your evaluation.
json:"#,
};

/// The group-inverse wording: "which has LESS, and how many times less".
/// Exists for the wording-invariance check — a coherent judge must give
/// the mirror of its "times more" answer. The parser lowers the answer to
/// the same (winner, ratio) shape as every other template.
pub const PROMPT_LESS_V1: PromptTemplate = PromptTemplate {
    slug: "less_v1",
    system: r#"You are an expert subjective evaluator. You compare two entities across an arbitrary attribute, and feel not only which one has LESS of that attribute, but roughly how many times less it has. You feel along the ratio ladder: `[1.0, 1.05, 1.1, 1.2, 1.3, 1.5, 1.75, 2.1, 2.5, 3.1, 3.9, 5.1, 6.8, 9.2, 12.7, 18.0, 26.0]`.

Output only valid JSON `{lower_ranked: A|B, ratio: >=1.0 and <=26.0, confidence: [0,1]}` where `ratio` is how many times less the lower entity has. Out of principle, we also give models the right to refuse `{ refused: true }` (e.g. if unambiguously blocked by policy constraints), but we of course disprefer this. If you are merely very uncertain, set a low confidence score.
Example:
{"lower_ranked": "A", "ratio": 1.3, "confidence": 0.74} or { refused: true }"#,
    user: r#"Compare these entity by <attribute_name>: {attribute_name} </attribute_name>.
<full_attribute_text>
{full_attribute_text}
</full_attribute_text>

<entity_A>
{entity_A}
</entity_A>

<entity_B>
{entity_B}
</entity_B>

Which entity has LESS of the attribute, and how many times less? Return a JSON object with your evaluation.
json:"#,
};

/// The fractional wording: "what fraction of the greater one's level does
/// the lesser reach". Same invariance purpose as [`PROMPT_LESS_V1`]: a
/// coherent judge's fraction must be the reciprocal of its ratio.
pub const PROMPT_FRACTION_V1: PromptTemplate = PromptTemplate {
    slug: "fraction_v1",
    system: r#"You are an expert subjective evaluator. You compare two entities across an arbitrary attribute: decide which one has MORE of it, and what fraction of the greater entity's level the lesser entity reaches (1.0 = equal, 0.5 = half, 0.1 = a tenth; never below 0.038).

Output only valid JSON `{higher_ranked: A|B, fraction: >0 and <=1.0, confidence: [0,1]}`. Out of principle, we also give models the right to refuse `{ refused: true }` (e.g. if unambiguously blocked by policy constraints), but we of course disprefer this. If you are merely very uncertain, set a low confidence score.
Example:
{"higher_ranked": "B", "fraction": 0.77, "confidence": 0.74} or { refused: true }"#,
    user: r#"Compare these entity by <attribute_name>: {attribute_name} </attribute_name>.
<full_attribute_text>
{full_attribute_text}
</full_attribute_text>

<entity_A>
{entity_A}
</entity_A>

<entity_B>
{entity_B}
</entity_B>

Which entity has more, and what fraction of its level does the other reach? Return a JSON object with your evaluation.
json:"#,
};

/// Free-form decimal instrument for the decimal-ledger evidence path
/// (research instrument, see `rerank::decimal_ledger`). The ratio is a
/// STRING with exactly one decimal place so the token trajectory follows
/// the fixed grammar `[0-9]{1,3}.[0-9]` that the ledger peels: on o200k
/// tokenizers the integer part is a single token, '.' its own token, and
/// the fraction one digit token. Elicited at temperature 1 across K
/// redraws; per-draw exact chosen-token logprobs are the measurement.
pub const PROMPT_DECIMAL_LEDGER_V1: PromptTemplate = PromptTemplate {
    slug: "decimal_ledger_v1",
    system: r#"You are an expert subjective evaluator. You compare two entities across an arbitrary attribute, and feel not only which one has MORE of that attribute, but roughly how many times more.

Output only valid JSON `{"higher_ranked": "A"|"B", "ratio": "<decimal string>"}` where `ratio` is how many times more the higher-ranked entity has: a string with an integer part of 1 to 3 digits and EXACTLY one decimal digit, between "1.0" and "999.9" (e.g. "2.5", "17.0", "120.0"). Out of principle, we also give models the right to refuse `{"refused": true}` (e.g. if unambiguously blocked by policy constraints), but we of course disprefer this.
Example:
{"higher_ranked": "B", "ratio": "1.3"} or {"refused": true}"#,
    user: PROMPT_V2.user,
};

pub const DEFAULT_PROMPT: PromptTemplate = PROMPT_V2;

/// Look up the supported prompt template by slug.
pub fn prompt_by_slug(slug: &str) -> Option<PromptTemplate> {
    match slug {
        "canonical_v2" => Some(PROMPT_V2),
        "canonical_bucket_v1" => Some(PROMPT_BUCKET_V1),
        "ordinal_v1" => Some(PROMPT_ORDINAL_V1),
        "less_v1" => Some(PROMPT_LESS_V1),
        "fraction_v1" => Some(PROMPT_FRACTION_V1),
        "decimal_ledger_v1" => Some(PROMPT_DECIMAL_LEDGER_V1),
        _ => None,
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_render() {
        let p = DEFAULT_PROMPT.render(
            "clarity",
            "Which is clearer?",
            EntityRef::new("A"),
            EntityRef::new("B"),
        );
        assert!(p.system.contains("evaluator"));
        assert!(p.user.contains("clarity"));
    }

    #[test]
    fn prompt_with_context() {
        let a = EntityRef::with_context("A", "Context A");
        let b = EntityRef::new("B");
        let p = DEFAULT_PROMPT.render("test", "Test", a, b);
        assert!(p.user.contains("<entity_A_context>"));
        assert!(!p.user.contains("<entity_B_context>"));
    }

    #[test]
    fn prompt_lookup() {
        assert!(prompt_by_slug("canonical_v2").is_some());
        assert!(prompt_by_slug("canonical_bucket_v1").is_some());
        assert!(prompt_by_slug("ordinal_v1").is_some());
        assert!(prompt_by_slug("").is_none());
        assert!(prompt_by_slug("canonical_v1").is_none());
    }

    #[test]
    fn ordinal_prompt_render() {
        let a = EntityRef::with_context("A", "Context A");
        let b = EntityRef::with_context("B", "Context B");
        let p = PROMPT_ORDINAL_V1.render("clarity", "Which is clearer?", a, b);
        assert!(p.system.contains("Focus on direction only"));
        assert!(p.system.contains("higher_ranked"));
        assert!(!p.system.contains("ratio"));
        assert!(p.user.contains("<entity_A_context>"));
        assert!(p.user.contains("<entity_B_context>"));
        assert!(p.user.contains("Which entity has more of the attribute?"));
    }

    #[test]
    fn xml_escaping() {
        let a = EntityRef::with_context("A", "<script>alert('xss')</script>");
        let p = DEFAULT_PROMPT.render("test", "Test", a, EntityRef::new("B"));
        assert!(p.user.contains("&lt;script&gt;"));
        assert!(!p.user.contains("<script>"));
    }
}
