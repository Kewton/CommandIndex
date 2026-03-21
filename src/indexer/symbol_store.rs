use std::fmt;
use std::path::Path;

use rusqlite::{Connection, params};

const CURRENT_SYMBOL_SCHEMA_VERSION: u32 = 2;

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
        }
    }
}

impl std::error::Error for SymbolStoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Sqlite(e) => Some(e),
            Self::Io(e) => Some(e),
            Self::SchemaVersionMismatch { .. } => None,
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
            CREATE INDEX IF NOT EXISTS idx_file_links_target ON file_links(target_file);",
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
}
