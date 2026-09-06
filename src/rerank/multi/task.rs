#[derive(Clone, Copy)]
pub(super) struct CompareTask {
    pub(super) key: (usize, usize, usize),
    pub(super) attr_idx: usize,
    pub(super) i: usize,
    pub(super) j: usize,
    /// When true, entity j is presented as "A" and entity i as "B"
    /// to counteract position bias.
    pub(super) swapped: bool,
}

#[derive(Clone)]
pub(super) struct TraceFields {
    pub(super) attribute_prompt_hash: String,
    pub(super) prompt_template_slug: String,
    pub(super) template_hash: String,
    pub(super) rendered_prompt_digest: String,
    /// Exact bytes behind `rendered_prompt_digest` (recomputability: the
    /// store retains the bytes, not just their hash).
    pub(super) rendered_prompt: crate::rerank::trace::RenderedPromptBytes,
    pub(super) entity_a_hash: String,
    pub(super) entity_b_hash: String,
    pub(super) cache_key_hash: String,
}
