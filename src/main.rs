use std::path::PathBuf;
use std::process;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "commandindex")]
#[command(about = "Git-native knowledge CLI \u{2014} search across Markdown, Code, and Git")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
#[allow(clippy::large_enum_variant)]
enum Commands {
    /// Build search index from repository
    Index {
        /// Target directory to index
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Generate embeddings during indexing
        #[arg(long)]
        with_embedding: bool,
    },
    /// Search the index
    Search {
        /// Search query (full-text search)
        query: Option<String>,
        /// Search by symbol name (function, class, method)
        #[arg(long, conflicts_with_all = ["query", "semantic", "workspace"])]
        symbol: Option<String>,
        /// Search for related files
        #[arg(long, conflicts_with_all = ["query", "symbol", "semantic", "tag", "path", "file_type", "heading", "workspace"])]
        related: Option<String>,
        /// Semantic search query (embedding-based similarity search)
        #[arg(long, conflicts_with_all = ["query", "symbol", "related", "heading", "workspace"])]
        semantic: Option<String>,
        /// Disable hybrid (BM25 + Semantic) search, use BM25 only
        #[arg(long, conflicts_with_all = ["semantic", "symbol", "related"])]
        no_semantic: bool,
        /// Output format (human, json, path)
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
        /// Maximum number of results (default: from config or 20)
        #[arg(long)]
        limit: Option<usize>,
        /// Number of snippet lines (default: from config or 2)
        #[arg(long)]
        snippet_lines: Option<usize>,
        /// Number of snippet characters for single-line body (default: from config or 120)
        #[arg(long)]
        snippet_chars: Option<usize>,
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
    },
    /// Incrementally update the index
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
    /// Show index status
    Status {
        /// Target directory
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Output format (human, json)
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
    /// Remove index and prepare for rebuild
    Clean {
        /// Target directory containing .commandindex/
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Keep embeddings database when cleaning
        #[arg(long)]
        keep_embeddings: bool,
    },
    /// Generate AI-oriented context pack for specified files
    Context {
        /// Target file paths (multiple allowed)
        #[arg(required = true)]
        files: Vec<String>,

        /// Maximum number of related files to include
        #[arg(long, default_value = "20")]
        max_files: usize,

        /// Estimated token limit
        #[arg(long)]
        max_tokens: Option<usize>,
    },
    /// Generate embeddings for indexed sections
    Embed {
        /// Target directory
        #[arg(long, default_value = ".")]
        path: PathBuf,
    },
    /// Show or manage configuration
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },
    /// Export index as portable tar.gz archive
    Export {
        /// Output file path (.tar.gz)
        output: PathBuf,
        /// Include embedding database
        #[arg(long)]
        with_embeddings: bool,
    },
    /// Import index from tar.gz archive
    Import {
        /// Input archive file path (.tar.gz)
        input: PathBuf,
        /// Overwrite existing index
        #[arg(long)]
        force: bool,
    },
}

#[derive(Subcommand)]
enum ConfigCommands {
    /// Show current effective config (secrets masked)
    Show,
    /// Show loaded config file paths
    Path,
}

fn main() {
    let cli = Cli::parse();

    let exit_code = match cli.command {
        Commands::Index {
            path,
            with_embedding,
        } => {
            let options = commandindex::cli::index::IndexOptions { with_embedding };
            match commandindex::cli::index::run(&path, &options) {
                Ok(summary) => {
                    println!("Indexing {}...", path.display());
                    println!("  Scanned: {} files", summary.scanned);
                    println!("  Indexed: {} sections", summary.indexed_sections);
                    println!("  Skipped: {} files (parse error)", summary.skipped);
                    println!("  Ignored: {} files (.cmindexignore)", summary.ignored);
                    println!("  Duration: {:.1}s", summary.duration.as_secs_f64());
                    println!("Index saved to .commandindex/");
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
            rerank,
            rerank_top,
            workspace,
            repo,
        } => {
            // Build SearchContext for config resolution
            let ctx = commandindex::cli::search::SearchContext::from_current_dir().ok();
            let (effective_limit, effective_snippet_lines, effective_snippet_chars) = match &ctx {
                Some(c) => (
                    limit.unwrap_or(c.config.search.default_limit).min(1000),
                    snippet_lines.unwrap_or(c.config.search.snippet_lines),
                    snippet_chars.unwrap_or(c.config.search.snippet_chars),
                ),
                None => (
                    limit.unwrap_or(20).min(1000),
                    snippet_lines.unwrap_or(2),
                    snippet_chars.unwrap_or(120),
                ),
            };
            let snippet_config = commandindex::output::SnippetConfig {
                lines: effective_snippet_lines,
                chars: effective_snippet_chars,
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
                            None => match commandindex::cli::search::SearchContext::from_current_dir() {
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
                        commandindex::cli::search::run(&ctx, &options, &filters, format, snippet_config, rerank, rerank_top)
                    }
                    (None, Some(s), None, None) => {
                        commandindex::cli::search::run_symbol_search(&s, effective_limit, format)
                    }
                    (None, None, Some(f), None) => {
                        commandindex::cli::search::run_related_search(&f, effective_limit, format)
                    }
                    (None, None, None, Some(q)) => {
                        let filters = commandindex::indexer::reader::SearchFilters {
                            path_prefix: path,
                            file_type,
                        };
                        commandindex::cli::search::run_semantic_search(
                            &q,
                            effective_limit,
                            format,
                            tag.as_deref(),
                            &filters,
                        )
                    }
                    (None, None, None, None) => Err(commandindex::cli::search::SearchError::InvalidArgument(
                        "Either <QUERY>, --symbol <NAME>, --related <FILE>, or --semantic <QUERY> is required".to_string(),
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
                let options = commandindex::cli::index::IndexOptions { with_embedding };
                match commandindex::cli::index::run_incremental(&path, &options) {
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
                let options = commandindex::cli::status::StatusOptions {
                    detail,
                    coverage,
                    format,
                    verify,
                };
                match commandindex::cli::status::run(&path, &options, &mut std::io::stdout()) {
                    Ok(()) => 0,
                    Err(e) => {
                        eprintln!("{e}");
                        1
                    }
                }
            }
        }
        Commands::Context {
            files,
            max_files,
            max_tokens,
        } => match commandindex::cli::context::run_context(&files, max_files, max_tokens) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("Error: {e}");
                1
            }
        },
        Commands::Clean {
            path,
            keep_embeddings,
        } => {
            let options = commandindex::cli::clean::CleanOptions { keep_embeddings };
            match commandindex::cli::clean::run(&path, &options) {
                Ok(commandindex::cli::clean::CleanResult::Removed) => {
                    if keep_embeddings {
                        println!("Removed index (embeddings preserved)");
                    } else {
                        println!("Removed index at .commandindex/");
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
        Commands::Embed { path } => match commandindex::cli::embed::run(&path) {
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
        },
        Commands::Config { command } => match command {
            ConfigCommands::Show => match commandindex::cli::config::run_show() {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("Error: {e}");
                    1
                }
            },
            ConfigCommands::Path => match commandindex::cli::config::run_path() {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("Error: {e}");
                    1
                }
            },
        },
        Commands::Export {
            output,
            with_embeddings,
        } => {
            let options = commandindex::cli::export::ExportOptions { with_embeddings };
            match commandindex::cli::export::run(std::path::Path::new("."), &output, &options) {
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
            let options = commandindex::cli::import_index::ImportOptions { force };
            match commandindex::cli::import_index::run(std::path::Path::new("."), &input, &options)
            {
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
    };

    process::exit(exit_code);
}
