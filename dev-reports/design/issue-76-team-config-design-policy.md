# 設計方針書 - Issue #76: チーム共有設定ファイル（config.toml）

## 1. 概要

| 項目 | 内容 |
|------|------|
| Issue | #76 [Feature] チーム共有設定ファイル（config.toml） |
| 種別 | 新機能 |
| 影響範囲 | 中規模（config新設 + 既存6箇所の移行 + CLI追加） |
| 作成日 | 2026-03-22 |
| レビュー反映 | Stage 1-4 完了（設計原則・整合性・影響分析・セキュリティ） |

## 2. システムアーキテクチャ上の位置づけ

### 現状のアーキテクチャ

```
┌──────────┐
│  main.rs │  CLI エントリポイント (clap)
└────┬─────┘
     │
┌────┴─────────────────────────────────────────┐
│                   cli/                        │
│  search.rs  embed.rs  index.rs  clean.rs ...  │
└────┬──────────┬──────────┬───────────────────┘
     │          │          │
┌────┴────┐ ┌──┴───┐ ┌───┴────┐
│embedding│ │rerank│ │indexer │  ...
│ Config  │ │Config│ │        │
└─────────┘ └──────┘ └────────┘
```

**問題点**: 設定読み込みが `embedding::Config` に集約されており、embedding/rerank 以外の設定（search, index）を扱えない。

### 新アーキテクチャ

```
┌──────────┐
│  main.rs │  CLI エントリポイント (clap)
└────┬─────┘
     │
┌────┴─────────────────────────────────────────────┐
│                   cli/                            │
│  search.rs  embed.rs  index.rs  clean.rs config.rs│
└────┬──────────┬──────────┬───────────────────────┘
     │          │          │
     └──────────┴──────────┘
                │
     ┌──────────┴──────────┐
     │   config/mod.rs     │  ← 新規: 唯一の設定読込入口
     │  load_config()      │  ← ローダー関数（SRP: データと I/O を分離）
     │  AppConfig (データ)  │
     └──┬───┬───┬──────────┘
        │   │   │
     ┌──┘   │   └──────┐
     │      │          │
┌────┴────┐ ┌──┴───┐ ┌───┴────┐
│embedding│ │rerank│ │indexer │
│  Config │ │Config│ │        │
│ (型のみ)│ │(型のみ)│        │
└─────────┘ └──────┘ └────────┘
```

**設計原則**: config モジュールを唯一の設定読込入口とし、各 CLI コマンドは `load_config()` を1回だけ呼び出して `&AppConfig` を各関数に引き回す。

## 3. レイヤー構成と責務

| レイヤー | モジュール | 変更種別 | 責務 |
|---------|-----------|---------|------|
| **Config（新規）** | `src/config/mod.rs` | 新規作成 | 設定ファイルの読み込み・マージ・バリデーション |
| **CLI** | `src/main.rs` | 変更 | Commands enum に Config サブコマンド追加、検索引数を Option 化 |
| **CLI** | `src/cli/config.rs` | 新規作成 | config show / config path サブコマンド実装 |
| **CLI** | `src/cli/mod.rs` | 変更 | `pub mod config;` 追加 |
| **CLI** | `src/cli/search.rs` | 変更 | Config::load() → load_config() への移行（4箇所: L130, L291, L424, L650）、関数シグネチャに &AppConfig 追加 |
| **CLI** | `src/cli/embed.rs` | 変更 | Config::load() → load_config() への移行（1箇所: L110） |
| **CLI** | `src/cli/index.rs` | 変更 | Config::load() → load_config() への移行（1箇所: L795） |
| **CLI** | `src/cli/clean.rs` | 変更 | 保持対象ファイル名の更新（embeddings.db, config.toml, config.local.toml） |
| **Embedding** | `src/embedding/mod.rs` | 変更 | Config 構造体と load() を削除、ProviderType に Serialize 追加 |
| **Embedding** | `src/embedding/openai.rs` | 変更 | OpenAiProvider に Custom Debug 実装（api_key マスク） |
| **Rerank** | `src/rerank/mod.rs` | 変更 | RerankConfig に Serialize 追加、Custom Debug 実装（api_key マスク） |
| **Lib** | `src/lib.rs` | 変更 | `pub mod config;` 追加 |

## 4. 新規モジュール設計: `src/config/mod.rs`

### 4.1 定数定義

```rust
/// チーム共有設定ファイル（リポジトリルート）
pub const TEAM_CONFIG_FILE: &str = "commandindex.toml";
/// ローカル個人設定ファイル（.commandindex/ 配下）
pub const LOCAL_CONFIG_FILE: &str = "config.local.toml";
/// 旧設定ファイル（deprecated fallback）
pub const LEGACY_CONFIG_FILE: &str = "config.toml";
```

### 4.2 エラー型

**方針**: 既存プロジェクトのパターン（手動 Display + Error 実装）に合わせる。thiserror は導入しない。

```rust
#[derive(Debug)]
pub enum ConfigError {
    ReadError {
        path: PathBuf,
        source: std::io::Error,
    },
    ParseError {
        path: PathBuf,
        source: toml::de::Error,
    },
    SerializeError(toml::ser::Error),
    /// チーム共有設定に api_key が含まれている（セキュリティ違反）
    SecretInTeamConfig {
        path: PathBuf,
        field: String,
    },
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReadError { path, source } =>
                write!(f, "Failed to read config file '{}': {}", path.display(), source),
            Self::ParseError { path, source } =>
                write!(f, "Failed to parse config file '{}': {}", path.display(), source),
            Self::SerializeError(e) =>
                write!(f, "Failed to serialize config: {}", e),
            Self::SecretInTeamConfig { path, field } =>
                write!(f, "Security: '{}' contains '{}'. API keys must be in config.local.toml or environment variables.",
                    path.display(), field),
        }
    }
}

impl std::error::Error for ConfigError {}
```

**エラー型伝播**: 各 CLI モジュールのエラー型に `From<ConfigError>` を実装する。

```rust
// 例: cli/search.rs
impl From<ConfigError> for SearchError {
    fn from(e: ConfigError) -> Self {
        SearchError::Config(e.to_string())
    }
}
```

### 4.3 マージ用中間構造体（RawConfig）

```rust
/// TOML ファイルから読み込む中間構造体（全フィールド Option）
#[derive(Debug, Default, Deserialize)]
pub struct RawConfig {
    pub index: Option<RawIndexConfig>,
    pub search: Option<RawSearchConfig>,
    pub embedding: Option<RawEmbeddingConfig>,
    pub rerank: Option<RawRerankConfig>,
}

#[derive(Debug, Default, Deserialize)]
pub struct RawSearchConfig {
    pub default_limit: Option<usize>,
    pub snippet_lines: Option<usize>,
    pub snippet_chars: Option<usize>,
}

#[derive(Debug, Default, Deserialize)]
pub struct RawIndexConfig {
    pub languages: Option<Vec<String>>,
}

#[derive(Debug, Default, Deserialize)]
pub struct RawEmbeddingConfig {
    pub provider: Option<ProviderType>,
    pub model: Option<String>,
    pub endpoint: Option<String>,
    pub api_key: Option<String>,
}

/// RerankConfig のマージ用中間構造体
/// 注: provider フィールドは含まない（既存 RerankConfig に provider がなく、Ollama 固定）
#[derive(Debug, Default, Deserialize)]
pub struct RawRerankConfig {
    pub model: Option<String>,
    pub top_candidates: Option<usize>,
    pub endpoint: Option<String>,
    pub api_key: Option<String>,
    pub timeout_secs: Option<u64>,
}
```

**DRY 対策**: RawConfig と最終型のフィールド同期を保証するため、テストで全フィールドのラウンドトリップ検証を実装する。

### 4.4 最終設定構造体（AppConfig）

**注**: AppConfig 自体には `Serialize` を付与しない（api_key 露出防止）。表示は `to_masked_view()` 経由のみ。

```rust
/// マージ済みの最終設定（Serialize なし: 秘匿値保護）
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub index: IndexConfig,
    pub search: SearchConfig,
    pub embedding: EmbeddingConfig,   // embedding::EmbeddingConfig を再利用
    pub rerank: RerankConfig,          // rerank::RerankConfig を再利用
    /// 読み込まれた設定ファイルのパス情報
    pub loaded_sources: Vec<ConfigSource>,
}

#[derive(Debug, Clone)]
pub struct ConfigSource {
    pub path: PathBuf,
    pub kind: ConfigSourceKind,
}

#[derive(Debug, Clone)]
pub enum ConfigSourceKind {
    Team,       // commandindex.toml
    Local,      // .commandindex/config.local.toml
    Legacy,     // .commandindex/config.toml (deprecated)
}

#[derive(Debug, Clone, Serialize)]
pub struct IndexConfig {
    pub languages: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchConfig {
    pub default_limit: usize,   // デフォルト: 20
    pub snippet_lines: usize,   // デフォルト: 2
    pub snippet_chars: usize,   // デフォルト: 120
}
```

### 4.5 ローダー関数（SRP: データと I/O を分離）

```rust
/// 設定を読み込み、優先順位に従ってマージする（公開 API）
///
/// 優先順位: 環境変数 > config.local.toml > commandindex.toml > 旧config.toml > デフォルト
///
/// base_path の決定:
/// - --path を持つコマンド（index, update, embed, clean）: --path の値
/// - --path を持たないコマンド（search, config）: カレントディレクトリ "."
pub fn load_config(base_path: &Path) -> Result<AppConfig, ConfigError> {
    let mut sources = Vec::new();
    let mut merged = RawConfig::default();

    let legacy_path = base_path.join(".commandindex").join(LEGACY_CONFIG_FILE);
    let team_path = base_path.join(TEAM_CONFIG_FILE);
    let local_path = base_path.join(".commandindex").join(LOCAL_CONFIG_FILE);

    // 1. 旧設定ファイル（deprecated fallback）
    if legacy_path.exists() {
        if team_path.exists() {
            eprintln!("Warning: {} is ignored because {} exists.",
                legacy_path.display(), team_path.display());
        } else {
            let raw = read_toml(&legacy_path)?;
            merged = merge_raw(merged, raw);
            sources.push(ConfigSource { path: legacy_path, kind: ConfigSourceKind::Legacy });
            eprintln!("Warning: {} is deprecated. Please migrate to {}",
                legacy_path.display(), team_path.display());
        }
    }

    // 2. チーム共有設定（api_key バリデーション付き）
    if team_path.exists() {
        let raw = read_toml(&team_path)?;
        validate_no_secrets(&team_path, &raw)?;
        merged = merge_raw(merged, raw);
        sources.push(ConfigSource { path: team_path, kind: ConfigSourceKind::Team });
    }

    // 3. ローカル個人設定
    if local_path.exists() {
        let raw = read_toml(&local_path)?;
        merged = merge_raw(merged, raw);
        sources.push(ConfigSource { path: local_path, kind: ConfigSourceKind::Local });
    }

    // 4. RawConfig → AppConfig に変換（デフォルト値適用）
    // 環境変数は EmbeddingConfig::resolve_api_key() に委譲
    Ok(resolve_config(merged, sources))
}

/// チーム共有設定に api_key が含まれていないことを検証
fn validate_no_secrets(path: &Path, raw: &RawConfig) -> Result<(), ConfigError> {
    if let Some(ref emb) = raw.embedding {
        if emb.api_key.is_some() {
            return Err(ConfigError::SecretInTeamConfig {
                path: path.to_path_buf(),
                field: "embedding.api_key".to_string(),
            });
        }
    }
    if let Some(ref rer) = raw.rerank {
        if rer.api_key.is_some() {
            return Err(ConfigError::SecretInTeamConfig {
                path: path.to_path_buf(),
                field: "rerank.api_key".to_string(),
            });
        }
    }
    Ok(())
}

/// フィールドレベルマージ: higher が優先
fn merge_raw(base: RawConfig, higher: RawConfig) -> RawConfig {
    // 各フィールドで higher が Some なら higher、None なら base を採用
}

fn read_toml(path: &Path) -> Result<RawConfig, ConfigError> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| ConfigError::ReadError { path: path.to_path_buf(), source: e })?;
    toml::from_str(&content)
        .map_err(|e| ConfigError::ParseError { path: path.to_path_buf(), source: e })
}

fn resolve_config(raw: RawConfig, sources: Vec<ConfigSource>) -> AppConfig {
    // RawConfig の Option フィールドにデフォルト値を適用して AppConfig に変換
}
```

### 4.6 config show 用 view model

AppConfig からの変換のみ。`Serialize` は view model にのみ付与。

```rust
/// 秘匿値をマスクした表示用構造体
#[derive(Serialize)]
pub struct AppConfigView {
    pub index: IndexConfig,
    pub search: SearchConfig,
    pub embedding: EmbeddingConfigView,
    pub rerank: RerankConfigView,
}

#[derive(Serialize)]
pub struct EmbeddingConfigView {
    pub provider: String,    // ProviderType を文字列に変換
    pub model: String,
    pub endpoint: String,
    pub api_key: String,     // "***" or "(not set)"
}

#[derive(Serialize)]
pub struct RerankConfigView {
    pub model: String,
    pub top_candidates: usize,
    pub endpoint: String,
    pub api_key: String,     // "***" or "(not set)"
    pub timeout_secs: u64,
}

impl AppConfig {
    pub fn to_masked_view(&self) -> AppConfigView {
        // api_key を "***" にマスクした view model を生成
        // ProviderType は to_string() で文字列化
    }
}
```

## 5. CLI設計

### 5.1 Commands enum 変更

```rust
// src/main.rs
#[derive(Subcommand)]
enum Commands {
    // ... 既存コマンド ...
    /// 設定の表示・管理
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },
}

#[derive(Subcommand)]
enum ConfigCommands {
    /// 現在の有効設定を表示（秘匿値はマスク）
    Show,
    /// 読み込まれた設定ファイルのパスを表示
    Path,
}
```

### 5.2 検索引数の Option 化

```rust
// 変更前
#[arg(long, default_value_t = 20)]
limit: usize,

// 変更後
#[arg(long, help = "Maximum number of results (default: from config or 20)")]
limit: Option<usize>,
```

**値解決フロー**:
```rust
let config = load_config(base_path)?;
let limit = cli_limit.unwrap_or(config.search.default_limit);
```

### 5.3 search.rs の AppConfig 引き回し

```rust
// run() で1回だけロードし、各内部関数に &AppConfig を渡す
pub fn run(/* ... */) -> Result<(), SearchError> {
    let config = load_config(&base_path)?;
    // ...
    try_hybrid_search(/* ... */, &config)?;
    try_rerank(/* ... */, &config)?;
}

fn try_hybrid_search(/* ... */, config: &AppConfig) -> Result<(), SearchError> {
    // config.embedding, config.rerank を使用
}

fn try_rerank(/* ... */, config: &AppConfig) -> Result<(), SearchError> {
    // config.rerank を使用
}
```

## 6. 設定ファイル優先順位のフロー図

```
                    ┌─────────────┐
                    │ CLI引数     │  明示指定時のみ
                    └──────┬──────┘
                           │ (None なら次へ)
                    ┌──────┴──────┐
                    │ 環境変数    │  COMMANDINDEX_OPENAI_API_KEY
                    └──────┬──────┘
                           │ (None なら次へ)
                    ┌──────┴──────┐
                    │config.local │  .commandindex/config.local.toml
                    └──────┬──────┘
                           │ (None なら次へ)
                    ┌──────┴──────┐
                    │commandindex │  commandindex.toml (チーム共有)
                    │             │  ※ api_key 禁止（バリデーション）
                    └──────┬──────┘
                           │ (None なら次へ)
                    ┌──────┴──────┐
                    │旧config.toml│  .commandindex/config.toml (deprecated)
                    └──────┬──────┘
                           │ (None なら次へ)
                    ┌──────┴──────┐
                    │デフォルト値  │  ハードコード
                    └─────────────┘
```

## 7. 設計判断とトレードオフ

### 判断1: RawConfig（中間構造体）パターン

**選択**: 全フィールド `Option<T>` の `RawConfig` でファイルを読み込み、マージ後に `AppConfig` に変換
**理由**: フィールドレベルマージには「未指定」と「デフォルト値」を区別する必要がある
**トレードオフ**: 構造体が2重定義になるが、マージの正確性が保証される
**DRY 対策**: テストでフィールド同期のラウンドトリップ検証を実装

### 判断2: EmbeddingConfig/RerankConfig の配置

**選択**: 型定義は `embedding/mod.rs`, `rerank/mod.rs` に残す
**理由**: 循環依存を回避。各モジュールが自身の型を所有し、config モジュールがそれを参照
**トレードオフ**: config モジュールが embedding/rerank に依存するが、逆方向の依存は発生しない

### 判断3: deprecated fallback の維持

**選択**: 旧 `.commandindex/config.toml` を deprecated fallback として読み込み継続
**理由**: breaking change を回避し、既存ユーザーの動作を維持
**トレードオフ**: コード複雑性が増すが、移行期間中のユーザー体験を優先

### 判断4: config show の view model 分離

**選択**: `AppConfigView` を別途作成し、秘匿値をマスク。AppConfig 自体に Serialize は付与しない
**理由**: AppConfig に Serialize を付けると api_key が平文出力される潜在リスク
**トレードオフ**: View 構造体が増えるが、セキュリティが保証される

### 判断5: CLI引数の Option 化

**選択**: `--limit` 等を `Option<usize>` に変更
**理由**: clap の `default_value_t` では「ユーザーが明示指定したか」を判別できない
**対策**: help テキストにデフォルト値を明示して UX 劣化を最小化

### 判断6: ローダー関数の分離（SRP）

**選択**: `load_config()` を公開関数として分離し、`AppConfig` は純粋なデータ構造に
**理由**: SRP/OCP準拠。テスタビリティ向上（ファイルシステムに依存しないテストが可能）

### 判断7: チーム共有設定での api_key 禁止

**選択**: `commandindex.toml`（Git 管理対象）に api_key が含まれる場合はエラーとする
**理由**: Git にコミットされる設定ファイルに秘匿情報を含めるのはセキュリティリスク
**代替**: api_key は `config.local.toml`（.gitignore 対象）または環境変数のみに許可

### 判断8: RawRerankConfig に provider フィールドを含めない

**選択**: 既存 RerankConfig に provider フィールドがないため、Raw にも含めない
**理由**: 現在 Ollama 固定。YAGNI 原則に従い、不要なフィールドは追加しない

## 8. 影響範囲マトリクス

| ファイル | 変更内容 | リスク |
|---------|---------|--------|
| `src/config/mod.rs` | 新規作成 | 低（新規） |
| `src/cli/config.rs` | 新規作成 | 低（新規） |
| `src/cli/mod.rs` | `pub mod config;` 追加 | 低 |
| `src/lib.rs` | `pub mod config;` 追加 | 低 |
| `src/main.rs` | Commands enum 追加、検索引数 Option 化 | 中（既存動作変更） |
| `src/embedding/mod.rs` | Config 削除、ProviderType に Serialize 追加 | 高（6箇所の呼び出し影響） |
| `src/embedding/openai.rs` | Custom Debug 実装（api_key マスク） | 低 |
| `src/rerank/mod.rs` | Serialize 追加、Custom Debug 実装（api_key マスク） | 低 |
| `src/cli/search.rs` | load_config() 1回呼出 + &AppConfig 引き回し（関数シグネチャ変更） | 中 |
| `src/cli/embed.rs` | Config::load → load_config()（1箇所: L110） | 低 |
| `src/cli/index.rs` | Config::load → load_config()（1箇所: L795） | 低 |
| `src/cli/clean.rs` | 保持対象: embeddings.db, config.toml, config.local.toml | 中 |
| `tests/e2e_embedding.rs` | 設定ファイルパス更新 + legacy fallback テスト追加 | 中 |
| `tests/e2e_semantic_hybrid.rs` | 設定ファイルパス更新 | 中 |
| `tests/cli_args.rs` | config show/path テスト追加、help 出力に "config" 含む検証 | 低 |

## 9. セキュリティ設計

| 脅威 | 対策 | 優先度 |
|------|------|--------|
| チーム設定への API キー混入 | `validate_no_secrets()` でチーム設定の api_key を拒否 | **高** |
| config show での API キー平文表示 | view model 経由のマスク表示（`***`）。AppConfig に Serialize なし | 高 |
| RerankConfig Debug での api_key 露出 | Custom Debug impl でマスク（既存の EmbeddingConfig と同様） | 高 |
| OpenAiProvider Debug での api_key 露出 | Custom Debug impl でマスク | 高 |
| 設定ファイル経由の不正値注入 | TOML パースエラーで早期失敗 | 中 |
| パストラバーサル | base_path 基準のハードコードファイル名のみ使用 | 中 |
| 環境変数経由の秘匿情報漏洩 | resolve_api_key() の既存パターンを維持 | 低 |

## 10. テスト戦略

### 単体テスト（config モジュール内）
- `merge_raw()`: フィールドレベルマージの正確性
- `load_config()`: ファイル読み込み・優先順位・deprecated fallback
- `validate_no_secrets()`: チーム設定の api_key 拒否
- `to_masked_view()`: 秘匿値マスクの正確性
- `ConfigError`: エラー型の表示
- **DRY 検証**: RawConfig ↔ AppConfig のフィールド同期テスト

### 統合テスト
- E2E 3系統: 新設定のみ / ローカル上書き / レガシーfallback
- CLI引数優先順位: 未指定（ハードコードデフォルト） / 設定あり（設定値） / CLI明示（CLI優先）
- config show / config path サブコマンド
- clean --keep-embeddings 回帰テスト（config.local.toml 保持確認）
- 各コマンドのベースパス別設定検出テスト（--path 有無）
- cli_args.rs: help 出力に "config" サブコマンドが含まれること

### テスト移行計画
- e2e_embedding.rs: `.commandindex/config.toml` → `commandindex.toml`（リポジトリルート）に変更
- e2e_semantic_hybrid.rs: `create_test_config()` を `commandindex.toml` に変更
- legacy fallback テスト: 旧 `.commandindex/config.toml` のみ存在するケースを新規追加

## 11. 品質基準

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |
