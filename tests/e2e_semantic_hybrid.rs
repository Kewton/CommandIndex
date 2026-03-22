mod common;

use std::error::Error;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

use commandindex::indexer::reader::SearchResult;
use commandindex::indexer::symbol_store::{EmbeddingInfo, SymbolStore};
use commandindex::search::hybrid::rrf_merge;

// ---------------------------------------------------------------------------
// Fixed test vectors (4-dimensional)
// ---------------------------------------------------------------------------

const QUERY_VEC: [f32; 4] = [1.0, 0.0, 0.0, 0.0];
const SIMILAR_VEC: [f32; 4] = [0.9, 0.1, 0.0, 0.0]; // cosine ~ 0.994
const DIFFERENT_VEC: [f32; 4] = [0.0, 0.0, 1.0, 0.0]; // cosine = 0.0

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Create a temp directory with two Markdown files and run `commandindex index`.
fn setup_semantic_test_dir() -> Result<(TempDir, PathBuf), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;

    fs::write(
        dir.path().join("alpha.md"),
        "\
---
tags:
  - rust
---
# Alpha Document

This document explains the Rust setup process.

## Installation

Install Rust using rustup.
",
    )?;

    fs::write(
        dir.path().join("beta.md"),
        "\
---
tags:
  - python
---
# Beta Document

This document explains Python package management.

## pip

Use pip to install packages.
",
    )?;

    common::run_index(dir.path());

    let commandindex_dir = dir.path().join(".commandindex");
    Ok((dir, commandindex_dir))
}

/// Open SymbolStore, create tables, and insert embeddings.
fn insert_test_embeddings(
    symbols_db_path: &std::path::Path,
    embeddings: &[EmbeddingInfo],
) -> Result<(), Box<dyn Error>> {
    let store = SymbolStore::open(symbols_db_path)?;
    store.create_tables()?;
    store.insert_embeddings(embeddings)?;
    Ok(())
}

/// Create a minimal commandindex.toml for embedding configuration.
/// Placed at the repo root (parent of .commandindex/).
fn create_test_config(commandindex_dir: &std::path::Path) -> Result<(), Box<dyn Error>> {
    let config_content = "\
[embedding]
provider = \"ollama\"
model = \"nomic-embed-text\"
endpoint = \"http://localhost:11434\"
dimension = 4
";
    // commandindex_dir is .commandindex/, so parent is the repo root
    let base_path = commandindex_dir
        .parent()
        .expect("commandindex_dir should have parent");
    fs::write(base_path.join("commandindex.toml"), config_content)?;
    Ok(())
}

/// Build a SearchResult for rrf_merge tests.
fn make_search_result(path: &str, heading: &str, score: f32) -> SearchResult {
    SearchResult {
        path: path.to_string(),
        heading: heading.to_string(),
        body: String::new(),
        tags: String::new(),
        heading_level: 1,
        line_start: 1,
        score,
    }
}

// ===========================================================================
// Library API layer tests (no Ollama required)
// ===========================================================================

#[test]
fn test_embedding_insert_and_count() {
    let dir = tempfile::tempdir().expect("test_embedding_insert_and_count: create temp dir");
    let db_path = dir.path().join("symbols.db");

    let store =
        SymbolStore::open(&db_path).expect("test_embedding_insert_and_count: open SymbolStore");
    store
        .create_tables()
        .expect("test_embedding_insert_and_count: create tables");

    // Initially zero embeddings
    let count = store
        .count_embeddings()
        .expect("test_embedding_insert_and_count: count before insert");
    assert_eq!(
        count, 0,
        "test_embedding_insert_and_count: should start with 0 embeddings"
    );

    // Insert 2 embeddings
    let embeddings = vec![
        EmbeddingInfo {
            id: None,
            file_path: "alpha.md".to_string(),
            section_heading: "Alpha Document".to_string(),
            embedding: SIMILAR_VEC.to_vec(),
            model_name: "test-model".to_string(),
            file_hash: "hash_a".to_string(),
        },
        EmbeddingInfo {
            id: None,
            file_path: "beta.md".to_string(),
            section_heading: "Beta Document".to_string(),
            embedding: DIFFERENT_VEC.to_vec(),
            model_name: "test-model".to_string(),
            file_hash: "hash_b".to_string(),
        },
    ];
    store
        .insert_embeddings(&embeddings)
        .expect("test_embedding_insert_and_count: insert embeddings");

    let count = store
        .count_embeddings()
        .expect("test_embedding_insert_and_count: count after insert");
    assert_eq!(
        count, 2,
        "test_embedding_insert_and_count: should have 2 embeddings after insert"
    );
}

#[test]
fn test_semantic_search_basic() {
    let dir = tempfile::tempdir().expect("test_semantic_search_basic: create temp dir");
    let db_path = dir.path().join("symbols.db");

    let store = SymbolStore::open(&db_path).expect("test_semantic_search_basic: open SymbolStore");
    store
        .create_tables()
        .expect("test_semantic_search_basic: create tables");

    let embeddings = vec![
        EmbeddingInfo {
            id: None,
            file_path: "alpha.md".to_string(),
            section_heading: "Alpha".to_string(),
            embedding: SIMILAR_VEC.to_vec(),
            model_name: "test-model".to_string(),
            file_hash: "hash_a".to_string(),
        },
        EmbeddingInfo {
            id: None,
            file_path: "beta.md".to_string(),
            section_heading: "Beta".to_string(),
            embedding: DIFFERENT_VEC.to_vec(),
            model_name: "test-model".to_string(),
            file_hash: "hash_b".to_string(),
        },
    ];
    store
        .insert_embeddings(&embeddings)
        .expect("test_semantic_search_basic: insert embeddings");

    // Search with QUERY_VEC - should rank alpha (similar) above beta (different)
    let results = store
        .search_similar(&QUERY_VEC, 10)
        .expect("test_semantic_search_basic: search_similar");

    assert!(
        results.len() >= 2,
        "test_semantic_search_basic: should return at least 2 results, got {}",
        results.len()
    );

    // First result should be alpha.md (higher cosine similarity)
    assert_eq!(
        results[0].file_path, "alpha.md",
        "test_semantic_search_basic: most similar should be alpha.md"
    );
    assert_eq!(
        results[1].file_path, "beta.md",
        "test_semantic_search_basic: least similar should be beta.md"
    );

    // Verify similarity ordering
    assert!(
        results[0].similarity > results[1].similarity,
        "test_semantic_search_basic: alpha similarity ({}) should be greater than beta similarity ({})",
        results[0].similarity,
        results[1].similarity,
    );
}

#[test]
fn test_semantic_search_top_k() {
    let dir = tempfile::tempdir().expect("test_semantic_search_top_k: create temp dir");
    let db_path = dir.path().join("symbols.db");

    let store = SymbolStore::open(&db_path).expect("test_semantic_search_top_k: open SymbolStore");
    store
        .create_tables()
        .expect("test_semantic_search_top_k: create tables");

    // Insert 3 embeddings
    let embeddings = vec![
        EmbeddingInfo {
            id: None,
            file_path: "a.md".to_string(),
            section_heading: "A".to_string(),
            embedding: SIMILAR_VEC.to_vec(),
            model_name: "test-model".to_string(),
            file_hash: "hash_a".to_string(),
        },
        EmbeddingInfo {
            id: None,
            file_path: "b.md".to_string(),
            section_heading: "B".to_string(),
            embedding: DIFFERENT_VEC.to_vec(),
            model_name: "test-model".to_string(),
            file_hash: "hash_b".to_string(),
        },
        EmbeddingInfo {
            id: None,
            file_path: "c.md".to_string(),
            section_heading: "C".to_string(),
            embedding: QUERY_VEC.to_vec(),
            model_name: "test-model".to_string(),
            file_hash: "hash_c".to_string(),
        },
    ];
    store
        .insert_embeddings(&embeddings)
        .expect("test_semantic_search_top_k: insert embeddings");

    // top_k=2 should return only 2 results
    let results = store
        .search_similar(&QUERY_VEC, 2)
        .expect("test_semantic_search_top_k: search_similar with top_k=2");

    assert_eq!(
        results.len(),
        2,
        "test_semantic_search_top_k: top_k=2 should return exactly 2 results"
    );

    // The top 2 should be c.md (exact match, sim=1.0) and a.md (similar)
    assert_eq!(
        results[0].file_path, "c.md",
        "test_semantic_search_top_k: first result should be c.md (exact match)"
    );
    assert_eq!(
        results[1].file_path, "a.md",
        "test_semantic_search_top_k: second result should be a.md (similar)"
    );
}

#[test]
fn test_rrf_merge_integration() {
    // BM25 results: alpha at rank 1, beta at rank 2
    let bm25 = vec![
        make_search_result("alpha.md", "Alpha Document", 10.0),
        make_search_result("beta.md", "Beta Document", 5.0),
    ];

    // Semantic results: beta at rank 1, gamma at rank 2
    let semantic = vec![
        make_search_result("beta.md", "Beta Document", 0.95),
        make_search_result("gamma.md", "Gamma Document", 0.80),
    ];

    let merged = rrf_merge(&bm25, &semantic, 10);

    assert!(
        !merged.is_empty(),
        "test_rrf_merge_integration: merged results should not be empty"
    );

    // beta.md appears in both lists, so it should have the highest RRF score
    // beta: 1/(60+2) [BM25 rank 2] + 1/(60+1) [Semantic rank 1] = 1/62 + 1/61
    // alpha: 1/(60+1) [BM25 rank 1] + 0 = 1/61
    // gamma: 0 + 1/(60+2) = 1/62
    let beta_idx = merged
        .iter()
        .position(|r| r.path == "beta.md")
        .expect("test_rrf_merge_integration: beta.md should be in results");
    let alpha_idx = merged
        .iter()
        .position(|r| r.path == "alpha.md")
        .expect("test_rrf_merge_integration: alpha.md should be in results");
    let gamma_idx = merged
        .iter()
        .position(|r| r.path == "gamma.md")
        .expect("test_rrf_merge_integration: gamma.md should be in results");

    assert!(
        beta_idx < alpha_idx,
        "test_rrf_merge_integration: beta (in both lists) should rank above alpha"
    );
    assert!(
        alpha_idx < gamma_idx,
        "test_rrf_merge_integration: alpha (BM25 rank 1) should rank above gamma (semantic rank 2)"
    );

    // Verify score values
    let beta_score = merged[beta_idx].score;
    let expected_beta = 1.0 / 62.0 + 1.0 / 61.0;
    assert!(
        (beta_score - expected_beta).abs() < 1e-6,
        "test_rrf_merge_integration: beta score {beta_score} should be ~{expected_beta}"
    );
}

// ===========================================================================
// CLI layer tests (no Ollama required)
// ===========================================================================

#[test]
fn test_embed_without_ollama_fails() {
    let (dir, _commandindex_dir) =
        setup_semantic_test_dir().expect("test_embed_without_ollama_fails: setup");

    // Running embed without Ollama available exits successfully but reports
    // failures in stderr warnings and "Failed: N" in stdout.
    let output = common::cmd()
        .args(["embed", "--path", dir.path().to_str().unwrap()])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let stderr = String::from_utf8_lossy(&output.get_output().stderr);

    // Should report failed embeddings in stdout
    assert!(
        stdout.contains("Failed:"),
        "test_embed_without_ollama_fails: stdout should report failed embeddings, got: {stdout}"
    );

    // stderr should contain warning messages about embedding failures
    assert!(
        !stderr.is_empty(),
        "test_embed_without_ollama_fails: stderr should contain warning messages"
    );
}

#[test]
fn test_hybrid_no_semantic() {
    let (dir, _commandindex_dir) =
        setup_semantic_test_dir().expect("test_hybrid_no_semantic: setup");

    // search --no-semantic should use BM25 only and succeed
    let output = common::cmd()
        .args(["search", "Rust", "--no-semantic", "--format", "json"])
        .current_dir(dir.path())
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let results = common::parse_jsonl(&stdout);
    assert!(
        !results.is_empty(),
        "test_hybrid_no_semantic: --no-semantic search for 'Rust' should return results"
    );
}

#[test]
fn test_hybrid_no_embeddings() {
    let (dir, _commandindex_dir) =
        setup_semantic_test_dir().expect("test_hybrid_no_embeddings: setup");

    // Search without any embeddings generated - should not error out
    // (falls back to BM25-only)
    common::cmd()
        .args(["search", "Rust", "--format", "json"])
        .current_dir(dir.path())
        .assert()
        .success();
}

#[test]
fn test_rerank_fallback_via_cli() {
    let (dir, commandindex_dir) =
        setup_semantic_test_dir().expect("test_rerank_fallback_via_cli: setup");

    // Create config so rerank can attempt to load settings
    create_test_config(&commandindex_dir).expect("test_rerank_fallback_via_cli: create config");

    // --rerank without Ollama should fall back gracefully (not crash)
    // It may print a warning to stderr but should still return results
    common::cmd()
        .args(["search", "Rust", "--rerank", "--format", "json"])
        .current_dir(dir.path())
        .assert()
        .success();
}

#[test]
fn test_rerank_top_accepted_via_cli() {
    let (dir, commandindex_dir) =
        setup_semantic_test_dir().expect("test_rerank_top_accepted_via_cli: setup");

    create_test_config(&commandindex_dir).expect("test_rerank_top_accepted_via_cli: create config");

    // --rerank --rerank-top 5 should be accepted as valid arguments
    common::cmd()
        .args([
            "search",
            "Rust",
            "--rerank",
            "--rerank-top",
            "5",
            "--format",
            "json",
        ])
        .current_dir(dir.path())
        .assert()
        .success();
}

#[test]
fn test_context_with_embeddings() {
    let (dir, commandindex_dir) =
        setup_semantic_test_dir().expect("test_context_with_embeddings: setup");

    // Insert embeddings into the symbols.db
    let symbols_db = commandindex_dir.join("symbols.db");
    insert_test_embeddings(
        &symbols_db,
        &[
            EmbeddingInfo {
                id: None,
                file_path: "alpha.md".to_string(),
                section_heading: "Alpha Document".to_string(),
                embedding: SIMILAR_VEC.to_vec(),
                model_name: "test-model".to_string(),
                file_hash: "hash_a".to_string(),
            },
            EmbeddingInfo {
                id: None,
                file_path: "beta.md".to_string(),
                section_heading: "Beta Document".to_string(),
                embedding: DIFFERENT_VEC.to_vec(),
                model_name: "test-model".to_string(),
                file_hash: "hash_b".to_string(),
            },
        ],
    )
    .expect("test_context_with_embeddings: insert embeddings");

    create_test_config(&commandindex_dir).expect("test_context_with_embeddings: create config");

    // context command should work normally even with embeddings present
    let output = common::cmd()
        .args(["context", "alpha.md"])
        .current_dir(dir.path())
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    assert!(
        !stdout.is_empty(),
        "test_context_with_embeddings: context output should not be empty"
    );

    // Should be valid JSON
    let pack: serde_json::Value = serde_json::from_str(&stdout)
        .expect("test_context_with_embeddings: context output should be valid JSON");
    assert!(
        pack.is_object(),
        "test_context_with_embeddings: context output should be a JSON object"
    );
}

// ===========================================================================
// Environment-dependent tests (require Ollama)
// ===========================================================================

#[test]
#[ignore]
fn test_hybrid_auto_switch() {
    // Requires Ollama running locally
    let (dir, commandindex_dir) =
        setup_semantic_test_dir().expect("test_hybrid_auto_switch: setup");

    create_test_config(&commandindex_dir).expect("test_hybrid_auto_switch: create config");

    // Insert fixture embeddings into symbols.db so hybrid path is triggered
    insert_test_embeddings(
        &commandindex_dir.join("symbols.db"),
        &[
            EmbeddingInfo {
                id: None,
                file_path: "alpha.md".to_string(),
                section_heading: "Alpha".to_string(),
                embedding: SIMILAR_VEC.to_vec(),
                model_name: "test-model".to_string(),
                file_hash: "hash_a".to_string(),
            },
            EmbeddingInfo {
                id: None,
                file_path: "beta.md".to_string(),
                section_heading: "Beta".to_string(),
                embedding: DIFFERENT_VEC.to_vec(),
                model_name: "test-model".to_string(),
                file_hash: "hash_b".to_string(),
            },
        ],
    )
    .expect("test_hybrid_auto_switch: insert embeddings");

    // With Ollama running + embeddings present, search should use hybrid mode
    let output = common::cmd()
        .args(["search", "Rust", "--format", "json"])
        .current_dir(dir.path())
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let results = common::parse_jsonl(&stdout);
    assert!(
        !results.is_empty(),
        "test_hybrid_auto_switch: hybrid search should return results"
    );
}

#[test]
#[ignore]
fn test_hybrid_bm25_fallback() {
    // Requires Ollama to be STOPPED — tests BM25 fallback when embeddings exist
    // but query embedding generation fails
    let (dir, commandindex_dir) =
        setup_semantic_test_dir().expect("test_hybrid_bm25_fallback: setup");

    create_test_config(&commandindex_dir).expect("test_hybrid_bm25_fallback: create config");

    // Insert fixture embeddings into symbols.db so hybrid path is triggered
    insert_test_embeddings(
        &commandindex_dir.join("symbols.db"),
        &[
            EmbeddingInfo {
                id: None,
                file_path: "alpha.md".to_string(),
                section_heading: "Alpha".to_string(),
                embedding: SIMILAR_VEC.to_vec(),
                model_name: "test-model".to_string(),
                file_hash: "hash_a".to_string(),
            },
            EmbeddingInfo {
                id: None,
                file_path: "beta.md".to_string(),
                section_heading: "Beta".to_string(),
                embedding: DIFFERENT_VEC.to_vec(),
                model_name: "test-model".to_string(),
                file_hash: "hash_b".to_string(),
            },
        ],
    )
    .expect("test_hybrid_bm25_fallback: insert embeddings");

    // With embeddings present but Ollama stopped, search should fall back to BM25-only
    let output = common::cmd()
        .args(["search", "Rust", "--format", "json"])
        .current_dir(dir.path())
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let results = common::parse_jsonl(&stdout);
    assert!(
        !results.is_empty(),
        "test_hybrid_bm25_fallback: BM25 fallback should still return results"
    );
}
