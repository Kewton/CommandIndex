use std::path::Path;

use crate::parser::code::{
    CodeParseError, CodeParseResult, ImportInfo, MAX_DEPTH, SymbolInfo, SymbolKind, get_child_text,
    parse_with_tree_sitter,
};

/// Parse TypeScript source code and extract symbols and imports.
pub fn parse_typescript_content(
    source: &str,
    file_path: &Path,
) -> Result<CodeParseResult, CodeParseError> {
    let language: tree_sitter::Language = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
    parse_ts_like(source, file_path, &language)
}

/// Parse TSX source code and extract symbols and imports.
pub fn parse_tsx_content(
    source: &str,
    file_path: &Path,
) -> Result<CodeParseResult, CodeParseError> {
    let language: tree_sitter::Language = tree_sitter_typescript::LANGUAGE_TSX.into();
    parse_ts_like(source, file_path, &language)
}

struct TsWalkContext<'a> {
    src: &'a [u8],
    file_path: &'a Path,
    symbols: Vec<SymbolInfo>,
    imports: Vec<ImportInfo>,
}

fn parse_ts_like(
    source: &str,
    file_path: &Path,
    language: &tree_sitter::Language,
) -> Result<CodeParseResult, CodeParseError> {
    let tree = parse_with_tree_sitter(source, language)?;
    let root = tree.root_node();

    let mut ctx = TsWalkContext {
        src: source.as_bytes(),
        file_path,
        symbols: Vec::new(),
        imports: Vec::new(),
    };

    walk_ts_node(&mut ctx, root, false, None, 0);

    Ok(CodeParseResult {
        file_path: file_path.to_path_buf(),
        symbols: ctx.symbols,
        imports: ctx.imports,
    })
}

fn walk_ts_node(
    ctx: &mut TsWalkContext<'_>,
    node: tree_sitter::Node,
    is_exported: bool,
    parent_class: Option<&str>,
    depth: usize,
) {
    if depth > MAX_DEPTH {
        return;
    }

    let kind = node.kind();

    match kind {
        "export_statement" => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                walk_ts_node(ctx, child, true, parent_class, depth + 1);
            }
            return;
        }
        "function_declaration" => {
            if let Some(name) = get_child_text(&node, "name", ctx.src) {
                ctx.symbols.push(SymbolInfo {
                    name,
                    kind: SymbolKind::Function,
                    file_path: ctx.file_path.to_path_buf(),
                    line_start: node.start_position().row + 1,
                    line_end: node.end_position().row + 1,
                    parent: parent_class.map(String::from),
                    is_exported,
                });
            }
        }
        "class_declaration" => {
            let class_name = get_child_text(&node, "name", ctx.src);
            if let Some(ref name) = class_name {
                ctx.symbols.push(SymbolInfo {
                    name: name.clone(),
                    kind: SymbolKind::Class,
                    file_path: ctx.file_path.to_path_buf(),
                    line_start: node.start_position().row + 1,
                    line_end: node.end_position().row + 1,
                    parent: parent_class.map(String::from),
                    is_exported,
                });
            }
            let cname = class_name.as_deref();
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                walk_ts_node(ctx, child, false, cname, depth + 1);
            }
            return;
        }
        "method_definition" => {
            if let Some(name) = get_child_text(&node, "name", ctx.src) {
                ctx.symbols.push(SymbolInfo {
                    name,
                    kind: SymbolKind::Method,
                    file_path: ctx.file_path.to_path_buf(),
                    line_start: node.start_position().row + 1,
                    line_end: node.end_position().row + 1,
                    parent: parent_class.map(String::from),
                    is_exported: false,
                });
            }
        }
        "lexical_declaration" => {
            extract_arrow_functions(ctx, &node, is_exported, parent_class);
            return;
        }
        "import_statement" => {
            extract_ts_import(ctx, &node);
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_ts_node(ctx, child, is_exported, parent_class, depth + 1);
    }
}

fn extract_arrow_functions(
    ctx: &mut TsWalkContext<'_>,
    node: &tree_sitter::Node,
    is_exported: bool,
    parent_class: Option<&str>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "variable_declarator" {
            let name = get_child_text(&child, "name", ctx.src);
            let has_arrow = child_has_kind(&child, "arrow_function");
            if let (Some(name), true) = (name, has_arrow) {
                ctx.symbols.push(SymbolInfo {
                    name,
                    kind: SymbolKind::Function,
                    file_path: ctx.file_path.to_path_buf(),
                    line_start: child.start_position().row + 1,
                    line_end: child.end_position().row + 1,
                    parent: parent_class.map(String::from),
                    is_exported,
                });
            }
        }
    }
}

fn child_has_kind(node: &tree_sitter::Node, target_kind: &str) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == target_kind {
            return true;
        }
        let mut inner_cursor = child.walk();
        for inner in child.children(&mut inner_cursor) {
            if inner.kind() == target_kind {
                return true;
            }
        }
    }
    false
}

fn extract_ts_import(ctx: &mut TsWalkContext<'_>, node: &tree_sitter::Node) {
    let mut source_module = String::new();
    let mut imported_names = Vec::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "string" => {
                if let Ok(text) = child.utf8_text(ctx.src) {
                    source_module = text.trim_matches(|c| c == '\'' || c == '"').to_string();
                }
            }
            "import_clause" => {
                collect_import_names(&child, ctx.src, &mut imported_names);
            }
            _ => {}
        }
    }

    if !source_module.is_empty() {
        ctx.imports.push(ImportInfo {
            source: source_module,
            imported_names,
            file_path: ctx.file_path.to_path_buf(),
        });
    }
}

fn collect_import_names(node: &tree_sitter::Node, src: &[u8], names: &mut Vec<String>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "identifier" => {
                if let Ok(text) = child.utf8_text(src) {
                    names.push(text.to_string());
                }
            }
            "named_imports" => {
                let mut inner_cursor = child.walk();
                for inner in child.children(&mut inner_cursor) {
                    if inner.kind() == "import_specifier"
                        && let Some(name_node) = inner.child_by_field_name("name")
                        && let Ok(text) = name_node.utf8_text(src)
                    {
                        names.push(text.to_string());
                    }
                }
            }
            _ => {
                collect_import_names(&child, src, names);
            }
        }
    }
}
