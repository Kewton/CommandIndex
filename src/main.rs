use clap::{Parser, Subcommand};
use std::process;

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
    Index,
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
    Status,
    /// Remove index and prepare for rebuild
    Clean,
}

fn main() {
    let cli = Cli::parse();

    let exit_code = match cli.command {
        Commands::Index => {
            eprintln!("Error: `index` command is not yet implemented. Coming in Phase 1.");
            1
        }
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
