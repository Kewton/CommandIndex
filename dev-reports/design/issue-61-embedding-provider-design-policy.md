# 設計方針書: Issue #61 Embedding生成基盤（ローカルLLM / API対応）

## 1. 概要

Phase 5 (Semantic Extension) の基盤として、ドキュメント・コードセクションのEmbedding（ベクトル表現）を生成する機能を構築する。

## 2. システムアーキテクチャ概要

### 現在のアーキテクチャ

```
CLI Layer (main.rs, cli/)
  ├── index: ファイルスキャン → パース → tantivy/SQLiteインデックス構築
  ├── update: 増分更新（diff検出 → 変更分のみ再インデックス）
  ├── search: tantivy全文検索 + シンボル検索 + 関連検索
  ├── clean: .commandindex/ 削除
  ├── status: インデックス状態表示
  └── context: AI用コンテキストパック生成

Parser Layer (parser/)
  ├── markdown.rs: Markdown → Section[] (heading/body/tags/level)
  ├── code.rs: ソースコード → SymbolInfo[] + ImportInfo[]
  ├── typescript.rs / python.rs: tree-sitter AST解析
  └── ignore.rs: .cmindexignore フィルタ

Indexer Layer (indexer/)
  ├── writer.rs: SectionDoc → tantivy ドキュメント書き込み
  ├── reader.rs: tantivy 検索クエリ実行
  ├── schema.rs: IndexSchema (path/heading/body/tags/heading_level/line_start)
  ├── symbol_store.rs: SQLite symbols/dependencies/file_links
  ├── manifest.rs: FileEntry[] (path/hash/last_modified/sections/file_type)
  ├── state.rs: IndexState (version/schema_version/timestamps/counts)
  └── diff.rs: ファイル変更検出

Search Layer (search/)
  └── related.rs: 関連ファイル検索（リンク/import/タグ/パス類似度）

Output Layer (output/)
  └── mod.rs: Human/JSON/Path フォーマット出力
```

### 追加するアーキテクチャ

```
Embedding Layer (embedding/) ← 新規
  ├── mod.rs: EmbeddingProvider trait, EmbeddingError, EmbeddingConfig
  ├── ollama.rs: OllamaProvider (reqwest::blocking → localhost:11434)
  ├── openai.rs: OpenAiProvider (reqwest::blocking → api.openai.com)
  └── store.rs: EmbeddingStore (SQLite embeddings.db)

CLI拡張
  ├── embed.rs: commandindex embed サブコマンド（新規）
  ├── index.rs: --with-embedding オプション追加（変更）
  └── clean.rs: --keep-embeddings オプション追加（変更）
```

## 3. レイヤー構成と責務

| レイヤー | モジュール | 責務 |
|---------|-----------|------|
| **CLI** | `src/main.rs` | clapサブコマンド定義、Commands::Embed追加 |
| **CLI** | `src/cli/embed.rs` | embedサブコマンド実行（新規） |
| **CLI** | `src/cli/index.rs` | `--with-embedding` 対応（変更） |
| **CLI** | `src/cli/clean.rs` | `--keep-embeddings` 対応（変更） |
| **Embedding** | `src/embedding/mod.rs` | EmbeddingProviderトレイト、EmbeddingError、EmbeddingConfig |
| **Embedding** | `src/embedding/ollama.rs` | Ollama APIクライアント |
| **Embedding** | `src/embedding/openai.rs` | OpenAI APIクライアント |
| **Embedding** | `src/embedding/store.rs` | SQLite embeddings.db CRUD操作 |
| **Indexer** | `src/indexer/mod.rs` | embeddings_db_path() 追加（変更） |

## 4. 技術選定

| カテゴリ | 選定技術 | 選定理由 |
|---------|---------|---------|
| HTTPクライアント | reqwest 0.12 (blocking) | デファクトスタンダード、現行コードベースが同期のためblocking使用 |
| 設定ファイルパース | toml 0.8 | TOML形式のデファクト、serde連携 |
| ベクトル格納 | SQLite (rusqlite) | 既存依存、独立DB(embeddings.db)で管理 |
| シリアライズ | serde 1 (既存) | config.tomlのデシリアライズ |

### 依存追加

```toml
# Cargo.toml に追加
reqwest = { version = "0.12", features = ["blocking", "json"] }
toml = "0.8"
```

※ `serde`, `rusqlite` は既存依存のため追加不要

## 5. 設計パターン

### 5.1 EmbeddingProvider トレイト

```rust
/// Embedding生成プロバイダーのトレイト
pub trait EmbeddingProvider: Send + Sync {
    /// テキスト群のEmbeddingを生成
    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError>;
    /// Embeddingの次元数を返す（初回embed時にレスポンスから取得してキャッシュ）
    fn dimension(&self) -> usize;
    /// プロバイダー名を返す
    fn provider_name(&self) -> &str;
    /// モデル名を返す
    fn model_name(&self) -> &str;
}
```

> **設計判断**: `dimension()` はモデルに依存する値のため、ハードコードしない。
> 各プロバイダーはモデル名→次元数のマッピングテーブルを内部に持ち、
> 未知モデルの場合は初回 `embed()` 呼出し時にレスポンスのベクトル長から自動検出する。
> `dimension(&self)` は不変参照のため、キャッシュには `std::cell::OnceCell<usize>` を使用する
> （interior mutability パターン）。Send + Sync が必要なため `std::sync::OnceLock<usize>` を使用。

### 5.2 エラー型設計（単一責任分離）

**EmbeddingError**: Embedding生成の責務のみ
```rust
#[derive(Debug)]
pub enum EmbeddingError {
    /// HTTP接続失敗
    NetworkError(String),
    /// API応答エラー（ステータスコード + メッセージ）
    ApiError { status: u16, message: String },
    /// モデルが見つからない
    ModelNotFound(String),
    /// レスポンスのパースに失敗
    InvalidResponse(String),
    /// レート制限
    RateLimited,
    /// タイムアウト
    Timeout,
    /// 設定エラー
    ConfigError(String),
}

impl std::fmt::Display for EmbeddingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NetworkError(msg) => write!(f, "Network error: {}", msg),
            Self::ApiError { status, message } => write!(f, "API error ({}): {}", status, message),
            Self::ModelNotFound(model) => write!(f, "Model not found: {}", model),
            Self::InvalidResponse(msg) => write!(f, "Invalid response: {}", msg),
            Self::RateLimited => write!(f, "Rate limited"),
            Self::Timeout => write!(f, "Request timeout"),
            Self::ConfigError(msg) => write!(f, "Config error: {}", msg),
        }
    }
}
impl std::error::Error for EmbeddingError {}
```

**EmbeddingStoreError**: SQLite格納の責務（SymbolStoreErrorパターンに準拠）
```rust
#[derive(Debug)]
pub enum EmbeddingStoreError {
    Sqlite(rusqlite::Error),
    Io(std::io::Error),
    SchemaVersionMismatch { expected: u32, found: u32 },
}

impl From<rusqlite::Error> for EmbeddingStoreError {
    fn from(err: rusqlite::Error) -> Self { Self::Sqlite(err) }
}
impl From<std::io::Error> for EmbeddingStoreError {
    fn from(err: std::io::Error) -> Self { Self::Io(err) }
}
impl std::fmt::Display for EmbeddingStoreError { ... }
impl std::error::Error for EmbeddingStoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Sqlite(e) => Some(e),
            Self::Io(e) => Some(e),
            Self::SchemaVersionMismatch { .. } => None,
        }
    }
}
```

### 5.3 EmbeddingConfig（設定管理）

```rust
/// プロバイダー種別（型安全）
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ProviderType {
    Ollama,
    OpenAi,
}

impl Default for ProviderType {
    fn default() -> Self { Self::Ollama }
}

#[derive(Debug, Clone, Deserialize)]
pub struct EmbeddingConfig {
    #[serde(default)]
    pub provider: ProviderType,
    #[serde(default = "default_model")]
    pub model: String,             // "nomic-embed-text" | "text-embedding-3-small"
    #[serde(default = "default_endpoint")]
    pub endpoint: String,          // "http://localhost:11434"
    pub api_key: Option<String>,   // OpenAI API key（環境変数 COMMANDINDEX_OPENAI_API_KEY 優先）
}

// Config は src/config.rs に独立配置（将来の拡張性を考慮）
// embedding/mod.rs には EmbeddingConfig のみ配置
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub embedding: Option<EmbeddingConfig>,
}

impl Config {
    /// .commandindex/config.toml を読み込む。存在しない場合はNone
    pub fn load(commandindex_dir: &Path) -> Result<Option<Self>, EmbeddingError>;
}

// api_key 解決順序: 環境変数 > config.toml
impl EmbeddingConfig {
    pub fn resolve_api_key(&self) -> Option<String> {
        std::env::var("COMMANDINDEX_OPENAI_API_KEY").ok()
            .or_else(|| self.api_key.clone())
    }
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            provider: ProviderType::Ollama,
            model: "nomic-embed-text".to_string(),
            endpoint: "http://localhost:11434".to_string(),
            api_key: None,
        }
    }
}

// カスタムDebug実装（api_keyマスキング）
impl std::fmt::Debug for EmbeddingConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmbeddingConfig")
            .field("provider", &self.provider)
            .field("model", &self.model)
            .field("endpoint", &self.endpoint)
            .field("api_key", &self.api_key.as_ref().map(|_| "***"))
            .finish()
    }
}
```

### 5.4 OllamaProvider

```rust
pub struct OllamaProvider {
    model: String,
    endpoint: String,
    client: reqwest::blocking::Client,
}

impl OllamaProvider {
    pub fn new(model: &str, endpoint: &str) -> Self;
    /// config から生成
    pub fn from_config(config: &EmbeddingConfig) -> Self;
}

impl EmbeddingProvider for OllamaProvider {
    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        // POST {endpoint}/api/embed
        // Body: { "model": "nomic-embed-text", "input": texts }
        // Response: { "embeddings": [[f32; 768], ...] }
        // バッチサイズ: 10テキスト/リクエスト
    }
    fn dimension(&self) -> usize {
        // モデル名→次元数マッピング: {"nomic-embed-text": 768, ...}
        // 未知モデルは cached_dimension から取得（初回embed時にキャッシュ）
    }
    fn provider_name(&self) -> &str { "ollama" }
    fn model_name(&self) -> &str { &self.model }
}
```

### 5.5 OpenAiProvider

```rust
pub struct OpenAiProvider {
    api_key: String,
    model: String,
    endpoint: String,    // config.endpoint を使用（Azure OpenAI等にも対応）
    client: reqwest::blocking::Client,
}

impl OpenAiProvider {
    pub fn new(api_key: &str, model: &str, endpoint: &str) -> Self;
    pub fn from_config(config: &EmbeddingConfig) -> Result<Self, EmbeddingError>;
}

impl EmbeddingProvider for OpenAiProvider {
    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        // POST {endpoint}/v1/embeddings  ← config.endpoint を使用
        // Headers: Authorization: Bearer {api_key}
        // Body: { "model": "text-embedding-3-small", "input": texts }
        // Response: { "data": [{ "embedding": [f32; 1536] }] }
        // バッチサイズ: 100テキスト/リクエスト
    }
    fn dimension(&self) -> usize {
        // モデル名→次元数マッピング: {"text-embedding-3-small": 1536, "text-embedding-3-large": 3072, ...}
        // 未知モデルは cached_dimension から取得
    }
    fn provider_name(&self) -> &str { "openai" }
    fn model_name(&self) -> &str { &self.model }
}
```

### 5.6 EmbeddingStore（SQLite）

```rust
pub struct EmbeddingStore {
    conn: rusqlite::Connection,
}

impl EmbeddingStore {
    pub fn open(db_path: &Path) -> Result<Self, EmbeddingStoreError>;
    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self, EmbeddingStoreError>;
    pub fn create_tables(&self) -> Result<(), EmbeddingStoreError>;

    /// Embedding を保存
    pub fn upsert_embedding(
        &self,
        section_path: &str,
        section_heading: &str,
        embedding: &[f32],
        dimension: usize,
        model: &str,
        file_hash: &str,
    ) -> Result<(), EmbeddingStoreError>;

    /// ファイルパスでembeddingを検索
    pub fn find_by_path(&self, path: &str) -> Result<Vec<EmbeddingRecord>, EmbeddingStoreError>;

    /// ファイルハッシュでキャッシュチェック
    pub fn has_current_embedding(&self, path: &str, file_hash: &str) -> Result<bool, EmbeddingStoreError>;

    /// ファイルのembeddingを削除
    pub fn delete_by_path(&self, path: &str) -> Result<(), EmbeddingStoreError>;

    /// 全embedding数を取得
    pub fn count(&self) -> Result<u64, EmbeddingStoreError>;
}
```

**テーブル定義:**
```sql
CREATE TABLE IF NOT EXISTS schema_meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS embeddings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    section_path TEXT NOT NULL,
    section_heading TEXT NOT NULL,
    embedding BLOB NOT NULL,
    dimension INTEGER NOT NULL,
    model TEXT NOT NULL,
    file_hash TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_embeddings_unique ON embeddings(section_path, section_heading, model);
CREATE INDEX IF NOT EXISTS idx_embeddings_path ON embeddings(section_path);
CREATE INDEX IF NOT EXISTS idx_embeddings_hash ON embeddings(section_path, file_hash);
```

### 5.7 プロバイダーファクトリ

```rust
/// 設定からプロバイダーを生成（ProviderType enumで型安全にマッチ）
pub fn create_provider(config: &EmbeddingConfig) -> Result<Box<dyn EmbeddingProvider>, EmbeddingError> {
    match &config.provider {
        ProviderType::Ollama => Ok(Box::new(OllamaProvider::from_config(config))),
        ProviderType::OpenAi => Ok(Box::new(OpenAiProvider::from_config(config)?)),
    }
}
```

## 6. CLI設計

### 6.1 Commands enum 変更

```rust
#[derive(Subcommand)]
enum Commands {
    // 既存...
    Index {
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Generate embeddings during indexing
        #[arg(long)]
        with_embedding: bool,
    },
    Update {
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Generate embeddings during update
        #[arg(long)]
        with_embedding: bool,
    },
    Clean {
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Keep embeddings database when cleaning
        #[arg(long)]
        keep_embeddings: bool,
    },
    /// Generate embeddings for indexed sections
    Embed {
        #[arg(long, default_value = ".")]
        path: PathBuf,
    },
    // 既存のSearch, Status, Context...
}
```

### 6.2 cli/embed.rs

```rust
pub struct EmbedSummary {
    pub total_sections: u64,
    pub generated: u64,
    pub cached: u64,
    pub failed: u64,
    pub duration: Duration,
}

pub fn run(path: &Path) -> Result<EmbedSummary, EmbedError>;

#[derive(Debug)]
pub enum EmbedError {
    IndexNotFound,
    Embedding(EmbeddingError),
    Store(EmbeddingStoreError),
    Manifest(ManifestError),
    Reader(ReaderError),
    Io(std::io::Error),
}

impl std::fmt::Display for EmbedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IndexNotFound => write!(f, "Index not found"),
            Self::Embedding(e) => write!(f, "Embedding error: {}", e),
            Self::Store(e) => write!(f, "Store error: {}", e),
            Self::Manifest(e) => write!(f, "Manifest error: {:?}", e),
            Self::Reader(e) => write!(f, "Reader error: {:?}", e),
            Self::Io(e) => write!(f, "IO error: {}", e),
        }
    }
}
impl std::error::Error for EmbedError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> { ... }
}
impl From<EmbeddingError> for EmbedError { ... }
impl From<EmbeddingStoreError> for EmbedError { ... }
impl From<ManifestError> for EmbedError { ... }
impl From<ReaderError> for EmbedError { ... }
impl From<std::io::Error> for EmbedError { ... }
```

### 6.3 index.rs シグネチャ変更（Optionsパターン）

```rust
/// インデックスオプション（Default実装で後方互換性を維持）
#[derive(Debug, Default)]
pub struct IndexOptions {
    pub with_embedding: bool,
}

// Before:
pub fn run(path: &Path) -> Result<IndexSummary, IndexError>
pub fn run_incremental(path: &Path) -> Result<IncrementalSummary, IndexError>

// After:
pub fn run(path: &Path, options: &IndexOptions) -> Result<IndexSummary, IndexError>
pub fn run_incremental(path: &Path, options: &IndexOptions) -> Result<IncrementalSummary, IndexError>
```

IndexError に追加:
```rust
pub enum IndexError {
    // 既存...
    Embedding(EmbeddingError),
    EmbeddingStore(EmbeddingStoreError),
}
impl From<EmbeddingError> for IndexError { ... }
impl From<EmbeddingStoreError> for IndexError { ... }
```

### 6.4 clean.rs シグネチャ変更（Optionsパターン）

```rust
/// クリーンオプション（Default実装で後方互換性を維持）
#[derive(Debug, Default)]
pub struct CleanOptions {
    pub keep_embeddings: bool,
}

// Before:
pub fn run(path: &Path) -> Result<CleanResult, CleanError>

// After:
pub fn run(path: &Path, options: &CleanOptions) -> Result<CleanResult, CleanError>
```

`keep_embeddings: true` の場合:
1. `.commandindex/tantivy/` を削除
2. `.commandindex/manifest.json` を削除
3. `.commandindex/state.json` を削除
4. `.commandindex/symbols.db` を削除
5. `.commandindex/embeddings.db` は保持
6. `.commandindex/config.toml` も保持（embedding設定が失われないように）
7. main.rs 出力メッセージ: "Removed index (embeddings and config preserved)"

### 6.5 main.rs の match アーム変更例

```rust
Commands::Index { path, with_embedding } => {
    let options = IndexOptions { with_embedding };
    match cli::index::run(&path, &options) { ... }
}
Commands::Update { path, with_embedding } => {
    let options = IndexOptions { with_embedding };
    match cli::index::run_incremental(&path, &options) { ... }
}
Commands::Clean { path, keep_embeddings } => {
    let options = CleanOptions { keep_embeddings };
    match cli::clean::run(&path, &options) {
        Ok(CleanResult::Removed) => {
            if keep_embeddings {
                println!("Removed index (embeddings preserved)");
            } else {
                println!("Removed index");
            }
        }
        ...
    }
}
Commands::Embed { path } => {
    match cli::embed::run(&path) { ... }
}
```

## 7. データフロー

### 7.1 embed コマンドのフロー

```
1. .commandindex/ 存在チェック
2. config.toml 読み込み → EmbeddingConfig
3. EmbeddingProvider 生成（create_provider）
4. Manifest 読み込み → FileEntry[]
5. EmbeddingStore オープン
6. 各 FileEntry について:
   a. has_current_embedding(path, hash) でキャッシュチェック
   b. キャッシュヒット → スキップ
   c. キャッシュミス → tantivy Reader でセクション取得
   d. provider.embed(texts) でベクトル生成
   e. store.upsert_embedding() で保存
7. サマリー出力
```

### 7.2 index --with-embedding のフロー

```
1. 通常の index フロー（ファイルスキャン → パース → tantivy書き込み）
2. with_embedding == true の場合:
   a. config.toml 読み込み
   b. EmbeddingProvider 生成
   c. EmbeddingStore オープン
   d. インデックス済みセクションに対してembedding生成
   e. store に保存
```

## 8. セキュリティ設計

| 脅威 | 対策 | 優先度 |
|------|------|--------|
| APIキー漏洩 | config.toml の api_key はファイルシステム上のみ管理。git管理対象外（.gitignore推奨）。ログ出力時にマスキング | 高 |
| パストラバーサル | embeddings.db パスは commandindex_dir() からの相対パスで固定 | 高 |
| SSRF | エンドポイントURLバリデーション: スキームhttp/https限定、Ollamaはデフォルトlocalhostのみ（外部ホスト指定時は警告）。url crateでパース安全性確保 | 高 |
| unsafe使用 | 原則禁止 | 高 |
| 大量リクエスト | バッチサイズ制限（Ollama: 10, OpenAI: 100）。タイムアウト: connect 10秒、request 30秒 | 中 |
| テキスト入力サイズ | embed()への入力テキスト最大長: Ollama 8192文字、OpenAI 32000文字。超過時はトランケート | 中 |
| ファイルパーミッション | config.toml作成時に0600パーミッションを設定（api_key保護） | 中 |

## 9. 設計判断とトレードオフ

### 判断1: SQLite vs tantivy拡張

- **決定**: SQLite（embeddings.db）に格納
- **理由**: tantivy 0.25 はネイティブベクトル検索に非対応。SQLiteは既存依存で追加コスト最小
- **トレードオフ**: ANN検索はSQLiteでは非効率。Phase 5でhnswlib等の専用ライブラリ導入を検討

### 判断2: 同期 vs 非同期HTTP

- **決定**: reqwest::blocking（同期）
- **理由**: 現行コードベースが完全同期。tokio導入はmain関数変更・依存追加が大きい
- **トレードオフ**: 並列リクエスト不可。大量セクションのembedding生成で性能制限あり

### 判断3: embeddings.db を独立ファイル

- **決定**: symbols.db とは別の embeddings.db
- **理由**: symbols.db のスキーマバージョン変更を回避し、既存インデックスとの互換性を維持。cleanコマンドでの選択的削除が容易
- **トレードオフ**: ファイル数増加（.commandindex/ 内に6種のファイル/ディレクトリ）

### 判断4: ファイルハッシュベースのキャッシュ

- **決定**: 既存のmanifest.jsonのsha256ハッシュを再利用
- **理由**: 実装コスト最小。compute_file_hash() が既存
- **トレードオフ**: 1セクション変更で全セクションのembeddingを再生成。将来セクション単位キャッシュで最適化可能

### 判断5: config.toml の配置

- **決定**: `.commandindex/config.toml`
- **理由**: プロジェクトローカル設定。既存の .commandindex/ 内に統一
- **トレードオフ**: グローバル設定（~/.config/commandindex/config.toml）は未対応。必要になれば別Issueで対応

## 10. 影響範囲

### 直接変更ファイル

| ファイル | 変更内容 | 影響度 |
|---------|---------|--------|
| `Cargo.toml` | reqwest, toml 依存追加 | 低 |
| `src/main.rs` | Commands::Embed追加、Index/Update/Clean引数追加 | 中 |
| `src/lib.rs` | `pub mod embedding;` 追加 | 低 |
| `src/cli/mod.rs` | `pub mod embed;` 追加 | 低 |
| `src/cli/index.rs` | `run(path, with_embedding)` シグネチャ変更 | 中 |
| `src/cli/clean.rs` | `run(path, keep_embeddings)` シグネチャ変更、選択的削除 | 中 |
| `src/indexer/mod.rs` | `embeddings_db_path()` 追加 | 低 |

### 新規作成ファイル

| ファイル | 内容 |
|---------|------|
| `src/embedding/mod.rs` | EmbeddingProvider trait, EmbeddingError, EmbeddingConfig, create_provider |
| `src/embedding/ollama.rs` | OllamaProvider |
| `src/embedding/openai.rs` | OpenAiProvider |
| `src/embedding/store.rs` | EmbeddingStore (SQLite) |
| `src/cli/embed.rs` | embed サブコマンド |

### 既存テストへの影響

- `run()` / `run_incremental()` のシグネチャ変更により、直接呼び出しているテストは引数追加が必要
- `--with-embedding` のデフォルト `false` により、CLIレベルのテストは影響なし
- clean のシグネチャ変更により `cli/clean.rs` 内のテストは引数追加が必要

## 11. 品質基準

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |
| 既存テスト | 21テストファイル | 全テストパス（後方互換性） |

## 12. .commandindex/ ディレクトリ構成（変更後）

```
.commandindex/
├── tantivy/          # tantivy インデックス（既存）
├── manifest.json     # ファイル一覧・ハッシュ（既存）
├── state.json        # インデックスメタ情報（既存）
├── symbols.db        # シンボル/依存関係 SQLite（既存）
├── config.toml       # 設定ファイル（新規）
└── embeddings.db     # Embeddingベクトル SQLite（新規）
```
