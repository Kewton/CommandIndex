use std::fmt;
use std::path::{Path, PathBuf};

const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024; // 10MB

/// Maximum AST traversal depth to prevent stack overflow on deeply nested code.
pub const MAX_DEPTH: usize = 256;

#[derive(Debug, Clone, PartialEq)]
pub enum SymbolKind {
    Function,
    Class,
    Method,
}

impl fmt::Display for SymbolKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SymbolKind::Function => write!(f, "Function"),
            SymbolKind::Class => write!(f, "Class"),
            SymbolKind::Method => write!(f, "Method"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SymbolInfo {
    pub name: String,
    pub kind: SymbolKind,
    pub file_path: PathBuf,
    pub line_start: usize,
    pub line_end: usize,
    pub parent: Option<String>,
    pub is_exported: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportInfo {
    pub source: String,
    pub imported_names: Vec<String>,
    pub file_path: PathBuf,
}

#[derive(Debug)]
pub struct CodeParseResult {
    pub file_path: PathBuf,
    pub symbols: Vec<SymbolInfo>,
    pub imports: Vec<ImportInfo>,
}

#[derive(Debug)]
pub enum CodeParseError {
    Io(std::io::Error),
    TreeSitter(String),
    UnsupportedLanguage(String),
    FileTooLarge { path: PathBuf, size: u64 },
}

impl fmt::Display for CodeParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CodeParseError::Io(e) => write!(f, "IO error: {e}"),
            CodeParseError::TreeSitter(msg) => write!(f, "Tree-sitter error: {msg}"),
            CodeParseError::UnsupportedLanguage(lang) => {
                write!(f, "Unsupported language: {lang}")
            }
            CodeParseError::FileTooLarge { path, size } => {
                write!(
                    f,
                    "File too large: {} ({size} bytes, max {MAX_FILE_SIZE})",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for CodeParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CodeParseError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for CodeParseError {
    fn from(e: std::io::Error) -> Self {
        CodeParseError::Io(e)
    }
}

/// Parse content with tree-sitter using the given language.
pub fn parse_with_tree_sitter(
    content: &str,
    language: &tree_sitter::Language,
) -> Result<tree_sitter::Tree, CodeParseError> {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(language)
        .map_err(|e| CodeParseError::TreeSitter(format!("Failed to set language: {e}")))?;
    parser
        .parse(content, None)
        .ok_or_else(|| CodeParseError::TreeSitter("Failed to parse content".to_string()))
}

/// Dispatch to the appropriate parser based on file extension (with file size check).
pub fn parse_code_file(path: &Path) -> Result<CodeParseResult, CodeParseError> {
    // Check file size
    let metadata = std::fs::metadata(path)?;
    let size = metadata.len();
    if size > MAX_FILE_SIZE {
        return Err(CodeParseError::FileTooLarge {
            path: path.to_path_buf(),
            size,
        });
    }

    let content = std::fs::read_to_string(path)?;

    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    match ext {
        "ts" => super::typescript::parse_typescript_content(&content, path),
        "tsx" => super::typescript::parse_tsx_content(&content, path),
        "py" => super::python::parse_python_content(&content, path),
        other => Err(CodeParseError::UnsupportedLanguage(other.to_string())),
    }
}

/// Extract the text of a named child field from a tree-sitter node.
pub fn get_child_text(node: &tree_sitter::Node, field_name: &str, src: &[u8]) -> Option<String> {
    node.child_by_field_name(field_name)
        .and_then(|n| n.utf8_text(src).ok())
        .map(|s| s.to_string())
}
