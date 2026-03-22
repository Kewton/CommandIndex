use std::fmt;
use std::path::Path;

use rusqlite::{Connection, params};

const CURRENT_EMBEDDING_SCHEMA_VERSION: u32 = 1;

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// A single embedding record retrieved from the database.
#[derive(Debug, Clone)]
pub struct EmbeddingRecord {
    pub id: i64,
    pub section_path: String,
    pub section_heading: String,
    pub embedding: Vec<f32>,
    pub dimension: usize,
    pub model: String,
    pub file_hash: String,
    pub created_at: String,
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur when operating on the embedding store.
#[derive(Debug)]
pub enum EmbeddingStoreError {
    Sqlite(rusqlite::Error),
    Io(std::io::Error),
    SchemaVersionMismatch { expected: u32, found: u32 },
}

impl fmt::Display for EmbeddingStoreError {
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
        }
    }
}

impl std::error::Error for EmbeddingStoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Sqlite(e) => Some(e),
            Self::Io(e) => Some(e),
            Self::SchemaVersionMismatch { .. } => None,
        }
    }
}

impl From<rusqlite::Error> for EmbeddingStoreError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Sqlite(e)
    }
}

impl From<std::io::Error> for EmbeddingStoreError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

// ---------------------------------------------------------------------------
// Helper: f32 slice <-> BLOB conversion
// ---------------------------------------------------------------------------

fn f32_slice_to_bytes(data: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(data.len() * 4);
    for &val in data {
        bytes.extend_from_slice(&val.to_le_bytes());
    }
    bytes
}

fn bytes_to_f32_vec(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes(chunk.try_into().expect("chunks_exact guarantees 4 bytes")))
        .collect()
}

// ---------------------------------------------------------------------------
// EmbeddingStore
// ---------------------------------------------------------------------------

/// SQLite-backed store for embedding vectors.
#[derive(Debug)]
pub struct EmbeddingStore {
    conn: Connection,
}

impl EmbeddingStore {
    /// Open (or create) an embedding store backed by the given file path.
    pub fn open(db_path: &Path) -> Result<Self, EmbeddingStoreError> {
        let conn = Connection::open(db_path)?;

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
            if version != CURRENT_EMBEDDING_SCHEMA_VERSION {
                return Err(EmbeddingStoreError::SchemaVersionMismatch {
                    expected: CURRENT_EMBEDDING_SCHEMA_VERSION,
                    found: version,
                });
            }
        }

        Ok(Self { conn })
    }

    /// Open an in-memory database (for testing).
    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self, EmbeddingStoreError> {
        let conn = Connection::open_in_memory()?;
        Ok(Self { conn })
    }

    /// Create all required tables and indices (idempotent).
    pub fn create_tables(&self) -> Result<(), EmbeddingStoreError> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS embeddings (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                section_path TEXT NOT NULL,
                section_heading TEXT NOT NULL,
                embedding BLOB NOT NULL,
                dimension INTEGER NOT NULL,
                model TEXT NOT NULL,
                file_hash TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE UNIQUE INDEX IF NOT EXISTS idx_embeddings_unique
                ON embeddings(section_path, section_heading, model);
            CREATE INDEX IF NOT EXISTS idx_embeddings_path
                ON embeddings(section_path);
            CREATE INDEX IF NOT EXISTS idx_embeddings_hash
                ON embeddings(section_path, file_hash);",
        )?;

        self.conn.execute(
            "INSERT OR REPLACE INTO schema_meta (key, value) VALUES (?1, ?2)",
            params![
                "schema_version",
                CURRENT_EMBEDDING_SCHEMA_VERSION.to_string()
            ],
        )?;

        Ok(())
    }

    /// Embedding を保存（INSERT OR REPLACE で upsert）
    pub fn upsert_embedding(
        &self,
        section_path: &str,
        section_heading: &str,
        embedding: &[f32],
        dimension: usize,
        model: &str,
        file_hash: &str,
    ) -> Result<(), EmbeddingStoreError> {
        let blob = f32_slice_to_bytes(embedding);
        self.conn.execute(
            "INSERT OR REPLACE INTO embeddings
                (section_path, section_heading, embedding, dimension, model, file_hash, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'))",
            params![
                section_path,
                section_heading,
                blob,
                dimension as i64,
                model,
                file_hash
            ],
        )?;
        Ok(())
    }

    /// ファイルパスでembeddingを検索
    pub fn find_by_path(&self, path: &str) -> Result<Vec<EmbeddingRecord>, EmbeddingStoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, section_path, section_heading, embedding, dimension, model, file_hash, created_at
             FROM embeddings WHERE section_path = ?1",
        )?;
        let rows = stmt.query_map(params![path], |row| {
            let blob: Vec<u8> = row.get(3)?;
            Ok(EmbeddingRecord {
                id: row.get(0)?,
                section_path: row.get(1)?,
                section_heading: row.get(2)?,
                embedding: bytes_to_f32_vec(&blob),
                dimension: row.get::<_, i64>(4)? as usize,
                model: row.get(5)?,
                file_hash: row.get(6)?,
                created_at: row.get(7)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// ファイルハッシュでキャッシュチェック
    pub fn has_current_embedding(
        &self,
        path: &str,
        file_hash: &str,
    ) -> Result<bool, EmbeddingStoreError> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM embeddings WHERE section_path = ?1 AND file_hash = ?2",
            params![path, file_hash],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// ファイルのembeddingを削除
    pub fn delete_by_path(&self, path: &str) -> Result<(), EmbeddingStoreError> {
        self.conn.execute(
            "DELETE FROM embeddings WHERE section_path = ?1",
            params![path],
        )?;
        Ok(())
    }

    /// 全embedding数を取得
    pub fn count(&self) -> Result<u64, EmbeddingStoreError> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM embeddings", [], |row| row.get(0))?;
        Ok(count as u64)
    }

    /// ユニークなファイル数を取得
    pub fn count_distinct_files(&self) -> Result<u64, EmbeddingStoreError> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(DISTINCT section_path) FROM embeddings",
            [],
            |row| row.get(0),
        )?;
        Ok(count as u64)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_open_and_create_tables() {
        let store = EmbeddingStore::open_in_memory().unwrap();
        store.create_tables().unwrap();
    }

    #[test]
    fn test_create_tables_idempotent() {
        let store = EmbeddingStore::open_in_memory().unwrap();
        store.create_tables().unwrap();
        store.create_tables().unwrap();
    }

    #[test]
    fn test_upsert_and_find_by_path() {
        let store = EmbeddingStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        let embedding = vec![0.1_f32, 0.2, 0.3];
        store
            .upsert_embedding("src/main.rs", "main", &embedding, 3, "nomic", "hash123")
            .unwrap();

        let results = store.find_by_path("src/main.rs").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].section_path, "src/main.rs");
        assert_eq!(results[0].section_heading, "main");
        assert_eq!(results[0].dimension, 3);
        assert_eq!(results[0].model, "nomic");
        assert_eq!(results[0].file_hash, "hash123");
        assert_eq!(results[0].embedding.len(), 3);
        assert!((results[0].embedding[0] - 0.1).abs() < f32::EPSILON);
        assert!((results[0].embedding[1] - 0.2).abs() < f32::EPSILON);
        assert!((results[0].embedding[2] - 0.3).abs() < f32::EPSILON);
    }

    #[test]
    fn test_upsert_replaces_on_duplicate() {
        let store = EmbeddingStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        let embedding1 = vec![0.1_f32, 0.2, 0.3];
        store
            .upsert_embedding("src/main.rs", "main", &embedding1, 3, "nomic", "hash1")
            .unwrap();

        // Same section_path + section_heading + model => replace
        let embedding2 = vec![0.4_f32, 0.5, 0.6];
        store
            .upsert_embedding("src/main.rs", "main", &embedding2, 3, "nomic", "hash2")
            .unwrap();

        let results = store.find_by_path("src/main.rs").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].file_hash, "hash2");
        assert!((results[0].embedding[0] - 0.4).abs() < f32::EPSILON);
    }

    #[test]
    fn test_upsert_different_headings_are_separate() {
        let store = EmbeddingStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        store
            .upsert_embedding("src/main.rs", "heading1", &[0.1], 1, "nomic", "hash1")
            .unwrap();
        store
            .upsert_embedding("src/main.rs", "heading2", &[0.2], 1, "nomic", "hash1")
            .unwrap();

        let results = store.find_by_path("src/main.rs").unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_has_current_embedding_true() {
        let store = EmbeddingStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        store
            .upsert_embedding("src/main.rs", "main", &[0.1], 1, "nomic", "hash123")
            .unwrap();

        assert!(
            store
                .has_current_embedding("src/main.rs", "hash123")
                .unwrap()
        );
    }

    #[test]
    fn test_has_current_embedding_false_different_hash() {
        let store = EmbeddingStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        store
            .upsert_embedding("src/main.rs", "main", &[0.1], 1, "nomic", "hash123")
            .unwrap();

        assert!(
            !store
                .has_current_embedding("src/main.rs", "hash456")
                .unwrap()
        );
    }

    #[test]
    fn test_has_current_embedding_false_no_record() {
        let store = EmbeddingStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        assert!(
            !store
                .has_current_embedding("nonexistent.rs", "hash123")
                .unwrap()
        );
    }

    #[test]
    fn test_delete_by_path() {
        let store = EmbeddingStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        store
            .upsert_embedding("src/main.rs", "main", &[0.1], 1, "nomic", "hash1")
            .unwrap();
        store
            .upsert_embedding("src/lib.rs", "lib", &[0.2], 1, "nomic", "hash2")
            .unwrap();

        store.delete_by_path("src/main.rs").unwrap();

        assert!(store.find_by_path("src/main.rs").unwrap().is_empty());
        assert_eq!(store.find_by_path("src/lib.rs").unwrap().len(), 1);
    }

    #[test]
    fn test_count_empty() {
        let store = EmbeddingStore::open_in_memory().unwrap();
        store.create_tables().unwrap();
        assert_eq!(store.count().unwrap(), 0);
    }

    #[test]
    fn test_count_after_inserts() {
        let store = EmbeddingStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        store
            .upsert_embedding("src/main.rs", "main", &[0.1], 1, "nomic", "hash1")
            .unwrap();
        store
            .upsert_embedding("src/lib.rs", "lib", &[0.2], 1, "nomic", "hash2")
            .unwrap();

        assert_eq!(store.count().unwrap(), 2);
    }

    #[test]
    fn test_find_by_path_empty() {
        let store = EmbeddingStore::open_in_memory().unwrap();
        store.create_tables().unwrap();
        assert!(store.find_by_path("nonexistent.rs").unwrap().is_empty());
    }

    #[test]
    fn test_open_creates_db_file() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("embeddings.db");
        assert!(!db_path.exists());
        let _store = EmbeddingStore::open(&db_path).unwrap();
        assert!(db_path.exists());
    }

    #[test]
    fn test_schema_version_check() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("embeddings.db");

        // Create store and tables, then tamper with version
        {
            let store = EmbeddingStore::open(&db_path).unwrap();
            store.create_tables().unwrap();
            store
                .conn
                .execute(
                    "UPDATE schema_meta SET value = ?1 WHERE key = 'schema_version'",
                    params!["999"],
                )
                .unwrap();
        }

        // Re-open should fail
        let result = EmbeddingStore::open(&db_path);
        assert!(result.is_err());
        match result.unwrap_err() {
            EmbeddingStoreError::SchemaVersionMismatch { expected, found } => {
                assert_eq!(expected, CURRENT_EMBEDDING_SCHEMA_VERSION);
                assert_eq!(found, 999);
            }
            other => panic!("Expected SchemaVersionMismatch, got: {other}"),
        }
    }

    #[test]
    fn test_count_distinct_files_empty() {
        let store = EmbeddingStore::open_in_memory().unwrap();
        store.create_tables().unwrap();
        assert_eq!(store.count_distinct_files().unwrap(), 0);
    }

    #[test]
    fn test_count_distinct_files_with_data() {
        let store = EmbeddingStore::open_in_memory().unwrap();
        store.create_tables().unwrap();

        // Same file, two different headings
        store
            .upsert_embedding("src/main.rs", "heading1", &[0.1], 1, "nomic", "hash1")
            .unwrap();
        store
            .upsert_embedding("src/main.rs", "heading2", &[0.2], 1, "nomic", "hash1")
            .unwrap();
        // Different file
        store
            .upsert_embedding("src/lib.rs", "lib", &[0.3], 1, "nomic", "hash2")
            .unwrap();

        assert_eq!(store.count_distinct_files().unwrap(), 2);
    }

    #[test]
    fn test_f32_blob_roundtrip() {
        let original = vec![1.0_f32, -2.5, 3.125, 0.0, f32::MAX, f32::MIN];
        let bytes = f32_slice_to_bytes(&original);
        let restored = bytes_to_f32_vec(&bytes);
        assert_eq!(original, restored);
    }
}
