use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};

const STATE_FILE: &str = "state.json";
const CURRENT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug)]
pub enum StateError {
    Io(std::io::Error),
    Json(serde_json::Error),
    SchemaVersionMismatch { expected: u32, found: u32 },
}

impl fmt::Display for StateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StateError::Io(e) => write!(f, "IO error: {e}"),
            StateError::Json(e) => write!(f, "JSON error: {e}"),
            StateError::SchemaVersionMismatch { expected, found } => {
                write!(
                    f,
                    "Schema version mismatch: expected {expected}, found {found}"
                )
            }
        }
    }
}

impl std::error::Error for StateError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            StateError::Io(e) => Some(e),
            StateError::Json(e) => Some(e),
            StateError::SchemaVersionMismatch { .. } => None,
        }
    }
}

impl From<std::io::Error> for StateError {
    fn from(e: std::io::Error) -> Self {
        StateError::Io(e)
    }
}

impl From<serde_json::Error> for StateError {
    fn from(e: serde_json::Error) -> Self {
        StateError::Json(e)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IndexState {
    pub version: String,
    pub schema_version: u32,
    pub created_at: DateTime<Utc>,
    pub last_updated_at: DateTime<Utc>,
    pub total_files: u64,
    pub total_sections: u64,
    pub index_root: PathBuf,
    #[serde(default)]
    pub last_commit_hash: Option<String>,
}

impl IndexState {
    /// 新しいインデックス状態を作成する
    pub fn new(index_root: PathBuf) -> Self {
        let now = Utc::now();
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            schema_version: CURRENT_SCHEMA_VERSION,
            created_at: now,
            last_updated_at: now,
            total_files: 0,
            total_sections: 0,
            index_root,
            last_commit_hash: None,
        }
    }

    /// `.commandindex/` ディレクトリから state.json を読み込む
    pub fn load(commandindex_dir: &Path) -> Result<Self, StateError> {
        let path = commandindex_dir.join(STATE_FILE);
        let content = std::fs::read_to_string(&path)?;
        let state: Self = serde_json::from_str(&content)?;
        Ok(state)
    }

    /// `.commandindex/` ディレクトリに state.json を書き込む
    pub fn save(&self, commandindex_dir: &Path) -> Result<(), StateError> {
        std::fs::create_dir_all(commandindex_dir)?;
        let path = commandindex_dir.join(STATE_FILE);
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    /// スキーマバージョンの整合性をチェックする
    pub fn check_schema_version(&self) -> Result<(), StateError> {
        if self.schema_version != CURRENT_SCHEMA_VERSION {
            return Err(StateError::SchemaVersionMismatch {
                expected: CURRENT_SCHEMA_VERSION,
                found: self.schema_version,
            });
        }
        Ok(())
    }

    /// インデックスが作成済みかどうかを判定する
    pub fn exists(commandindex_dir: &Path) -> bool {
        commandindex_dir.join(STATE_FILE).exists()
    }

    /// 更新時刻を現在時刻に更新する
    pub fn touch(&mut self) {
        self.last_updated_at = Utc::now();
    }
}
