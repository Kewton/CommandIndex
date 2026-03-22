# 設計方針書: Issue #88 --index-path オプション（インデックスパス指定）

## 1. 概要

`--index-path` グローバルオプションを追加し、任意のパスにあるインデックスを参照可能にする。
複数 worktree が共通のインデックスを共有するシナリオ（並列エージェントの共有知識）を実現する。

## 2. システムアーキテクチャ概要

### レイヤー構成と本Issue の影響

```
┌──────────────────────────────────────────────────────────┐
│  CLI Layer (src/main.rs)                                  │
│  ┌─ Cli struct ──────────────────────────────────────┐   │
│  │  --index-path: Option<PathBuf>  ← 【新規追加】      │   │
│  │  command: Commands                                 │   │
│  └────────────────────────────────────────────────────┘   │
│  【変更】各サブコマンド実行時に index_path を解決・伝搬     │
├──────────────────────────────────────────────────────────┤
│  Config Layer (src/config/mod.rs)                         │
│  【変更】load_config(base_path) は維持（循環依存回避）      │
│  【変更】RawIndexConfig に path フィールド追加              │
│  【変更】merge_index に path フィールドマージ追加           │
├──────────────────────────────────────────────────────────┤
│  Indexer Layer (src/indexer/mod.rs)                        │
│  【変更】ヘルパー関数が commandindex_dir を直接受け取る     │
│  【削除】COMMANDINDEX_DIR 定数 → lib.rs に統一（非pub確認済）│
│  【新設】resolve_index_path 関数（Result 型）              │
├──────────────────────────────────────────────────────────┤
│  CLI Subcommands (src/cli/*.rs)                           │
│  【変更】各 run 関数に commandindex_dir: &Path を追加      │
│  【変更】SearchContext に commandindex_dir フィールドを追加 │
│  【修正】Path::new(".") ハードコード箇所の解消（12箇所以上）│
├──────────────────────────────────────────────────────────┤
│  Parser Layer (src/parser/ignore.rs)                      │
│  【変更】IgnoreFilter にパターンリスト保持 + 動的除外       │
└──────────────────────────────────────────────────────────┘
```

## 3. 設計判断とトレードオフ

### 判断 1: グローバルオプション vs サブコマンド個別オプション

**決定**: `Cli` 構造体にグローバルオプションとして追加

```rust
#[derive(Parser)]
#[command(name = "commandindex")]
struct Cli {
    /// Custom index directory path (overrides default .commandindex/)
    #[arg(long, global = true)]
    index_path: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}
```

**理由**:
- 全サブコマンドで一貫した挙動を保証
- 各サブコマンドへの個別追加は冗長でメンテナンスコスト増
- clap の `global = true` で自然にサポート

**トレードオフ**:
- `config` のようにインデックスを直接操作しないコマンドにもオプションが見える
- → ヘルプメッセージで用途を明確にすることで対処

### 判断 2: インデックスパス解決の中央集約（2段階解決）

**決定**: `resolve_index_path` 関数を新設し、パス解決ロジックを一元管理。
**循環依存の回避**: config 読み込みとパス解決を2段階に分離する。

**データフロー**:
```
1. load_config(base_path) でデフォルト位置から config を読み込み
2. config の [index].path と CLI の --index-path を使って resolve_index_path で最終パスを決定
```
> **Note**: local/legacy config は常に `{base_path}/.commandindex/` から読み込む（判断10参照）。
> 共有インデックスパスからの config 読み込みは行わない（機密情報保護のため）。

```rust
// src/indexer/mod.rs に追加

/// インデックスパスを解決する
/// 優先順位: CLI --index-path > config [index].path > デフォルト {base_path}/.commandindex/
pub fn resolve_index_path(
    cli_index_path: Option<&Path>,
    config_index_path: Option<&str>,
    base_path: &Path,
) -> Result<PathBuf, ResolveIndexPathError> {
    let raw_path = if let Some(cli_path) = cli_index_path {
        // CLI 指定: cwd 基準で解決
        if cli_path.is_relative() {
            std::env::current_dir()
                .map_err(|e| ResolveIndexPathError::CurrentDirUnavailable(e))?
                .join(cli_path)
        } else {
            cli_path.to_path_buf()
        }
    } else if let Some(config_path) = config_index_path {
        // config 指定: リポジトリルート基準で解決
        let p = Path::new(config_path);
        if p.is_relative() {
            base_path.join(p)
        } else {
            p.to_path_buf()
        }
    } else {
        // デフォルト
        base_path.join(crate::INDEX_DIR_NAME)
    };

    // パストラバーサル検出: ".." を含む未作成パスは明示的エラー
    if !raw_path.exists() && raw_path.components().any(|c| c == std::path::Component::ParentDir) {
        return Err(ResolveIndexPathError::PathTraversal(raw_path));
    }

    // パス正規化: 存在するパスは canonicalize
    if raw_path.exists() {
        std::fs::canonicalize(&raw_path)
            .map_err(|e| ResolveIndexPathError::CanonicalizeFailed(raw_path, e))
    } else {
        Ok(raw_path)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ResolveIndexPathError {
    #[error("Cannot determine current directory: {0}")]
    CurrentDirUnavailable(std::io::Error),
    #[error("Cannot canonicalize path {0}: {1}")]
    CanonicalizeFailed(PathBuf, std::io::Error),
    #[error("Path traversal detected in index path: {0}")]
    PathTraversal(PathBuf),
    #[error("Symlink detected at index path: {0}")]
    SymlinkDetected(PathBuf),
}
```

**理由**:
- パス解決ロジックが散在するとバグの温床
- 優先順位の変更が1箇所で済む
- Result 型で current_dir() 失敗を明示的に伝搬（unwrap_or_default は使わない）
- canonicalize でパストラバーサルを防止

### 判断 3: ヘルパー関数のシグネチャ変更

**決定**: `base_path` ベースから `commandindex_dir` 直接受け取りに変更

```rust
// Before (現在)
pub fn index_dir(base_path: &Path) -> PathBuf {
    base_path.join(COMMANDINDEX_DIR).join(TANTIVY_DIR)
}

// After (変更後)
pub fn index_dir(commandindex_dir: &Path) -> PathBuf {
    commandindex_dir.join(TANTIVY_DIR)
}

pub fn symbol_db_path(commandindex_dir: &Path) -> PathBuf {
    commandindex_dir.join(SYMBOLS_DB_FILE)
}

pub fn embeddings_db_path(commandindex_dir: &Path) -> PathBuf {
    commandindex_dir.join(EMBEDDINGS_DB_FILE)
}
```

**影響箇所（全28箇所以上）**:
- `src/cli/index.rs`: 6箇所以上（run, run_incremental, generate_embeddings_for_manifest）
- `src/cli/search.rs`: 9箇所（SearchContext + run_symbol/related/semantic_search）
- `src/cli/status/mod.rs`: 6箇所（run, get_symbol_count, get_embedding_file_count, compute_storage_breakdown, run_verify）
- `src/cli/embed.rs`: 3箇所
- `src/cli/context.rs`: 2箇所
- `src/cli/export.rs`: 1箇所
- `src/cli/import_index.rs`: 1箇所
- `tests/e2e_phase3_integration.rs`: 1箇所

**トレードオフ**:
- 影響範囲大だがコンパイラが未修正箇所を検出してくれる

### 判断 4: SearchContext の拡張

**決定**: `commandindex_dir` フィールドを追加（旧名 `index_path_override` → `commandindex_dir` に統一）

```rust
pub struct SearchContext {
    pub base_path: PathBuf,
    pub commandindex_dir: PathBuf,  // 解決済みインデックスパス
    pub config: AppConfig,
}

impl SearchContext {
    /// 新コンストラクタ（from_current_dir / from_path を置き換え）
    pub fn new(base_path: &Path, index_path: Option<&Path>) -> Result<Self, SearchError> {
        let config = load_config(base_path)?;
        let commandindex_dir = resolve_index_path(
            index_path,
            config.index.path.as_deref(),  // raw String → &str
            base_path,
        )?;
        Ok(Self {
            base_path: base_path.to_path_buf(),
            commandindex_dir,
            config,
        })
    }

    pub fn index_dir(&self) -> PathBuf {
        crate::indexer::index_dir(&self.commandindex_dir)
    }

    pub fn symbol_db_path(&self) -> PathBuf {
        crate::indexer::symbol_db_path(&self.commandindex_dir)
    }

    pub fn embeddings_db_path(&self) -> PathBuf {
        crate::indexer::embeddings_db_path(&self.commandindex_dir)
    }
}
```

**移行ガイド**:
- `SearchContext::from_current_dir()` → `SearchContext::new(Path::new("."), cli.index_path.as_deref())`
- `SearchContext::from_path(p)` → `SearchContext::new(p, None)` （workspace 内）

**run_symbol_search / run_related_search / run_semantic_search の修正**:
各関数に `ctx: &SearchContext` パラメータを追加し、内部の `Path::new(".")` を `ctx.commandindex_dir` 経由のパスに置き換える。

### 判断 5: load_config のシグネチャ（現行維持）

**決定**: 循環依存を回避するため、`load_config(base_path)` のシグネチャは現行維持。
config 内の `[index].path` は `load_config` の戻り値から取得し、`resolve_index_path` に渡す。

**重要方針**: `load_config` は常に `{base_path}/.commandindex/` から team/local/legacy config を読む。
共有インデックスパス（`--index-path`）からの config 読み込みは**行わない**（判断10: 機密情報保護）。

```rust
// load_config のシグネチャは変更しない
pub fn load_config(base_path: &Path) -> Result<AppConfig, ConfigError> {
    // 既存実装のまま（base_path/.commandindex/ から local/legacy を読む）
}
```

**AppConfig の値はすべて raw 設定値**:
- `AppConfig.index.path` → raw config value（`Option<String>`、未解決）
- effective index path → `resolve_index_path()` の戻り値として別変数で管理
- `config show` → raw 設定値を表示 + effective index path を追加表示
- `config path` → 設定ファイル探索結果 + effective index dir を追加表示

### 判断 6: RawIndexConfig への path フィールド追加

```rust
#[derive(Debug, Default, Deserialize)]
pub struct RawIndexConfig {
    pub path: Option<String>,       // ← 新規追加
    pub languages: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IndexConfig {
    /// Raw config value (未解決の設定値)。effective path は resolve_index_path() で別途解決する。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,       // ← 新規追加（raw 値として String で保持）
    pub languages: Vec<String>,
}
```

**merge_index の変更**:
```rust
fn merge_index(b: Option<RawIndexConfig>, h: Option<RawIndexConfig>) -> Option<RawIndexConfig> {
    match (b, h) {
        (Some(b), Some(h)) => Some(RawIndexConfig {
            path: h.path.or(b.path),               // ← 追加
            languages: h.languages.or(b.languages),
        }),
        (b, h) => h.or(b),
    }
}
```

**resolve_config の変更**:
```rust
fn resolve_config(raw: RawConfig, sources: Vec<ConfigSource>) -> AppConfig {
    let index = IndexConfig {
        path: raw.index.as_ref().and_then(|i| i.path.clone()),  // raw 値として保持
        languages: raw.index.and_then(|i| i.languages).unwrap_or_default(),
    };
    // ...
}
```

### 判断 7: IgnoreFilter の動的除外（パターンリスト保持方式）

**決定**: `IgnoreFilter` にパターン文字列のリストを保持し、追加パターンで正しく再構築

```rust
pub struct IgnoreFilter {
    patterns: Vec<String>,      // ← パターンリストを保持
    glob_set: GlobSet,
}

impl IgnoreFilter {
    fn build_glob_set(patterns: &[String]) -> GlobSet {
        let mut builder = GlobSetBuilder::new();
        for pattern in patterns {
            if let Ok(glob) = Glob::new(pattern) {
                builder.add(glob);
            }
        }
        builder.build().unwrap_or_default()
    }

    /// カスタムインデックスパスがリポジトリ内にある場合、除外パターンに追加
    pub fn with_custom_index_path(mut self, index_path: &Path, repo_root: &Path) -> Self {
        if let Ok(rel) = index_path.strip_prefix(repo_root) {
            let pattern = format!("{}/**", rel.display());
            self.patterns.push(pattern);
            self.glob_set = Self::build_glob_set(&self.patterns);
        }
        self
    }
}
```

**理由**: from_content で読み込んだ .cmindexignore のカスタムパターンも patterns に保持されるため、再構築時に喪失しない。

### 判断 8: clean の安全ガード（ホワイトリスト方式）

```rust
/// インデックスディレクトリとして妥当か検証
fn validate_index_directory(dir: &Path) -> Result<(), CleanError> {
    // symlink チェック
    if dir.symlink_metadata().map(|m| m.file_type().is_symlink()).unwrap_or(false) {
        return Err(CleanError::SymlinkDetected);
    }
    // インデックス固有ファイルの存在確認（少なくとも1つ）
    let index_markers = ["tantivy", "manifest.json", "state.json", "symbols.db"];
    let has_marker = index_markers.iter().any(|m| dir.join(m).exists());
    if !has_marker {
        return Err(CleanError::NotAnIndexDirectory(dir.to_path_buf()));
    }
    Ok(())
}

pub fn run(
    path: &Path,
    commandindex_dir: &Path,
    options: &CleanOptions,
) -> Result<CleanResult, CleanError> {
    if !commandindex_dir.exists() {
        return Ok(CleanResult::NotFound);
    }
    validate_index_directory(commandindex_dir)?;

    // 既存挙動ベース: 通常時はディレクトリ全削除、keep_embeddings時は部分削除
    if options.keep_embeddings {
        // embeddings.db と config ファイルを保持し、それ以外を削除
        for entry in std::fs::read_dir(commandindex_dir)? {
            let entry = entry?;
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str == "embeddings.db" || name_str.ends_with(".toml") {
                continue; // 保持
            }
            if entry.file_type()?.is_dir() {
                std::fs::remove_dir_all(entry.path())?;
            } else {
                std::fs::remove_file(entry.path())?;
            }
        }
    } else {
        // ディレクトリ全削除（既存挙動と同じ）
        std::fs::remove_dir_all(commandindex_dir)?;
    }
    Ok(CleanResult::Removed)
}
```

**注意**: `CleanError::SymlinkDetected` は既存の unit variant を維持。`CleanError::NotAnIndexDirectory(PathBuf)` を新規追加。`CleanResult::AlreadyClean` は存在しないため `CleanResult::NotFound` を使用。

### 判断 9: symlink チェック（read-only vs destructive で分離）

**決定**: symlink ポリシーをコマンド種別で分ける。
- **destructive 系（clean, import）**: symlink 拒否（既存挙動）
- **write 系（index, update, embed）**: symlink 拒否（安全側）
- **read-only 系（search, status, context, export, config）**: symlink 許可（互換性重視）

`resolve_index_path` は symlink チェックを行わず、各コマンドの呼び出し側で判断する。

```rust
/// symlink チェック（destructive/write コマンドの呼び出し側で使用）
pub fn reject_symlink(path: &Path) -> Result<(), ResolveIndexPathError> {
    if path.exists() && path.symlink_metadata()
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
    {
        return Err(ResolveIndexPathError::SymlinkDetected(path.to_path_buf()));
    }
    Ok(())
}
```

**使い分け**:
- `clean`, `import`, `index`, `update`, `embed`: `reject_symlink(&commandindex_dir)?;`
- `search`, `status`, `context`, `export`, `config`: symlink チェック不要

### 判断 10: config.local.toml の機密情報保護

**決定**: `--index-path` 指定時、config.local.toml は常にリポジトリローカル（`{base_path}/.commandindex/`）から読み込む。
共有パス配下の config.local.toml は読み込まない。

**理由**: 共有インデックスディレクトリに config.local.toml（API key 含む）が存在すると、他ユーザーに機密情報が漏洩するリスクがある。

## 4. 影響範囲

### 変更対象ファイル

| ファイル | 変更内容 | 影響度 |
|---------|---------|--------|
| `src/main.rs` | Cli にグローバルオプション追加、各サブコマンド呼び出し修正（export/import含む）、出力メッセージ動的化 | 高 |
| `src/lib.rs` | INDEX_DIR_NAME 定数維持（変更なし） | 低 |
| `src/indexer/mod.rs` | COMMANDINDEX_DIR 削除（非pub確認済み）、ヘルパー関数シグネチャ変更、resolve_index_path 新設 | 高 |
| `src/config/mod.rs` | RawIndexConfig/IndexConfig に path 追加、merge_index に path マージ追加、resolve_config 変更 | 高 |
| `src/cli/search.rs` | SearchContext 拡張（commandindex_dir フィールド）、Path::new(".") 9箇所修正（fulltext + symbol + related + semantic） | 高 |
| `src/cli/context.rs` | run_context に commandindex_dir パラメータ追加、Path::new(".") 2箇所 → commandindex_dir | 中 |
| `src/cli/status/mod.rs` | run + 内部関数(get_symbol_count, get_embedding_file_count, compute_storage_breakdown, run_verify)全修正、get_embedding_model のcwdバグ修正 | 高 |
| `src/cli/clean.rs` | run に commandindex_dir パラメータ追加（2引数→3引数）、ホワイトリスト方式削除、NotAnIndexDirectory エラー追加 | 高 |
| `src/cli/export.rs` | run に commandindex_dir を外部から受け取る形に変更 | 中 |
| `src/cli/import_index.rs` | run に commandindex_dir を外部から受け取る形に変更 | 中 |
| `src/cli/embed.rs` | run に commandindex_dir パラメータ追加、内部ヘルパー呼び出し修正 | 中 |
| `src/cli/config.rs` | run_show/run_path に base_path パラメータ追加、Path::new(".") 2箇所修正 | 中 |
| `src/cli/workspace.rs` | SearchContext::new 経由で各リポジトリの config [index].path を尊重、.commandindex ハードコード（L159, L203）修正 | 高 |
| `src/cli/index.rs` | commandindex_dir パラメータの受け渡し（10箇所以上） | 高 |
| `src/parser/ignore.rs` | IgnoreFilter にパターンリスト保持、with_custom_index_path メソッド追加 | 中 |

### テスト影響

| テストファイル | 影響 |
|---------------|------|
| `src/config/mod.rs` 内テスト | load_config は変更なしのため影響なし |
| `tests/cli_clean.rs` | 出力メッセージ期待値の更新（"Removed index" 部分一致に緩和） |
| `tests/cli_index.rs` | 出力メッセージ期待値の更新（"Index saved to" 部分一致に緩和） |
| `tests/e2e_embedding.rs` | clean メッセージ期待値の更新 |
| `tests/e2e_phase3_integration.rs` | symbol_db_path 呼び出し修正（base_path.join(INDEX_DIR_NAME) を事前計算） |
| その他既存テスト | 後方互換テストとして維持 |

### 新規テスト

| テストケース | 目的 |
|-------------|------|
| resolve_index_path: CLI指定優先 | 優先順位テスト |
| resolve_index_path: config指定優先 | 優先順位テスト |
| resolve_index_path: デフォルトフォールバック | 後方互換テスト |
| resolve_index_path: 相対パス解決（cwd基準） | パス解決テスト |
| resolve_index_path: 絶対パス解決 | パス解決テスト |
| resolve_index_path: 存在しないパス | エッジケース |
| resolve_index_path: symlink 検出 | セキュリティテスト |
| --index-path でカスタムパスに index/search/update/clean | E2Eテスト |
| commandindex.toml [index].path での指定 | 設定ファイルテスト |
| search 全モード（fulltext/symbol/related/semantic） | 一貫性テスト |
| workspace 横断の search/status/update | workspace テスト |
| clean の安全ガード（symlink拒否、非インデックスディレクトリ拒否） | セキュリティテスト |
| IgnoreFilter: リポジトリ内カスタムパス除外 | 除外テスト |
| IgnoreFilter: リポジトリ外パス（除外不要） | 除外テスト |
| export/import で custom index path 使用時の state 整合性 | E2Eテスト |

## 5. セキュリティ設計

| 脅威 | 対策 | 優先度 |
|------|------|--------|
| パストラバーサル | `resolve_index_path` で canonicalize を実施。未作成パスは手動 ".." 除去 | 高 |
| symlink フォロー攻撃 | `resolve_index_path` 内で全コマンド共通の symlink チェック（clean だけでなく全コマンド） | 高 |
| 任意パス削除 | clean はホワイトリスト方式（既知ファイル名のみ削除）+ インデックスマーカー検証 | 高 |
| 機密情報漏洩 | `--index-path` 指定時も config.local.toml はリポジトリローカルから読み込み。共有パスからは読まない | 高 |
| 共有インデックスの同時書き込み | tantivy の meta.lock + SQLite WAL に依存。NFS等では信頼性が低いためローカルFS推奨をドキュメント化 | 中 |
| TOCTOU | ローカルCLIのため低リスク。セキュリティドキュメントに制約として記載 | 低 |

## 6. データフロー

```
ユーザー入力
    │
    ▼
Cli::parse()
    │ --index-path を取得
    ▼
load_config(base_path)  ← 【Step 1: デフォルト位置から config 読み込み】
    │ config.index.path を取得
    ▼
resolve_index_path(cli_index_path, config_index_path, base_path)
    │ 優先順位: CLI > config > default     ← 【Step 2: パス解決】
    │ canonicalize + symlink チェック
    ▼
commandindex_dir: PathBuf  (解決済み・正規化済み)
    │
    ├──▶ index_dir(commandindex_dir)        → tantivy ディレクトリ
    ├──▶ symbol_db_path(commandindex_dir)    → symbols.db パス
    ├──▶ embeddings_db_path(commandindex_dir) → embeddings.db パス
    │
    ▼
各サブコマンドの run 関数に commandindex_dir を渡す
```

## 7. 品質基準

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |

## 8. 実装順序

1. **定数統一**: `indexer/mod.rs` の `COMMANDINDEX_DIR` 削除（非pub確認済み）、`crate::INDEX_DIR_NAME` に統一
2. **resolve_index_path 新設**: `indexer/mod.rs` にパス解決関数を追加（Result型、canonicalize、symlink チェック）
3. **ヘルパー関数変更**: `index_dir`, `symbol_db_path`, `embeddings_db_path` のシグネチャ変更（28箇所以上）
4. **config 拡張**: `RawIndexConfig.path` 追加、`merge_index` に path マージ、`resolve_config` 変更
5. **Cli グローバルオプション**: `--index-path` を `Cli` 構造体に追加
6. **main.rs**: 各サブコマンド呼び出しで load_config → resolve_index_path → commandindex_dir の2段階解決。出力メッセージ動的化（format! で実パス表示）
7. **SearchContext 拡張**: commandindex_dir フィールド追加、new コンストラクタ、from_current_dir/from_path 置き換え
8. **各サブコマンド修正**: search（全モード9箇所）, context(2箇所), status(6箇所+cwd バグ), clean(ホワイトリスト方式), export/import(main.rs+内部), embed(3箇所), config(2箇所), index(10箇所以上)
9. **workspace 対応**: SearchContext::new 経由で per-repo config [index].path を尊重、.commandindex ハードコード修正
10. **IgnoreFilter 拡張**: パターンリスト保持 + with_custom_index_path
11. **テスト**: 既存テスト期待値更新（出力メッセージ部分一致化）+ 新規テスト15項目
