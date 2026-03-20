use commandindex::parser::code::{SymbolKind, parse_code_file};
use commandindex::parser::typescript::{parse_tsx_content, parse_typescript_content};
use std::io::Write;
use std::path::Path;
use tempfile::NamedTempFile;

#[test]
fn test_parse_simple_function() {
    let source = r#"
function greet(name: string): string {
    return `Hello, ${name}`;
}
"#;
    let result = parse_typescript_content(source, Path::new("test.ts")).unwrap();
    assert_eq!(result.symbols.len(), 1);
    assert_eq!(result.symbols[0].name, "greet");
    assert_eq!(result.symbols[0].kind, SymbolKind::Function);
    assert!(!result.symbols[0].is_exported);
}

#[test]
fn test_parse_exported_function() {
    let source = r#"
export function add(a: number, b: number): number {
    return a + b;
}
"#;
    let result = parse_typescript_content(source, Path::new("test.ts")).unwrap();
    assert_eq!(result.symbols.len(), 1);
    assert_eq!(result.symbols[0].name, "add");
    assert_eq!(result.symbols[0].kind, SymbolKind::Function);
    assert!(result.symbols[0].is_exported);
}

#[test]
fn test_parse_class_with_methods() {
    let source = r#"
class Calculator {
    add(a: number, b: number): number {
        return a + b;
    }

    subtract(a: number, b: number): number {
        return a - b;
    }
}
"#;
    let result = parse_typescript_content(source, Path::new("test.ts")).unwrap();

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
fn test_parse_exported_class() {
    let source = r#"
export class Logger {
    log(msg: string): void {
        console.log(msg);
    }
}
"#;
    let result = parse_typescript_content(source, Path::new("test.ts")).unwrap();

    let class = result.symbols.iter().find(|s| s.kind == SymbolKind::Class);
    assert!(class.is_some());
    assert!(class.unwrap().is_exported);
}

#[test]
fn test_parse_imports() {
    let source = r#"
import { readFile, writeFile } from 'fs';
import path from 'path';
"#;
    let result = parse_typescript_content(source, Path::new("test.ts")).unwrap();
    assert!(!result.imports.is_empty());

    let fs_import = result.imports.iter().find(|i| i.source == "fs");
    assert!(fs_import.is_some());
    let fs_import = fs_import.unwrap();
    assert!(fs_import.imported_names.contains(&"readFile".to_string()));
    assert!(fs_import.imported_names.contains(&"writeFile".to_string()));
}

#[test]
fn test_parse_arrow_function() {
    let source = r#"
const double = (x: number): number => x * 2;
"#;
    let result = parse_typescript_content(source, Path::new("test.ts")).unwrap();
    assert_eq!(result.symbols.len(), 1);
    assert_eq!(result.symbols[0].name, "double");
    assert_eq!(result.symbols[0].kind, SymbolKind::Function);
}

#[test]
fn test_parse_exported_arrow_function() {
    let source = r#"
export const multiply = (a: number, b: number): number => a * b;
"#;
    let result = parse_typescript_content(source, Path::new("test.ts")).unwrap();
    assert_eq!(result.symbols.len(), 1);
    assert_eq!(result.symbols[0].name, "multiply");
    assert!(result.symbols[0].is_exported);
}

#[test]
fn test_parse_empty_file() {
    let source = "";
    let result = parse_typescript_content(source, Path::new("test.ts")).unwrap();
    assert!(result.symbols.is_empty());
    assert!(result.imports.is_empty());
}

#[test]
fn test_parse_syntax_error_file() {
    let source = r#"
function {{{ broken syntax
"#;
    // tree-sitter is error-tolerant, so this should still return a result
    let result = parse_typescript_content(source, Path::new("test.ts"));
    assert!(result.is_ok());
}

#[test]
fn test_line_numbers_are_1_indexed() {
    let source = r#"function first() {}
function second() {}
function third() {}"#;
    let result = parse_typescript_content(source, Path::new("test.ts")).unwrap();
    assert_eq!(result.symbols[0].name, "first");
    assert_eq!(result.symbols[0].line_start, 1);
    assert_eq!(result.symbols[1].name, "second");
    assert_eq!(result.symbols[1].line_start, 2);
    assert_eq!(result.symbols[2].name, "third");
    assert_eq!(result.symbols[2].line_start, 3);
}

#[test]
fn test_file_path_is_preserved() {
    let source = "function test() {}";
    let path = Path::new("src/utils/helper.ts");
    let result = parse_typescript_content(source, path).unwrap();
    assert_eq!(result.file_path, path);
    assert_eq!(result.symbols[0].file_path, path);
}

#[test]
fn test_tsx_parser() {
    let source = r#"
import React from 'react';

export function App(): JSX.Element {
    return <div>Hello</div>;
}
"#;
    let result = parse_tsx_content(source, Path::new("app.tsx")).unwrap();
    let func = result.symbols.iter().find(|s| s.name == "App");
    assert!(func.is_some());
    assert_eq!(func.unwrap().kind, SymbolKind::Function);
    assert!(func.unwrap().is_exported);
}

#[test]
fn test_multiple_imports() {
    let source = r#"
import { useState, useEffect } from 'react';
import axios from 'axios';
import { Config } from './config';
"#;
    let result = parse_typescript_content(source, Path::new("test.ts")).unwrap();
    assert_eq!(result.imports.len(), 3);
}

#[test]
fn test_nested_function_inside_function() {
    let source = r#"
function outer() {
    function inner() {
        return 42;
    }
    return inner();
}
"#;
    let result = parse_typescript_content(source, Path::new("test.ts")).unwrap();
    let functions: Vec<_> = result
        .symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Function)
        .collect();
    // Both outer and inner should be extracted
    assert!(functions.len() >= 2);
    let names: Vec<&str> = functions.iter().map(|f| f.name.as_str()).collect();
    assert!(names.contains(&"outer"));
    assert!(names.contains(&"inner"));
}

#[test]
fn test_parse_code_file_ts_dispatch() {
    let mut file = NamedTempFile::with_suffix(".ts").unwrap();
    writeln!(file, "export function hello(): string {{ return 'hi'; }}").unwrap();
    let result = parse_code_file(file.path()).unwrap();
    assert_eq!(result.symbols.len(), 1);
    assert_eq!(result.symbols[0].name, "hello");
    assert!(result.symbols[0].is_exported);
}

#[test]
fn test_parse_code_file_tsx_dispatch() {
    let mut file = NamedTempFile::with_suffix(".tsx").unwrap();
    writeln!(
        file,
        "import React from 'react';\nexport function App() {{ return <div />; }}"
    )
    .unwrap();
    let result = parse_code_file(file.path()).unwrap();
    let func = result.symbols.iter().find(|s| s.name == "App");
    assert!(func.is_some());
    assert!(func.unwrap().is_exported);
}

#[test]
fn test_parse_code_file_unsupported_extension() {
    let mut file = NamedTempFile::with_suffix(".rb").unwrap();
    writeln!(file, "def hello; end").unwrap();
    let result = parse_code_file(file.path());
    assert!(result.is_err());
}
