use std::fmt;
use std::path::Path;

use crate::config::{ConfigError, ConfigSourceKind, load_config};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum ConfigCliError {
    Config(String),
    Serialize(String),
}

impl fmt::Display for ConfigCliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(msg) => write!(f, "Config error: {msg}"),
            Self::Serialize(msg) => write!(f, "Serialize error: {msg}"),
        }
    }
}

impl std::error::Error for ConfigCliError {}

impl From<ConfigError> for ConfigCliError {
    fn from(e: ConfigError) -> Self {
        Self::Config(e.to_string())
    }
}

// ---------------------------------------------------------------------------
// config show
// ---------------------------------------------------------------------------

pub fn run_show(base_path: &Path, commandindex_dir: &Path) -> Result<(), ConfigCliError> {
    let config = load_config(base_path)?;
    let view = config.to_masked_view();
    let toml_str =
        toml::to_string_pretty(&view).map_err(|e| ConfigCliError::Serialize(e.to_string()))?;
    print!("{toml_str}");
    println!();
    println!(
        "# Effective index directory: {}",
        commandindex_dir.display()
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// config path
// ---------------------------------------------------------------------------

pub fn run_path(base_path: &Path, commandindex_dir: &Path) -> Result<(), ConfigCliError> {
    let config = load_config(base_path)?;

    if config.loaded_sources.is_empty() {
        println!("No config files loaded (using defaults).");
    } else {
        for source in &config.loaded_sources {
            let kind_label = match source.kind {
                ConfigSourceKind::Team => "[team]",
                ConfigSourceKind::Local => "[local]",
                ConfigSourceKind::Legacy => "[deprecated]",
            };
            println!("{} {}", kind_label, source.path.display());
        }
    }
    println!("Effective index directory: {}", commandindex_dir.display());
    Ok(())
}
