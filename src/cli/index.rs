use std::fmt;
use std::path::Path;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};

use crate::embedding::store::{EmbeddingStore, EmbeddingStoreError};
use crate::embedding::{Config, EmbeddingError, create_provider};
use crate::indexer::diff::{DiffError, detect_changes, scan_files};
use crate::indexer::manifest::{
    self, FileEntry, FileType, Manifest, ManifestError, to_relative_path_string,
};
use crate::indexer::reader::IndexReaderWrapper;
use crate::indexer::state::{IndexState, StateError};
use crate::indexer::symbol_store::{SymbolStore, SymbolStoreError};
use crate::indexer::writer::{IndexWriterWrapper, SectionDoc, WriterError};
use crate::parser::code::{CodeParseError, parse_code_file};
use crate::parser::ignore::{IgnoreError, IgnoreFilter};
use crate::parser::markdown::{self, ParseError};

#[derive(Debug)]
pub enum IndexError {
    Io(std::io::Error),
    Parse(ParseError),
    Writer(WriterError),
    State(StateError),
    Manifest(ManifestError),
    Ignore(IgnoreError),
    Diff(DiffError),
    CodeParse(CodeParseError),
    SymbolStore(SymbolStoreError),
    Embedding(EmbeddingError),
    EmbeddingStore(EmbeddingStoreError),
    IndexNotFound,
    SchemaVersionMismatch,
    IndexCorrupted(String),
}

impl fmt::Display for IndexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IndexError::Io(e) => write!(f, "IO error: {e}"),
            IndexError::Parse(e) => write!(f, "Parse error: {e}"),
            IndexError::Writer(e) => write!(f, "Index writer error: {e}"),
            IndexError::State(e) => write!(f, "State error: {e}"),
            IndexError::Manifest(e) => write!(f, "Manifest error: {e}"),
            IndexError::Ignore(e) => write!(f, "Ignore filter error: {e}"),
            IndexError::Diff(e) => write!(f, "Diff error: {e}"),
            IndexError::CodeParse(e) => write!(f, "Code parse error: {e}"),
            IndexError::SymbolStore(e) => write!(f, "Symbol store error: {e}"),
            IndexError::Embedding(e) => write!(f, "Embedding error: {e}"),
            IndexError::EmbeddingStore(e) => write!(f, "Embedding store error: {e}"),
            IndexError::IndexNotFound => write!(
                f,
                "No index found. Run `commandindex index` to build the index first."
            ),
            IndexError::SchemaVersionMismatch => write!(
                f,
                "Index schema version mismatch. Run `commandindex clean` then `commandindex index` to rebuild."
            ),
            IndexError::IndexCorrupted(detail) => write!(
                f,
                "Failed to read index state: {detail}. Run `commandindex clean` then `commandindex index` to rebuild."
            ),
        }
    }
}

impl std::error::Error for IndexError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            IndexError::Io(e) => Some(e),
            IndexError::Parse(e) => Some(e),
            IndexError::Writer(e) => Some(e),
            IndexError::State(e) => Some(e),
            IndexError::Manifest(e) => Some(e),
            IndexError::Ignore(e) => Some(e),
            IndexError::Diff(e) => Some(e),
            IndexError::CodeParse(e) => Some(e),
            IndexError::SymbolStore(e) => Some(e),
            IndexError::Embedding(e) => Some(e),
            IndexError::EmbeddingStore(e) => Some(e),
            IndexError::IndexNotFound
            | IndexError::SchemaVersionMismatch
            | IndexError::IndexCorrupted(_) => None,
        }
    }
}

impl From<std::io::Error> for IndexError {
    fn from(e: std::io::Error) -> Self {
        IndexError::Io(e)
    }
}

impl From<ParseError> for IndexError {
    fn from(e: ParseError) -> Self {
        IndexError::Parse(e)
    }
}

impl From<WriterError> for IndexError {
    fn from(e: WriterError) -> Self {
        IndexError::Writer(e)
    }
}

impl From<StateError> for IndexError {
    fn from(e: StateError) -> Self {
        IndexError::State(e)
    }
}

impl From<ManifestError> for IndexError {
    fn from(e: ManifestError) -> Self {
        IndexError::Manifest(e)
    }
}

impl From<IgnoreError> for IndexError {
    fn from(e: IgnoreError) -> Self {
        IndexError::Ignore(e)
    }
}

impl From<DiffError> for IndexError {
    fn from(e: DiffError) -> Self {
        IndexError::Diff(e)
    }
}

impl From<CodeParseError> for IndexError {
    fn from(e: CodeParseError) -> Self {
        IndexError::CodeParse(e)
    }
}

impl From<SymbolStoreError> for IndexError {
    fn from(e: SymbolStoreError) -> Self {
        match e {
            SymbolStoreError::SchemaVersionMismatch { .. } => IndexError::SchemaVersionMismatch,
            other => IndexError::SymbolStore(other),
        }
    }
}

impl From<EmbeddingError> for IndexError {
    fn from(e: EmbeddingError) -> Self {
        IndexError::Embedding(e)
    }
}

impl From<EmbeddingStoreError> for IndexError {
    fn from(e: EmbeddingStoreError) -> Self {
        IndexError::EmbeddingStore(e)
    }
}

/// インデックスオプション（Default実装で後方互換性を維持）
#[derive(Debug, Default)]
pub struct IndexOptions {
    pub with_embedding: bool,
}

/// コードファイル識別用の heading_level 定数（Markdown の heading_level 1-6 と区別するため 0 を使用）
const CODE_FILE_HEADING_LEVEL: u64 = 0;

/// 1ファイルあたりのリンク格納上限
const MAX_FILE_LINKS: usize = 10_000;

/// インデックスに格納すべきリンクかどうかを判定する
fn is_indexable_link(link: &crate::parser::link::Link) -> bool {
    let target = &link.target;
    if target.len() > 1024 {
        return false;
    }
    if target.starts_with('#') {
        return false;
    }
    // Exclude absolute URIs: any scheme matching ^[a-zA-Z][a-zA-Z0-9+.-]*:
    if let Some(colon_pos) = target.find(':') {
        let scheme = &target[..colon_pos];
        if !scheme.is_empty()
            && scheme.as_bytes()[0].is_ascii_alphabetic()
            && scheme
                .bytes()
                .skip(1)
                .all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'-' || b == b'.')
        {
            return false;
        }
    }
    true
}

/// parser::code::SymbolInfo を symbol_store::SymbolInfo に変換する
fn convert_symbol(
    src: &crate::parser::code::SymbolInfo,
    file_path: &str,
    file_hash: &str,
) -> crate::indexer::symbol_store::SymbolInfo {
    crate::indexer::symbol_store::SymbolInfo {
        id: None,
        name: src.name.clone(),
        kind: src.kind.to_string(),
        file_path: file_path.to_string(),
        line_start: u32::try_from(src.line_start).unwrap_or(u32::MAX),
        line_end: u32::try_from(src.line_end).unwrap_or(u32::MAX),
        parent_symbol_id: None,
        file_hash: file_hash.to_string(),
    }
}

pub struct IndexSummary {
    pub scanned: u64,
    pub indexed_sections: u64,
    pub skipped: u64,
    pub ignored: u64,
    pub duration: Duration,
}

fn section_to_doc(
    section: &crate::parser::markdown::Section,
    file_path: &str,
    frontmatter: Option<&crate::parser::Frontmatter>,
) -> SectionDoc {
    let tags = frontmatter.map(|fm| fm.tags.join(" ")).unwrap_or_default();

    SectionDoc {
        path: file_path.to_string(),
        heading: section.heading.clone(),
        body: section.body.clone(),
        tags,
        heading_level: section.level as u64,
        line_start: section.line_start as u64,
    }
}

pub fn run(path: &Path, options: &IndexOptions) -> Result<IndexSummary, IndexError> {
    let start = Instant::now();

    // 1. Validate target directory
    if !path.is_dir() {
        return Err(IndexError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Directory not found: {}", path.display()),
        )));
    }

    // 2. Load .cmindexignore
    let ignore_filter = IgnoreFilter::from_file(&path.join(".cmindexignore"))?;

    // 3. Scan files using scan_files() with all supported extensions
    let scan_result = scan_files(path, &ignore_filter, FileType::all_extensions())?;
    let ignored = scan_result.ignored_count;

    // 4. Remove existing tantivy directory if present
    let commandindex_dir = crate::indexer::commandindex_dir(path);
    let tantivy_dir = crate::indexer::index_dir(path);
    if tantivy_dir.exists() {
        std::fs::remove_dir_all(&tantivy_dir)?;
    }

    // Ensure .commandindex directory exists before opening SQLite DB
    std::fs::create_dir_all(&commandindex_dir)?;

    // 5. Initialize SymbolStore: delete old symbols.db + WAL/SHM files, then open fresh
    let db_path = crate::indexer::symbol_db_path(path);
    for suffix in &["", "-wal", "-shm"] {
        let p = db_path.with_extension(format!("db{suffix}"));
        if p.exists() {
            let _ = std::fs::remove_file(&p);
        }
    }
    let symbol_store = SymbolStore::open(&db_path)?;
    symbol_store.create_tables()?;

    // 6. Create new tantivy index
    let mut writer = IndexWriterWrapper::open(&tantivy_dir)?;

    let mut scanned: u64 = 0;
    let mut indexed_sections: u64 = 0;
    let mut skipped: u64 = 0;
    let mut manifest = Manifest::new();

    // 7. Parse each file and add to index using index_file_and_upsert()
    for file_path in &scan_result.files {
        scanned += 1;

        let rel_path = to_relative_path_string(file_path, path);

        match index_file_and_upsert(
            file_path,
            &rel_path,
            &mut writer,
            &mut manifest,
            Some(&symbol_store),
        ) {
            Ok(section_count) => {
                indexed_sections += section_count;
            }
            Err(IndexFileResult::Skipped) => {
                skipped += 1;
            }
            Err(IndexFileResult::Error(e)) => return Err(e),
        }
    }

    // 8. Commit index
    writer.commit()?;

    // 9. Save manifest
    manifest.save(&commandindex_dir)?;

    // 10. Save state
    let mut state = IndexState::new(path.to_path_buf());
    state.total_files = scanned;
    state.total_sections = indexed_sections;
    state.save(&commandindex_dir)?;

    // 11. Generate embeddings if requested
    if options.with_embedding {
        generate_embeddings_for_manifest(path, &commandindex_dir, &manifest)?;
    }

    // 12. Return summary
    Ok(IndexSummary {
        scanned,
        indexed_sections,
        skipped,
        ignored,
        duration: start.elapsed(),
    })
}

pub struct IncrementalSummary {
    pub added_files: u64,
    pub added_sections: u64,
    pub modified_files: u64,
    pub modified_sections: u64,
    pub deleted_files: u64,
    pub unchanged: u64,
    pub skipped: u64,
    pub duration: Duration,
}

/// Result type for parse failures that should be skipped vs fatal errors.
enum IndexFileResult {
    Skipped,
    Error(IndexError),
}

/// Markdown ファイルのインデックス処理
fn index_markdown_file(
    file_path: &Path,
    rel_path: &str,
    writer: &mut IndexWriterWrapper,
    manifest: &mut Manifest,
    symbol_store: Option<&SymbolStore>,
) -> Result<u64, IndexFileResult> {
    let doc = match markdown::parse_file(file_path) {
        Ok(doc) => doc,
        Err(e) => {
            eprintln!("Warning: skipping {rel_path}: {e}");
            return Err(IndexFileResult::Skipped);
        }
    };

    let hash =
        manifest::compute_file_hash(file_path).unwrap_or_else(|_| "sha256:unknown".to_string());

    // Delete old file links if symbol_store is available
    if let Some(store) = symbol_store {
        store
            .delete_by_file(rel_path)
            .map_err(|e| IndexFileResult::Error(e.into()))?;
    }

    let section_count = doc.sections.len() as u64;
    for section in &doc.sections {
        let section_doc = section_to_doc(section, rel_path, doc.frontmatter.as_ref());
        if let Err(e) = writer.add_section(&section_doc) {
            // Tantivy write failed: rollback SQLite deletion
            if let Some(store) = symbol_store {
                let _ = store.delete_by_file(rel_path);
            }
            return Err(IndexFileResult::Error(e.into()));
        }
    }

    // Store file links after successful Tantivy writes
    if let Some(store) = symbol_store {
        let mut filtered_links: Vec<_> =
            doc.links.iter().filter(|l| is_indexable_link(l)).collect();
        if filtered_links.len() > MAX_FILE_LINKS {
            eprintln!(
                "Warning: {rel_path} has {} links, truncating to {MAX_FILE_LINKS}",
                filtered_links.len()
            );
            filtered_links.truncate(MAX_FILE_LINKS);
        }
        let file_links: Vec<_> = filtered_links
            .iter()
            .map(|l| crate::indexer::symbol_store::FileLinkInfo {
                id: None,
                source_file: rel_path.to_string(),
                target_file: l.target.clone(),
                link_type: l.link_type.to_string(),
                file_hash: hash.clone(),
            })
            .collect();
        if !file_links.is_empty()
            && let Err(e) = store.insert_file_links(&file_links)
        {
            eprintln!("Warning: file link insert failed for {rel_path}: {e}");
        }
    }

    let last_modified = std::fs::metadata(file_path)
        .and_then(|m| m.modified())
        .map(DateTime::<Utc>::from)
        .unwrap_or_else(|_| Utc::now());

    manifest.upsert_entry(FileEntry {
        path: rel_path.to_string(),
        hash,
        last_modified,
        sections: section_count,
        file_type: FileType::Markdown,
    });

    Ok(section_count)
}

/// parent_symbol_id を解決する（2パス方式の2パス目）
fn resolve_parent_symbols(
    symbol_store: &SymbolStore,
    rel_path: &str,
    parser_symbols: &[crate::parser::code::SymbolInfo],
) -> Result<(), IndexError> {
    let inserted = symbol_store.find_by_file(rel_path)?;
    if inserted.len() != parser_symbols.len() {
        return Ok(());
    }

    // name → id マッピング構築
    let name_to_id: std::collections::HashMap<&str, i64> = inserted
        .iter()
        .filter(|s| s.parent_symbol_id.is_none()) // 親候補のみ（クラス等）
        .filter_map(|s| s.id.map(|id| (s.name.as_str(), id)))
        .collect();

    for (parser_sym, store_sym) in parser_symbols.iter().zip(inserted.iter()) {
        if let (Some(parent_name), Some(child_id)) = (&parser_sym.parent, store_sym.id)
            && let Some(&parent_id) = name_to_id.get(parent_name.as_str())
        {
            symbol_store.update_parent_symbol_id(child_id, parent_id)?;
        }
    }
    Ok(())
}

/// コードファイルのインデックス処理（symbols.db + tantivy）
fn index_code_file(
    file_path: &Path,
    rel_path: &str,
    writer: &mut IndexWriterWrapper,
    manifest: &mut Manifest,
    symbol_store: &SymbolStore,
    file_type: FileType,
) -> Result<u64, IndexFileResult> {
    // 空ファイルスキップ
    let content = match std::fs::read_to_string(file_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Warning: skipping {rel_path}: {e}");
            return Err(IndexFileResult::Skipped);
        }
    };
    if content.is_empty() {
        return Ok(0);
    }

    // tree-sitter パース
    let result = match parse_code_file(file_path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Warning: skipping {rel_path}: {e}");
            return Err(IndexFileResult::Skipped);
        }
    };

    let hash =
        manifest::compute_file_hash(file_path).unwrap_or_else(|_| "sha256:unknown".to_string());

    // symbols.db: delete → insert パターン
    symbol_store
        .delete_by_file(rel_path)
        .map_err(|e| IndexFileResult::Error(e.into()))?;

    let symbols: Vec<_> = result
        .symbols
        .iter()
        .map(|s| convert_symbol(s, rel_path, &hash))
        .collect();

    if let Err(e) = symbol_store.insert_symbols(&symbols) {
        eprintln!("Warning: symbol store insert failed for {rel_path}: {e}");
        return Err(IndexFileResult::Skipped);
    }

    // 2パス方式: parent_symbol_id を解決
    if let Err(e) = resolve_parent_symbols(symbol_store, rel_path, &result.symbols) {
        eprintln!("Warning: parent symbol resolution failed for {rel_path}: {e}");
    }

    // import/依存関係を symbols.db に格納
    let deps: Vec<_> = result
        .imports
        .iter()
        .map(|imp| crate::indexer::symbol_store::ImportInfo {
            id: None,
            source_file: rel_path.to_string(),
            target_module: imp.source.clone(),
            imported_names: if imp.imported_names.is_empty() {
                None
            } else {
                Some(imp.imported_names.join(", "))
            },
            file_hash: hash.clone(),
        })
        .collect();

    if !deps.is_empty()
        && let Err(e) = symbol_store.insert_dependencies(&deps)
    {
        eprintln!("Warning: dependency insert failed for {rel_path}: {e}");
    }

    // tantivy: heading=ファイル名, body=全文
    let filename = file_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    if let Err(e) = writer.add_section(&SectionDoc {
        path: rel_path.to_string(),
        heading: filename,
        body: content,
        tags: String::new(),
        heading_level: CODE_FILE_HEADING_LEVEL,
        line_start: 1,
    }) {
        // tantivy add_section 失敗時は symbols.db をロールバック
        let _ = symbol_store.delete_by_file(rel_path);
        return Err(IndexFileResult::Error(e.into()));
    }

    let last_modified = std::fs::metadata(file_path)
        .and_then(|m| m.modified())
        .map(DateTime::<Utc>::from)
        .unwrap_or_else(|_| Utc::now());

    manifest.upsert_entry(FileEntry {
        path: rel_path.to_string(),
        hash,
        last_modified,
        sections: 1,
        file_type,
    });

    Ok(1)
}

/// ファイル種別に応じてインデックス処理をディスパッチする
fn index_file_and_upsert(
    file_path: &Path,
    rel_path: &str,
    writer: &mut IndexWriterWrapper,
    manifest: &mut Manifest,
    symbol_store: Option<&SymbolStore>,
) -> Result<u64, IndexFileResult> {
    let ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");
    match FileType::from_extension(ext) {
        Some(FileType::Markdown) => {
            index_markdown_file(file_path, rel_path, writer, manifest, symbol_store)
        }
        Some(ft) if ft.is_code() => {
            let store = symbol_store.ok_or(IndexFileResult::Skipped)?;
            index_code_file(file_path, rel_path, writer, manifest, store, ft)
        }
        _ => Ok(0),
    }
}

pub fn run_incremental(
    path: &Path,
    options: &IndexOptions,
) -> Result<IncrementalSummary, IndexError> {
    let start = Instant::now();

    // 1. Validate target directory
    if !path.is_dir() {
        return Err(IndexError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Directory not found: {}", path.display()),
        )));
    }

    // 2. Check preconditions: existing index with valid schema
    let commandindex_dir = crate::indexer::commandindex_dir(path);
    if !IndexState::exists(&commandindex_dir) {
        return Err(IndexError::IndexNotFound);
    }
    let mut state = match IndexState::load(&commandindex_dir) {
        Ok(s) => s,
        Err(e) => return Err(IndexError::IndexCorrupted(e.to_string())),
    };
    if state.check_schema_version().is_err() {
        return Err(IndexError::SchemaVersionMismatch);
    }

    // 3. Load ignore filter (from_file returns default if file not found)
    let ignore_filter = IgnoreFilter::from_file(&path.join(".cmindexignore"))?;

    // 4. Scan files
    let scan_result = scan_files(path, &ignore_filter, FileType::all_extensions())?;

    // 5. Load old manifest
    let mut old_manifest = Manifest::load_or_default(&commandindex_dir)?;

    // 6. Detect changes
    let diff_result = detect_changes(path, &old_manifest, &scan_result.files)?;

    // 7. Early return if no changes
    if diff_result.is_empty() {
        return Ok(IncrementalSummary {
            added_files: 0,
            added_sections: 0,
            modified_files: 0,
            modified_sections: 0,
            deleted_files: 0,
            unchanged: diff_result.unchanged as u64,
            skipped: 0,
            duration: start.elapsed(),
        });
    }

    // 8. Open existing tantivy index
    let tantivy_dir = crate::indexer::index_dir(path);
    let mut writer = match IndexWriterWrapper::open_existing(&tantivy_dir) {
        Ok(w) => w,
        Err(e) => {
            return Err(IndexError::IndexCorrupted(format!(
                "Failed to open tantivy index: {e}"
            )));
        }
    };

    // 9. Open SymbolStore (create tables if not yet created)
    let db_path = crate::indexer::symbol_db_path(path);
    let symbol_store = SymbolStore::open(&db_path)?;
    symbol_store.create_tables()?;

    let mut added_files: u64 = 0;
    let mut added_sections: u64 = 0;
    let mut modified_files: u64 = 0;
    let mut modified_sections: u64 = 0;
    let mut deleted_files: u64 = 0;
    let mut skipped: u64 = 0;

    // Track old sections for state update
    let mut old_deleted_sections: u64 = 0;
    let mut old_modified_sections: u64 = 0;

    // 10. Process deleted files
    for del_path in &diff_result.deleted {
        let path_str = del_path.to_string_lossy().to_string();
        writer.delete_by_path(&path_str)?;

        // Track old sections count
        if let Some(entry) = old_manifest.find_by_path(&path_str) {
            old_deleted_sections += entry.sections;
        }
        // Delete symbols/dependencies/file_links for all file types
        symbol_store.delete_by_file(&path_str)?;

        old_manifest.remove_by_path(&path_str);
        deleted_files += 1;
    }

    // 11. Process modified files
    for mod_path in &diff_result.modified {
        let rel_path = to_relative_path_string(mod_path, path);

        // Track old sections count
        if let Some(entry) = old_manifest.find_by_path(&rel_path) {
            old_modified_sections += entry.sections;
        }

        writer.delete_by_path(&rel_path)?;

        match index_file_and_upsert(
            mod_path,
            &rel_path,
            &mut writer,
            &mut old_manifest,
            Some(&symbol_store),
        ) {
            Ok(section_count) => {
                modified_files += 1;
                modified_sections += section_count;
            }
            Err(IndexFileResult::Skipped) => {
                skipped += 1;
            }
            Err(IndexFileResult::Error(e)) => return Err(e),
        }
    }

    // 12. Process added files
    for add_path in &diff_result.added {
        let rel_path = to_relative_path_string(add_path, path);

        match index_file_and_upsert(
            add_path,
            &rel_path,
            &mut writer,
            &mut old_manifest,
            Some(&symbol_store),
        ) {
            Ok(section_count) => {
                added_files += 1;
                added_sections += section_count;
            }
            Err(IndexFileResult::Skipped) => {
                skipped += 1;
            }
            Err(IndexFileResult::Error(e)) => return Err(e),
        }
    }

    // 13. Commit index
    writer.commit()?;

    // 14. Save updated manifest
    old_manifest.save(&commandindex_dir)?;

    // 15. Update state (use saturating arithmetic to prevent underflow on corrupted state)
    state.total_files = state.total_files.saturating_add(added_files);
    if deleted_files > state.total_files {
        eprintln!(
            "Warning: state inconsistency detected (total_files underflow). Consider running `commandindex clean` and `commandindex index`."
        );
    }
    state.total_files = state.total_files.saturating_sub(deleted_files);
    state.total_sections = state
        .total_sections
        .saturating_add(added_sections + modified_sections);
    let sections_to_remove = old_deleted_sections + old_modified_sections;
    if sections_to_remove > state.total_sections {
        eprintln!(
            "Warning: state inconsistency detected (total_sections underflow). Consider running `commandindex clean` and `commandindex index`."
        );
    }
    state.total_sections = state.total_sections.saturating_sub(sections_to_remove);
    state.touch();
    state.save(&commandindex_dir)?;

    // 16. Generate embeddings if requested
    if options.with_embedding {
        let updated_manifest = Manifest::load(&commandindex_dir)?;
        generate_embeddings_for_manifest(path, &commandindex_dir, &updated_manifest)?;
    }

    // 17. Return summary
    Ok(IncrementalSummary {
        added_files,
        added_sections,
        modified_files,
        modified_sections,
        deleted_files,
        unchanged: diff_result.unchanged as u64,
        skipped,
        duration: start.elapsed(),
    })
}

/// Embedding生成の共通ロジック（run / run_incremental から呼ばれる）
fn generate_embeddings_for_manifest(
    path: &Path,
    commandindex_dir: &Path,
    manifest: &Manifest,
) -> Result<(), IndexError> {
    let config = Config::load(commandindex_dir)?;
    let embedding_config = config.and_then(|c| c.embedding).unwrap_or_default();
    let provider = create_provider(&embedding_config)?;

    let db_path = crate::indexer::embeddings_db_path(path);
    let store = EmbeddingStore::open(&db_path)?;
    store.create_tables()?;

    let tantivy_dir = crate::indexer::index_dir(path);
    let reader = IndexReaderWrapper::open(&tantivy_dir).map_err(|e| {
        IndexError::IndexCorrupted(format!("Failed to open tantivy for embedding: {e}"))
    })?;

    for entry in &manifest.files {
        if store.has_current_embedding(&entry.path, &entry.hash)? {
            continue;
        }

        let sections = reader.search_by_exact_path(&entry.path).map_err(|e| {
            IndexError::IndexCorrupted(format!("Failed to read sections for embedding: {e}"))
        })?;
        if sections.is_empty() {
            continue;
        }

        let texts: Vec<String> = sections
            .iter()
            .map(|s| {
                if s.heading.is_empty() {
                    s.body.clone()
                } else {
                    format!("{}\n{}", s.heading, s.body)
                }
            })
            .collect();

        match provider.embed(&texts) {
            Ok(embeddings) => {
                let dimension = provider.dimension();
                let model = provider.model_name();
                for (section, embedding) in sections.iter().zip(embeddings.iter()) {
                    if let Err(e) = store.upsert_embedding(
                        &entry.path,
                        &section.heading,
                        embedding,
                        dimension,
                        model,
                        &entry.hash,
                    ) {
                        eprintln!(
                            "Warning: failed to store embedding for {}#{}: {e}",
                            entry.path, section.heading
                        );
                    }
                }
            }
            Err(e) => {
                eprintln!(
                    "Warning: embedding generation failed for {}: {e}",
                    entry.path
                );
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::code::{SymbolInfo as CodeSymbolInfo, SymbolKind};
    use crate::parser::link::{Link, LinkType};
    use std::path::PathBuf;

    #[test]
    fn test_convert_symbol_normal() {
        let src = CodeSymbolInfo {
            name: "my_func".to_string(),
            kind: SymbolKind::Function,
            file_path: PathBuf::from("src/main.ts"),
            line_start: 10,
            line_end: 20,
            parent: None,
            is_exported: true,
        };
        let result = convert_symbol(&src, "src/main.ts", "sha256:abc");
        assert_eq!(result.name, "my_func");
        assert_eq!(result.kind, "Function");
        assert_eq!(result.file_path, "src/main.ts");
        assert_eq!(result.line_start, 10);
        assert_eq!(result.line_end, 20);
        assert!(result.parent_symbol_id.is_none());
        assert!(result.id.is_none());
        assert_eq!(result.file_hash, "sha256:abc");
    }

    #[test]
    fn test_convert_symbol_large_line_numbers() {
        let src = CodeSymbolInfo {
            name: "big".to_string(),
            kind: SymbolKind::Class,
            file_path: PathBuf::from("big.py"),
            line_start: usize::MAX,
            line_end: usize::MAX,
            parent: None,
            is_exported: false,
        };
        let result = convert_symbol(&src, "big.py", "sha256:xyz");
        assert_eq!(result.line_start, u32::MAX);
        assert_eq!(result.line_end, u32::MAX);
    }

    #[test]
    fn test_code_file_heading_level_is_zero() {
        assert_eq!(CODE_FILE_HEADING_LEVEL, 0);
    }

    #[test]
    fn test_index_error_display_code_parse() {
        let err = IndexError::CodeParse(CodeParseError::UnsupportedLanguage("rs".to_string()));
        let msg = format!("{err}");
        assert!(msg.contains("Code parse error"));
    }

    #[test]
    fn test_index_error_display_symbol_store() {
        let err = IndexError::SymbolStore(SymbolStoreError::SchemaVersionMismatch {
            expected: 1,
            found: 2,
        });
        let msg = format!("{err}");
        assert!(msg.contains("Symbol store error"));
    }

    #[test]
    fn test_index_error_from_code_parse_error() {
        let e = CodeParseError::UnsupportedLanguage("go".to_string());
        let ie: IndexError = e.into();
        assert!(matches!(ie, IndexError::CodeParse(_)));
    }

    #[test]
    fn test_index_error_from_symbol_store_error() {
        let e = SymbolStoreError::Sqlite(rusqlite::Error::QueryReturnedNoRows);
        let ie: IndexError = e.into();
        assert!(matches!(ie, IndexError::SymbolStore(_)));
    }

    #[test]
    fn test_index_error_from_symbol_store_schema_version_mismatch() {
        let e = SymbolStoreError::SchemaVersionMismatch {
            expected: 1,
            found: 99,
        };
        let ie: IndexError = e.into();
        assert!(matches!(ie, IndexError::SchemaVersionMismatch));
    }

    #[test]
    fn test_is_indexable_link_local_file() {
        let link = Link {
            target: "docs/other.md".to_string(),
            link_type: LinkType::MarkdownLink,
        };
        assert!(is_indexable_link(&link));
    }

    #[test]
    fn test_is_indexable_link_wikilink() {
        let link = Link {
            target: "other-note".to_string(),
            link_type: LinkType::WikiLink,
        };
        assert!(is_indexable_link(&link));
    }

    #[test]
    fn test_is_indexable_link_rejects_http() {
        let link = Link {
            target: "http://example.com".to_string(),
            link_type: LinkType::MarkdownLink,
        };
        assert!(!is_indexable_link(&link));
    }

    #[test]
    fn test_is_indexable_link_rejects_https() {
        let link = Link {
            target: "https://example.com/page".to_string(),
            link_type: LinkType::MarkdownLink,
        };
        assert!(!is_indexable_link(&link));
    }

    #[test]
    fn test_is_indexable_link_rejects_other_protocol() {
        let link = Link {
            target: "ftp://example.com/file".to_string(),
            link_type: LinkType::MarkdownLink,
        };
        assert!(!is_indexable_link(&link));
    }

    #[test]
    fn test_is_indexable_link_rejects_mailto() {
        let link = Link {
            target: "mailto:user@example.com".to_string(),
            link_type: LinkType::MarkdownLink,
        };
        assert!(!is_indexable_link(&link));
    }

    #[test]
    fn test_is_indexable_link_rejects_anchor_only() {
        let link = Link {
            target: "#section-heading".to_string(),
            link_type: LinkType::MarkdownLink,
        };
        assert!(!is_indexable_link(&link));
    }

    #[test]
    fn test_is_indexable_link_rejects_too_long() {
        let link = Link {
            target: "a".repeat(1025),
            link_type: LinkType::MarkdownLink,
        };
        assert!(!is_indexable_link(&link));
    }

    #[test]
    fn test_is_indexable_link_accepts_max_length() {
        let link = Link {
            target: "a".repeat(1024),
            link_type: LinkType::MarkdownLink,
        };
        assert!(is_indexable_link(&link));
    }

    #[test]
    fn test_is_indexable_link_accepts_relative_with_anchor() {
        // "docs/page.md#section" is a local file link with anchor - should be indexable
        let link = Link {
            target: "docs/page.md#section".to_string(),
            link_type: LinkType::MarkdownLink,
        };
        assert!(is_indexable_link(&link));
    }
}
