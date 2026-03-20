use commandindex::parser::code::{SymbolKind, parse_code_file};
use commandindex::parser::python::parse_python_content;
use std::io::Write;
use std::path::Path;
use tempfile::NamedTempFile;

#[test]
fn test_parse_simple_function() {
    let source = r#"
def greet(name):
    return f"Hello, {name}"
"#;
    let result = parse_python_content(source, Path::new("test.py")).unwrap();
    assert_eq!(result.symbols.len(), 1);
    assert_eq!(result.symbols[0].name, "greet");
    assert_eq!(result.symbols[0].kind, SymbolKind::Function);
    assert!(!result.symbols[0].is_exported);
}

#[test]
fn test_parse_class_with_methods() {
    let source = r#"
class Calculator:
    def add(self, a, b):
        return a + b

    def subtract(self, a, b):
        return a - b
"#;
    let result = parse_python_content(source, Path::new("test.py")).unwrap();

    let class = result.symbols.iter().find(|s| s.kind == SymbolKind::Class);
    assert!(class.is_some());
    assert_eq!(class.unwrap().name, "Calculator");

    let methods: Vec<_> = result
        .symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Method)
        .collect();
    assert_eq!(methods.len(), 2);
    assert_eq!(methods[0].name, "add");
    assert_eq!(methods[0].parent.as_deref(), Some("Calculator"));
    assert_eq!(methods[1].name, "subtract");
    assert_eq!(methods[1].parent.as_deref(), Some("Calculator"));
}

#[test]
fn test_parse_import_statement() {
    let source = r#"
import os
import sys
"#;
    let result = parse_python_content(source, Path::new("test.py")).unwrap();
    assert_eq!(result.imports.len(), 2);
    assert_eq!(result.imports[0].source, "os");
    assert_eq!(result.imports[1].source, "sys");
}

#[test]
fn test_parse_from_import_statement() {
    let source = r#"
from pathlib import Path, PurePath
from os import getcwd
"#;
    let result = parse_python_content(source, Path::new("test.py")).unwrap();
    assert_eq!(result.imports.len(), 2);

    let pathlib_import = result.imports.iter().find(|i| i.source == "pathlib");
    assert!(pathlib_import.is_some());
    let pathlib_import = pathlib_import.unwrap();
    assert!(pathlib_import.imported_names.contains(&"Path".to_string()));
    assert!(
        pathlib_import
            .imported_names
            .contains(&"PurePath".to_string())
    );
}

#[test]
fn test_parse_nested_class() {
    let source = r#"
class Outer:
    class Inner:
        def method(self):
            pass
"#;
    let result = parse_python_content(source, Path::new("test.py")).unwrap();

    let classes: Vec<_> = result
        .symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Class)
        .collect();
    assert!(classes.len() >= 2);
}

#[test]
fn test_parse_empty_file() {
    let source = "";
    let result = parse_python_content(source, Path::new("test.py")).unwrap();
    assert!(result.symbols.is_empty());
    assert!(result.imports.is_empty());
}

#[test]
fn test_parse_syntax_error() {
    let source = r#"
def broken(::
    pass pass pass
"#;
    let result = parse_python_content(source, Path::new("test.py"));
    assert!(result.is_ok()); // tree-sitter is error-tolerant
}

#[test]
fn test_line_numbers_1_indexed() {
    let source = "def first():\n    pass\ndef second():\n    pass";
    let result = parse_python_content(source, Path::new("test.py")).unwrap();
    assert_eq!(result.symbols[0].name, "first");
    assert_eq!(result.symbols[0].line_start, 1);
    assert_eq!(result.symbols[1].name, "second");
    assert_eq!(result.symbols[1].line_start, 3);
}

#[test]
fn test_file_path_preserved() {
    let source = "def test():\n    pass";
    let path = Path::new("src/utils/helper.py");
    let result = parse_python_content(source, path).unwrap();
    assert_eq!(result.file_path, path);
    assert_eq!(result.symbols[0].file_path, path);
}

#[test]
fn test_multiple_top_level_functions() {
    let source = r#"
def func_a():
    pass

def func_b():
    pass

def func_c():
    pass
"#;
    let result = parse_python_content(source, Path::new("test.py")).unwrap();
    let functions: Vec<_> = result
        .symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Function)
        .collect();
    assert_eq!(functions.len(), 3);
}

#[test]
fn test_is_exported_always_false() {
    let source = r#"
def public_func():
    pass

class MyClass:
    def method(self):
        pass
"#;
    let result = parse_python_content(source, Path::new("test.py")).unwrap();
    for sym in &result.symbols {
        assert!(!sym.is_exported);
    }
}

#[test]
fn test_nested_function_inside_function() {
    let source = r#"
def outer():
    def inner():
        return 42
    return inner()
"#;
    let result = parse_python_content(source, Path::new("test.py")).unwrap();
    // Python parser returns early on function_definition, so inner is not traversed.
    // At minimum, outer should be detected.
    let functions: Vec<_> = result
        .symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Function)
        .collect();
    assert!(!functions.is_empty());
    let names: Vec<&str> = functions.iter().map(|f| f.name.as_str()).collect();
    assert!(names.contains(&"outer"));
}

#[test]
fn test_parse_code_file_py_dispatch() {
    let mut file = NamedTempFile::with_suffix(".py").unwrap();
    writeln!(file, "def greet():\n    pass").unwrap();
    let result = parse_code_file(file.path()).unwrap();
    assert_eq!(result.symbols.len(), 1);
    assert_eq!(result.symbols[0].name, "greet");
    assert_eq!(result.symbols[0].kind, SymbolKind::Function);
}
