//! Token counting for prompt budgets.

use tiktoken_rs::cl100k_base;

/// Count tokens in text using the cl100k_base tokenizer.
pub(crate) fn count_tokens(text: &str) -> usize {
    let bpe = cl100k_base().expect("Failed to load cl100k_base tokenizer");
    bpe.encode_with_special_tokens(text).len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_tokens() {
        let count = count_tokens("Hello, world!");
        assert!(count > 0);
        assert!(count < 10);
    }
}
