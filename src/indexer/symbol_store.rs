use std::fmt;
use std::path::Path;

use rusqlite::{Connection, params};

const CURRENT_SYMBOL_SCHEMA_VERSION: u32 = 4;

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// A single code symbol (function, struct, method, etc.) stored in the symbol database.
#[derive(Debug, Clone, PartialEq)]
pub struct SymbolInfo {
    pub id: Option<i64>,
    pub name: String,
    pub kind: String,
    pub file_path: String,
    pub line_start: u32,
    pub line_end: u32,
    pub parent_symbol_id: Option<i64>,
    pub file_hash: String,
}

/// A file-to-file link record (WikiLink or MarkdownLink) stored in the symbol database.
#[derive(Debug, Clone, PartialEq)]
pub struct FileLinkInfo {
    pub id: Option<i64>,
    pub source_file: String,
    pub target_file: String,
    pub link_type: String, // "WikiLink" / "MarkdownLink"
    pub file_hash: String,
}

/// An import / dependency record linking a source file to the module it imports.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportInfo {
    pub id: Option<i64>,
    pub source_file: String,
    pub target_module: String,
    pub imported_names: Option<String>,
    pub file_hash: String,
}

/// Embedding格納用の情報構造体
#[derive(Debug, Clone, PartialEq)]
pub struct EmbeddingInfo {
    pub id: Option<i64>,
    pub file_path: String,
    pub section_heading: String, // 空文字 = ファイル全体
    pub embedding: Vec<f32>,
    pub model_name: String,
    pub file_hash: String,
}

/// コサイン類似度検索の結果構造体
#[derive(Debug, Clone, PartialEq)]
pub struct EmbeddingSimilarityResult {
    pub file_path: String,
    pub section_heading: String,
    pub similarity: f32,
}

/// ナレッジグラフ Issue → ドキュメント検索の結果構造体
#[derive(Debug, Clone, PartialEq)]
pub struct KnowledgeDocResult {
    pub issue_number: String,
    pub relation: crate::indexer::knowledge::KnowledgeRelation,
    pub file_path: String,
    pub title: Option<String>,
}

// ---------------------------------------------------------------------------
// Row mapping helpers
// ---------------------------------------------------------------------------

/// Map a SQLite row to a [`SymbolInfo`]. The row must contain columns in the
/// order: id, name, kind, file_path, line_start, line_end, parent_symbol_id, file_hash.
fn symbol_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SymbolInfo> {
    Ok(SymbolInfo {
        id: Some(row.get(0)?),
        name: row.get(1)?,
        kind: row.get(2)?,
        file_path: row.get(3)?,
        line_start: row.get(4)?,
        line_end: row.get(5)?,
        parent_symbol_id: row.get(6)?,
        file_hash: row.get(7)?,
    })
}

/// Map a SQLite row to a [`FileLinkInfo`]. The row must contain columns in the
/// order: id, source_file, target_file, link_type, file_hash.
fn file_link_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<FileLinkInfo> {
    Ok(FileLinkInfo {
        id: Some(row.get(0)?),
        source_file: row.get(1)?,
        target_file: row.get(2)?,
        link_type: row.get(3)?,
        file_hash: row.get(4)?,
    })
}

/// Map a SQLite row to an [`ImportInfo`]. The row must contain columns in the
/// order: id, source_file, target_module, imported_names, file_hash.
fn import_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ImportInfo> {
    Ok(ImportInfo {
        id: Some(row.get(0)?),
        source_file: row.get(1)?,
        target_module: row.get(2)?,
        imported_names: row.get(3)?,
        file_hash: row.get(4)?,
    })
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur when operating on the symbol store.
#[derive(Debug)]
pub enum SymbolStoreError {
    Sqlite(rusqlite::Error),
    Io(std::io::Error),
    SchemaVersionMismatch { expected: u32, found: u32 },
    InvalidEmbedding { reason: String },
}

impl fmt::Display for SymbolStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sqlite(e) => write!(f, "SQLite error: {e}"),
            Self::Io(e) => write!(f, "IO error: {e}"),
            Self::SchemaVersionMismatch { expected, found } => {
                write!(
                    f,
                    "Schema version mismatch: expected {expected}, found {found}"
                )
            }
            Self::InvalidEmbedding { reason } => {
                write!(f, "Invalid embedding: {reason}")
            }
        }
    }
}

impl std::error::Error for SymbolStoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Sqlite(e) => Some(e),
            Self::Io(e) => Some(e),
            Self::SchemaVersionMismatch { .. } => None,
            Self::InvalidEmbedding { .. } => None,
        }
    }
}

impl From<rusqlite::Error> for SymbolStoreError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Sqlite(e)
    }
}

impl From<std::io::Error> for SymbolStoreError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Escape LIKE pattern special characters (`%`, `_`, `\`) for safe use in SQL LIKE queries.
pub fn escape_like_pattern(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '\\' => result.push_str("\\\\"),
            '%' => result.push_str("\\%"),
            '_' => result.push_str("\\_"),
            other => result.push(other),
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Embedding helpers
// ---------------------------------------------------------------------------

/// Convert a `Vec<f32>` embedding to a little-endian BLOB.
fn embedding_to_blob(embedding: &[f32]) -> Vec<u8> {
    let mut blob = Vec::with_capacity(embedding.len() * 4);
    for &val in embedding {
        blob.extend_from_slice(&val.to_le_bytes());
    }
    blob
}

/// Convert a little-endian BLOB back to `Vec<f32>` with size validation.
fn blob_to_embedding(blob: &[u8], expected_dimension: usize) -> Result<Vec<f32>, SymbolStoreError> {
    if blob.len() != expected_dimension * 4 {
        return Err(SymbolStoreError::InvalidEmbedding {
            reason: format!(
                "BLOB size {} != expected {}",
                blob.len(),
                expected_dimension * 4
            ),
        });
    }
    Ok(blob
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect())
}

/// Compute cosine similarity between two vectors of equal length.
/// Returns 0.0 if either vector has zero norm.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let mut dot = 0.0_f32;
    let mut norm_a = 0.0_f32;
    let mut norm_b = 0.0_f32;
    for (&ai, &bi) in a.iter().zip(b.iter()) {
        dot += ai * bi;
        norm_a += ai * ai;
        norm_b += bi * bi;
    }
    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom == 0.0 { 0.0 } else { dot / denom }
}

// ---------------------------------------------------------------------------
// SymbolStore
// ---------------------------------------------------------------------------

/// SQLite-backed store for code symbols and dependency (import) records.
#[derive(Debug)]
pub struct SymbolStore {
    conn: Connection,
}

impl SymbolStore {
    /// Open (or create) a symbol store backed by the given file path.
    pub fn open(db_path: &Path) -> Result<Self, SymbolStoreError> {
        let conn = Connection::open(db_path)?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;

        // Check schema version only when schema_meta table already exists.
        let table_exists: bool = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='schema_meta'",
            [],
            |row| row.get::<_, i64>(0),
        )? > 0;

        if table_exists {
            let version: u32 = conn.query_row(
                "SELECT value FROM schema_meta WHERE key = 'schema_version'",
                [],
                |row| {
                    let v: String = row.get(0)?;
                    Ok(v.parse::<u32>().unwrap_or(0))
                },
            )?;
            if version != CURRENT_SYMBOL_SCHEMA_VERSION {
                return Err(SymbolStoreError::SchemaVersionMismatch {
                    expected: CURRENT_SYMBOL_SCHEMA_VERSION,
                    found: version,
                });
            }
        }

        Ok(Self { conn })
    }

    /// Open an in-memory database (for testing).
    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self, SymbolStoreError> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        Ok(Self { conn })
    }

    /// Create all required tables and indices (idempotent).
    pub fn create_tables(&self) -> Result<(), SymbolStoreError> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS symbols (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                kind TEXT NOT NULL,
                file_path TEXT NOT NULL,
                line_start INTEGER NOT NULL,
                line_end INTEGER NOT NULL,
                parent_symbol_id INTEGER REFERENCES symbols(id) ON DELETE CASCADE,
                file_hash TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS dependencies (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_file TEXT NOT NULL,
                target_module TEXT NOT NULL,
                imported_names TEXT,
                file_hash TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_symbols_name ON symbols(name);
            CREATE INDEX IF NOT EXISTS idx_symbols_file ON symbols(file_path);
            CREATE INDEX IF NOT EXISTS idx_symbols_kind ON symbols(kind);
            CREATE INDEX IF NOT EXISTS idx_symbols_parent ON symbols(parent_symbol_id);
            CREATE INDEX IF NOT EXISTS idx_deps_source ON dependencies(source_file);
            CREATE INDEX IF NOT EXISTS idx_deps_target ON dependencies(target_module);

            CREATE TABLE IF NOT EXISTS file_links (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_file TEXT NOT NULL,
                target_file TEXT NOT NULL,
                link_type TEXT NOT NULL,
                file_hash TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_file_links_source ON file_links(source_file);
            CREATE INDEX IF NOT EXISTS idx_file_links_target ON file_links(target_file);

            CREATE TABLE IF NOT EXISTS embeddings (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                file_path TEXT NOT NULL,
                section_heading TEXT NOT NULL DEFAULT '',
                embedding BLOB NOT NULL,
                dimension INTEGER NOT NULL,
                model_name TEXT NOT NULL,
                file_hash TEXT NOT NULL,
                created_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_embeddings_path ON embeddings(file_path);
            CREATE INDEX IF NOT EXISTS idx_embeddings_hash ON embeddings(file_hash);
            CREATE UNIQUE INDEX IF NOT EXISTS idx_embeddings_path_section ON embeddings(file_path, section_heading);

            CREATE TABLE IF NOT EXISTS knowledge_nodes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                type TEXT NOT NULL,
                identifier TEXT NOT NULL,
                title TEXT,
                file_path TEXT,
                created_at TEXT DEFAULT (datetime('now')),
                updated_at TEXT DEFAULT (datetime('now')),
                UNIQUE(type, identifier)
            );

            CREATE INDEX IF NOT EXISTS idx_kn_type ON knowledge_nodes(type);
            CREATE INDEX IF NOT EXISTS idx_kn_identifier ON knowledge_nodes(identifier);
            CREATE INDEX IF NOT EXISTS idx_kn_file_path ON knowledge_nodes(file_path);

            CREATE TABLE IF NOT EXISTS knowledge_edges (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_id INTEGER NOT NULL REFERENCES knowledge_nodes(id) ON DELETE CASCADE,
                target_id INTEGER NOT NULL REFERENCES knowledge_nodes(id) ON DELETE CASCADE,
                relation TEXT NOT NULL,
                metadata TEXT,
                UNIQUE(source_id, target_id, relation)
            );

            CREATE INDEX IF NOT EXISTS idx_ke_source ON knowledge_edges(source_id);
            CREATE INDEX IF NOT EXISTS idx_ke_target ON knowledge_edges(target_id);
            CREATE INDEX IF NOT EXISTS idx_ke_relation ON knowledge_edges(relation);",
        )?;

        self.conn.execute(
            "INSERT OR REPLACE INTO schema_meta (key, value) VALUES (?1, ?2)",
            params!["schema_version", CURRENT_SYMBOL_SCHEMA_VERSION.to_string()],
        )?;

        Ok(())
    }

    /// Bulk-insert symbols inside a single transaction.
    pub fn insert_symbols(&self, symbols: &[SymbolInfo]) -> Result<(), SymbolStoreError> {
        let tx = self.conn.unchecked_transaction()?;
        for sym in symbols {
            tx.execute(
                "INSERT INTO symbols (name, kind, file_path, line_start, line_end, parent_symbol_id, file_hash)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    sym.name,
                    sym.kind,
                    sym.file_path,
                    sym.line_start,
                    sym.line_end,
                    sym.parent_symbol_id,
                    sym.file_hash,
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Bulk-insert dependency (import) records inside a single transaction.
    pub fn insert_dependencies(&self, deps: &[ImportInfo]) -> Result<(), SymbolStoreError> {
        let tx = self.conn.unchecked_transaction()?;
        for dep in deps {
            tx.execute(
                "INSERT INTO dependencies (source_file, target_module, imported_names, file_hash)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    dep.source_file,
                    dep.target_module,
                    dep.imported_names,
                    dep.file_hash,
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Bulk-insert file link records inside a single transaction.
    pub fn insert_file_links(&self, links: &[FileLinkInfo]) -> Result<(), SymbolStoreError> {
        let tx = self.conn.unchecked_transaction()?;
        for link in links {
            tx.execute(
                "INSERT INTO file_links (source_file, target_file, link_type, file_hash)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    link.source_file,
                    link.target_file,
                    link.link_type,
                    link.file_hash,
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Find file links originating from the given source file.
    pub fn find_file_links_by_source(
        &self,
        source: &str,
    ) -> Result<Vec<FileLinkInfo>, SymbolStoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, source_file, target_file, link_type, file_hash
             FROM file_links WHERE source_file = ?1",
        )?;
        let rows = stmt.query_map(params![source], file_link_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Delete all symbols, dependencies, and file links that belong to the given file.
    pub fn delete_by_file(&self, file_path: &str) -> Result<(), SymbolStoreError> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM symbols WHERE file_path = ?1",
            params![file_path],
        )?;
        tx.execute(
            "DELETE FROM dependencies WHERE source_file = ?1",
            params![file_path],
        )?;
        tx.execute(
            "DELETE FROM file_links WHERE source_file = ?1",
            params![file_path],
        )?;
        tx.execute(
            "DELETE FROM embeddings WHERE file_path = ?1",
            params![file_path],
        )?;
        // Knowledge nodes (ON DELETE CASCADE removes edges automatically)
        tx.execute(
            "DELETE FROM knowledge_nodes WHERE file_path = ?1",
            params![file_path],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Find symbols whose name matches exactly.
    pub fn find_by_name(&self, name: &str) -> Result<Vec<SymbolInfo>, SymbolStoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, kind, file_path, line_start, line_end, parent_symbol_id, file_hash
             FROM symbols WHERE name = ?1",
        )?;
        let rows = stmt.query_map(params![name], symbol_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Find symbols belonging to the given file path.
    pub fn find_by_file(&self, file_path: &str) -> Result<Vec<SymbolInfo>, SymbolStoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, kind, file_path, line_start, line_end, parent_symbol_id, file_hash
             FROM symbols WHERE file_path = ?1",
        )?;
        let rows = stmt.query_map(params![file_path], symbol_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Count all symbols in the store.
    pub fn count_all(&self) -> Result<u64, SymbolStoreError> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM symbols", [], |row| row.get(0))?;
        Ok(count as u64)
    }

    /// Find symbols whose name partially matches (LIKE %name%, case-insensitive).
    pub fn find_by_name_like(
        &self,
        name: &str,
        limit: usize,
    ) -> Result<Vec<SymbolInfo>, SymbolStoreError> {
        let escaped = escape_like_pattern(name);
        let pattern = format!("%{escaped}%");
        let mut stmt = self.conn.prepare(
            "SELECT id, name, kind, file_path, line_start, line_end, parent_symbol_id, file_hash
             FROM symbols WHERE name LIKE ?1 ESCAPE '\\' COLLATE NOCASE
             ORDER BY name LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![pattern, limit as i64], symbol_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Find child symbols belonging to a parent symbol.
    pub fn find_children_by_parent_id(
        &self,
        parent_id: i64,
    ) -> Result<Vec<SymbolInfo>, SymbolStoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, kind, file_path, line_start, line_end, parent_symbol_id, file_hash
             FROM symbols WHERE parent_symbol_id = ?1 ORDER BY line_start LIMIT 100",
        )?;
        let rows = stmt.query_map(params![parent_id], symbol_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Update the parent_symbol_id for a given symbol (used by 2-pass parent resolution).
    pub fn update_parent_symbol_id(
        &self,
        symbol_id: i64,
        parent_id: i64,
    ) -> Result<(), SymbolStoreError> {
        self.conn.execute(
            "UPDATE symbols SET parent_symbol_id = ?1 WHERE id = ?2",
            params![parent_id, symbol_id],
        )?;
        Ok(())
    }

    /// Find import records whose target module matches exactly.
    pub fn find_imports_by_target(
        &self,
        target_module: &str,
    ) -> Result<Vec<ImportInfo>, SymbolStoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, source_file, target_module, imported_names, file_hash
             FROM dependencies WHERE target_module = ?1",
        )?;
        let rows = stmt.query_map(params![target_module], import_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Find import records whose source file matches exactly.
    /// Returns all modules that the given file imports.
    pub fn find_imports_by_source(
        &self,
        source_file: &str,
    ) -> Result<Vec<ImportInfo>, SymbolStoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, source_file, target_module, imported_names, file_hash
             FROM dependencies WHERE source_file = ?1",
        )?;
        let rows = stmt.query_map(params![source_file], import_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Retrieve all import (dependency) records from the database.
    pub fn find_all_imports(&self) -> Result<Vec<ImportInfo>, SymbolStoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, source_file, target_module, imported_names, file_hash
             FROM dependencies",
        )?;
        let rows = stmt.query_map([], import_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Find file links where the given file is the target.
    /// Returns all files that link to the given target file.
    pub fn find_file_links_by_target(
        &self,
        target_file: &str,
    ) -> Result<Vec<FileLinkInfo>, SymbolStoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, source_file, target_file, link_type, file_hash
             FROM file_links WHERE target_file = ?1",
        )?;
        let rows = stmt.query_map(params![target_file], file_link_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Retrieve all file link records from the database.
    pub fn find_all_file_links(&self) -> Result<Vec<FileLinkInfo>, SymbolStoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, source_file, target_file, link_type, file_hash
             FROM file_links",
        )?;
        let rows = stmt.query_map([], file_link_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Bulk-insert embedding records inside a single transaction.
    ///
    /// Uses `INSERT OR REPLACE` so that duplicate `(file_path, section_heading)`
    /// pairs are overwritten rather than causing a constraint error.
    pub fn insert_embeddings(&self, embeddings: &[EmbeddingInfo]) -> Result<(), SymbolStoreError> {
        // Validate all embeddings before starting the transaction.
        for emb in embeddings {
            if emb.embedding.is_empty() {
                return Err(SymbolStoreError::InvalidEmbedding {
                    reason: "embedding vector must not be empty".to_string(),
                });
            }
            for &val in &emb.embedding {
                if val.is_nan() || val.is_infinite() {
                    return Err(SymbolStoreError::InvalidEmbedding {
                        reason: format!("embedding contains invalid value: {val}"),
                    });
                }
            }
        }

        let tx = self.conn.unchecked_transaction()?;
        for emb in embeddings {
            let blob = embedding_to_blob(&emb.embedding);
            let dimension = emb.embedding.len() as i64;
            let created_at = chrono::Utc::now().to_rfc3339();
            tx.execute(
                "INSERT OR REPLACE INTO embeddings
                 (file_path, section_heading, embedding, dimension, model_name, file_hash, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    emb.file_path,
                    emb.section_heading,
                    blob,
                    dimension,
                    emb.model_name,
                    emb.file_hash,
                    created_at,
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Count all embeddings in the store.
    pub fn count_embeddings(&self) -> Result<u64, SymbolStoreError> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM embeddings", [], |row| row.get(0))?;
        Ok(count as u64)
    }

    /// Search for the top-k most similar embeddings using cosine similarity.
    ///
    /// Loads all stored embeddings, filters out records whose dimension does not
    /// match the query, computes cosine similarity, and returns the top-k results
    /// sorted by descending similarity.
    pub fn search_similar(
        &self,
        query_embedding: &[f32],
        top_k: usize,
    ) -> Result<Vec<EmbeddingSimilarityResult>, SymbolStoreError> {
        let query_dim = query_embedding.len();
        let mut stmt = self
            .conn
            .prepare("SELECT file_path, section_heading, embedding, dimension FROM embeddings")?;

        let mut results: Vec<EmbeddingSimilarityResult> = Vec::new();

        let rows = stmt.query_map([], |row| {
            let file_path: String = row.get(0)?;
            let section_heading: String = row.get(1)?;
            let blob: Vec<u8> = row.get(2)?;
            let dimension: i64 = row.get(3)?;
            Ok((file_path, section_heading, blob, dimension as usize))
        })?;

        for row_result in rows {
            let (file_path, section_heading, blob, dimension) = row_result?;

            // Validate BLOB size against stored dimension
            let stored_embedding = match blob_to_embedding(&blob, dimension) {
                Ok(emb) => emb,
                Err(_) => {
                    tracing::warn!(
                        "Skipping embedding for {file_path}: BLOB size mismatch (expected dimension={dimension})"
                    );
                    continue;
                }
            };

            // Filter out dimension mismatches with query
            if stored_embedding.len() != query_dim {
                tracing::warn!(
                    "Skipping embedding for {file_path}: dimension {} != query dimension {query_dim}",
                    stored_embedding.len()
                );
                continue;
            }

            let similarity = cosine_similarity(query_embedding, &stored_embedding);
            results.push(EmbeddingSimilarityResult {
                file_path,
                section_heading,
                similarity,
            });
        }

        // Sort by descending similarity
        results.sort_by(|a, b| {
            b.similarity
                .partial_cmp(&a.similarity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(top_k);
        Ok(results)
    }
}

// ---------------------------------------------------------------------------
// Knowledge Graph Methods
// ---------------------------------------------------------------------------

impl SymbolStore {
    /// Insert or update a knowledge node. Returns the node ID.
    pub fn upsert_knowledge_node(
        &self,
        node_type: &str,
        identifier: &str,
        title: Option<&str>,
        file_path: Option<&str>,
    ) -> Result<i64, SymbolStoreError> {
        self.conn.execute(
            "INSERT INTO knowledge_nodes (type, identifier, title, file_path)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(type, identifier) DO UPDATE SET
                title = excluded.title,
                file_path = excluded.file_path,
                updated_at = datetime('now')",
            params![node_type, identifier, title, file_path],
        )?;
        let id = self.conn.query_row(
            "SELECT id FROM knowledge_nodes WHERE type = ?1 AND identifier = ?2",
            params![node_type, identifier],
            |row| row.get(0),
        )?;
        Ok(id)
    }

    /// Insert or update a knowledge edge.
    pub fn upsert_knowledge_edge(
        &self,
        source_id: i64,
        target_id: i64,
        relation: &str,
        metadata: Option<&str>,
    ) -> Result<(), SymbolStoreError> {
        self.conn.execute(
            "INSERT INTO knowledge_edges (source_id, target_id, relation, metadata)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(source_id, target_id, relation) DO UPDATE SET
                metadata = excluded.metadata",
            params![source_id, target_id, relation, metadata],
        )?;
        Ok(())
    }

    /// Bulk-insert knowledge entries in a single transaction.
    pub fn insert_knowledge_entries(
        &self,
        entries: &[crate::indexer::knowledge::KnowledgeEntry],
    ) -> Result<(), SymbolStoreError> {
        let tx = self.conn.unchecked_transaction()?;

        for entry in entries {
            // Upsert issue node
            tx.execute(
                "INSERT INTO knowledge_nodes (type, identifier)
                 VALUES ('issue', ?1)
                 ON CONFLICT(type, identifier) DO NOTHING",
                params![entry.issue_number],
            )?;
            let issue_id: i64 = tx.query_row(
                "SELECT id FROM knowledge_nodes WHERE type = 'issue' AND identifier = ?1",
                params![entry.issue_number],
                |row| row.get(0),
            )?;

            // Upsert document node
            tx.execute(
                "INSERT INTO knowledge_nodes (type, identifier, file_path)
                 VALUES ('document', ?1, ?1)
                 ON CONFLICT(type, identifier) DO UPDATE SET
                    file_path = excluded.file_path,
                    updated_at = datetime('now')",
                params![entry.file_path],
            )?;
            let doc_id: i64 = tx.query_row(
                "SELECT id FROM knowledge_nodes WHERE type = 'document' AND identifier = ?1",
                params![entry.file_path],
                |row| row.get(0),
            )?;

            // Upsert edge
            let metadata =
                serde_json::json!({"doc_subtype": entry.doc_subtype.as_str()}).to_string();
            tx.execute(
                "INSERT INTO knowledge_edges (source_id, target_id, relation, metadata)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(source_id, target_id, relation) DO UPDATE SET
                    metadata = excluded.metadata",
                params![issue_id, doc_id, entry.relation.as_str(), metadata],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    /// Delete all knowledge nodes and edges.
    pub fn clear_knowledge_graph(&self) -> Result<(), SymbolStoreError> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute_batch(
            "DELETE FROM knowledge_edges;
             DELETE FROM knowledge_nodes;",
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Delete knowledge nodes by file_path (ON DELETE CASCADE removes edges).
    pub fn delete_knowledge_by_file(&self, file_path: &str) -> Result<(), SymbolStoreError> {
        self.conn.execute(
            "DELETE FROM knowledge_nodes WHERE file_path = ?1",
            params![file_path],
        )?;
        Ok(())
    }

    /// Find all documents related to a given Issue number through the knowledge graph.
    pub fn find_documents_by_issue(
        &self,
        issue_number: &str,
    ) -> Result<Vec<crate::indexer::knowledge::IssueDocumentEntry>, SymbolStoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT kn_doc.file_path, ke.relation, ke.metadata
             FROM knowledge_nodes kn_issue
             JOIN knowledge_edges ke ON ke.source_id = kn_issue.id
             JOIN knowledge_nodes kn_doc ON ke.target_id = kn_doc.id AND kn_doc.type = 'document'
             WHERE kn_issue.type = 'issue' AND kn_issue.identifier = ?1
             LIMIT 100",
        )?;

        let mut results = Vec::new();
        let rows = stmt.query_map(params![issue_number], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })?;

        for row in rows {
            let (file_path, relation_str, metadata_opt) = row?;

            let relation = match relation_str.as_str() {
                "has_design" => crate::indexer::knowledge::KnowledgeRelation::HasDesign,
                "has_review" => crate::indexer::knowledge::KnowledgeRelation::HasReview,
                "has_workplan" => crate::indexer::knowledge::KnowledgeRelation::HasWorkplan,
                other => {
                    return Err(SymbolStoreError::InvalidEmbedding {
                        reason: format!("Unknown relation type: {other}"),
                    });
                }
            };

            let metadata_str = metadata_opt.unwrap_or_default();
            let doc_subtype = if metadata_str.is_empty() {
                return Err(SymbolStoreError::InvalidEmbedding {
                    reason: format!("Missing metadata for document: {file_path}"),
                });
            } else {
                let parsed: serde_json::Value =
                    serde_json::from_str(&metadata_str).map_err(|e| {
                        SymbolStoreError::InvalidEmbedding {
                            reason: format!("Failed to parse metadata for {file_path}: {e}"),
                        }
                    })?;
                let subtype_str = parsed["doc_subtype"].as_str().ok_or_else(|| {
                    SymbolStoreError::InvalidEmbedding {
                        reason: format!("Missing doc_subtype in metadata for {file_path}"),
                    }
                })?;
                crate::indexer::knowledge::DocSubtype::parse(subtype_str).ok_or_else(|| {
                    SymbolStoreError::InvalidEmbedding {
                        reason: format!("Unknown doc_subtype: {subtype_str}"),
                    }
                })?
            };

            results.push(crate::indexer::knowledge::IssueDocumentEntry {
                file_path,
                relation,
                doc_subtype,
            });
        }

        Ok(results)
    }

    /// Find documents related to the given file through the knowledge graph.
    /// If the file is a document node, find its issue and return all sibling documents.
    /// Issue番号群からナレッジグラフ経由でドキュメントを検索する。
    /// Issue → document の 1ホップ走査。
    pub fn find_knowledge_by_issue(
        &self,
        issue_numbers: &[String],
    ) -> Result<Vec<KnowledgeDocResult>, SymbolStoreError> {
        if issue_numbers.is_empty() {
            return Ok(Vec::new());
        }

        // Build IN clause with placeholders
        let placeholders: Vec<String> = issue_numbers
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect();
        let in_clause = placeholders.join(", ");
        let sql = format!(
            "SELECT kn_issue.identifier AS issue_number,
                    ke.relation,
                    kn_doc.file_path,
                    kn_doc.title
             FROM knowledge_nodes kn_issue
             JOIN knowledge_edges ke ON ke.source_id = kn_issue.id
             JOIN knowledge_nodes kn_doc ON ke.target_id = kn_doc.id AND kn_doc.type IN ('document', 'file')
             WHERE kn_issue.type = 'issue'
               AND kn_issue.identifier IN ({in_clause})
             ORDER BY kn_issue.identifier, ke.relation"
        );

        let mut stmt = self.conn.prepare(&sql)?;
        let params: Vec<&dyn rusqlite::types::ToSql> = issue_numbers
            .iter()
            .map(|s| s as &dyn rusqlite::types::ToSql)
            .collect();
        let rows = stmt.query_map(params.as_slice(), |row| {
            let issue_number: String = row.get(0)?;
            let relation_str: String = row.get(1)?;
            let file_path: String = row.get(2)?;
            let title: Option<String> = row.get(3)?;
            Ok((issue_number, relation_str, file_path, title))
        })?;

        let mut results = Vec::new();
        for row in rows {
            let (issue_number, relation_str, file_path, title) = row?;
            if let Some(relation) =
                crate::indexer::knowledge::KnowledgeRelation::parse(&relation_str)
            {
                results.push(KnowledgeDocResult {
                    issue_number,
                    relation,
                    file_path,
                    title,
                });
            } else {
                let sanitized: String = relation_str
                    .chars()
                    .map(|c| if c.is_control() { '\u{FFFD}' } else { c })
                    .collect();
                eprintln!("Warning: unknown knowledge relation '{sanitized}', skipping");
            }
        }
        Ok(results)
    }

    /// Bulk-insert file-modifies entries (issue → file edges) in a single transaction.
    /// Each entry creates an issue node (if not exists), a file node (if not exists),
    /// and a "modifies" edge between them.
    pub fn insert_file_modifies_entries(
        &self,
        entries: &[crate::indexer::knowledge::FileModifiesEntry],
    ) -> Result<(), SymbolStoreError> {
        let tx = self.conn.unchecked_transaction()?;

        for entry in entries {
            // Upsert issue node
            tx.execute(
                "INSERT INTO knowledge_nodes (type, identifier)
                 VALUES ('issue', ?1)
                 ON CONFLICT(type, identifier) DO NOTHING",
                params![entry.issue_number],
            )?;
            let issue_id: i64 = tx.query_row(
                "SELECT id FROM knowledge_nodes WHERE type = 'issue' AND identifier = ?1",
                params![entry.issue_number],
                |row| row.get(0),
            )?;

            // Upsert file node
            tx.execute(
                "INSERT INTO knowledge_nodes (type, identifier, file_path)
                 VALUES ('file', ?1, ?1)
                 ON CONFLICT(type, identifier) DO NOTHING",
                params![entry.file_path],
            )?;
            let file_id: i64 = tx.query_row(
                "SELECT id FROM knowledge_nodes WHERE type = 'file' AND identifier = ?1",
                params![entry.file_path],
                |row| row.get(0),
            )?;

            // Insert modifies edge
            tx.execute(
                "INSERT INTO knowledge_edges (source_id, target_id, relation)
                 VALUES (?1, ?2, 'modifies')
                 ON CONFLICT(source_id, target_id, relation) DO NOTHING",
                params![issue_id, file_id],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    /// Clear all file-modifies data: modifies edges, orphan file nodes, orphan issue nodes.
    /// File nodes are currently only used as edge targets.
    /// If file nodes become edge sources in the future, this query needs updating.
    pub fn clear_file_modifies(&self) -> Result<(), SymbolStoreError> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM knowledge_edges WHERE relation = 'modifies'",
            [],
        )?;
        tx.execute(
            "DELETE FROM knowledge_nodes WHERE type = 'file'
             AND id NOT IN (SELECT target_id FROM knowledge_edges)",
            [],
        )?;
        // Remove issue nodes that no longer have any edges
        tx.execute(
            "DELETE FROM knowledge_nodes WHERE type = 'issue'
             AND id NOT IN (SELECT source_id FROM knowledge_edges)",
            [],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn find_knowledge_related(
        &self,
        file_path: &str,
    ) -> Result<Vec<crate::indexer::knowledge::KnowledgeRelatedResult>, SymbolStoreError> {
        let mut results = Vec::new();

        // Find issue(s) that this file belongs to
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT kn_issue.identifier, ke2.relation, kn_sibling.file_path, kn_issue.title
             FROM knowledge_nodes kn_doc
             JOIN knowledge_edges ke1 ON ke1.target_id = kn_doc.id
             JOIN knowledge_nodes kn_issue ON ke1.source_id = kn_issue.id AND kn_issue.type = 'issue'
             JOIN knowledge_edges ke2 ON ke2.source_id = kn_issue.id
             JOIN knowledge_nodes kn_sibling ON ke2.target_id = kn_sibling.id
             WHERE kn_doc.file_path = ?1
             AND kn_sibling.file_path != ?1
             ORDER BY CASE WHEN ke2.relation = 'modifies' THEN 1 ELSE 0 END, ke2.relation
             LIMIT 100",
        )?;

        let rows = stmt.query_map(params![file_path], |row| {
            Ok(crate::indexer::knowledge::KnowledgeRelatedResult {
                issue_number: row.get(0)?,
                relation: row.get(1)?,
                file_path: row.get(2)?,
                title: row.get(3)?,
            })
        })?;

        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn sample_symbol(name: &str, file_path: &str) -> SymbolInfo {
        SymbolInfo {
            id: None,
            name: name.to_string(),
            kind: "function".to_string(),
            file_path: file_path.to_string(),
            line_start: 1,
            line_end: 10,
            parent_symbol_id: None,
            file_hash: "abc123".to_string(),
        }
    }

    fn sample_import(source: &str, target: &str) -> ImportInfo {
        ImportInfo {
            id: None,
            source_file: source.to_string(),
            target_module: target.to_string(),
            imported_names: Some("foo, bar".to_string()),
            file_hash: "abc123".to_string(),
        }
    }

    #[test]
    fn test_open_and_create_tables() {
        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();
    }

    #[test]
    fn test_create_tables_idempotent() {
        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();
        store.create_tables().unwrap();
    }

    #[test]
    fn test_insert_and_find_by_name() {
        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        let sym = sample_symbol("my_func", "src/main.rs");
        store.insert_symbols(&[sym]).unwrap();

        let results = store.find_by_name("my_func").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "my_func");
        assert_eq!(results[0].file_path, "src/main.rs");
        assert!(results[0].id.is_some());
    }

    #[test]
    fn test_insert_and_find_by_file() {
        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        let syms = vec![
            sample_symbol("func_a", "src/lib.rs"),
            sample_symbol("func_b", "src/lib.rs"),
            sample_symbol("func_c", "src/other.rs"),
        ];
        store.insert_symbols(&syms).unwrap();

        let results = store.find_by_file("src/lib.rs").unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_insert_and_find_dependencies() {
        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        let deps = vec![
            sample_import("src/main.rs", "std::io"),
            sample_import("src/lib.rs", "std::io"),
        ];
        store.insert_dependencies(&deps).unwrap();

        let results = store.find_imports_by_target("std::io").unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].target_module, "std::io");
    }

    #[test]
    fn test_delete_by_file_removes_symbols() {
        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        let syms = vec![
            sample_symbol("func_a", "src/lib.rs"),
            sample_symbol("func_b", "src/other.rs"),
        ];
        store.insert_symbols(&syms).unwrap();

        store.delete_by_file("src/lib.rs").unwrap();

        let results = store.find_by_file("src/lib.rs").unwrap();
        assert!(results.is_empty());

        let remaining = store.find_by_file("src/other.rs").unwrap();
        assert_eq!(remaining.len(), 1);
    }

    #[test]
    fn test_delete_by_file_removes_dependencies() {
        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        let deps = vec![
            sample_import("src/main.rs", "std::io"),
            sample_import("src/lib.rs", "serde"),
        ];
        store.insert_dependencies(&deps).unwrap();

        store.delete_by_file("src/main.rs").unwrap();

        let results = store.find_imports_by_target("std::io").unwrap();
        assert!(results.is_empty());

        let remaining = store.find_imports_by_target("serde").unwrap();
        assert_eq!(remaining.len(), 1);
    }

    #[test]
    fn test_delete_by_file_cascade() {
        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        // Insert parent symbol
        let parent = sample_symbol("MyStruct", "src/lib.rs");
        store.insert_symbols(&[parent]).unwrap();

        // Get parent id
        let parents = store.find_by_name("MyStruct").unwrap();
        let parent_id = parents[0].id.unwrap();

        // Insert child symbol referencing parent
        let child = SymbolInfo {
            id: None,
            name: "my_method".to_string(),
            kind: "method".to_string(),
            file_path: "src/lib.rs".to_string(),
            line_start: 5,
            line_end: 8,
            parent_symbol_id: Some(parent_id),
            file_hash: "abc123".to_string(),
        };
        store.insert_symbols(&[child]).unwrap();

        // Verify both exist
        let all = store.find_by_file("src/lib.rs").unwrap();
        assert_eq!(all.len(), 2);

        // Delete by file removes parent, CASCADE should remove child
        store.delete_by_file("src/lib.rs").unwrap();

        let remaining = store.find_by_file("src/lib.rs").unwrap();
        assert!(remaining.is_empty());
    }

    #[test]
    fn test_schema_version_check() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("symbols.db");

        // Create store and tables
        {
            let store = SymbolStore::open(&db_path).unwrap();
            store.create_tables().unwrap();
            // Tamper with version
            store
                .conn
                .execute(
                    "UPDATE schema_meta SET value = ?1 WHERE key = 'schema_version'",
                    params!["999"],
                )
                .unwrap();
        }

        // Re-open should fail with version mismatch
        let result = SymbolStore::open(&db_path);
        assert!(result.is_err());
        match result.unwrap_err() {
            SymbolStoreError::SchemaVersionMismatch { expected, found } => {
                assert_eq!(expected, CURRENT_SYMBOL_SCHEMA_VERSION);
                assert_eq!(found, 999);
            }
            other => panic!("Expected SchemaVersionMismatch, got: {other}"),
        }
    }

    #[test]
    fn test_count_all_empty() {
        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();
        assert_eq!(store.count_all().unwrap(), 0);
    }

    #[test]
    fn test_count_all_after_insert() {
        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        let syms = vec![
            sample_symbol("func_a", "src/lib.rs"),
            sample_symbol("func_b", "src/lib.rs"),
            sample_symbol("func_c", "src/other.rs"),
        ];
        store.insert_symbols(&syms).unwrap();
        assert_eq!(store.count_all().unwrap(), 3);
    }

    #[test]
    fn test_find_nonexistent_returns_empty() {
        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        assert!(store.find_by_name("nonexistent").unwrap().is_empty());
        assert!(store.find_by_file("no/such/file.rs").unwrap().is_empty());
        assert!(
            store
                .find_imports_by_target("no::module")
                .unwrap()
                .is_empty()
        );
    }

    fn sample_file_link(source: &str, target: &str, link_type: &str) -> FileLinkInfo {
        FileLinkInfo {
            id: None,
            source_file: source.to_string(),
            target_file: target.to_string(),
            link_type: link_type.to_string(),
            file_hash: "abc123".to_string(),
        }
    }

    #[test]
    fn test_file_links_table_created() {
        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        // Verify table exists by inserting
        let link = sample_file_link("docs/a.md", "docs/b.md", "WikiLink");
        store.insert_file_links(&[link]).unwrap();
    }

    #[test]
    fn test_insert_and_find_file_links_by_source() {
        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        let links = vec![
            sample_file_link("docs/a.md", "docs/b.md", "WikiLink"),
            sample_file_link("docs/a.md", "docs/c.md", "MarkdownLink"),
            sample_file_link("docs/other.md", "docs/b.md", "WikiLink"),
        ];
        store.insert_file_links(&links).unwrap();

        let results = store.find_file_links_by_source("docs/a.md").unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].source_file, "docs/a.md");
        assert_eq!(results[0].target_file, "docs/b.md");
        assert_eq!(results[0].link_type, "WikiLink");
        assert_eq!(results[1].target_file, "docs/c.md");
        assert_eq!(results[1].link_type, "MarkdownLink");
        assert!(results[0].id.is_some());
    }

    #[test]
    fn test_find_file_links_by_source_empty() {
        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        let results = store.find_file_links_by_source("nonexistent.md").unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_delete_by_file_removes_file_links() {
        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        let links = vec![
            sample_file_link("docs/a.md", "docs/b.md", "WikiLink"),
            sample_file_link("docs/other.md", "docs/b.md", "WikiLink"),
        ];
        store.insert_file_links(&links).unwrap();

        store.delete_by_file("docs/a.md").unwrap();

        let results = store.find_file_links_by_source("docs/a.md").unwrap();
        assert!(results.is_empty());

        let remaining = store.find_file_links_by_source("docs/other.md").unwrap();
        assert_eq!(remaining.len(), 1);
    }

    #[test]
    fn test_insert_file_links_empty() {
        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        // Inserting empty slice should succeed
        store.insert_file_links(&[]).unwrap();
    }

    #[test]
    fn test_open_creates_db_file() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("symbols.db");

        assert!(!db_path.exists());

        let _store = SymbolStore::open(&db_path).unwrap();

        assert!(db_path.exists());
    }

    #[test]
    fn test_find_imports_by_source() {
        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        let deps = vec![
            sample_import("src/main.rs", "std::io"),
            sample_import("src/main.rs", "serde"),
            sample_import("src/lib.rs", "std::io"),
        ];
        store.insert_dependencies(&deps).unwrap();

        let results = store.find_imports_by_source("src/main.rs").unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].source_file, "src/main.rs");
    }

    #[test]
    fn test_find_imports_by_source_empty() {
        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();
        assert!(
            store
                .find_imports_by_source("nonexistent.rs")
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn test_find_file_links_by_target() {
        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        let links = vec![
            sample_file_link("docs/a.md", "docs/b.md", "WikiLink"),
            sample_file_link("docs/c.md", "docs/b.md", "MarkdownLink"),
            sample_file_link("docs/a.md", "docs/d.md", "WikiLink"),
        ];
        store.insert_file_links(&links).unwrap();

        let results = store.find_file_links_by_target("docs/b.md").unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].target_file, "docs/b.md");
    }

    #[test]
    fn test_find_file_links_by_target_empty() {
        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();
        assert!(
            store
                .find_file_links_by_target("nonexistent.md")
                .unwrap()
                .is_empty()
        );
    }

    // -----------------------------------------------------------------------
    // Embedding tests
    // -----------------------------------------------------------------------

    fn sample_embedding(file_path: &str, section_heading: &str, values: Vec<f32>) -> EmbeddingInfo {
        EmbeddingInfo {
            id: None,
            file_path: file_path.to_string(),
            section_heading: section_heading.to_string(),
            embedding: values,
            model_name: "test-model".to_string(),
            file_hash: "hash123".to_string(),
        }
    }

    #[test]
    fn test_embedding_blob_roundtrip() {
        let original: Vec<f32> = vec![1.0, -2.5, 3.125, 0.0, f32::MIN, f32::MAX];
        let blob = embedding_to_blob(&original);
        assert_eq!(blob.len(), original.len() * 4);
        let restored = blob_to_embedding(&blob, original.len()).unwrap();
        assert_eq!(original, restored);
    }

    #[test]
    fn test_blob_to_embedding_invalid_size() {
        let blob = vec![0u8; 10]; // Not a multiple of 4 that matches expected dimension
        let result = blob_to_embedding(&blob, 3); // expects 12 bytes
        assert!(result.is_err());
        match result.unwrap_err() {
            SymbolStoreError::InvalidEmbedding { reason } => {
                assert!(reason.contains("BLOB size 10 != expected 12"));
            }
            other => panic!("Expected InvalidEmbedding, got: {other}"),
        }
    }

    #[test]
    fn test_cosine_similarity_basic() {
        // Identical vectors → similarity = 1.0
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 1e-6);

        // Orthogonal vectors → similarity = 0.0
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < 1e-6);

        // Opposite vectors → similarity = -1.0
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim + 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_zero_vector() {
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![1.0, 2.0, 3.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
        assert_eq!(cosine_similarity(&b, &a), 0.0);
        assert_eq!(cosine_similarity(&a, &a), 0.0);
    }

    #[test]
    fn test_create_embeddings_table() {
        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        // Verify embeddings table exists by querying it
        let count: i64 = store
            .conn
            .query_row("SELECT COUNT(*) FROM embeddings", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_insert_embedding_validation() {
        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        // Empty vector should fail
        let emb = sample_embedding("test.rs", "", vec![]);
        let result = store.insert_embeddings(&[emb]);
        assert!(result.is_err());
        match result.unwrap_err() {
            SymbolStoreError::InvalidEmbedding { reason } => {
                assert!(reason.contains("empty"));
            }
            other => panic!("Expected InvalidEmbedding, got: {other}"),
        }

        // NaN should fail
        let emb = sample_embedding("test.rs", "", vec![1.0, f32::NAN, 3.0]);
        let result = store.insert_embeddings(&[emb]);
        assert!(result.is_err());
        match result.unwrap_err() {
            SymbolStoreError::InvalidEmbedding { reason } => {
                assert!(reason.contains("invalid value"));
            }
            other => panic!("Expected InvalidEmbedding, got: {other}"),
        }

        // Infinity should fail
        let emb = sample_embedding("test.rs", "", vec![1.0, f32::INFINITY, 3.0]);
        let result = store.insert_embeddings(&[emb]);
        assert!(result.is_err());
        match result.unwrap_err() {
            SymbolStoreError::InvalidEmbedding { reason } => {
                assert!(reason.contains("invalid value"));
            }
            other => panic!("Expected InvalidEmbedding, got: {other}"),
        }
    }

    #[test]
    fn test_embedding_unique_constraint() {
        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        // Insert first embedding
        let emb1 = sample_embedding("test.rs", "heading1", vec![1.0, 2.0, 3.0]);
        store.insert_embeddings(&[emb1]).unwrap();

        // Insert with same file_path + section_heading should replace (INSERT OR REPLACE)
        let emb2 = sample_embedding("test.rs", "heading1", vec![4.0, 5.0, 6.0]);
        store.insert_embeddings(&[emb2]).unwrap();

        // Should have only 1 record
        let count: i64 = store
            .conn
            .query_row(
                "SELECT COUNT(*) FROM embeddings WHERE file_path = 'test.rs'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);

        // Verify the replaced embedding has the new values
        let results = store.search_similar(&[4.0, 5.0, 6.0], 10).unwrap();
        assert_eq!(results.len(), 1);
        assert!((results[0].similarity - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_delete_by_file_removes_embeddings() {
        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        let emb1 = sample_embedding("src/a.rs", "", vec![1.0, 2.0, 3.0]);
        let emb2 = sample_embedding("src/b.rs", "", vec![4.0, 5.0, 6.0]);
        store.insert_embeddings(&[emb1, emb2]).unwrap();

        store.delete_by_file("src/a.rs").unwrap();

        let count: i64 = store
            .conn
            .query_row("SELECT COUNT(*) FROM embeddings", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);

        // Remaining embedding should be for src/b.rs
        let results = store.search_similar(&[4.0, 5.0, 6.0], 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].file_path, "src/b.rs");
    }

    #[test]
    fn test_delete_by_file_cascade_with_embeddings() {
        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        let file = "src/target.rs";

        // Insert into all 4 tables
        let sym = sample_symbol("func", file);
        store.insert_symbols(&[sym]).unwrap();

        let dep = sample_import(file, "std::io");
        store.insert_dependencies(&[dep]).unwrap();

        let link = sample_file_link(file, "docs/b.md", "WikiLink");
        store.insert_file_links(&[link]).unwrap();

        let emb = sample_embedding(file, "", vec![1.0, 2.0, 3.0]);
        store.insert_embeddings(&[emb]).unwrap();

        // Delete all records for the file
        store.delete_by_file(file).unwrap();

        // Verify all tables are empty for that file
        assert!(store.find_by_file(file).unwrap().is_empty());
        assert!(store.find_imports_by_source(file).unwrap().is_empty());
        assert!(store.find_file_links_by_source(file).unwrap().is_empty());

        let emb_count: i64 = store
            .conn
            .query_row(
                "SELECT COUNT(*) FROM embeddings WHERE file_path = ?1",
                params![file],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(emb_count, 0);
    }

    #[test]
    fn test_insert_and_search_embeddings() {
        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        // Insert 3 embeddings with known values
        let emb1 = sample_embedding("a.rs", "intro", vec![1.0, 0.0, 0.0]);
        let emb2 = sample_embedding("b.rs", "main", vec![0.0, 1.0, 0.0]);
        let emb3 = sample_embedding("c.rs", "", vec![1.0, 1.0, 0.0]);
        store.insert_embeddings(&[emb1, emb2, emb3]).unwrap();

        // Query with [1, 0, 0] → a.rs should be most similar
        let results = store.search_similar(&[1.0, 0.0, 0.0], 2).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].file_path, "a.rs");
        assert_eq!(results[0].section_heading, "intro");
        assert!((results[0].similarity - 1.0).abs() < 1e-6);

        // c.rs should be second (cos similarity of [1,0,0] and [1,1,0] = 1/sqrt(2))
        assert_eq!(results[1].file_path, "c.rs");
    }

    #[test]
    fn test_search_similar_empty() {
        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        let results = store.search_similar(&[1.0, 2.0, 3.0], 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_count_embeddings_empty() {
        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();
        assert_eq!(store.count_embeddings().unwrap(), 0);
    }

    #[test]
    fn test_count_embeddings_with_data() {
        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        let emb1 = sample_embedding("a.rs", "intro", vec![1.0, 0.0, 0.0]);
        let emb2 = sample_embedding("b.rs", "main", vec![0.0, 1.0, 0.0]);
        store.insert_embeddings(&[emb1, emb2]).unwrap();

        assert_eq!(store.count_embeddings().unwrap(), 2);
    }

    #[test]
    fn test_schema_version_v4() {
        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        let version: String = store
            .conn
            .query_row(
                "SELECT value FROM schema_meta WHERE key = 'schema_version'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(version, "4");
    }

    #[test]
    fn test_knowledge_tables_created() {
        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        // Verify knowledge_nodes table exists
        let count: i64 = store
            .conn
            .query_row("SELECT COUNT(*) FROM knowledge_nodes", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);

        // Verify knowledge_edges table exists
        let count: i64 = store
            .conn
            .query_row("SELECT COUNT(*) FROM knowledge_edges", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_upsert_knowledge_node() {
        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        let id1 = store
            .upsert_knowledge_node("issue", "299", None, None)
            .unwrap();
        assert!(id1 > 0);

        // Upsert same node returns same id
        let id2 = store
            .upsert_knowledge_node("issue", "299", Some("Updated"), None)
            .unwrap();
        assert_eq!(id1, id2);

        // Different node gets different id
        let id3 = store
            .upsert_knowledge_node("document", "path/to/file.md", None, Some("path/to/file.md"))
            .unwrap();
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_upsert_knowledge_edge() {
        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        let src = store
            .upsert_knowledge_node("issue", "100", None, None)
            .unwrap();
        let tgt = store
            .upsert_knowledge_node("document", "doc.md", None, Some("doc.md"))
            .unwrap();

        store
            .upsert_knowledge_edge(
                src,
                tgt,
                "has_design",
                Some(r#"{"doc_subtype":"design_policy"}"#),
            )
            .unwrap();

        // Verify edge exists
        let count: i64 = store
            .conn
            .query_row("SELECT COUNT(*) FROM knowledge_edges", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_insert_knowledge_entries() {
        use crate::indexer::knowledge::{DocSubtype, KnowledgeEntry, KnowledgeRelation};

        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        let entries = vec![
            KnowledgeEntry {
                issue_number: "100".to_string(),
                file_path: "dev-reports/design/issue-100-test-design-policy.md".to_string(),
                relation: KnowledgeRelation::HasDesign,
                doc_subtype: DocSubtype::DesignPolicy,
            },
            KnowledgeEntry {
                issue_number: "100".to_string(),
                file_path: "dev-reports/issue/100/work-plan.md".to_string(),
                relation: KnowledgeRelation::HasWorkplan,
                doc_subtype: DocSubtype::WorkPlan,
            },
        ];

        store.insert_knowledge_entries(&entries).unwrap();

        // 1 issue node + 2 document nodes = 3 nodes
        let node_count: i64 = store
            .conn
            .query_row("SELECT COUNT(*) FROM knowledge_nodes", [], |row| row.get(0))
            .unwrap();
        assert_eq!(node_count, 3);

        // 2 edges
        let edge_count: i64 = store
            .conn
            .query_row("SELECT COUNT(*) FROM knowledge_edges", [], |row| row.get(0))
            .unwrap();
        assert_eq!(edge_count, 2);
    }

    #[test]
    fn test_clear_knowledge_graph() {
        use crate::indexer::knowledge::{DocSubtype, KnowledgeEntry, KnowledgeRelation};

        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        let entries = vec![KnowledgeEntry {
            issue_number: "100".to_string(),
            file_path: "dev-reports/issue/100/work-plan.md".to_string(),
            relation: KnowledgeRelation::HasWorkplan,
            doc_subtype: DocSubtype::WorkPlan,
        }];
        store.insert_knowledge_entries(&entries).unwrap();

        store.clear_knowledge_graph().unwrap();

        let count: i64 = store
            .conn
            .query_row("SELECT COUNT(*) FROM knowledge_nodes", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_delete_knowledge_by_file_cascades() {
        use crate::indexer::knowledge::{DocSubtype, KnowledgeEntry, KnowledgeRelation};

        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        let entries = vec![KnowledgeEntry {
            issue_number: "100".to_string(),
            file_path: "dev-reports/issue/100/work-plan.md".to_string(),
            relation: KnowledgeRelation::HasWorkplan,
            doc_subtype: DocSubtype::WorkPlan,
        }];
        store.insert_knowledge_entries(&entries).unwrap();

        // Delete the document node by file path
        store
            .delete_knowledge_by_file("dev-reports/issue/100/work-plan.md")
            .unwrap();

        // Document node should be gone
        let doc_count: i64 = store
            .conn
            .query_row(
                "SELECT COUNT(*) FROM knowledge_nodes WHERE type = 'document'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(doc_count, 0);

        // Edge should be cascade-deleted
        let edge_count: i64 = store
            .conn
            .query_row("SELECT COUNT(*) FROM knowledge_edges", [], |row| row.get(0))
            .unwrap();
        assert_eq!(edge_count, 0);

        // Issue node should remain (orphan)
        let issue_count: i64 = store
            .conn
            .query_row(
                "SELECT COUNT(*) FROM knowledge_nodes WHERE type = 'issue'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(issue_count, 1);
    }

    #[test]
    fn test_find_knowledge_related() {
        use crate::indexer::knowledge::{DocSubtype, KnowledgeEntry, KnowledgeRelation};

        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        let entries = vec![
            KnowledgeEntry {
                issue_number: "100".to_string(),
                file_path: "dev-reports/design/issue-100-test-design-policy.md".to_string(),
                relation: KnowledgeRelation::HasDesign,
                doc_subtype: DocSubtype::DesignPolicy,
            },
            KnowledgeEntry {
                issue_number: "100".to_string(),
                file_path: "dev-reports/issue/100/work-plan.md".to_string(),
                relation: KnowledgeRelation::HasWorkplan,
                doc_subtype: DocSubtype::WorkPlan,
            },
            KnowledgeEntry {
                issue_number: "100".to_string(),
                file_path: "dev-reports/issue/100/issue-review/summary-report.md".to_string(),
                relation: KnowledgeRelation::HasReview,
                doc_subtype: DocSubtype::IssueReview,
            },
        ];
        store.insert_knowledge_entries(&entries).unwrap();

        // Query from design policy -> should find work-plan and issue-review
        let related = store
            .find_knowledge_related("dev-reports/design/issue-100-test-design-policy.md")
            .unwrap();
        assert_eq!(related.len(), 2);

        let paths: Vec<&str> = related.iter().map(|r| r.file_path.as_str()).collect();
        assert!(paths.contains(&"dev-reports/issue/100/work-plan.md"));
        assert!(paths.contains(&"dev-reports/issue/100/issue-review/summary-report.md"));

        // All should reference issue 100
        assert!(related.iter().all(|r| r.issue_number == "100"));

        // title should be None (insert_knowledge_entries does not set issue title)
        assert!(related.iter().all(|r| r.title.is_none()));
    }

    #[test]
    fn test_find_knowledge_related_with_title() {
        use crate::indexer::knowledge::{DocSubtype, KnowledgeEntry, KnowledgeRelation};

        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        let entries = vec![
            KnowledgeEntry {
                issue_number: "200".to_string(),
                file_path: "dev-reports/design/issue-200-test-design-policy.md".to_string(),
                relation: KnowledgeRelation::HasDesign,
                doc_subtype: DocSubtype::DesignPolicy,
            },
            KnowledgeEntry {
                issue_number: "200".to_string(),
                file_path: "dev-reports/issue/200/work-plan.md".to_string(),
                relation: KnowledgeRelation::HasWorkplan,
                doc_subtype: DocSubtype::WorkPlan,
            },
        ];
        store.insert_knowledge_entries(&entries).unwrap();

        // Set the issue title via upsert_knowledge_node
        store
            .upsert_knowledge_node("issue", "200", Some("Add why command"), None)
            .unwrap();

        let related = store
            .find_knowledge_related("dev-reports/design/issue-200-test-design-policy.md")
            .unwrap();
        assert_eq!(related.len(), 1);
        assert_eq!(related[0].title.as_deref(), Some("Add why command"));
    }

    #[test]
    fn test_find_knowledge_related_no_results() {
        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        let related = store.find_knowledge_related("src/main.rs").unwrap();
        assert!(related.is_empty());
    }

    #[test]
    fn test_find_knowledge_by_issue_normal() {
        use crate::indexer::knowledge::{DocSubtype, KnowledgeEntry, KnowledgeRelation};

        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        let entries = vec![
            KnowledgeEntry {
                issue_number: "100".to_string(),
                file_path: "dev-reports/design/issue-100-test-design-policy.md".to_string(),
                relation: KnowledgeRelation::HasDesign,
                doc_subtype: DocSubtype::DesignPolicy,
            },
            KnowledgeEntry {
                issue_number: "100".to_string(),
                file_path: "dev-reports/issue/100/work-plan.md".to_string(),
                relation: KnowledgeRelation::HasWorkplan,
                doc_subtype: DocSubtype::WorkPlan,
            },
            KnowledgeEntry {
                issue_number: "200".to_string(),
                file_path: "dev-reports/design/issue-200-feature-design-policy.md".to_string(),
                relation: KnowledgeRelation::HasDesign,
                doc_subtype: DocSubtype::DesignPolicy,
            },
        ];
        store.insert_knowledge_entries(&entries).unwrap();

        // Query for issue 100
        let results = store.find_knowledge_by_issue(&["100".to_string()]).unwrap();
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.issue_number == "100"));

        // Query for issue 200
        let results = store.find_knowledge_by_issue(&["200".to_string()]).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].issue_number, "200");

        // Query for both
        let results = store
            .find_knowledge_by_issue(&["100".to_string(), "200".to_string()])
            .unwrap();
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_find_knowledge_by_issue_empty_input() {
        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        let results = store.find_knowledge_by_issue(&[]).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_find_knowledge_by_issue_no_match() {
        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        let results = store.find_knowledge_by_issue(&["999".to_string()]).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_find_documents_by_issue() {
        use crate::indexer::knowledge::{DocSubtype, KnowledgeEntry, KnowledgeRelation};

        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        let entries = vec![
            KnowledgeEntry {
                issue_number: "140".to_string(),
                file_path: "dev-reports/design/issue-140-issue-cmd-design-policy.md".to_string(),
                relation: KnowledgeRelation::HasDesign,
                doc_subtype: DocSubtype::DesignPolicy,
            },
            KnowledgeEntry {
                issue_number: "140".to_string(),
                file_path: "dev-reports/issue/140/work-plan.md".to_string(),
                relation: KnowledgeRelation::HasWorkplan,
                doc_subtype: DocSubtype::WorkPlan,
            },
            KnowledgeEntry {
                issue_number: "140".to_string(),
                file_path: "dev-reports/issue/140/issue-review/summary-report.md".to_string(),
                relation: KnowledgeRelation::HasReview,
                doc_subtype: DocSubtype::IssueReview,
            },
        ];
        store.insert_knowledge_entries(&entries).unwrap();

        let docs = store.find_documents_by_issue("140").unwrap();
        assert_eq!(docs.len(), 3);

        let paths: Vec<&str> = docs.iter().map(|d| d.file_path.as_str()).collect();
        assert!(paths.contains(&"dev-reports/design/issue-140-issue-cmd-design-policy.md"));
        assert!(paths.contains(&"dev-reports/issue/140/work-plan.md"));
        assert!(paths.contains(&"dev-reports/issue/140/issue-review/summary-report.md"));

        // Verify relation types
        let design = docs
            .iter()
            .find(|d| d.doc_subtype == DocSubtype::DesignPolicy)
            .unwrap();
        assert_eq!(design.relation, KnowledgeRelation::HasDesign);
    }

    #[test]
    fn test_find_documents_by_issue_no_results() {
        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        let docs = store.find_documents_by_issue("999").unwrap();
        assert!(docs.is_empty());
    }

    #[test]
    fn test_find_documents_by_issue_metadata_parsed() {
        use crate::indexer::knowledge::{DocSubtype, KnowledgeEntry, KnowledgeRelation};

        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        let entries = vec![KnowledgeEntry {
            issue_number: "42".to_string(),
            file_path: "dev-reports/issue/42/pm-auto-dev/iteration-1/progress-report.md"
                .to_string(),
            relation: KnowledgeRelation::HasReview,
            doc_subtype: DocSubtype::ProgressReport,
        }];
        store.insert_knowledge_entries(&entries).unwrap();

        let docs = store.find_documents_by_issue("42").unwrap();
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].doc_subtype, DocSubtype::ProgressReport);
        assert_eq!(docs[0].relation, KnowledgeRelation::HasReview);
    }

    // --- insert_file_modifies_entries tests ---

    #[test]
    fn test_insert_file_modifies_entries_basic() {
        use crate::indexer::knowledge::FileModifiesEntry;

        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        let entries = vec![
            FileModifiesEntry {
                issue_number: "100".to_string(),
                file_path: "src/main.rs".to_string(),
            },
            FileModifiesEntry {
                issue_number: "100".to_string(),
                file_path: "src/lib.rs".to_string(),
            },
        ];
        store.insert_file_modifies_entries(&entries).unwrap();

        // Verify nodes created
        let count: i64 = store
            .conn
            .query_row(
                "SELECT COUNT(*) FROM knowledge_nodes WHERE type = 'file'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 2);

        // Verify edges created
        let edge_count: i64 = store
            .conn
            .query_row(
                "SELECT COUNT(*) FROM knowledge_edges WHERE relation = 'modifies'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(edge_count, 2);
    }

    #[test]
    fn test_insert_file_modifies_entries_duplicate() {
        use crate::indexer::knowledge::FileModifiesEntry;

        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        let entries = vec![FileModifiesEntry {
            issue_number: "100".to_string(),
            file_path: "src/main.rs".to_string(),
        }];
        store.insert_file_modifies_entries(&entries).unwrap();
        // Insert same again - should not fail
        store.insert_file_modifies_entries(&entries).unwrap();

        let edge_count: i64 = store
            .conn
            .query_row(
                "SELECT COUNT(*) FROM knowledge_edges WHERE relation = 'modifies'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(edge_count, 1);
    }

    #[test]
    fn test_insert_file_modifies_entries_empty() {
        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        let entries: Vec<crate::indexer::knowledge::FileModifiesEntry> = vec![];
        store.insert_file_modifies_entries(&entries).unwrap();
    }

    // --- clear_file_modifies tests ---

    #[test]
    fn test_clear_file_modifies() {
        use crate::indexer::knowledge::FileModifiesEntry;

        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        let entries = vec![
            FileModifiesEntry {
                issue_number: "100".to_string(),
                file_path: "src/main.rs".to_string(),
            },
            FileModifiesEntry {
                issue_number: "100".to_string(),
                file_path: "src/lib.rs".to_string(),
            },
        ];
        store.insert_file_modifies_entries(&entries).unwrap();

        store.clear_file_modifies().unwrap();

        let file_count: i64 = store
            .conn
            .query_row(
                "SELECT COUNT(*) FROM knowledge_nodes WHERE type = 'file'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(file_count, 0);

        let edge_count: i64 = store
            .conn
            .query_row(
                "SELECT COUNT(*) FROM knowledge_edges WHERE relation = 'modifies'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(edge_count, 0);
    }

    #[test]
    fn test_clear_file_modifies_preserves_document_edges() {
        use crate::indexer::knowledge::{
            DocSubtype, FileModifiesEntry, KnowledgeEntry, KnowledgeRelation,
        };

        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        // Insert document knowledge entry
        let doc_entries = vec![KnowledgeEntry {
            issue_number: "100".to_string(),
            file_path: "dev-reports/design/issue-100-design-policy.md".to_string(),
            relation: KnowledgeRelation::HasDesign,
            doc_subtype: DocSubtype::DesignPolicy,
        }];
        store.insert_knowledge_entries(&doc_entries).unwrap();

        // Insert file-modifies entry for same issue
        let file_entries = vec![FileModifiesEntry {
            issue_number: "100".to_string(),
            file_path: "src/main.rs".to_string(),
        }];
        store.insert_file_modifies_entries(&file_entries).unwrap();

        // Clear file-modifies only
        store.clear_file_modifies().unwrap();

        // Document edges should still exist
        let doc_count: i64 = store
            .conn
            .query_row(
                "SELECT COUNT(*) FROM knowledge_edges WHERE relation = 'has_design'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(doc_count, 1);

        // Issue node should still exist (has document edges)
        let issue_count: i64 = store
            .conn
            .query_row(
                "SELECT COUNT(*) FROM knowledge_nodes WHERE type = 'issue'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(issue_count, 1);
    }

    // --- find_knowledge_related with file nodes ---

    #[test]
    fn test_find_knowledge_related_file_to_document() {
        use crate::indexer::knowledge::{
            DocSubtype, FileModifiesEntry, KnowledgeEntry, KnowledgeRelation,
        };

        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        // Issue 100 has a design document
        let doc_entries = vec![KnowledgeEntry {
            issue_number: "100".to_string(),
            file_path: "dev-reports/design/issue-100-design-policy.md".to_string(),
            relation: KnowledgeRelation::HasDesign,
            doc_subtype: DocSubtype::DesignPolicy,
        }];
        store.insert_knowledge_entries(&doc_entries).unwrap();

        // Issue 100 modifies src/main.rs
        let file_entries = vec![FileModifiesEntry {
            issue_number: "100".to_string(),
            file_path: "src/main.rs".to_string(),
        }];
        store.insert_file_modifies_entries(&file_entries).unwrap();

        // Search from file node should find the document
        let related = store.find_knowledge_related("src/main.rs").unwrap();
        assert!(!related.is_empty());
        let doc_paths: Vec<&str> = related.iter().map(|r| r.file_path.as_str()).collect();
        assert!(doc_paths.contains(&"dev-reports/design/issue-100-design-policy.md"));
    }

    #[test]
    fn test_find_knowledge_related_document_to_file() {
        use crate::indexer::knowledge::{
            DocSubtype, FileModifiesEntry, KnowledgeEntry, KnowledgeRelation,
        };

        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        // Issue 100 has a design document
        let doc_entries = vec![KnowledgeEntry {
            issue_number: "100".to_string(),
            file_path: "dev-reports/design/issue-100-design-policy.md".to_string(),
            relation: KnowledgeRelation::HasDesign,
            doc_subtype: DocSubtype::DesignPolicy,
        }];
        store.insert_knowledge_entries(&doc_entries).unwrap();

        // Issue 100 modifies src/main.rs
        let file_entries = vec![FileModifiesEntry {
            issue_number: "100".to_string(),
            file_path: "src/main.rs".to_string(),
        }];
        store.insert_file_modifies_entries(&file_entries).unwrap();

        // Search from document node should find file nodes too
        let related = store
            .find_knowledge_related("dev-reports/design/issue-100-design-policy.md")
            .unwrap();
        let file_paths: Vec<&str> = related.iter().map(|r| r.file_path.as_str()).collect();
        assert!(file_paths.contains(&"src/main.rs"));
    }

    // --- find_knowledge_related DISTINCT dedup ---

    #[test]
    fn test_find_knowledge_related_distinct_dedup() {
        use crate::indexer::knowledge::{
            DocSubtype, FileModifiesEntry, KnowledgeEntry, KnowledgeRelation,
        };

        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        // Create issue #100 with two different relation edges to the same document
        // (has_design and has_workplan pointing to the same doc)
        let doc_entries = vec![
            KnowledgeEntry {
                issue_number: "100".to_string(),
                file_path: "dev-reports/design/issue-100-test-design-policy.md".to_string(),
                relation: KnowledgeRelation::HasDesign,
                doc_subtype: DocSubtype::DesignPolicy,
            },
            KnowledgeEntry {
                issue_number: "100".to_string(),
                file_path: "dev-reports/issue/100/work-plan.md".to_string(),
                relation: KnowledgeRelation::HasWorkplan,
                doc_subtype: DocSubtype::WorkPlan,
            },
        ];
        store.insert_knowledge_entries(&doc_entries).unwrap();

        // Also add modifies edges to the same issue
        let file_entries = vec![
            FileModifiesEntry {
                issue_number: "100".to_string(),
                file_path: "src/main.rs".to_string(),
            },
            FileModifiesEntry {
                issue_number: "100".to_string(),
                file_path: "src/lib.rs".to_string(),
            },
        ];
        store.insert_file_modifies_entries(&file_entries).unwrap();

        // Query from one of the documents: should find sibling docs + modifies files
        // Without DISTINCT, the Cartesian product of ke1 paths x ke2 paths could produce duplicates
        let results = store
            .find_knowledge_related("dev-reports/design/issue-100-test-design-policy.md")
            .unwrap();

        // Verify no duplicates: collect (issue, file_path, relation) tuples
        let mut seen = std::collections::HashSet::new();
        for r in &results {
            let key = (
                r.issue_number.clone(),
                r.file_path.clone(),
                r.relation.clone(),
            );
            assert!(seen.insert(key.clone()), "Duplicate entry found: {:?}", key);
        }

        // Should find: work-plan.md (has_workplan), src/main.rs (modifies), src/lib.rs (modifies)
        // Should NOT find: the query file itself
        assert_eq!(results.len(), 3);
    }

    // --- find_knowledge_by_issue with file nodes ---

    #[test]
    fn test_find_knowledge_by_issue_includes_file_nodes() {
        use crate::indexer::knowledge::{FileModifiesEntry, KnowledgeRelation};

        let store = SymbolStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        let file_entries = vec![FileModifiesEntry {
            issue_number: "100".to_string(),
            file_path: "src/main.rs".to_string(),
        }];
        store.insert_file_modifies_entries(&file_entries).unwrap();

        let results = store.find_knowledge_by_issue(&["100".to_string()]).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].file_path, "src/main.rs");
        assert_eq!(results[0].relation, KnowledgeRelation::Modifies);
    }
}
