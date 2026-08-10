//! Decouples semantic search ranking from the embedding backend, so
//! that logic can be unit-tested without a model download or network
//! access.
#![allow(dead_code)] // wired into SearchMode::Semantic in a later commit

use std::cmp::Ordering;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::SearchMatch;
use crate::mcp::workspace_path::{IgnorePattern, WorkspacePath, not_found_or_io};
use rmcp::model::ErrorData as McpError;

/// Turns a line of text into a dense vector for similarity ranking.
/// Implementations must be deterministic: the same input always
/// produces the same output, since [`SemanticSearch`] skips
/// re-embedding unchanged files based on content hash alone.
pub(super) trait Embedder {
    fn embed(&self, text: &str) -> Vec<f32>;
}

/// A single embedded line within an indexed file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(super) struct Chunk {
    pub(super) line: u64,
    pub(super) text: String,
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
pub(super) fn content_hash(content: &str) -> String {
    let digest = Sha256::digest(content.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Semantic search over a workspace: keeps a per-workspace [`SemanticIndex`]
/// current — embedding new or changed files, reusing cached vectors for
/// unchanged ones — then ranks every indexed line under `root` by cosine
/// similarity to the embedded query.
///
/// Ranking is brute-force cosine similarity, not an approximate index: at
/// the scale of one codebase's lines, this is simple, exact, and fast
/// enough. Revisit only if that stops being true.
pub(super) struct SemanticSearch<'a> {
    embedder: &'a dyn Embedder,
}

impl<'a> SemanticSearch<'a> {
    pub(super) fn new(embedder: &'a dyn Embedder) -> Self {
        Self { embedder }
    }

    /// Searches `root` — a file or a directory, walked recursively — and
    /// returns every indexed line ranked best-first by cosine similarity
    /// to `query`. Re-embeds only files that are new or whose content hash
    /// changed since the index was last saved; the index is persisted back
    /// to `<workspace_root>/.kid/semantic-index.json` before returning.
    pub(super) fn run(
        &self,
        query: &str,
        root: &WorkspacePath,
        workspace: &WorkspacePath,
        ignore: &[IgnorePattern],
    ) -> Result<Vec<SearchMatch>, McpError> {
        let index_path = workspace
            .absolute()
            .join(".kid")
            .join("semantic-index.json");
        let mut index = SemanticIndex::load(&index_path)
            .map_err(|err| McpError::internal_error(format!("semantic index: {err}"), None))?;

        // The index persists under `.kid/` inside the workspace itself, so
        // the walk must never index that file — otherwise the index would
        // index its own previous JSON dump, growing without bound. This is
        // enforced here rather than left to the caller-provided `ignore`,
        // since it's not something a workspace owner configures.
        let mut ignore = ignore.to_vec();
        ignore.push("/.kid".parse().expect("literal pattern is valid"));

        let metadata = root.metadata().map_err(|e| not_found_or_io(root, e))?;
        if metadata.is_dir() {
            self.walk_dir(root, 0, &ignore, &mut index)?;
        } else {
            self.index_file(root, &mut index);
        }

        index
            .save(&index_path)
            .map_err(|err| McpError::internal_error(format!("semantic index: {err}"), None))?;

        let query_vector = self.embedder.embed(query);
        let root_prefix = root.relative();
        let mut scored: Vec<(SearchMatch, f32)> = index
            .entries
            .iter()
            .filter(|entry| entry.path.starts_with(root_prefix))
            .flat_map(|entry| {
                entry.chunks.iter().map(|chunk| {
                    let score = cosine_similarity(&query_vector, &chunk.vector);
                    (
                        SearchMatch {
                            file: entry.path.clone(),
                            line: chunk.line,
                            text: chunk.text.clone(),
                        },
                        score,
                    )
                })
            })
            .collect();

        scored.sort_by(|(a, a_score), (b, b_score)| {
            b_score
                .partial_cmp(a_score)
                .unwrap_or(Ordering::Equal)
                .then(a.file.cmp(&b.file))
                .then(a.line.cmp(&b.line))
        });

        Ok(scored.into_iter().map(|(m, _)| m).collect())
    }

    /// Depth-first walk mirroring `FuzzySearch::walk_dir`'s ignore
    /// handling; kept separate for now, unified once search modes share
    /// one walker.
    fn walk_dir(
        &self,
        dir: &WorkspacePath,
        depth: usize,
        ignore: &[IgnorePattern],
        index: &mut SemanticIndex,
    ) -> Result<(), McpError> {
        let entries = dir.read_dir().map_err(|e| not_found_or_io(dir, e))?;
        let mut entries: Vec<_> = entries
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                !ignore
                    .iter()
                    .any(|pattern| pattern.matches_name_at_depth(&name, depth))
            })
            .collect();
        entries.sort_by_key(|entry| entry.file_name());

        for entry in entries {
            let child = dir.child(&entry.file_name());
            if entry.path().is_dir() {
                self.walk_dir(&child, depth + 1, ignore, index)?;
            } else {
                self.index_file(&child, index);
            }
        }
        Ok(())
    }

    /// Brings one file's index entry up to date. Unreadable (typically
    /// non-UTF-8/binary) files are silently skipped, matching
    /// `FuzzySearch::search_file`. A file whose content hash matches the
    /// cached entry is left untouched — its vectors are reused rather than
    /// recomputed. Empty lines are never embedded; they carry no meaning.
    fn index_file(&self, path: &WorkspacePath, index: &mut SemanticIndex) {
        let Ok(content) = path.read_to_string() else {
            return;
        };
        let relative = path.relative().to_path_buf();
        let hash = content_hash(&content);

        if index
            .entries
            .iter()
            .any(|entry| entry.path == relative && entry.hash == hash)
        {
            return;
        }

        let chunks = content
            .lines()
            .enumerate()
            .filter(|(_, line)| !line.trim().is_empty())
            .map(|(number, line)| Chunk {
                line: (number + 1) as u64,
                text: line.trim_end().to_owned(),
                vector: self.embedder.embed(line),
            })
            .collect();

        index.entries.retain(|entry| entry.path != relative);
        index.entries.push(IndexEntry {
            path: relative,
            hash,
            chunks,
        });
    }
}

/// Cosine similarity of two vectors via `innr`; `0.0` (not `NaN`) if
/// either is the zero vector, so a degenerate vector sorts last rather
/// than poisoning the ranking.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let score = innr::cosine(a, b);
    if score.is_nan() { 0.0 } else { score }
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
                    text: "fn main() {}".to_owned(),
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

    use crate::mcp::workspace_path::UnresolvedPath;
    use std::cell::Cell;

    fn workspace_path(root: &Path, relative: &str, ignore: &[IgnorePattern]) -> WorkspacePath {
        UnresolvedPath::new(relative).resolve(root, ignore).unwrap()
    }

    /// Wraps an [`Embedder`] to count how often `embed` is called, so
    /// tests can assert on invalidation behavior (unchanged files must
    /// not be re-embedded) without inspecting `SemanticSearch`'s
    /// internals.
    struct CountingEmbedder<'a> {
        inner: &'a dyn Embedder,
        calls: Cell<usize>,
    }

    impl Embedder for CountingEmbedder<'_> {
        fn embed(&self, text: &str) -> Vec<f32> {
            self.calls.set(self.calls.get() + 1);
            self.inner.embed(text)
        }
    }

    #[test]
    fn ranks_lines_by_similarity_to_query() {
        let dir = assert_fs::TempDir::new().unwrap();
        fs::write(dir.join("f.txt"), "apple banana\nunrelated stuff\n").unwrap();
        let embedder = FakeEmbedder::new(16);
        let search = SemanticSearch::new(&embedder);
        let root = workspace_path(&dir, ".", &[]);

        let matches = search.run("apple banana", &root, &root, &[]).unwrap();

        assert_eq!(matches[0].file, PathBuf::from("f.txt"));
        assert_eq!(matches[0].line, 1);
        assert_eq!(matches[0].text, "apple banana");
    }

    #[test]
    fn unchanged_file_is_not_re_embedded_on_second_run() {
        let dir = assert_fs::TempDir::new().unwrap();
        fs::write(dir.join("f.txt"), "hello world\n").unwrap();
        let fake = FakeEmbedder::new(16);
        let root = workspace_path(&dir, ".", &[]);

        let first = CountingEmbedder {
            inner: &fake,
            calls: Cell::new(0),
        };
        SemanticSearch::new(&first)
            .run("hello", &root, &root, &[])
            .unwrap();

        let second = CountingEmbedder {
            inner: &fake,
            calls: Cell::new(0),
        };
        SemanticSearch::new(&second)
            .run("hello", &root, &root, &[])
            .unwrap();

        // Only the query itself was embedded; the unchanged line was
        // reused from the index rather than recomputed.
        assert_eq!(second.calls.get(), 1);
    }

    #[test]
    fn changed_file_is_re_embedded() {
        let dir = assert_fs::TempDir::new().unwrap();
        fs::write(dir.join("f.txt"), "hello world\n").unwrap();
        let embedder = FakeEmbedder::new(16);
        let root = workspace_path(&dir, ".", &[]);
        let search = SemanticSearch::new(&embedder);
        search.run("hello", &root, &root, &[]).unwrap();

        fs::write(dir.join("f.txt"), "goodbye moon\n").unwrap();
        let matches = search.run("goodbye", &root, &root, &[]).unwrap();

        assert_eq!(matches[0].text, "goodbye moon");
    }

    #[test]
    fn results_are_scoped_to_root_subdirectory() {
        let dir = assert_fs::TempDir::new().unwrap();
        fs::create_dir(dir.join("sub")).unwrap();
        fs::write(dir.join("sub/in.txt"), "target line\n").unwrap();
        fs::write(dir.join("out.txt"), "target line\n").unwrap();
        let embedder = FakeEmbedder::new(16);
        let search = SemanticSearch::new(&embedder);
        let workspace = workspace_path(&dir, ".", &[]);
        search
            .run("target line", &workspace, &workspace, &[])
            .unwrap();

        let sub_root = workspace_path(&dir, "sub", &[]);
        let matches = search
            .run("target line", &sub_root, &workspace, &[])
            .unwrap();

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].file, PathBuf::from("sub/in.txt"));
    }

    #[test]
    fn ignored_paths_are_not_indexed() {
        let dir = assert_fs::TempDir::new().unwrap();
        fs::create_dir(dir.join("target")).unwrap();
        fs::write(dir.join("target/out.txt"), "needle\n").unwrap();
        fs::write(dir.join("keep.txt"), "needle\n").unwrap();
        let ignore: Vec<IgnorePattern> = vec!["target".parse().unwrap()];
        let embedder = FakeEmbedder::new(16);
        let search = SemanticSearch::new(&embedder);
        let root = workspace_path(&dir, ".", &ignore);

        let matches = search.run("needle", &root, &root, &ignore).unwrap();

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].file, PathBuf::from("keep.txt"));
    }
}
