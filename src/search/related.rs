use std::collections::HashMap;
use std::fmt;

use crate::indexer::reader::{IndexReaderWrapper, ReaderError};
use crate::indexer::symbol_store::{SymbolStore, SymbolStoreError};
use crate::output::{RelatedSearchResult, RelationType};

// Score weight constants
pub const MARKDOWN_LINK_WEIGHT: f32 = 1.0;
pub const IMPORT_DEP_WEIGHT: f32 = 0.9;
pub const TAG_MATCH_WEIGHT: f32 = 0.5;
pub const PATH_SIMILARITY_WEIGHT: f32 = 0.4;
pub const DIR_PROXIMITY_WEIGHT: f32 = 0.2;
pub const DIR_PROXIMITY_1UP_WEIGHT: f32 = 0.1;

#[derive(Debug)]
pub enum RelatedSearchError {
    Reader(ReaderError),
    SymbolStore(SymbolStoreError),
    FileNotFound(String),
    FileNotIndexed(String),
}

impl fmt::Display for RelatedSearchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RelatedSearchError::Reader(e) => write!(f, "{e}"),
            RelatedSearchError::SymbolStore(e) => write!(f, "{e}"),
            RelatedSearchError::FileNotFound(path) => {
                write!(f, "File not found: {path}")
            }
            RelatedSearchError::FileNotIndexed(path) => {
                write!(
                    f,
                    "File not indexed: {path}. Run `commandindex index` first."
                )
            }
        }
    }
}

impl std::error::Error for RelatedSearchError {}

impl From<ReaderError> for RelatedSearchError {
    fn from(e: ReaderError) -> Self {
        RelatedSearchError::Reader(e)
    }
}

impl From<SymbolStoreError> for RelatedSearchError {
    fn from(e: SymbolStoreError) -> Self {
        RelatedSearchError::SymbolStore(e)
    }
}

/// Normalize a file path for consistent matching.
pub(crate) fn normalize_path(path: &str) -> Result<String, RelatedSearchError> {
    if path.is_empty() {
        return Err(RelatedSearchError::FileNotFound("empty path".to_string()));
    }
    if path.len() > 1024 {
        return Err(RelatedSearchError::FileNotFound(
            "path too long (max 1024 characters)".to_string(),
        ));
    }
    let path = path.strip_prefix("./").unwrap_or(path);
    let path = path.replace('\\', "/");
    let path = path.trim_end_matches('/');
    let components: Vec<&str> = path
        .split('/')
        .filter(|c| !c.is_empty() && *c != "." && *c != "..")
        .collect();
    Ok(components.join("/"))
}

pub struct RelatedSearchEngine<'a> {
    reader: &'a IndexReaderWrapper,
    store: &'a SymbolStore,
}

impl<'a> RelatedSearchEngine<'a> {
    pub fn new(reader: &'a IndexReaderWrapper, store: &'a SymbolStore) -> Self {
        Self { reader, store }
    }

    pub fn find_related(
        &self,
        target_path: &str,
        limit: usize,
    ) -> Result<Vec<RelatedSearchResult>, RelatedSearchError> {
        let target = normalize_path(target_path)?;

        // Collect scores from all sources into a single HashMap
        let mut scores: HashMap<String, (f32, Vec<RelationType>)> = HashMap::new();

        // 1. Markdown links (bidirectional)
        self.score_markdown_links(&target, &mut scores)?;

        // 2. Import dependencies (bidirectional)
        self.score_import_deps(&target, &mut scores)?;

        // 3. Tag match
        self.score_tag_match(&target, &mut scores)?;

        // 4. Path proximity (uses all known paths from scores + tantivy)
        self.score_path_proximity(&target, &mut scores);

        // Remove self from results
        scores.remove(&target);

        // Convert to results and sort by score descending
        let mut results: Vec<RelatedSearchResult> = scores
            .into_iter()
            .map(|(path, (score, relation_types))| RelatedSearchResult {
                file_path: path,
                score,
                relation_types,
            })
            .collect();

        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);

        Ok(results)
    }

    pub(crate) fn score_markdown_links(
        &self,
        target: &str,
        scores: &mut HashMap<String, (f32, Vec<RelationType>)>,
    ) -> Result<(), RelatedSearchError> {
        // Files that the target links to (outgoing)
        let outgoing = self.store.find_file_links_by_source(target)?;
        for link in &outgoing {
            let entry = scores
                .entry(link.target_file.clone())
                .or_insert((0.0, Vec::new()));
            entry.0 += MARKDOWN_LINK_WEIGHT;
            if !entry
                .1
                .iter()
                .any(|r| matches!(r, RelationType::MarkdownLink))
            {
                entry.1.push(RelationType::MarkdownLink);
            }
        }

        // Files that link to the target (incoming)
        let incoming = self.store.find_file_links_by_target(target)?;
        for link in &incoming {
            let entry = scores
                .entry(link.source_file.clone())
                .or_insert((0.0, Vec::new()));
            entry.0 += MARKDOWN_LINK_WEIGHT;
            if !entry
                .1
                .iter()
                .any(|r| matches!(r, RelationType::MarkdownLink))
            {
                entry.1.push(RelationType::MarkdownLink);
            }
        }

        Ok(())
    }

    pub(crate) fn score_import_deps(
        &self,
        target: &str,
        scores: &mut HashMap<String, (f32, Vec<RelationType>)>,
    ) -> Result<(), RelatedSearchError> {
        // What the target file imports (source -> target direction)
        let imports = self.store.find_imports_by_source(target)?;
        for imp in &imports {
            let entry = scores
                .entry(imp.target_module.clone())
                .or_insert((0.0, Vec::new()));
            entry.0 += IMPORT_DEP_WEIGHT;
            if !entry
                .1
                .iter()
                .any(|r| matches!(r, RelationType::ImportDependency))
            {
                entry.1.push(RelationType::ImportDependency);
            }
        }

        // Files that import the target (target -> source direction)
        let all_deps = self.store.find_imports_by_target(target)?;
        for dep in &all_deps {
            let entry = scores
                .entry(dep.source_file.clone())
                .or_insert((0.0, Vec::new()));
            entry.0 += IMPORT_DEP_WEIGHT;
            if !entry
                .1
                .iter()
                .any(|r| matches!(r, RelationType::ImportDependency))
            {
                entry.1.push(RelationType::ImportDependency);
            }
        }

        Ok(())
    }

    pub(crate) fn score_tag_match(
        &self,
        target: &str,
        scores: &mut HashMap<String, (f32, Vec<RelationType>)>,
    ) -> Result<(), RelatedSearchError> {
        // Get tags for the target file from tantivy
        let target_docs = self.reader.search_by_exact_path(target)?;
        if target_docs.is_empty() {
            return Ok(());
        }

        // Collect unique tags from all sections of the target file
        let mut target_tags: Vec<String> = Vec::new();
        for doc in &target_docs {
            for tag in doc.tags.split_whitespace() {
                if !tag.is_empty() && !target_tags.contains(&tag.to_string()) {
                    target_tags.push(tag.to_string());
                }
            }
        }

        if target_tags.is_empty() {
            return Ok(());
        }

        // Search for each tag in tantivy to find other files with matching tags
        for tag in &target_tags {
            if let Ok(tag_results) = self.reader.search(tag, 100) {
                for result in &tag_results {
                    let path = &result.path;
                    if path == target {
                        continue;
                    }
                    // Check actual tag match (not just full-text match)
                    let result_tags: Vec<&str> = result.tags.split_whitespace().collect();
                    let matched: Vec<String> = target_tags
                        .iter()
                        .filter(|t| result_tags.contains(&t.as_str()))
                        .cloned()
                        .collect();
                    if !matched.is_empty() {
                        let entry = scores.entry(path.clone()).or_insert((0.0, Vec::new()));
                        // Only add tag score if not already added for this file
                        let already_tagged = entry
                            .1
                            .iter()
                            .any(|r| matches!(r, RelationType::TagMatch { .. }));
                        if !already_tagged {
                            entry.0 += TAG_MATCH_WEIGHT * matched.len() as f32;
                            entry.1.push(RelationType::TagMatch {
                                matched_tags: matched,
                            });
                        }
                    }
                }
            }
        }

        Ok(())
    }

    pub(crate) fn score_path_proximity(
        &self,
        target: &str,
        scores: &mut HashMap<String, (f32, Vec<RelationType>)>,
    ) {
        let target_segments: Vec<&str> = target.split('/').collect();
        let target_dir = if target_segments.len() > 1 {
            &target_segments[..target_segments.len() - 1]
        } else {
            &[]
        };

        // Get all known file paths from current scores
        let known_paths: Vec<String> = scores.keys().cloned().collect();

        for path in &known_paths {
            let path_segments: Vec<&str> = path.split('/').collect();
            let path_dir = if path_segments.len() > 1 {
                &path_segments[..path_segments.len() - 1]
            } else {
                &[]
            };

            // Directory proximity: same directory or 1 level up
            if !target_dir.is_empty() && !path_dir.is_empty() {
                if target_dir == path_dir {
                    // Same directory
                    let entry = scores.get_mut(path).unwrap();
                    entry.0 += DIR_PROXIMITY_WEIGHT;
                    if !entry
                        .1
                        .iter()
                        .any(|r| matches!(r, RelationType::DirectoryProximity))
                    {
                        entry.1.push(RelationType::DirectoryProximity);
                    }
                } else if target_dir.len() >= 2
                    && path_dir.len() >= 2
                    && target_dir[..target_dir.len() - 1] == path_dir[..path_dir.len() - 1]
                {
                    // Parent directory is common (1 level up)
                    let entry = scores.get_mut(path).unwrap();
                    entry.0 += DIR_PROXIMITY_1UP_WEIGHT;
                    if !entry
                        .1
                        .iter()
                        .any(|r| matches!(r, RelationType::DirectoryProximity))
                    {
                        entry.1.push(RelationType::DirectoryProximity);
                    }
                }
            }

            // Path segment similarity: different roots but same sub-directory names
            if target_dir != path_dir {
                let target_set: std::collections::HashSet<&str> = target_segments
                    [..target_segments.len().saturating_sub(1)]
                    .iter()
                    .copied()
                    .collect();
                let path_set: std::collections::HashSet<&str> = path_segments
                    [..path_segments.len().saturating_sub(1)]
                    .iter()
                    .copied()
                    .collect();
                let common: Vec<&&str> = target_set.intersection(&path_set).collect();
                if !common.is_empty() {
                    let entry = scores.get_mut(path).unwrap();
                    // Only add if not already from directory proximity
                    if !entry
                        .1
                        .iter()
                        .any(|r| matches!(r, RelationType::PathSimilarity))
                    {
                        entry.0 += PATH_SIMILARITY_WEIGHT;
                        entry.1.push(RelationType::PathSimilarity);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_path_basic() {
        assert_eq!(normalize_path("src/main.rs").unwrap(), "src/main.rs");
        assert_eq!(normalize_path("./src/main.rs").unwrap(), "src/main.rs");
        assert_eq!(normalize_path("src/main.rs/").unwrap(), "src/main.rs");
    }

    #[test]
    fn test_normalize_path_backslash() {
        assert_eq!(normalize_path("src\\main.rs").unwrap(), "src/main.rs");
    }

    #[test]
    fn test_normalize_path_dotdot() {
        assert_eq!(normalize_path("src/../lib.rs").unwrap(), "src/lib.rs");
    }

    #[test]
    fn test_normalize_path_empty() {
        assert!(normalize_path("").is_err());
    }

    #[test]
    fn test_normalize_path_too_long() {
        let long_path = "a/".repeat(600);
        assert!(normalize_path(&long_path).is_err());
    }

    #[test]
    fn test_path_proximity_same_dir() {
        let target_segments: Vec<&str> = "src/auth/handler.ts".split('/').collect();
        let other_segments: Vec<&str> = "src/auth/utils.ts".split('/').collect();
        let target_dir = &target_segments[..target_segments.len() - 1];
        let other_dir = &other_segments[..other_segments.len() - 1];
        assert_eq!(target_dir, other_dir);
    }

    #[test]
    fn test_path_segment_similarity() {
        let target_set: std::collections::HashSet<&str> = ["docs", "auth"].into_iter().collect();
        let other_set: std::collections::HashSet<&str> = ["src", "auth"].into_iter().collect();
        let common: Vec<&&str> = target_set.intersection(&other_set).collect();
        assert_eq!(common.len(), 1);
        assert!(common.contains(&&"auth"));
    }
}
