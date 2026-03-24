use std::path::PathBuf;
use std::process;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "commandindex")]
#[command(about = "Git-native knowledge CLI \u{2014} search across Markdown, Code, and Git")]
#[command(version)]
struct Cli {
    /// Custom index directory path (overrides default .commandindex/)
    #[arg(long, global = true)]
    index_path: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
#[allow(clippy::large_enum_variant)]
enum Commands {
    /// Build full search index from repository (--with-embedding supported)
    #[command(after_help = commandindex::cli::index::INDEX_AFTER_HELP)]
    Index {
        /// Target directory to index
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Generate embeddings during indexing
        #[arg(long)]
        with_embedding: bool,
    },
    /// Search the index (full-text, --related, --semantic, --changed-since, --symbol)
    #[command(after_help = commandindex::cli::search::SEARCH_AFTER_HELP)]
    Search {
        /// Search query (full-text search)
        query: Option<String>,
        /// Search by symbol name (function, class, method)
        #[arg(long, conflicts_with_all = ["query", "semantic", "workspace"])]
        symbol: Option<String>,
        /// Search for related files
        #[arg(long, num_args(1..), conflicts_with_all = ["query", "symbol", "semantic", "tag", "path", "file_type", "heading", "workspace", "related_stdin"])]
        related: Option<Vec<String>>,
        /// Read related file paths from stdin (one per line)
        #[arg(long, conflicts_with_all = ["query", "symbol", "related", "semantic", "tag", "path", "file_type", "heading", "workspace", "no_semantic", "rerank"])]
        related_stdin: bool,
        /// Semantic search query (embedding-based similarity search)
        #[arg(long, conflicts_with_all = ["query", "symbol", "related", "heading", "workspace"])]
        semantic: Option<String>,
        /// Disable hybrid (BM25 + Semantic) search, use BM25 only
        #[arg(long, conflicts_with_all = ["semantic", "symbol", "related"])]
        no_semantic: bool,
        /// Output format (human, json, path, llm)
        #[arg(long, value_enum, default_value_t = commandindex::output::OutputFormat::Human)]
        format: commandindex::output::OutputFormat,
        /// Filter by tag
        #[arg(long)]
        tag: Option<String>,
        /// Filter by path prefix
        #[arg(long)]
        path: Option<String>,
        /// Filter by file type
        #[arg(
            long = "type",
            value_parser = clap::builder::PossibleValuesParser::new(
                commandindex::indexer::manifest::FileType::valid_type_filter_names()
            )
        )]
        file_type: Option<String>,
        /// Filter by heading
        #[arg(long)]
        heading: Option<String>,
        /// Maximum number of results (default: 20, with --rerank: 5)
        #[arg(long)]
        limit: Option<usize>,
        /// Number of snippet lines (default: from config or 2)
        #[arg(long)]
        snippet_lines: Option<usize>,
        /// Number of snippet characters for single-line body (default: from config or 120)
        #[arg(long)]
        snippet_chars: Option<usize>,
        /// Enable snippet output for related search results
        #[arg(long)]
        with_snippet: bool,
        /// Enable LLM-based reranking of search results
        #[arg(long, conflicts_with_all = ["symbol", "related", "semantic"])]
        rerank: bool,
        /// Number of top candidates to rerank (requires --rerank)
        #[arg(long, requires = "rerank")]
        rerank_top: Option<usize>,
        /// Workspace config file path
        #[arg(long)]
        workspace: Option<String>,
        /// Filter by repository alias
        #[arg(long, requires = "workspace")]
        repo: Option<String>,
        /// Show related files changed since a time/commit (e.g. "12 hours ago", "yesterday", commit hash)
        #[arg(long, conflicts_with_all = ["query", "symbol", "related", "semantic", "workspace"])]
        changed_since: Option<String>,
        /// Limits total estimated tokens in output (approx. 1 token per 4 chars)
        #[arg(long, value_parser = clap::value_parser!(u64).range(1..=1_000_000))]
        max_tokens: Option<u64>,
    },
    /// Incrementally update index (Git-aware delta, --workspace supported)
    #[command(after_help = commandindex::cli::index::UPDATE_AFTER_HELP)]
    Update {
        /// Target directory
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Generate embeddings during update
        #[arg(long)]
        with_embedding: bool,
        /// Workspace config file path
        #[arg(long)]
        workspace: Option<String>,
    },
    /// Show index status (--detail, --coverage, --verify, --format json)
    #[command(after_help = commandindex::cli::status::STATUS_AFTER_HELP)]
    Status {
        /// Target directory
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Output format for status (human, json)
        #[arg(long, value_enum, default_value_t = commandindex::cli::status::StatusFormat::Human)]
        format: commandindex::cli::status::StatusFormat,
        /// Workspace config file path
        #[arg(long)]
        workspace: Option<String>,
        /// Show detailed statistics (coverage, staleness, storage)
        #[arg(long, conflicts_with = "coverage")]
        detail: bool,
        /// Show coverage statistics only
        #[arg(long, conflicts_with = "detail")]
        coverage: bool,
        /// Verify index integrity
        #[arg(long)]
        verify: bool,
    },
    /// Remove index and prepare for rebuild (--keep-embeddings supported)
    #[command(after_help = commandindex::cli::clean::CLEAN_AFTER_HELP)]
    Clean {
        /// Target directory containing .commandindex/
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Keep embeddings database when cleaning
        #[arg(long)]
        keep_embeddings: bool,
    },
    /// Compare related files between two files (conflict/overlap detection, JSON output)
    #[command(after_help = commandindex::cli::diff::DIFF_AFTER_HELP)]
    Diff {
        /// Two files to compare
        #[arg(required = true, num_args = 2)]
        files: Vec<String>,

        /// Output format
        #[arg(long, value_enum, default_value_t = commandindex::output::OutputFormat::Human)]
        format: commandindex::output::OutputFormat,

        /// Maximum related files per input file
        #[arg(long, default_value = "100", value_parser = clap::value_parser!(u64).range(1..=10000))]
        limit: u64,

        /// Custom index directory path (overrides default .commandindex/)
        #[arg(long)]
        index_path: Option<PathBuf>,
    },
    /// Generate AI-oriented context pack for specified files (JSON output)
    #[command(after_help = commandindex::cli::context::CONTEXT_AFTER_HELP)]
    Context {
        /// Target file paths (multiple allowed)
        #[arg(required = true)]
        files: Vec<String>,

        /// Maximum number of related files to include
        #[arg(long, default_value = "20", value_parser = clap::value_parser!(u64).range(1..=1000))]
        max_files: u64,

        /// Estimated token limit (approx. 1 token per 4 chars)
        #[arg(long, value_parser = clap::value_parser!(u64).range(1..=1_000_000))]
        max_tokens: Option<u64>,
    },
    /// Generate embeddings for semantic search (requires Ollama)
    #[command(after_help = commandindex::cli::embed::EMBED_AFTER_HELP)]
    Embed {
        /// Target directory
        #[arg(long, default_value = ".")]
        path: PathBuf,
    },
    /// Show or manage configuration (show/path subcommands)
    #[command(after_help = commandindex::cli::config::CONFIG_AFTER_HELP)]
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },
    /// Export index as portable tar.gz archive (--with-embeddings supported)
    #[command(after_help = commandindex::cli::export::EXPORT_AFTER_HELP)]
    Export {
        /// Output file path (.tar.gz)
        output: PathBuf,
        /// Include embedding database
        #[arg(long)]
        with_embeddings: bool,
    },
    /// Analyze impact of file changes (stdin pipe supported, JSON output)
    #[command(after_help = commandindex::cli::impact::IMPACT_AFTER_HELP)]
    Impact {
        /// Input files (if not provided, reads from stdin)
        #[arg()]
        files: Vec<String>,

        /// Output format (human, json, path, llm)
        #[arg(long, value_enum, default_value_t = commandindex::output::OutputFormat::Human)]
        format: commandindex::output::OutputFormat,

        /// Maximum number of impacted files to show
        #[arg(long)]
        limit: Option<usize>,

        /// Enable snippet output for impacted files
        #[arg(long)]
        with_snippet: bool,

        /// Number of snippet lines (default: from config or 2)
        #[arg(long, value_parser = clap::value_parser!(u64).range(1..))]
        snippet_lines: Option<u64>,

        /// Number of snippet characters for single-line body (default: from config or 120)
        #[arg(long, value_parser = clap::value_parser!(u64).range(1..))]
        snippet_chars: Option<u64>,

        /// Custom index directory path (overrides default .commandindex/)
        #[arg(long)]
        index_path: Option<PathBuf>,
        /// Limits total estimated tokens in output (approx. 1 token per 4 chars)
        #[arg(long, value_parser = clap::value_parser!(u64).range(1..=1_000_000))]
        max_tokens: Option<u64>,
    },
    /// Import index from tar.gz archive (--force to overwrite)
    #[command(after_help = commandindex::cli::import_index::IMPORT_AFTER_HELP)]
    Import {
        /// Input archive file path (.tar.gz)
        input: PathBuf,
        /// Overwrite existing index
        #[arg(long)]
        force: bool,
    },
    /// Show structured JSON help for LLM integration
    #[command(name = "help-llm")]
    HelpLlm,
    /// Suggest search strategy based on task description (LLM-oriented)
    #[command(after_help = commandindex::cli::suggest::SUGGEST_AFTER_HELP)]
    Suggest {
        /// Task description
        #[arg(long = "for")]
        for_task: String,

        /// Output format (human, json, path)
        #[arg(long, value_enum, default_value_t = commandindex::output::OutputFormat::Human)]
        format: commandindex::output::OutputFormat,
    },
    /// Show documents related to an Issue from knowledge graph
    Issue {
        /// Issue number
        #[arg(value_parser = clap::value_parser!(u64).range(1..))]
        number: u64,
        /// Output format (human, json, path, llm)
        #[arg(long, value_enum, default_value_t = commandindex::output::OutputFormat::Human)]
        format: commandindex::output::OutputFormat,
    },
    /// Watch for file changes and auto-update index (daemon mode)
    #[command(after_help = commandindex::cli::watch::WATCH_AFTER_HELP)]
    Watch {
        /// Target directory to watch
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Debounce interval in seconds
        #[arg(long, default_value = "1")]
        debounce: u64,
        /// Generate embeddings during update
        #[arg(long)]
        with_embedding: bool,
    },
}

#[derive(Subcommand)]
enum ConfigCommands {
    /// Show current effective config (secrets masked)
    Show,
    /// Show loaded config file paths
    Path,
}

/// Resolve commandindex_dir from CLI --index-path, config, and base_path.
/// Returns (commandindex_dir, config) pair.
fn resolve_commandindex_dir(
    cli_index_path: Option<&std::path::Path>,
    base_path: &std::path::Path,
) -> Result<(PathBuf, commandindex::config::AppConfig), String> {
    let config = commandindex::config::load_config(base_path).map_err(|e| format!("{e}"))?;
    let commandindex_dir = commandindex::indexer::resolve_index_path(
        cli_index_path,
        config.index.path.as_deref(),
        base_path,
    )
    .map_err(|e| format!("{e}"))?;
    Ok((commandindex_dir, config))
}

fn main() {
    let cli = Cli::parse();

    let exit_code = match cli.command {
        Commands::Index {
            path,
            with_embedding,
        } => {
            let (commandindex_dir, _config) =
                match resolve_commandindex_dir(cli.index_path.as_deref(), &path) {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("Error: {e}");
                        process::exit(1);
                    }
                };
            // reject symlink for write command
            if let Err(e) = commandindex::indexer::reject_symlink(&commandindex_dir) {
                eprintln!("Error: {e}");
                process::exit(1);
            }
            let options = commandindex::cli::index::IndexOptions { with_embedding };
            match commandindex::cli::index::run(&path, &commandindex_dir, &options) {
                Ok(summary) => {
                    println!("Indexing {}...", path.display());
                    println!("  Scanned: {} files", summary.scanned);
                    println!("  Indexed: {} sections", summary.indexed_sections);
                    println!("  Skipped: {} files (parse error)", summary.skipped);
                    println!("  Ignored: {} files (.cmindexignore)", summary.ignored);
                    println!("  Duration: {:.1}s", summary.duration.as_secs_f64());
                    println!("Index saved to {}/", commandindex_dir.display());
                    if with_embedding {
                        println!("Embeddings generated.");
                    }
                    0
                }
                Err(e) => {
                    eprintln!("Error: {e}");
                    1
                }
            }
        }
        Commands::Search {
            query,
            symbol,
            related,
            related_stdin,
            semantic,
            no_semantic,
            format,
            tag,
            path,
            file_type,
            heading,
            limit,
            snippet_lines,
            snippet_chars,
            with_snippet,
            rerank,
            rerank_top,
            workspace,
            repo,
            changed_since,
            max_tokens,
        } => {
            let max_tokens = max_tokens.map(|t| t as usize);
            // Handle --changed-since: delegate to impact with git log
            if let Some(since) = changed_since {
                let index_path_opt = cli.index_path.as_deref();
                match commandindex::cli::changed_since::run_changed_since(
                    &since,
                    format,
                    limit,
                    index_path_opt,
                    max_tokens,
                ) {
                    Ok(()) => return,
                    Err(commandindex::cli::changed_since::ChangedSinceError::NoChanges) => {
                        eprintln!("No files changed in the specified period");
                        return;
                    }
                    Err(e) => {
                        eprintln!("Error: {e}");
                        process::exit(1);
                    }
                }
            }

            // Build SearchContext for config resolution
            let base_path = std::path::Path::new(".");
            let ctx =
                commandindex::cli::search::SearchContext::new(base_path, cli.index_path.as_deref())
                    .ok();
            let search_config = ctx
                .as_ref()
                .map(|c| c.config.search.clone())
                .unwrap_or_default();
            let effective_limit = search_config.resolve_limit(limit, rerank);
            let effective_snippet_lines = snippet_lines.unwrap_or(search_config.snippet_lines);
            let effective_snippet_chars = snippet_chars.unwrap_or(search_config.snippet_chars);
            let snippet_config = commandindex::output::SnippetConfig {
                lines: effective_snippet_lines,
                chars: effective_snippet_chars,
            };
            let snippet_options = commandindex::cli::snippet_helper::SnippetOptions {
                enabled: with_snippet,
                config: snippet_config,
            };

            // Workspace横断検索分岐
            if let Some(ws_path) = workspace {
                let q = match query {
                    Some(q) => q,
                    None => {
                        eprintln!("Error: <QUERY> is required for workspace search");
                        process::exit(1);
                    }
                };
                let options = commandindex::indexer::reader::SearchOptions {
                    query: q,
                    tag,
                    heading,
                    limit: effective_limit,
                    no_semantic,
                };
                let filters = commandindex::indexer::reader::SearchFilters {
                    path_prefix: path,
                    file_type,
                };
                let result = commandindex::cli::workspace::run_workspace_search(
                    &ws_path,
                    repo.as_deref(),
                    &options,
                    &filters,
                    format,
                    snippet_config,
                    rerank,
                    rerank_top,
                    max_tokens,
                );
                match result {
                    Ok(()) => 0,
                    Err(e) => {
                        eprintln!("Error: {e}");
                        1
                    }
                }
            } else if related_stdin {
                let result = commandindex::cli::search::run_related_search_from_stdin(
                    effective_limit,
                    format,
                    snippet_options.clone(),
                    max_tokens,
                );
                match result {
                    Ok(()) => 0,
                    Err(e) => {
                        eprintln!("Error: {e}");
                        1
                    }
                }
            } else {
                let result = match (query, symbol, related, semantic) {
                    (Some(q), None, None, None) => {
                        // SearchContext is required for full-text search
                        let ctx = match ctx {
                            Some(c) => c,
                            None => match commandindex::cli::search::SearchContext::new(
                                base_path,
                                cli.index_path.as_deref(),
                            ) {
                                Ok(c) => c,
                                Err(e) => {
                                    eprintln!("Error: {e}");
                                    process::exit(1);
                                }
                            },
                        };
                        let options = commandindex::indexer::reader::SearchOptions {
                            query: q,
                            tag,
                            heading,
                            limit: effective_limit,
                            no_semantic,
                        };
                        let filters = commandindex::indexer::reader::SearchFilters {
                            path_prefix: path,
                            file_type,
                        };
                        let llm_options = commandindex::output::LlmFormatOptions {
                            max_body_lines: snippet_lines,
                        };
                        commandindex::cli::search::run(&ctx, &options, &filters, format, snippet_config, rerank, rerank_top, max_tokens, &llm_options)
                    }
                    (None, Some(s), None, None) => {
                        let ctx_for_symbol = ctx.or_else(|| {
                            commandindex::cli::search::SearchContext::new(
                                base_path,
                                cli.index_path.as_deref(),
                            )
                            .ok()
                        });
                        commandindex::cli::search::run_symbol_search(&s, effective_limit, format, ctx_for_symbol.as_ref(), max_tokens)
                    }
                    (None, None, Some(ref files), None) => {
                        let ctx_for_related = ctx.or_else(|| {
                            commandindex::cli::search::SearchContext::new(
                                base_path,
                                cli.index_path.as_deref(),
                            )
                            .ok()
                        });
                        commandindex::cli::search::run_related_search(files, effective_limit, format, ctx_for_related.as_ref(), snippet_options.clone(), max_tokens)
                    }
                    (None, None, None, Some(q)) => {
                        let filters = commandindex::indexer::reader::SearchFilters {
                            path_prefix: path,
                            file_type,
                        };
                        let ctx_for_semantic = ctx.or_else(|| {
                            commandindex::cli::search::SearchContext::new(
                                base_path,
                                cli.index_path.as_deref(),
                            )
                            .ok()
                        });
                        commandindex::cli::search::run_semantic_search(
                            &q,
                            effective_limit,
                            format,
                            tag.as_deref(),
                            &filters,
                            ctx_for_semantic.as_ref(),
                            max_tokens,
                        )
                    }
                    (None, None, None, None) => Err(commandindex::cli::search::SearchError::InvalidArgument(
                        "Either <QUERY>, --symbol <NAME>, --related <FILE>, --related-stdin, or --semantic <QUERY> is required".to_string(),
                    )),
                    _ => unreachable!("clap conflicts_with prevents this"),
                };
                match result {
                    Ok(()) => 0,
                    Err(e) => {
                        eprintln!("Error: {e}");
                        1
                    }
                }
            }
        }
        Commands::Update {
            path,
            with_embedding,
            workspace,
        } => {
            if let Some(ws_path) = workspace {
                match commandindex::cli::workspace::run_workspace_update(&ws_path, with_embedding) {
                    Ok(code) => code,
                    Err(e) => {
                        eprintln!("Error: {e}");
                        1
                    }
                }
            } else {
                let (commandindex_dir, _config) =
                    match resolve_commandindex_dir(cli.index_path.as_deref(), &path) {
                        Ok(v) => v,
                        Err(e) => {
                            eprintln!("Error: {e}");
                            process::exit(1);
                        }
                    };
                if let Err(e) = commandindex::indexer::reject_symlink(&commandindex_dir) {
                    eprintln!("Error: {e}");
                    process::exit(1);
                }
                let options = commandindex::cli::index::IndexOptions { with_embedding };
                match commandindex::cli::index::run_incremental(&path, &commandindex_dir, &options)
                {
                    Ok(summary) => {
                        println!("Incremental update completed:");
                        println!(
                            "  Added:     {} files ({} sections)",
                            summary.added_files, summary.added_sections
                        );
                        println!(
                            "  Modified:  {} files ({} sections)",
                            summary.modified_files, summary.modified_sections
                        );
                        println!("  Deleted:   {} files", summary.deleted_files);
                        println!("  Unchanged: {} files", summary.unchanged);
                        println!("  Skipped:   {} files", summary.skipped);
                        println!("  Duration:  {:.2}s", summary.duration.as_secs_f64());
                        if with_embedding {
                            println!("Embeddings generated.");
                        }
                        0
                    }
                    Err(e) => {
                        eprintln!("Error: {e}");
                        1
                    }
                }
            }
        }
        Commands::Status {
            path,
            format,
            workspace,
            detail,
            coverage,
            verify,
        } => {
            if let Some(ws_path) = workspace {
                match commandindex::cli::workspace::run_workspace_status(&ws_path, format) {
                    Ok(()) => 0,
                    Err(e) => {
                        eprintln!("Error: {e}");
                        1
                    }
                }
            } else {
                let (commandindex_dir, _config) =
                    match resolve_commandindex_dir(cli.index_path.as_deref(), &path) {
                        Ok(v) => v,
                        Err(e) => {
                            eprintln!("Error: {e}");
                            process::exit(1);
                        }
                    };
                let options = commandindex::cli::status::StatusOptions {
                    detail,
                    coverage,
                    format,
                    verify,
                };
                match commandindex::cli::status::run(
                    &path,
                    &commandindex_dir,
                    &options,
                    &mut std::io::stdout(),
                ) {
                    Ok(()) => 0,
                    Err(e) => {
                        eprintln!("{e}");
                        1
                    }
                }
            }
        }
        Commands::Diff {
            files,
            format,
            limit,
            index_path,
        } => match commandindex::cli::diff::run_diff(
            &files,
            usize::try_from(limit).unwrap_or(usize::MAX),
            format,
            index_path.as_deref(),
        ) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("Error: {e}");
                1
            }
        },
        Commands::Impact {
            files,
            format,
            limit,
            with_snippet,
            snippet_lines,
            snippet_chars,
            index_path,
            max_tokens,
        } => {
            let max_tokens = max_tokens.map(|t| t as usize);
            let base_path = std::path::Path::new(".");
            let config = commandindex::config::load_config(base_path).ok();
            let impact_snippet_options = commandindex::cli::snippet_helper::SnippetOptions {
                enabled: with_snippet,
                config: commandindex::output::SnippetConfig {
                    lines: snippet_lines
                        .map(|v| usize::try_from(v).unwrap_or(usize::MAX))
                        .unwrap_or_else(|| config.as_ref().map_or(2, |c| c.search.snippet_lines)),
                    chars: snippet_chars
                        .map(|v| usize::try_from(v).unwrap_or(usize::MAX))
                        .unwrap_or_else(|| config.as_ref().map_or(120, |c| c.search.snippet_chars)),
                },
            };
            match commandindex::cli::impact::run_impact(
                &files,
                format,
                limit,
                index_path.as_deref(),
                impact_snippet_options,
                max_tokens,
                &commandindex::output::LlmFormatOptions {
                    max_body_lines: snippet_lines.map(|v| usize::try_from(v).unwrap_or(usize::MAX)),
                },
            ) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("Error: {e}");
                    1
                }
            }
        }
        Commands::Context {
            files,
            max_files,
            max_tokens,
        } => {
            let base_path = std::path::Path::new(".");
            let (commandindex_dir, _config) =
                match resolve_commandindex_dir(cli.index_path.as_deref(), base_path) {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("Error: {e}");
                        process::exit(1);
                    }
                };
            match commandindex::cli::context::run_context(
                &files,
                max_files as usize,
                max_tokens.map(|t| t as usize),
                &commandindex_dir,
            ) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("Error: {e}");
                    1
                }
            }
        }
        Commands::Clean {
            path,
            keep_embeddings,
        } => {
            let (commandindex_dir, _config) =
                match resolve_commandindex_dir(cli.index_path.as_deref(), &path) {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("Error: {e}");
                        process::exit(1);
                    }
                };
            if let Err(e) = commandindex::indexer::reject_symlink(&commandindex_dir) {
                eprintln!("Error: {e}");
                process::exit(1);
            }
            let options = commandindex::cli::clean::CleanOptions { keep_embeddings };
            match commandindex::cli::clean::run(&path, &commandindex_dir, &options) {
                Ok(commandindex::cli::clean::CleanResult::Removed) => {
                    if keep_embeddings {
                        println!("Removed index (embeddings preserved)");
                    } else {
                        println!("Removed index at {}/", commandindex_dir.display());
                    }
                    println!("Run `commandindex index` to rebuild.");
                    0
                }
                Ok(commandindex::cli::clean::CleanResult::NotFound) => {
                    println!("No index found. Nothing to clean.");
                    0
                }
                Err(e) => {
                    eprintln!("Error: {e}");
                    1
                }
            }
        }
        Commands::Embed { path } => {
            let (commandindex_dir, _config) =
                match resolve_commandindex_dir(cli.index_path.as_deref(), &path) {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("Error: {e}");
                        process::exit(1);
                    }
                };
            if let Err(e) = commandindex::indexer::reject_symlink(&commandindex_dir) {
                eprintln!("Error: {e}");
                process::exit(1);
            }
            match commandindex::cli::embed::run(&path, &commandindex_dir) {
                Ok(summary) => {
                    println!("Embedding generation completed:");
                    println!("  Total sections: {}", summary.total_sections);
                    println!("  Generated: {}", summary.generated);
                    println!("  Cached: {}", summary.cached);
                    println!("  Failed: {}", summary.failed);
                    println!("  Duration: {:.2}s", summary.duration.as_secs_f64());
                    0
                }
                Err(e) => {
                    eprintln!("Error: {e}");
                    1
                }
            }
        }
        Commands::Config { command } => {
            let base_path = std::path::Path::new(".");
            let (commandindex_dir, _config) =
                match resolve_commandindex_dir(cli.index_path.as_deref(), base_path) {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("Error: {e}");
                        process::exit(1);
                    }
                };
            match command {
                ConfigCommands::Show => {
                    match commandindex::cli::config::run_show(base_path, &commandindex_dir) {
                        Ok(()) => 0,
                        Err(e) => {
                            eprintln!("Error: {e}");
                            1
                        }
                    }
                }
                ConfigCommands::Path => {
                    match commandindex::cli::config::run_path(base_path, &commandindex_dir) {
                        Ok(()) => 0,
                        Err(e) => {
                            eprintln!("Error: {e}");
                            1
                        }
                    }
                }
            }
        }
        Commands::Export {
            output,
            with_embeddings,
        } => {
            let base_path = std::path::Path::new(".");
            let (commandindex_dir, _config) =
                match resolve_commandindex_dir(cli.index_path.as_deref(), base_path) {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("Error: {e}");
                        process::exit(1);
                    }
                };
            let options = commandindex::cli::export::ExportOptions { with_embeddings };
            match commandindex::cli::export::run(&commandindex_dir, &output, &options) {
                Ok(result) => {
                    println!("Export completed:");
                    println!("  Output: {}", result.output_path.display());
                    println!(
                        "  Size: {}",
                        commandindex::cli::status::format_size(result.archive_size)
                    );
                    if let Some(hash) = &result.git_commit_hash {
                        println!("  Git commit: {hash}");
                    }
                    0
                }
                Err(e) => {
                    eprintln!("Error: {e}");
                    1
                }
            }
        }
        Commands::Import { input, force } => {
            let base_path = std::path::Path::new(".");
            let (commandindex_dir, _config) =
                match resolve_commandindex_dir(cli.index_path.as_deref(), base_path) {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("Error: {e}");
                        process::exit(1);
                    }
                };
            if let Err(e) = commandindex::indexer::reject_symlink(&commandindex_dir) {
                eprintln!("Error: {e}");
                process::exit(1);
            }
            let options = commandindex::cli::import_index::ImportOptions { force };
            match commandindex::cli::import_index::run(
                base_path,
                &commandindex_dir,
                &input,
                &options,
            ) {
                Ok(result) => {
                    println!("Import completed:");
                    println!("  Imported files: {}", result.imported_files);
                    if result.git_hash_match {
                        println!("  Git commit: matches");
                    } else {
                        println!("  Git commit: mismatch");
                    }
                    for warning in &result.warnings {
                        println!("  Warning: {warning}");
                    }
                    0
                }
                Err(e) => {
                    eprintln!("Error: {e}");
                    1
                }
            }
        }
        Commands::Suggest { for_task, format } => {
            match commandindex::cli::suggest::run_suggest(
                &for_task,
                format,
                cli.index_path.as_deref(),
            ) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("Error: {e}");
                    1
                }
            }
        }
        Commands::Issue { number, format } => {
            let base_path = std::path::Path::new(".");
            let (commandindex_dir, _config) =
                match resolve_commandindex_dir(cli.index_path.as_deref(), base_path) {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("Error: {e}");
                        process::exit(1);
                    }
                };
            match commandindex::cli::issue::run(number, format, &commandindex_dir) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("Error: {e}");
                    1
                }
            }
        }
        Commands::HelpLlm => match commandindex::cli::help_llm::run_help_llm() {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("Error: {e}");
                1
            }
        },
        Commands::Watch {
            path,
            debounce,
            with_embedding,
        } => {
            if let Err(e) = commandindex::cli::watch::run(&path, debounce, with_embedding) {
                eprintln!("Watch error: {e}");
                1
            } else {
                0
            }
        }
    };

    process::exit(exit_code);
}
