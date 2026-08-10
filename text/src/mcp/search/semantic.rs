//! Decouples semantic search ranking from the embedding backend, so
//! that logic can be unit-tested without a model download or network
//! access.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Turns a line of text into a dense vector for similarity ranking.
/// Implementations must be deterministic: the same input always
/// produces the same output, since `SemanticSearch` skips re-embedding
/// unchanged files based on content hash alone.
#[allow(dead_code)]
pub(super) trait Embedder {
    fn embed(&self, text: &str) -> Vec<f32>;
}

/// A single embedded line within an indexed file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(super) struct Chunk {
    pub(super) line: u64,
    pub(super) vector: Vec<f32>,
}

/// One indexed file: its content hash (for invalidation) and the
/// chunks embedded from it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(super) struct IndexEntry {
    pub(super) path: PathBuf,
    pub(super) hash: String,
    pub(super) chunks: Vec<Chunk>,
}

/// Persisted semantic search index for one workspace: which files
/// were embedded, at what content hash, and their resulting vectors.
/// Stored as a single JSON file under `<workspace_root>/.kid/`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub(super) struct SemanticIndex {
    pub(super) entries: Vec<IndexEntry>,
}

impl SemanticIndex {
    /// Loads the index from `path`. A missing file is not an error —
    /// it means no index has been built yet, so an empty index is
    /// returned.
    #[allow(dead_code)]
    pub(super) fn load(path: &Path) -> io::Result<Self> {
        match fs::read_to_string(path) {
            Ok(contents) => serde_json::from_str(&contents)
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err)),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(err) => Err(err),
        }
    }

    /// Saves the index to `path`, creating parent directories as
    /// needed.
    #[allow(dead_code)]
    pub(super) fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let contents = serde_json::to_string(self)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
        fs::write(path, contents)
    }
}

/// Hex-encoded SHA-256 of `content`, used to detect changed files
/// without re-embedding unchanged ones.
#[allow(dead_code)]
pub(super) fn content_hash(content: &str) -> String {
    let digest = Sha256::digest(content.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
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

    #[test]
    fn loading_missing_index_returns_empty_index() {
        let dir = assert_fs::TempDir::new().unwrap();
        let index = SemanticIndex::load(&dir.join("semantic-index.json")).unwrap();
        assert_eq!(index, SemanticIndex::default());
    }

    #[test]
    fn saved_index_round_trips_through_load() {
        let dir = assert_fs::TempDir::new().unwrap();
        let path = dir.join(".kid").join("semantic-index.json");
        let index = SemanticIndex {
            entries: vec![IndexEntry {
                path: PathBuf::from("src/lib.rs"),
                hash: content_hash("fn main() {}"),
                chunks: vec![Chunk {
                    line: 1,
                    vector: vec![0.1, 0.2, 0.3],
                }],
            }],
        };

        index.save(&path).unwrap();
        let loaded = SemanticIndex::load(&path).unwrap();

        assert_eq!(loaded, index);
    }

    #[test]
    fn same_content_hashes_identically() {
        assert_eq!(content_hash("fn main() {}"), content_hash("fn main() {}"));
    }

    #[test]
    fn different_content_hashes_differently() {
        assert_ne!(content_hash("fn main() {}"), content_hash("struct Foo;"));
    }
}
