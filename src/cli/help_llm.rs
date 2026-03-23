use std::fmt;

use serde::Serialize;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum HelpLlmError {
    Serialize(String),
}

impl fmt::Display for HelpLlmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HelpLlmError::Serialize(msg) => {
                write!(f, "failed to generate help-llm output: {msg}")
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Output structures
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct HelpLlmOutput {
    pub schema_version: &'static str,
    pub tool: &'static str,
    pub version: String,
    pub description: &'static str,
    pub global_options: Vec<GlobalOption>,
    pub use_cases: Vec<UseCaseItem>,
    pub workflows: Vec<Workflow>,
    pub commands: Vec<CommandInfo>,
}

#[derive(Serialize)]
pub struct GlobalOption {
    pub name: &'static str,
    pub flag: &'static str,
    pub description: &'static str,
}

#[derive(Serialize)]
pub struct UseCaseItem {
    pub name: &'static str,
    pub command: &'static str,
}

#[derive(Serialize)]
pub struct Workflow {
    pub name: &'static str,
    pub description: &'static str,
    pub steps: Vec<&'static str>,
}

#[derive(Serialize)]
pub struct CommandInfo {
    pub name: &'static str,
    pub description: &'static str,
    pub when_to_use: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prerequisites: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modes: Option<Vec<SearchMode>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conflicts: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_options: Option<Vec<&'static str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_formats: Option<Vec<&'static str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pipe_support: Option<PipeSupport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subcommands: Option<Vec<&'static str>>,
    pub examples: Vec<&'static str>,
}

#[derive(Serialize)]
pub struct SearchMode {
    pub name: &'static str,
    pub description: &'static str,
    pub example: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_default: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prerequisites: Option<Vec<&'static str>>,
}

#[derive(Serialize)]
pub struct PipeSupport {
    pub stdin: &'static str,
}

// ---------------------------------------------------------------------------
// Build output
// ---------------------------------------------------------------------------

pub fn build_help_llm_output() -> HelpLlmOutput {
    HelpLlmOutput {
        schema_version: "1.0",
        tool: "commandindexdev",
        version: env!("CARGO_PKG_VERSION").to_string(),
        description: "Git-native knowledge CLI — search across Markdown, Code, and Git",
        global_options: vec![GlobalOption {
            name: "index-path",
            flag: "--index-path <DIR>",
            description: "Custom index directory path (overrides default .commandindex/)",
        }],
        use_cases: build_use_cases(),
        workflows: build_workflows(),
        commands: build_commands(),
    }
}

fn build_use_cases() -> Vec<UseCaseItem> {
    vec![
        UseCaseItem {
            name: "Full-text search",
            command: "commandindexdev search \"query\"",
        },
        UseCaseItem {
            name: "Semantic search",
            command: "commandindexdev search --semantic \"query\"",
        },
        UseCaseItem {
            name: "Symbol search",
            command: "commandindexdev search --symbol func_name",
        },
        UseCaseItem {
            name: "Related files",
            command: "commandindexdev search --related src/file.rs",
        },
        UseCaseItem {
            name: "Recent changes",
            command: "commandindexdev search --changed-since \"yesterday\"",
        },
        UseCaseItem {
            name: "Impact analysis",
            command: "commandindexdev impact src/file.rs --format json",
        },
        UseCaseItem {
            name: "File comparison",
            command: "commandindexdev diff file_a.rs file_b.rs --format json",
        },
        UseCaseItem {
            name: "AI context generation",
            command: "commandindexdev context src/main.rs --max-files 10",
        },
        UseCaseItem {
            name: "Build index",
            command: "commandindexdev index --path .",
        },
        UseCaseItem {
            name: "Incremental update",
            command: "commandindexdev update --path .",
        },
        UseCaseItem {
            name: "Check index status",
            command: "commandindexdev status --format json",
        },
        UseCaseItem {
            name: "Export/Import index",
            command: "commandindexdev export index.tar.gz",
        },
        UseCaseItem {
            name: "Watch for changes",
            command: "commandindexdev watch --path .",
        },
    ]
}

fn build_workflows() -> Vec<Workflow> {
    vec![
        Workflow {
            name: "Initial setup",
            description: "Build index for a repository",
            steps: vec![
                "commandindexdev index --path .",
                "commandindexdev embed --path . (optional, for semantic search)",
                "commandindexdev status --detail",
            ],
        },
        Workflow {
            name: "Investigation",
            description: "Investigate code structure and relationships",
            steps: vec![
                "commandindexdev search \"keyword\" --format json",
                "commandindexdev search --related src/target.rs --format json",
                "commandindexdev impact src/target.rs --format json",
                "commandindexdev context src/target.rs --max-files 20",
            ],
        },
        Workflow {
            name: "Maintenance",
            description: "Keep index up to date",
            steps: vec![
                "commandindexdev update --path .",
                "commandindexdev status --verify",
                "commandindexdev clean (if rebuild needed)",
            ],
        },
    ]
}

fn build_commands() -> Vec<CommandInfo> {
    vec![
        CommandInfo {
            name: "index",
            description: "Build full search index from repository",
            when_to_use: "First time setup or full rebuild of the search index",
            prerequisites: None,
            modes: None,
            conflicts: None,
            key_options: Some(vec![
                "--path <DIR>  Target directory to index (default: .)",
                "--with-embedding  Generate embeddings during indexing",
            ]),
            output_formats: None,
            output: Some("Index saved to .commandindex/"),
            input: None,
            pipe_support: None,
            subcommands: None,
            examples: vec![
                "commandindexdev index --path .",
                "commandindexdev index --path . --with-embedding",
            ],
        },
        CommandInfo {
            name: "search",
            description: "Search the index with multiple modes",
            when_to_use: "Find relevant documents, code, or symbols across your repository",
            prerequisites: Some("Requires index (run `index` first). Semantic search requires embeddings (run `embed` first).".to_string()),
            modes: Some(vec![
                SearchMode {
                    name: "full-text",
                    description: "Keyword search across all indexed content",
                    example: "commandindexdev search \"authentication\"",
                    is_default: Some(true),
                    prerequisites: None,
                },
                SearchMode {
                    name: "symbol",
                    description: "Search for code symbols (functions, structs, classes)",
                    example: "commandindexdev search --symbol parse_config",
                    is_default: None,
                    prerequisites: None,
                },
                SearchMode {
                    name: "related",
                    description: "Find files related to specified file(s)",
                    example: "commandindexdev search --related src/auth.rs",
                    is_default: None,
                    prerequisites: None,
                },
                SearchMode {
                    name: "semantic",
                    description: "Meaning-based search using embeddings",
                    example: "commandindexdev search --semantic \"login flow\"",
                    is_default: None,
                    prerequisites: Some(vec!["embeddings (run `embed` first)"]),
                },
                SearchMode {
                    name: "changed-since",
                    description: "Find content changed since a time expression",
                    example: "commandindexdev search --changed-since \"yesterday\"",
                    is_default: None,
                    prerequisites: None,
                },
            ]),
            conflicts: Some("Search modes are mutually exclusive"),
            key_options: Some(vec![
                "--format <FORMAT>  Output format: human, json, path",
                "--limit <N>  Maximum number of results",
                "--no-semantic  Disable hybrid search, use BM25 only",
                "--rerank  Enable LLM-based reranking",
                "--workspace <FILE>  Workspace config for cross-repo search",
            ]),
            output_formats: Some(vec!["human", "json", "path"]),
            output: None,
            input: None,
            pipe_support: None,
            subcommands: None,
            examples: vec![
                "commandindexdev search \"authentication\" --format json",
                "commandindexdev search --related src/auth.rs --format json",
                "commandindexdev search --semantic \"login flow\"",
                "commandindexdev search --changed-since \"yesterday\"",
                "commandindexdev search --symbol parse_config",
            ],
        },
        CommandInfo {
            name: "update",
            description: "Incrementally update index using Git-aware delta detection",
            when_to_use: "After code changes, to update the index without full rebuild",
            prerequisites: Some("Requires existing index (run `index` first)".to_string()),
            modes: None,
            conflicts: None,
            key_options: Some(vec![
                "--path <DIR>  Target directory (default: .)",
                "--with-embedding  Generate embeddings during update",
                "--workspace <FILE>  Workspace config for multi-repo update",
            ]),
            output_formats: None,
            output: Some("Summary of added/modified/deleted files"),
            input: None,
            pipe_support: None,
            subcommands: None,
            examples: vec![
                "commandindexdev update --path .",
                "commandindexdev update --with-embedding",
                "commandindexdev update --workspace workspace.toml",
            ],
        },
        CommandInfo {
            name: "status",
            description: "Show index status with optional detailed statistics",
            when_to_use: "Check if index exists, its health, coverage, or verify integrity",
            prerequisites: Some("Requires existing index".to_string()),
            modes: None,
            conflicts: None,
            key_options: Some(vec![
                "--path <DIR>  Target directory (default: .)",
                "--format <FORMAT>  Output format: human, json",
                "--detail  Show detailed statistics (coverage, staleness, storage)",
                "--coverage  Show coverage statistics only",
                "--verify  Verify index integrity",
                "--workspace <FILE>  Workspace config for multi-repo status",
            ]),
            output_formats: Some(vec!["human", "json"]),
            output: None,
            input: None,
            pipe_support: None,
            subcommands: None,
            examples: vec![
                "commandindexdev status --format json",
                "commandindexdev status --detail",
                "commandindexdev status --coverage",
                "commandindexdev status --verify",
            ],
        },
        CommandInfo {
            name: "clean",
            description: "Remove index and prepare for rebuild",
            when_to_use: "When index is corrupted or you want a fresh start",
            prerequisites: None,
            modes: None,
            conflicts: None,
            key_options: Some(vec![
                "--path <DIR>  Target directory (default: .)",
                "--keep-embeddings  Preserve embeddings database when cleaning",
            ]),
            output_formats: None,
            output: None,
            input: None,
            pipe_support: None,
            subcommands: None,
            examples: vec![
                "commandindexdev clean --path .",
                "commandindexdev clean --keep-embeddings",
            ],
        },
        CommandInfo {
            name: "diff",
            description: "Compare related files between two files for conflict/overlap detection",
            when_to_use: "Detect potential conflicts or overlapping concerns between two files",
            prerequisites: Some("Requires existing index".to_string()),
            modes: None,
            conflicts: None,
            key_options: Some(vec![
                "--format <FORMAT>  Output format: human, json",
                "--limit <N>  Maximum related files per input file",
            ]),
            output_formats: Some(vec!["human", "json"]),
            output: Some("JSON with overlapping and unique related files"),
            input: None,
            pipe_support: None,
            subcommands: None,
            examples: vec![
                "commandindexdev diff src/auth.rs src/session.rs --format json",
                "commandindexdev diff file_a.md file_b.md",
            ],
        },
        CommandInfo {
            name: "context",
            description: "Generate AI-oriented context pack for specified files",
            when_to_use: "Prepare context for LLM consumption (code review, refactoring, etc.)",
            prerequisites: Some("Requires existing index".to_string()),
            modes: None,
            conflicts: None,
            key_options: Some(vec![
                "--max-files <N>  Maximum number of related files to include",
                "--max-tokens <N>  Estimated token limit",
            ]),
            output_formats: None,
            output: Some("JSON context pack with file content and relationships"),
            input: None,
            pipe_support: None,
            subcommands: None,
            examples: vec![
                "commandindexdev context src/main.rs",
                "commandindexdev context src/auth.rs --max-files 10 --max-tokens 8000",
            ],
        },
        CommandInfo {
            name: "embed",
            description: "Generate embeddings for semantic search",
            when_to_use: "Enable semantic (meaning-based) search by generating vector embeddings",
            prerequisites: Some("Requires existing index and running Ollama instance".to_string()),
            modes: None,
            conflicts: None,
            key_options: Some(vec!["--path <DIR>  Target directory (default: .)"]),
            output_formats: None,
            output: Some("Summary of generated/cached/failed embeddings"),
            input: None,
            pipe_support: None,
            subcommands: None,
            examples: vec![
                "commandindexdev embed --path .",
            ],
        },
        CommandInfo {
            name: "config",
            description: "Show or manage configuration",
            when_to_use: "View current configuration or check config file locations",
            prerequisites: None,
            modes: None,
            conflicts: None,
            key_options: None,
            output_formats: None,
            output: None,
            input: None,
            pipe_support: None,
            subcommands: Some(vec!["show  Show current effective config (secrets masked)", "path  Show loaded config file paths"]),
            examples: vec![
                "commandindexdev config show",
                "commandindexdev config path",
            ],
        },
        CommandInfo {
            name: "export",
            description: "Export index as portable tar.gz archive",
            when_to_use: "Share or backup the index for use on another machine",
            prerequisites: Some("Requires existing index".to_string()),
            modes: None,
            conflicts: None,
            key_options: Some(vec![
                "--with-embeddings  Include embedding database in archive",
            ]),
            output_formats: None,
            output: Some("tar.gz archive file"),
            input: None,
            pipe_support: None,
            subcommands: None,
            examples: vec![
                "commandindexdev export index.tar.gz",
                "commandindexdev export index.tar.gz --with-embeddings",
            ],
        },
        CommandInfo {
            name: "impact",
            description: "Analyze impact of file changes",
            when_to_use: "Understand which files are affected by changes to specified files",
            prerequisites: Some("Requires existing index".to_string()),
            modes: None,
            conflicts: None,
            key_options: Some(vec![
                "--format <FORMAT>  Output format: human, json, path",
                "--limit <N>  Maximum number of impacted files to show",
            ]),
            output_formats: Some(vec!["human", "json", "path"]),
            output: Some("List of impacted files with relevance scores"),
            input: Some("File paths as arguments or from stdin (one per line)"),
            pipe_support: Some(PipeSupport {
                stdin: "Reads file paths from stdin when no arguments provided",
            }),
            subcommands: None,
            examples: vec![
                "commandindexdev impact src/auth.rs --format json",
                "echo src/auth.rs | commandindexdev impact --format json",
                "git diff --name-only | commandindexdev impact --format path",
            ],
        },
        CommandInfo {
            name: "import",
            description: "Import index from tar.gz archive",
            when_to_use: "Restore or use an index exported from another machine",
            prerequisites: None,
            modes: None,
            conflicts: None,
            key_options: Some(vec![
                "--force  Overwrite existing index",
            ]),
            output_formats: None,
            output: Some("Import summary with file count and git hash match status"),
            input: None,
            pipe_support: None,
            subcommands: None,
            examples: vec![
                "commandindexdev import index.tar.gz",
                "commandindexdev import index.tar.gz --force",
            ],
        },
        CommandInfo {
            name: "watch",
            description: "Watch for file changes and auto-update index",
            when_to_use: "Keep index automatically up to date during development (daemon mode)",
            prerequisites: Some("Requires existing index".to_string()),
            modes: None,
            conflicts: None,
            key_options: Some(vec![
                "--path <DIR>  Target directory to watch (default: .)",
                "--debounce <SECS>  Debounce interval in seconds (default: 1)",
                "--with-embedding  Generate embeddings during update",
            ]),
            output_formats: None,
            output: None,
            input: None,
            pipe_support: None,
            subcommands: None,
            examples: vec![
                "commandindexdev watch --path .",
                "commandindexdev watch --debounce 3 --with-embedding",
            ],
        },
    ]
}

// ---------------------------------------------------------------------------
// Run
// ---------------------------------------------------------------------------

pub fn run_help_llm() -> Result<(), HelpLlmError> {
    let output = build_help_llm_output();
    let json = serde_json::to_string_pretty(&output)
        .map_err(|e| HelpLlmError::Serialize(e.to_string()))?;
    println!("{json}");
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_output_serializes_to_json() {
        let output = build_help_llm_output();
        let json = serde_json::to_string_pretty(&output);
        assert!(
            json.is_ok(),
            "build_help_llm_output should serialize to JSON"
        );
        let json_str = json.unwrap();
        // Verify it parses back
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert!(parsed.is_object());
    }

    #[test]
    fn test_build_output_all_commands_have_examples() {
        let output = build_help_llm_output();
        for cmd in &output.commands {
            assert!(
                !cmd.examples.is_empty(),
                "Command '{}' must have at least one example",
                cmd.name
            );
        }
    }

    #[test]
    fn test_build_output_all_commands_have_name() {
        let output = build_help_llm_output();
        for cmd in &output.commands {
            assert!(
                !cmd.name.is_empty(),
                "All commands must have a non-empty name"
            );
        }
    }

    #[test]
    fn test_build_output_schema_version() {
        let output = build_help_llm_output();
        assert_eq!(output.schema_version, "1.0");
    }
}
