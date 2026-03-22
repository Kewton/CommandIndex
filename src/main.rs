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
        #[arg(long, conflicts_with_all = ["query", "semantic"])]
        symbol: Option<String>,
        /// Search for related files
        #[arg(long, conflicts_with_all = ["query", "symbol", "semantic", "tag", "path", "file_type", "heading"])]
        related: Option<String>,
        /// Semantic search query (embedding-based similarity search)
        #[arg(long, conflicts_with_all = ["query", "symbol", "related", "heading"])]
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
        /// Maximum number of results (1-1000)
        #[arg(long, default_value_t = 20)]
        limit: usize,
        /// Number of snippet lines (0 = unlimited)
        #[arg(long, default_value_t = 2)]
        snippet_lines: usize,
        /// Number of snippet characters for single-line body (0 = unlimited)
        #[arg(long, default_value_t = 120)]
        snippet_chars: usize,
        /// Enable LLM-based reranking of search results
        #[arg(long, conflicts_with_all = ["symbol", "related", "semantic"])]
        rerank: bool,
        /// Number of top candidates to rerank (requires --rerank)
        #[arg(long, requires = "rerank")]
        rerank_top: Option<usize>,
    },
    /// Incrementally update the index
    Update {
        /// Target directory
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Generate embeddings during update
        #[arg(long)]
        with_embedding: bool,
    },
    /// Show index status
    Status {
        /// Target directory
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Output format (human, json)
        #[arg(long, value_enum, default_value_t = commandindex::cli::status::StatusFormat::Human)]
        format: commandindex::cli::status::StatusFormat,
        /// Show detailed statistics (coverage, staleness, storage)
        #[arg(long, conflicts_with = "coverage")]
        detail: bool,
        /// Show coverage statistics only
        #[arg(long, conflicts_with = "detail")]
        coverage: bool,
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
        } => {
            let snippet_config = commandindex::output::SnippetConfig {
                lines: snippet_lines,
                chars: snippet_chars,
            };
            let result = match (query, symbol, related, semantic) {
                (Some(q), None, None, None) => {
                    let options = commandindex::indexer::reader::SearchOptions {
                        query: q,
                        tag,
                        heading,
                        limit: limit.min(1000),
                        no_semantic,
                    };
                    let filters = commandindex::indexer::reader::SearchFilters {
                        path_prefix: path,
                        file_type,
                    };
                    commandindex::cli::search::run(&options, &filters, format, snippet_config, rerank, rerank_top)
                }
                (None, Some(s), None, None) => {
                    commandindex::cli::search::run_symbol_search(&s, limit.min(1000), format)
                }
                (None, None, Some(f), None) => {
                    commandindex::cli::search::run_related_search(&f, limit.min(1000), format)
                }
                (None, None, None, Some(q)) => {
                    let filters = commandindex::indexer::reader::SearchFilters {
                        path_prefix: path,
                        file_type,
                    };
                    commandindex::cli::search::run_semantic_search(
                        &q,
                        limit.min(1000),
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
        Commands::Update {
            path,
            with_embedding,
        } => {
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
        Commands::Status {
            path,
            format,
            detail,
            coverage,
        } => {
            let options = commandindex::cli::status::StatusOptions {
                detail,
                coverage,
                format,
            };
            match commandindex::cli::status::run(&path, &options, &mut std::io::stdout()) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("{e}");
                    1
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
    };

    process::exit(exit_code);
}
