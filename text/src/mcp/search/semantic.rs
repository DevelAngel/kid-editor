//! Decouples semantic search ranking from the embedding backend, so
//! that logic can be unit-tested without a model download or network
//! access.

/// Turns a line of text into a dense vector for similarity ranking.
/// Implementations must be deterministic: the same input always
/// produces the same output, since `SemanticSearch` skips re-embedding
/// unchanged files based on content hash alone.
#[allow(dead_code)]
pub(super) trait Embedder {
    fn embed(&self, text: &str) -> Vec<f32>;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deterministic, hash-based bag-of-words embedder. Not a real
    /// semantic model: same words in any order produce the same
    /// vector, so it can't distinguish meaning, only shared
    /// vocabulary. That's enough to exercise indexing, invalidation,
    /// and ranking logic without a model download.
    struct FakeEmbedder {
        dims: usize,
    }

    impl FakeEmbedder {
        fn new(dims: usize) -> Self {
            Self { dims }
        }

        fn hash(word: &str) -> u64 {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            word.hash(&mut hasher);
            hasher.finish()
        }
    }

    impl Embedder for FakeEmbedder {
        fn embed(&self, text: &str) -> Vec<f32> {
            let mut vector = vec![0.0; self.dims];
            for word in text.split_whitespace() {
                let bucket = Self::hash(word) as usize % self.dims;
                vector[bucket] += 1.0;
            }
            vector
        }
    }

    #[test]
    fn same_text_embeds_identically() {
        let embedder = FakeEmbedder::new(16);
        assert_eq!(embedder.embed("hello world"), embedder.embed("hello world"));
    }

    #[test]
    fn different_words_embed_differently() {
        let embedder = FakeEmbedder::new(16);
        assert_ne!(
            embedder.embed("hello world"),
            embedder.embed("goodbye moon")
        );
    }

    #[test]
    fn word_order_does_not_change_bag_of_words_vector() {
        let embedder = FakeEmbedder::new(16);
        assert_eq!(embedder.embed("hello world"), embedder.embed("world hello"));
    }
}
