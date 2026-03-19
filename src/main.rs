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
    },
    /// Incrementally update the index
    Update,
    /// Show index status
    Status,
    /// Remove index and prepare for rebuild
    Clean,
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
        Commands::Search { query: _ } => {
            eprintln!("Error: `search` command is not yet implemented. Coming in Phase 1.");
            1
        }
        Commands::Update => {
            eprintln!("Error: `update` command is not yet implemented. Coming in Phase 2.");
            1
        }
        Commands::Status => {
            eprintln!("Error: `status` command is not yet implemented. Coming in Phase 1.");
            1
        }
        Commands::Clean => {
            eprintln!("Error: `clean` command is not yet implemented. Coming in Phase 1.");
            1
        }
    };

    process::exit(exit_code);
}
