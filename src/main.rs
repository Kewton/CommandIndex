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
    },
    /// Incrementally update the index
    Update,
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
            query: _,
            format: _,
        } => {
            eprintln!("Error: `search` command is not yet implemented. Coming in Phase 1.");
            1
        }
        Commands::Update => {
            eprintln!("Error: `update` command is not yet implemented. Coming in Phase 2.");
            1
        }
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
