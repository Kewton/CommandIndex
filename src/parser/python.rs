use std::path::Path;

use crate::parser::code::{
    CodeParseError, CodeParseResult, ImportInfo, MAX_DEPTH, SymbolInfo, SymbolKind, get_child_text,
    parse_with_tree_sitter,
};

/// Parse Python source code and extract symbols and imports.
pub fn parse_python_content(
    source: &str,
    file_path: &Path,
) -> Result<CodeParseResult, CodeParseError> {
    let language: tree_sitter::Language = tree_sitter_python::LANGUAGE.into();
    let tree = parse_with_tree_sitter(source, &language)?;
    let root = tree.root_node();

    let mut ctx = PyWalkContext {
        src: source.as_bytes(),
        file_path,
        symbols: Vec::new(),
        imports: Vec::new(),
    };

    walk_py_node(&mut ctx, root, None, 0);

    Ok(CodeParseResult {
        file_path: file_path.to_path_buf(),
        symbols: ctx.symbols,
        imports: ctx.imports,
    })
}

struct PyWalkContext<'a> {
    src: &'a [u8],
    file_path: &'a Path,
    symbols: Vec<SymbolInfo>,
    imports: Vec<ImportInfo>,
}

fn walk_py_node(
    ctx: &mut PyWalkContext<'_>,
    node: tree_sitter::Node,
    parent_class: Option<&str>,
    depth: usize,
) {
    if depth > MAX_DEPTH {
        return;
    }

    let kind = node.kind();

    match kind {
        "function_definition" => {
            if let Some(name) = get_child_text(&node, "name", ctx.src) {
                let sym_kind = if parent_class.is_some() {
                    SymbolKind::Method
                } else {
                    SymbolKind::Function
                };
                ctx.symbols.push(SymbolInfo {
                    name,
                    kind: sym_kind,
                    file_path: ctx.file_path.to_path_buf(),
                    line_start: node.start_position().row + 1,
                    line_end: node.end_position().row + 1,
                    parent: parent_class.map(String::from),
                    is_exported: false,
                });
            }
            return;
        }
        "class_definition" => {
            let class_name = get_child_text(&node, "name", ctx.src);
            if let Some(ref name) = class_name {
                ctx.symbols.push(SymbolInfo {
                    name: name.clone(),
                    kind: SymbolKind::Class,
                    file_path: ctx.file_path.to_path_buf(),
                    line_start: node.start_position().row + 1,
                    line_end: node.end_position().row + 1,
                    parent: parent_class.map(String::from),
                    is_exported: false,
                });
            }
            let cname = class_name.as_deref();
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                walk_py_node(ctx, child, cname, depth + 1);
            }
            return;
        }
        "import_statement" => {
            extract_py_import(ctx, &node);
        }
        "import_from_statement" => {
            extract_py_from_import(ctx, &node);
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_py_node(ctx, child, parent_class, depth + 1);
    }
}

fn extract_py_import(ctx: &mut PyWalkContext<'_>, node: &tree_sitter::Node) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "dotted_name"
            && let Ok(text) = child.utf8_text(ctx.src)
        {
            ctx.imports.push(ImportInfo {
                source: text.to_string(),
                imported_names: vec![text.to_string()],
                file_path: ctx.file_path.to_path_buf(),
            });
        }
    }
}

fn extract_py_from_import(ctx: &mut PyWalkContext<'_>, node: &tree_sitter::Node) {
    let mut source_module = String::new();
    let mut imported_names = Vec::new();

    if let Some(module_node) = node.child_by_field_name("module_name")
        && let Ok(text) = module_node.utf8_text(ctx.src)
    {
        source_module = text.to_string();
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "import_prefix" {
            continue;
        }
        collect_py_import_names(&child, ctx.src, &mut imported_names);
    }

    if !source_module.is_empty() {
        ctx.imports.push(ImportInfo {
            source: source_module,
            imported_names,
            file_path: ctx.file_path.to_path_buf(),
        });
    }
}

fn collect_py_import_names(node: &tree_sitter::Node, src: &[u8], names: &mut Vec<String>) {
    match node.kind() {
        "dotted_name" => {
            if let Ok(text) = node.utf8_text(src) {
                names.push(text.to_string());
            }
        }
        "aliased_import" => {
            if let Some(name_node) = node.child_by_field_name("name")
                && let Ok(text) = name_node.utf8_text(src)
            {
                names.push(text.to_string());
            }
        }
        _ => {}
    }
}
