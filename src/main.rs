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
    },
    /// Search the index
    Search {
        /// Search query
        query: String,
        /// Output format (human, json, path)
        #[arg(long, value_enum, default_value_t = commandindex::output::OutputFormat::Human)]
        format: commandindex::output::OutputFormat,
        /// Filter by tag
        #[arg(long)]
        tag: Option<String>,
        /// Filter by path prefix
        #[arg(long)]
        path: Option<String>,
        /// Filter by file type (e.g. "markdown")
        #[arg(long = "type")]
        file_type: Option<String>,
        /// Filter by heading
        #[arg(long)]
        heading: Option<String>,
        /// Maximum number of results (1-1000)
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// Incrementally update the index
    Update {
        /// Target directory
        #[arg(long, default_value = ".")]
        path: PathBuf,
    },
    /// Show index status
    Status {
        /// Target directory
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Output format (human, json)
        #[arg(long, value_enum, default_value_t = commandindex::cli::status::StatusFormat::Human)]
        format: commandindex::cli::status::StatusFormat,
    },
    /// Remove index and prepare for rebuild
    Clean {
        /// Target directory containing .commandindex/
        #[arg(long, default_value = ".")]
        path: PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();

    let exit_code = match cli.command {
        Commands::Index { path } => match commandindex::cli::index::run(&path) {
            Ok(summary) => {
                println!("Indexing {}...", path.display());
                println!("  Scanned: {} files", summary.scanned);
                println!("  Indexed: {} sections", summary.indexed_sections);
                println!("  Skipped: {} files (parse error)", summary.skipped);
                println!("  Ignored: {} files (.cmindexignore)", summary.ignored);
                println!("  Duration: {:.1}s", summary.duration.as_secs_f64());
                println!("Index saved to .commandindex/");
                0
            }
            Err(e) => {
                eprintln!("Error: {e}");
                1
            }
        },
        Commands::Search {
            query,
            format,
            tag,
            path,
            file_type,
            heading,
            limit,
        } => {
            let options = commandindex::indexer::reader::SearchOptions {
                query,
                tag,
                heading,
                limit: limit.min(1000),
            };
            let filters = commandindex::indexer::reader::SearchFilters {
                path_prefix: path,
                file_type,
            };
            match commandindex::cli::search::run(&options, &filters, format) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("Error: {e}");
                    1
                }
            }
        }
        Commands::Update { path } => match commandindex::cli::index::run_incremental(&path) {
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
                0
            }
            Err(e) => {
                eprintln!("Error: {e}");
                1
            }
        },
        Commands::Status { path, format } => {
            match commandindex::cli::status::run(&path, format, &mut std::io::stdout()) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("{e}");
                    1
                }
            }
        }
        Commands::Clean { path } => match commandindex::cli::clean::run(&path) {
            Ok(commandindex::cli::clean::CleanResult::Removed) => {
                println!("Removed index at .commandindex/");
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
        },
    };

    process::exit(exit_code);
}
