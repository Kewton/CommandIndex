use std::cell::OnceCell;
use std::collections::{HashMap, HashSet};
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
pub const KNOWLEDGE_GRAPH_WEIGHT: f32 = 0.8;

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

// ---------------------------------------------------------------------------
// Path resolution helpers (Task 1.3)
// ---------------------------------------------------------------------------

/// Resolve an import path (e.g. `@/components/Foo`) to an actual indexed file
/// path (e.g. `src/components/Foo.tsx`).  Returns `None` for external packages
/// or when no match is found.
fn resolve_import_path(import_path: &str, indexed_paths: &HashSet<String>) -> Option<String> {
    // Input validation
    if import_path.is_empty() || import_path.len() > 1024 {
        return None;
    }

    // 1. Exact match
    if indexed_paths.contains(import_path) {
        return Some(import_path.to_string());
    }

    // 2. Strip alias / relative prefixes
    let normalized = import_path
        .trim_start_matches("@/")
        .trim_start_matches("~/")
        .trim_start_matches("./")
        .trim_start_matches("../");

    // If nothing was stripped and it doesn't look like a relative/aliased path,
    // it's likely an external package (e.g. "react", "lodash").
    if normalized == import_path
        && !import_path.starts_with('@')
        && !import_path.starts_with('.')
        && !import_path.starts_with('~')
        && !import_path.contains('/')
    {
        return None;
    }

    // 3. Component-boundary suffix match
    indexed_paths
        .iter()
        .find(|p| path_component_suffix_matches(p, normalized))
        .cloned()
}

/// Check whether `indexed_path` ends with `import_suffix` at a path component
/// boundary (i.e. preceded by `/` or at the start).
///
/// Also handles extension stripping (`.ts`, `.tsx`, `.js`, `.jsx`, `.py`) and
/// `index` file patterns (e.g. `components/Foo/index.ts` matches `components/Foo`).
fn path_component_suffix_matches(indexed_path: &str, import_suffix: &str) -> bool {
    let extensions = [".ts", ".tsx", ".js", ".jsx", ".py"];

    // Strip a known extension from indexed_path to get the "stem"
    let stem = extensions
        .iter()
        .find(|ext| indexed_path.ends_with(*ext))
        .map(|ext| &indexed_path[..indexed_path.len() - ext.len()])
        .unwrap_or(indexed_path);

    let matches_at_boundary = |path: &str, suffix: &str| -> bool {
        path == suffix || path.ends_with(&format!("/{suffix}"))
    };

    // Direct match (with or without extension)
    matches_at_boundary(stem, import_suffix)
        || matches_at_boundary(indexed_path, import_suffix)
        // index file pattern: import_suffix "components/Foo" matches stem "components/Foo/index"
        || matches_at_boundary(stem, &format!("{import_suffix}/index"))
}

/// Resolve a file link target (e.g. `./ci-cd-plan.md`) to an indexed path.
/// Unlike `resolve_import_path`, this handles paths that already have extensions
/// and uses component-boundary matching without the external-package heuristic.
fn resolve_link_path(link_target: &str, indexed_paths: &HashSet<String>) -> Option<String> {
    if link_target.is_empty() || link_target.len() > 1024 {
        return None;
    }

    // 1. Exact match
    if indexed_paths.contains(link_target) {
        return Some(link_target.to_string());
    }

    // 2. Strip relative prefixes
    let normalized = link_target
        .trim_start_matches("./")
        .trim_start_matches("../");

    if normalized.is_empty() {
        return None;
    }

    // 3. Exact match after normalization
    if indexed_paths.contains(normalized) {
        return Some(normalized.to_string());
    }

    // 4. Component-boundary suffix match (the file may be nested deeper)
    let matches_at_boundary = |path: &str, suffix: &str| -> bool {
        path == suffix || path.ends_with(&format!("/{suffix}"))
    };

    indexed_paths
        .iter()
        .find(|p| matches_at_boundary(p, normalized))
        .cloned()
}

// ---------------------------------------------------------------------------
// Score helper (Task 1.4)
// ---------------------------------------------------------------------------

/// Add or accumulate a relation score for a given path.
/// Deduplicates relation types by discriminant.
fn add_relation(
    scores: &mut HashMap<String, (f32, Vec<RelationType>)>,
    path: &str,
    weight: f32,
    relation: RelationType,
) {
    let entry = scores.entry(path.to_string()).or_insert((0.0, Vec::new()));
    entry.0 += weight;
    if !entry
        .1
        .iter()
        .any(|r| std::mem::discriminant(r) == std::mem::discriminant(&relation))
    {
        entry.1.push(relation);
    }
}

// ---------------------------------------------------------------------------
// RelatedSearchEngine
// ---------------------------------------------------------------------------

pub struct RelatedSearchEngine<'a> {
    reader: &'a IndexReaderWrapper,
    store: &'a SymbolStore,
    indexed_paths: OnceCell<HashSet<String>>,
}

impl<'a> RelatedSearchEngine<'a> {
    pub fn new(reader: &'a IndexReaderWrapper, store: &'a SymbolStore) -> Self {
        Self {
            reader,
            store,
            indexed_paths: OnceCell::new(),
        }
    }

    /// Lazily load all indexed paths from tantivy.
    fn get_indexed_paths(&self) -> Result<&HashSet<String>, RelatedSearchError> {
        if let Some(paths) = self.indexed_paths.get() {
            return Ok(paths);
        }
        let paths = self.reader.all_indexed_paths()?;
        // If another call already initialized it, just return what's there.
        let _ = self.indexed_paths.set(paths);
        Ok(self.indexed_paths.get().unwrap())
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

        // 3. Knowledge graph (Issue-based document relationships)
        self.score_knowledge_graph(&target, &mut scores)?;

        // 4. Tag match
        self.score_tag_match(&target, &mut scores)?;

        // 5. Path proximity (uses all known paths from scores + tantivy)
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
                snippet: None,
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
        let indexed_paths = self.get_indexed_paths()?;

        // Files that the target links to (outgoing)
        let outgoing = self.store.find_file_links_by_source(target)?;
        for link in &outgoing {
            // Resolve target_file to an indexed path if it doesn't match directly
            let resolved = if indexed_paths.contains(&link.target_file) {
                link.target_file.clone()
            } else if let Some(r) = resolve_link_path(&link.target_file, indexed_paths) {
                r
            } else {
                continue;
            };
            add_relation(
                scores,
                &resolved,
                MARKDOWN_LINK_WEIGHT,
                RelationType::MarkdownLink,
            );
        }

        // Files that link to the target (incoming)
        let incoming = self.store.find_file_links_by_target(target)?;
        for link in &incoming {
            add_relation(
                scores,
                &link.source_file,
                MARKDOWN_LINK_WEIGHT,
                RelationType::MarkdownLink,
            );
        }

        // Also check all file_links whose target_file resolves to our target
        // (handles cases where target_file was stored as a relative path)
        let all_links = self.store.find_all_file_links()?;
        for link in &all_links {
            if link.source_file == target {
                continue; // already handled above
            }
            let resolved = if indexed_paths.contains(&link.target_file) {
                link.target_file.clone()
            } else if let Some(r) = resolve_link_path(&link.target_file, indexed_paths) {
                r
            } else {
                continue;
            };
            if resolved == target {
                add_relation(
                    scores,
                    &link.source_file,
                    MARKDOWN_LINK_WEIGHT,
                    RelationType::MarkdownLink,
                );
            }
        }

        Ok(())
    }

    pub(crate) fn score_import_deps(
        &self,
        target: &str,
        scores: &mut HashMap<String, (f32, Vec<RelationType>)>,
    ) -> Result<(), RelatedSearchError> {
        let indexed_paths = self.get_indexed_paths()?;

        // Forward direction: what the target file imports
        let imports = self.store.find_imports_by_source(target)?;
        for imp in &imports {
            // Resolve the import path to an actual indexed file path
            if let Some(resolved) = resolve_import_path(&imp.target_module, indexed_paths) {
                add_relation(
                    scores,
                    &resolved,
                    IMPORT_DEP_WEIGHT,
                    RelationType::ImportDependency,
                );
            }
        }

        // Reverse direction: files that import the target
        // We need to check all imports and see which ones resolve to the target
        let all_imports = self.store.find_all_imports()?;
        for imp in &all_imports {
            if imp.source_file == target {
                continue; // already handled above
            }
            if let Some(resolved) = resolve_import_path(&imp.target_module, indexed_paths)
                && resolved == target
            {
                add_relation(
                    scores,
                    &imp.source_file,
                    IMPORT_DEP_WEIGHT,
                    RelationType::ImportDependency,
                );
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

    /// Score files related through the knowledge graph (Issue-based document relationships).
    pub(crate) fn score_knowledge_graph(
        &self,
        target: &str,
        scores: &mut HashMap<String, (f32, Vec<RelationType>)>,
    ) -> Result<(), RelatedSearchError> {
        let related = self
            .store
            .find_knowledge_related(target)
            .map_err(RelatedSearchError::SymbolStore)?;
        for result in related {
            add_relation(
                scores,
                &result.file_path,
                KNOWLEDGE_GRAPH_WEIGHT,
                RelationType::KnowledgeGraph,
            );
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
                    add_relation(
                        scores,
                        path,
                        DIR_PROXIMITY_WEIGHT,
                        RelationType::DirectoryProximity,
                    );
                } else if target_dir.len() >= 2
                    && path_dir.len() >= 2
                    && target_dir[..target_dir.len() - 1] == path_dir[..path_dir.len() - 1]
                {
                    // Parent directory is common (1 level up)
                    add_relation(
                        scores,
                        path,
                        DIR_PROXIMITY_1UP_WEIGHT,
                        RelationType::DirectoryProximity,
                    );
                }
            }

            // Path segment similarity: different roots but same sub-directory names
            if target_dir != path_dir {
                let target_set: HashSet<&str> = target_segments
                    [..target_segments.len().saturating_sub(1)]
                    .iter()
                    .copied()
                    .collect();
                let path_set: HashSet<&str> = path_segments
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

    // -----------------------------------------------------------------------
    // normalize_path tests (existing)
    // -----------------------------------------------------------------------

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

    // -----------------------------------------------------------------------
    // resolve_import_path tests (Task 2.1)
    // -----------------------------------------------------------------------

    fn make_indexed_paths(paths: &[&str]) -> HashSet<String> {
        paths.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn test_resolve_import_path_exact_match() {
        let indexed = make_indexed_paths(&["src/utils.ts", "src/main.ts"]);
        assert_eq!(
            resolve_import_path("src/utils.ts", &indexed),
            Some("src/utils.ts".to_string())
        );
    }

    #[test]
    fn test_resolve_import_path_relative() {
        let indexed = make_indexed_paths(&["src/utils.ts", "src/main.ts"]);
        assert_eq!(
            resolve_import_path("./utils", &indexed),
            Some("src/utils.ts".to_string())
        );
    }

    #[test]
    fn test_resolve_import_path_alias() {
        let indexed = make_indexed_paths(&["src/components/Button.tsx", "src/main.ts"]);
        assert_eq!(
            resolve_import_path("@/components/Button", &indexed),
            Some("src/components/Button.tsx".to_string())
        );
    }

    #[test]
    fn test_resolve_import_path_tilde_alias() {
        let indexed = make_indexed_paths(&["src/components/Button.tsx"]);
        assert_eq!(
            resolve_import_path("~/components/Button", &indexed),
            Some("src/components/Button.tsx".to_string())
        );
    }

    #[test]
    fn test_resolve_import_path_external_package_none() {
        let indexed = make_indexed_paths(&["src/utils.ts", "src/main.ts"]);
        assert_eq!(resolve_import_path("react", &indexed), None);
        assert_eq!(resolve_import_path("lodash", &indexed), None);
    }

    #[test]
    fn test_resolve_import_path_index_ts_pattern() {
        let indexed = make_indexed_paths(&["src/components/Foo/index.ts"]);
        assert_eq!(
            resolve_import_path("@/components/Foo", &indexed),
            Some("src/components/Foo/index.ts".to_string())
        );
    }

    #[test]
    fn test_resolve_import_path_empty() {
        let indexed = make_indexed_paths(&["src/utils.ts"]);
        assert_eq!(resolve_import_path("", &indexed), None);
    }

    #[test]
    fn test_resolve_import_path_too_long() {
        let indexed = make_indexed_paths(&["src/utils.ts"]);
        let long = "a".repeat(1025);
        assert_eq!(resolve_import_path(&long, &indexed), None);
    }

    #[test]
    fn test_resolve_import_path_dotdot_relative() {
        let indexed = make_indexed_paths(&["src/helper.ts"]);
        assert_eq!(
            resolve_import_path("../helper", &indexed),
            Some("src/helper.ts".to_string())
        );
    }

    // -----------------------------------------------------------------------
    // path_component_suffix_matches tests (Task 2.1)
    // -----------------------------------------------------------------------

    #[test]
    fn test_suffix_match_auth_vs_oauth() {
        // "auth" should NOT match "src/oauth.ts" (no component boundary)
        assert!(!path_component_suffix_matches("src/oauth.ts", "auth"));
    }

    #[test]
    fn test_suffix_match_auth_matches() {
        assert!(path_component_suffix_matches("src/auth.ts", "auth"));
    }

    #[test]
    fn test_suffix_match_extension_complement() {
        assert!(path_component_suffix_matches("src/utils.ts", "utils"));
        assert!(path_component_suffix_matches("src/Button.tsx", "Button"));
        assert!(path_component_suffix_matches("src/app.js", "app"));
        assert!(path_component_suffix_matches("src/app.jsx", "app"));
        assert!(path_component_suffix_matches("lib/main.py", "main"));
    }

    #[test]
    fn test_suffix_match_full_path_with_ext() {
        assert!(path_component_suffix_matches(
            "src/components/Foo.tsx",
            "components/Foo"
        ));
    }

    #[test]
    fn test_suffix_match_index_pattern() {
        assert!(path_component_suffix_matches(
            "src/components/Foo/index.ts",
            "components/Foo"
        ));
    }

    #[test]
    fn test_suffix_match_no_false_substring() {
        // "bar" should not match "src/foobar.ts"
        assert!(!path_component_suffix_matches("src/foobar.ts", "bar"));
    }

    // -----------------------------------------------------------------------
    // add_relation tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_add_relation_accumulates_score() {
        let mut scores: HashMap<String, (f32, Vec<RelationType>)> = HashMap::new();
        add_relation(&mut scores, "file.ts", 0.5, RelationType::ImportDependency);
        add_relation(&mut scores, "file.ts", 0.3, RelationType::ImportDependency);
        let entry = scores.get("file.ts").unwrap();
        assert!((entry.0 - 0.8).abs() < 0.001);
        // Should not duplicate the relation type
        assert_eq!(entry.1.len(), 1);
    }

    #[test]
    fn test_add_relation_different_types() {
        let mut scores: HashMap<String, (f32, Vec<RelationType>)> = HashMap::new();
        add_relation(&mut scores, "file.ts", 0.5, RelationType::ImportDependency);
        add_relation(&mut scores, "file.ts", 1.0, RelationType::MarkdownLink);
        let entry = scores.get("file.ts").unwrap();
        assert!((entry.0 - 1.5).abs() < 0.001);
        assert_eq!(entry.1.len(), 2);
    }

    // -----------------------------------------------------------------------
    // resolve_link_path tests (markdown link resolution)
    // -----------------------------------------------------------------------

    #[test]
    fn test_resolve_link_path_exact_match() {
        let indexed = make_indexed_paths(&["docs/b.md", "docs/a.md"]);
        assert_eq!(
            resolve_link_path("docs/b.md", &indexed),
            Some("docs/b.md".to_string())
        );
    }

    #[test]
    fn test_resolve_link_path_relative_dot_slash() {
        let indexed = make_indexed_paths(&["docs/ci-cd-plan.md"]);
        assert_eq!(
            resolve_link_path("./ci-cd-plan.md", &indexed),
            Some("docs/ci-cd-plan.md".to_string())
        );
    }

    #[test]
    fn test_resolve_link_path_relative_dotdot() {
        let indexed = make_indexed_paths(&["src/c.ts"]);
        assert_eq!(
            resolve_link_path("../src/c.ts", &indexed),
            Some("src/c.ts".to_string())
        );
    }

    #[test]
    fn test_resolve_link_path_bare_filename() {
        let indexed = make_indexed_paths(&["docs/b.md"]);
        assert_eq!(
            resolve_link_path("b.md", &indexed),
            Some("docs/b.md".to_string())
        );
    }

    #[test]
    fn test_resolve_link_path_nonexistent() {
        let indexed = make_indexed_paths(&["docs/a.md"]);
        assert_eq!(resolve_link_path("nonexistent.md", &indexed), None);
    }

    #[test]
    fn test_resolve_link_path_empty() {
        let indexed = make_indexed_paths(&["docs/a.md"]);
        assert_eq!(resolve_link_path("", &indexed), None);
    }
}
