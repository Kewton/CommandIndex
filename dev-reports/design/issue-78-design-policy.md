# 設計方針書: Issue #78 マルチリポジトリ横断検索

## 1. Issue概要

| 項目 | 内容 |
|------|------|
| Issue番号 | #78 |
| タイトル | [Feature] マルチリポジトリ横断検索 |
| 種別 | enhancement |
| 依存Issue | #76 チーム共有設定ファイル（実装済み） |

### 目的
複数リポジトリにまたがる横断検索機能を実装し、チームの知識発見効率を向上させる。

### Phase 1 スコープ
- BM25（tantivy全文検索）のみ横断対応
- ハイブリッド検索（Embedding + BM25）は将来Phase
- 対応コマンド: search, status, update
- 非対応コマンド: embed, context, clean, config show/path

---

## 2. システムアーキテクチャ

### 現在のレイヤー構成

```
+---------------------------------------------+
|  CLI層 (src/main.rs)                        |
|  - clap サブコマンド定義                      |
|  - 引数解析 -> 各モジュール呼び出し             |
+---------------------------------------------+
|  CLI実装層 (src/cli/)                        |
|  - search.rs, index.rs, status.rs, etc.     |
|  - ビジネスロジック統合                        |
+----------+----------+----------+-------------+
| Parser   | Indexer  | Search  | Embedding   |
| 解析     | 索引管理  | 検索    | ベクトル    |
+----------+----------+----------+-------------+
|  Config層 (src/config/)                      |
|  - 設定ファイル読込・マージ・検証               |
+---------------------------------------------+
|  Output層 (src/output/)                      |
|  - Human / JSON / Path フォーマット           |
+---------------------------------------------+
```

### マルチリポ対応後のレイヤー構成

```
+---------------------------------------------+
|  CLI層 (src/main.rs)                        |
|  - --workspace / --repo オプション追加         |
|  - workspace有無の分岐フロー                   |
|  - SearchContext構築 -> run()に渡す            |
+---------------------------------------------+
|  Workspace層 (src/cli/workspace.rs) [新規]   |
|  - 横断検索オーケストレーション                 |
|  - リポジトリ列挙・フィルタ                    |
|  - 結果マージ（rrf_merge_multiple）            |
+---------------------------------------------+
|  Config層 (src/config/)                      |
|  - 既存: AppConfig（リポ固有設定）             |
|  - 新規: WorkspaceConfig（横断設定）           |
|    src/config/workspace.rs [新規]             |
|    WorkspaceConfig / WorkspaceConfigError     |
+---------------------------------------------+
|  CLI実装層 (src/cli/)                        |
|  - SearchContext経由でbase_path受取           |
|  - run()はSearchContextを受け取る             |
|  - 単一リポ検索（既存ロジック維持）              |
+----------+----------+----------+-------------+
| Parser   | Indexer  | Search  | Embedding   |
| (変更なし)| (変更なし)|(hybrid拡張)| (変更なし)  |
+----------+----------+----------+-------------+
|  Output層 (src/output/)                      |
|  - ワークスペースモード時のみ                   |
|    WorkspaceSearchResultを扱う分岐を追加       |
+---------------------------------------------+
```

**[Stage 1 M1反映]** WorkspaceConfig/WorkspaceConfigErrorは`src/config/workspace.rs`に分離配置。
`src/cli/workspace.rs`には横断検索オーケストレーションのみを残す。

---

## 3. 新規モジュール設計

### 3.1 WorkspaceConfig（src/config/workspace.rs）

**[Stage 1 M1反映]** Config層に配置し、SRP（単一責任原則）を遵守。

```rust
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// ワークスペース設定ファイルのルート
#[derive(Debug, Deserialize)]
pub struct WorkspaceConfig {
    pub workspace: WorkspaceDefinition,
}

#[derive(Debug, Deserialize)]
pub struct WorkspaceDefinition {
    pub name: String,
    pub repositories: Vec<RepositoryEntry>,
}

/// 個別リポジトリエントリ（TOML定義）
#[derive(Debug, Deserialize)]
pub struct RepositoryEntry {
    pub path: String,
    pub alias: Option<String>,
}

/// パス解決・バリデーション済みリポジトリ
#[derive(Debug, Clone)]
pub struct ResolvedRepository {
    pub path: PathBuf,       // canonicalize済み絶対パス
    pub alias: String,       // エイリアス（デフォルト: ディレクトリ名）
}
```

### 3.2 バリデーション規則

**[Should Fix反映]** alias/nameの入力制約:
- 使用可能文字: ASCII英数字、ハイフン(`-`)、アンダースコア(`_`)
- 長さ上限: 64文字
- TOMLファイルサイズ上限: 1MB（パース前にファイルサイズチェック）

### 3.3 WorkspaceConfigError（src/config/workspace.rs）

**[Stage 2 M4反映]** Display/Error traitを実装。

```rust
use std::fmt;

#[derive(Debug)]
pub enum WorkspaceConfigError {
    /// ファイル読込エラー
    ReadError(std::io::Error),
    /// TOMLパースエラー
    ParseError(toml::de::Error),
    /// エイリアス重複
    DuplicateAlias { alias: String },
    /// パス重複（canonicalize後に同一）
    DuplicatePath { path: PathBuf },
    /// リポジトリ数が上限超過
    TooManyRepositories { count: usize, max: usize },
    /// HOME環境変数未設定（チルダ展開失敗）
    HomeDirNotFound,
    /// ファイルサイズ上限超過
    FileTooLarge { size: u64, max: u64 },
    /// alias/name不正
    InvalidName { name: String, reason: String },
    /// パスに危険な文字列が含まれる
    UnsafePath { path: String, reason: String },
}

impl fmt::Display for WorkspaceConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadError(e) => write!(f, "workspace config read error: {e}"),
            Self::ParseError(e) => write!(f, "workspace config parse error: {e}"),
            Self::DuplicateAlias { alias } => write!(f, "duplicate alias: {alias}"),
            Self::DuplicatePath { path } => write!(f, "duplicate path: {}", path.display()),
            Self::TooManyRepositories { count, max } =>
                write!(f, "too many repositories: {count} (max: {max})"),
            Self::HomeDirNotFound => write!(f, "HOME directory not found"),
            Self::FileTooLarge { size, max } =>
                write!(f, "workspace config file too large: {size} bytes (max: {max})"),
            Self::InvalidName { name, reason } =>
                write!(f, "invalid name '{name}': {reason}"),
            Self::UnsafePath { path, reason } =>
                write!(f, "unsafe path '{path}': {reason}"),
        }
    }
}

impl std::error::Error for WorkspaceConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ReadError(e) => Some(e),
            Self::ParseError(e) => Some(e),
            _ => None,
        }
    }
}
```

### 3.4 WorkspaceWarning（src/config/workspace.rs）

**[Should Fix反映]** 警告バリアントをエラーから分離。RepositoryNotFound/IndexNotFoundは検索をスキップするだけの警告であり、致命的エラーではない。

```rust
/// 検索続行可能な警告（Graceful Degradation用）
///
/// **[Stage 5-7 Should Fix反映]** validate関数からI/O副作用を除去し、
/// 警告はこのenumで返却。出力はオーケストレーション層(workspace.rs)で一括処理。
#[derive(Debug)]
pub enum WorkspaceWarning {
    /// リポジトリパスが存在しない（検索スキップ）
    RepositoryNotFound { alias: String, path: PathBuf },
    /// インデックス未作成（検索スキップ）
    IndexNotFound { alias: String, path: PathBuf },
    /// パスがcanonicalize後に異なるパスに解決された
    PathResolved { original: PathBuf, resolved: PathBuf },
    /// シンボリックリンク検出
    SymlinkDetected { path: PathBuf, resolved: PathBuf },
}

impl fmt::Display for WorkspaceWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RepositoryNotFound { alias, path } =>
                write!(f, "warning: repository '{alias}' not found at {}", path.display()),
            Self::IndexNotFound { alias, path } =>
                write!(f, "warning: index not found for '{alias}' at {}", path.display()),
            Self::PathResolved { original, resolved } =>
                write!(f, "warning: path '{}' resolved to '{}'", original.display(), resolved.display()),
            Self::SymlinkDetected { path, resolved } =>
                write!(f, "warning: '{}' is a symlink pointing to '{}'", path.display(), resolved.display()),
        }
    }
}
```

### 3.5 SearchError拡張

**[Stage 2 M4反映]** SearchErrorにWorkspaceバリアントを追加。

```rust
// src/cli/search.rs 内の既存SearchError enumに追加
pub enum SearchError {
    // ... 既存バリアント ...

    /// ワークスペース設定エラー
    Workspace(WorkspaceConfigError),
}

impl From<WorkspaceConfigError> for SearchError {
    fn from(e: WorkspaceConfigError) -> Self {
        SearchError::Workspace(e)
    }
}
```

### 3.6 SearchContext（src/cli/search.rs）

**[Stage 3 M3反映]** context.rsは将来Phase対応予定として位置付ける。Phase 1ではSearchContextはsrc/cli/search.rs内に定義。

```rust
/// 検索実行に必要なコンテキスト（引数爆発防止）
pub struct SearchContext {
    /// リポジトリのベースパス（デフォルト: "."）
    pub base_path: PathBuf,
    /// ロード済み設定
    pub config: AppConfig,
}

impl SearchContext {
    /// 単一リポジトリ用（後方互換）
    pub fn from_current_dir() -> Result<Self, SearchError> {
        let base_path = PathBuf::from(".");
        let config = load_config(&base_path)?;
        Ok(Self { base_path, config })
    }

    /// 指定パス用（ワークスペース横断時）
    pub fn from_path(base_path: &Path) -> Result<Self, SearchError> {
        let config = load_config(base_path)?;
        Ok(Self { base_path: base_path.to_path_buf(), config })
    }

    pub fn index_dir(&self) -> PathBuf {
        crate::indexer::index_dir(&self.base_path)
    }

    pub fn symbol_db_path(&self) -> PathBuf {
        crate::indexer::symbol_db_path(&self.base_path)
    }

    // **[Stage 5-7 Should Fix反映]** embeddings_db_path()はPhase 1では不要（YAGNI）。
    // Embedding横断検索は将来Phaseで実装するため、その時点でSearchContextに追加する。
    // pub fn embeddings_db_path(&self) -> PathBuf { ... }  // Phase 2以降で追加
}
```

**[Stage 3 M3]** 将来Phase: `src/cli/context.rs`に独立モジュールとして移設予定。context/embed/cleanなど複数コマンドでSearchContextを共有する段階で実施。Phase 1ではrun()のみSearchContext化し、他関数は既存シグネチャを維持する（conflicts_withで弾かれるため安全）。

---

## 4. 既存構造体の方針: Composition パターン

### 4.1 SearchResult は変更しない

**[Stage 1 M4反映]** SearchResultへのフィールド追加は行わない。既存のSearchResultを破壊的に変更するのではなく、compositionパターンで対応する。

```rust
// src/indexer/reader.rs - 変更なし
pub struct SearchResult {
    pub path: String,
    pub heading: String,
    pub body: String,
    pub tags: String,
    pub heading_level: u64,
    pub line_start: u64,
    pub score: f32,
    // repository フィールドは追加しない
}
```

### 4.2 WorkspaceSearchResult（新規: src/output/mod.rs）

**[Stage 1 M4反映]** compositionで定義。ワークスペースモード専用のラッパー型。

**[Stage 5-7 Should Fix反映]** output層からcli層への逆依存を回避するため、WorkspaceSearchResult型は`src/output/mod.rs`に定義する。cli/workspace.rsとoutput/human.rs等の両方からimport可能な共通位置に配置。

```rust
// src/output/mod.rs
use crate::indexer::reader::SearchResult;

/// ワークスペース横断検索の結果ラッパー（compositionパターン）
pub struct WorkspaceSearchResult {
    /// リポジトリエイリアス
    pub repository: String,
    /// 検索結果本体（既存SearchResultをそのまま保持）
    pub result: SearchResult,
}
```

### 4.3 SearchResult構築箇所の完全一覧

**[Stage 2 M2反映]** SearchResultを生成する箇所を網羅的に列挙。Phase 1ではこれらの箇所に変更は不要（SearchResult自体を変更しないため）。

| 関数名 | ファイル:行 | 説明 |
|--------|-----------|------|
| `doc_to_search_result()` | `src/indexer/reader.rs:186-196` | tantivy Document -> SearchResult変換 |
| `enrich_semantic_to_search_results()` | `src/cli/search.rs:534-555` | Embedding検索結果のSearchResult化 |
| `make_result()` | `src/search/hybrid.rs:69-79` | ハイブリッド検索でのSearchResult構築 |
| `make_result()` | `tests/output_format.rs:4-14` | テスト用ヘルパー |

**[Stage 3 M4]** 将来検討: SearchResult構築をファクトリメソッドに集約。`SearchResult::new()`または`SearchResult::builder()`パターンで、構築箇所の散在を防ぐ。

**[Stage 3 M5]** 将来検討: JSON出力対応のため、SearchResultにSerialize deriveを追加。現在は手動でフィールドを出力しているが、`#[derive(serde::Serialize)]`を追加すれば`serde_json::to_string()`でシリアライズ可能になる。

---

## 5. RRFマージ設計

### 5.1 rrf_merge_multiple 汎用関数

**[Stage 1 M3反映]** `rrf_merge_cross_repo`は不要。代わりに`rrf_merge_multiple`汎用関数を新設し、既存`rrf_merge`をそのラッパーとして再定義する。

```rust
// src/search/hybrid.rs

/// 汎用RRFマージ: 複数のVec<SearchResult>をランク順位ベースで統合
///
/// キー: (path, heading) で同一結果を識別
/// スコア: sum(1 / (K + rank)) を各リストについて計算
///
/// **[Stage 7 M3反映]** マルチリポ横断時の同名ファイル衝突対策:
/// ワークスペース横断マージでは、各リポの結果のpathにaliasプレフィックスを
/// 付与してユニーク化してからマージする。マージ後、出力時にプレフィックスを
/// 除去して元のpathに戻す。具体的にはworkspace.rs側でマージ前に
/// `path = format!("{}:{}", alias, original_path)` に変換し、
/// WorkspaceSearchResult構築時にaliasとoriginal_pathに分離する。
/// 単一リポ内でのrrf_merge（BM25+semantic）では同一リポ内なので衝突しない。
pub fn rrf_merge_multiple(
    result_lists: &[Vec<SearchResult>],
    limit: usize,
) -> Vec<SearchResult> {
    // キー: (path, heading) の2タプル
    // 各リストの結果をランク順位ベースでスコアリング
    // スコア = sum(1/(K + rank))
    // ...
}

/// 既存のrrf_mergeはrrf_merge_multipleのラッパー（後方互換維持）
pub fn rrf_merge(
    bm25_results: &[SearchResult],
    semantic_results: &[SearchResult],
    limit: usize,
) -> Vec<SearchResult> {
    rrf_merge_multiple(&[bm25_results.to_vec(), semantic_results.to_vec()], limit)
}
```

### 5.2 ワークスペース横断でのrrf_merge_multiple使用

```rust
// src/cli/workspace.rs での使用例
let all_result_lists: Vec<Vec<SearchResult>> = repo_results
    .iter()
    .map(|(_, results)| results.clone())
    .collect();

let merged = rrf_merge_multiple(&all_result_lists, limit);

// マージ後、WorkspaceSearchResultに変換
// repository情報は各結果のpath等からマッピング
```

---

## 6. CLI設計

### 6.1 新規オプション

```rust
// src/main.rs - Commands::Search に追加
#[arg(long, help = "ワークスペース設定ファイルのパス")]
workspace: Option<String>,

#[arg(long, requires = "workspace", help = "検索対象リポジトリの絞り込み")]
repo: Option<String>,
```

### 6.2 conflicts_with設計

**[Stage 2 M3反映]** 既存のconflicts_with_allにworkspaceを網羅的に追加。

```rust
// --workspace と以下は競合（Phase 1非対応）
// 以下の既存オプションのconflicts_with_allに "workspace" を追加:
//   --symbol    conflicts_with_all = ["related", "semantic", "workspace"]
//   --related   conflicts_with_all = ["symbol", "semantic", "workspace"]
//   --semantic  conflicts_with_all = ["symbol", "related", "workspace"]
//
// --repo は --workspace を requires（既存設計通り）
```

### 6.3 main.rsの分岐フロー

**[Stage 2 M1, M2反映]** main.rsでSearchContext構築 -> run()に渡す方式。workspace有無で分岐。

```rust
// src/main.rs - Commands::Search マッチ部分
Commands::Search { query, workspace, repo, format, limit, snippet, ... } => {
    if let Some(ws_path) = workspace {
        // ワークスペースモード
        // 1. WorkspaceConfig読込（src/config/workspace.rs）
        // 2. リポジトリ解決・バリデーション
        // 3. --repo フィルタ適用
        // 4. 各リポでSearchContext構築 -> run() 逐次呼出
        // 5. rrf_merge_multiple で結果統合
        // 6. WorkspaceSearchResult として出力
        // **[Stage 5 M3反映]** main.rsでSearchOptions/SearchFiltersを構築し、
        // 構造化された引数でrun_workspace_searchに渡す（DRY原則遵守）
        let options = SearchOptions { query, limit, ... };
        let filters = SearchFilters { ... };
        cli::workspace::run_workspace_search(
            ws_path, repo, &options, &filters, format, snippet_config, rerank, rerank_top,
        )?;
    } else {
        // 単一リポモード（既存動作）
        // SearchContext構築してrun()に渡す
        let ctx = SearchContext::from_current_dir()?;
        let options = SearchOptions { query, limit, ... };
        let filters = SearchFilters { ... };
        cli::search::run(&ctx, &options, &filters, format, snippet_config, rerank, rerank_top)?;
    }
}
```

**[Stage 2 M1, Stage 5 M2反映]** run()の新シグネチャ。現実のコードに合わせた完全なシグネチャを記載:

```rust
// src/cli/search.rs
// 現在のrun()シグネチャ（SearchContext導入前）:
//   pub fn run(options: &SearchOptions, filters: &SearchFilters, format: OutputFormat,
//              snippet_config: SnippetConfig, rerank: bool, rerank_top: Option<usize>)
//              -> Result<(), SearchError>
//
/// Phase 1: run()のみSearchContext化。他関数は既存シグネチャ維持。
/// SearchContext引数を先頭に追加し、内部のPath::new(".")をctx.base_pathに置換。
pub fn run(
    ctx: &SearchContext,
    options: &SearchOptions,
    filters: &SearchFilters,
    format: OutputFormat,
    snippet_config: SnippetConfig,
    rerank: bool,
    rerank_top: Option<usize>,
) -> Result<(), SearchError> {
    // ctx.base_path, ctx.config を使用
    // 既存のPath::new(".")参照をctx.base_pathに置換
    // config読込はSearchContext構築時に完了済み
    // **[Stage 7 M2反映]** run()内部で呼び出すtry_hybrid_searchにもbase_pathを伝播。
    // SearchContextをtry_hybrid_searchに渡すことで、内部のPath::new(".")3箇所
    // （symbol_db_path, index_dir, embeddings_db_path）をctx.base_pathに置換。
    // try_hybrid_searchの新シグネチャ:
    //   fn try_hybrid_search(ctx: &SearchContext, bm25_results: Vec<SearchResult>,
    //       options: &SearchOptions, filters: &SearchFilters, config: &AppConfig)
    //       -> Result<Vec<SearchResult>, SearchError>
    // run()内部の他の関数（try_symbol_search, try_related_search等）も同様に
    // ctx.base_pathを利用してPath::new(".")を置換する。
}
```

**[Stage 1 M2反映]** main.rsのconfig読込をSearchContext経由に統一。main.rs内で直接`load_config()`を呼ぶのではなく、SearchContext構築時にconfigをロードする。

**[Stage 3 M1反映]** Phase 1ではrun()のみSearchContext化。context/embed/clean等の他関数は既存シグネチャを維持する。--workspaceはconflicts_withで他コマンドと排他のため安全。

### 6.4 Status/Update CLIオプション

**[Should Fix反映]** status/updateコマンドにも--workspace/--repoオプションを追加。

```rust
// src/main.rs - Commands::Status に追加
#[arg(long, help = "ワークスペース設定ファイルのパス")]
workspace: Option<String>,

// src/main.rs - Commands::Update に追加
#[arg(long, help = "ワークスペース設定ファイルのパス")]
workspace: Option<String>,

#[arg(long, requires = "workspace", help = "対象リポジトリの絞り込み")]
repo: Option<String>,
```

---

## 7. 設定階層設計

### 責務分離

| 設定ファイル | 責務 | スコープ |
|------------|------|---------|
| `commandindex-workspace.toml` | 横断対象リポジトリの定義のみ | ワークスペース全体 |
| `commandindex.toml`（各リポ内） | リポ固有設定（除外パターン等） | 個別リポジトリ |
| `.commandindex/config.local.toml` | 個人設定・API key | 個別リポジトリ |

### ワークスペース設定はリポ固有設定に干渉しない
- 各リポの検索時、そのリポの`commandindex.toml`をロード
- ワークスペース設定からリポ固有設定へのオーバーライドは行わない

---

## 8. セキュリティ設計

**[Stage 4 M1, M2, M3反映]** セキュリティ対策を強化。

| 脅威 | 対策 | 優先度 |
|------|------|--------|
| パストラバーサル | canonicalize()後に.commandindex/存在チェック + 許容範囲チェック | 高 |
| シンボリックリンク | canonicalize()でリンク解決、実パスでバリデーション。clean.rsパターン適用（後述） | 高 |
| チルダ展開の安全性 | dirs::home_dir()使用、HOME未設定時は明確なエラー | 高 |
| パス展開の安全性 | チルダ(`~`)のみ展開。`$`記号やバッククォート(`` ` ``)を含むパスは拒否 | 高 |
| 機密ディレクトリアクセス | .commandindex/存在をゲートとして使用 | 中 |
| 大量リポ登録 | リポ数上限50、超過時エラー | 中 |
| 大量設定ファイル | TOMLファイルサイズ上限1MB | 中 |

### 8.1 canonicalize後の許容範囲チェック

**[Stage 4 M1反映]** canonicalize()で解決されたパスが元のパスと大きく異なる場合（シンボリックリンク等で別ディレクトリに解決された場合）、stderrに警告を表示。

```rust
/// **[Stage 5-7 Should Fix反映]** validate関数からI/O副作用(eprintln)を除去。
/// WorkspaceWarning返却のみとし、出力はオーケストレーション層(workspace.rs)で行う。
/// また、original!=resolved比較が常にtrueになる問題を修正:
/// originalにはexpand_path後の未canonicalize値、resolvedにはcanonicalize済み値が入るため、
/// 相対パス指定時は常に不一致となる。比較対象をcanonicalize(original)とresolvedにする。
fn validate_resolved_path(original: &Path, resolved: &Path) -> Option<WorkspaceWarning> {
    // originalをcanonicalize()して比較（expand_path後の未解決パスとの不正な不一致を防止）
    let canonical_original = std::fs::canonicalize(original).ok();
    let differs = canonical_original.as_deref() != Some(resolved);
    if differs {
        Some(WorkspaceWarning::PathResolved {
            original: original.to_path_buf(),
            resolved: resolved.to_path_buf(),
        })
    } else {
        None
    }
}

// WorkspaceWarningにPathResolvedバリアントを追加:
// PathResolved { original: PathBuf, resolved: PathBuf },
// Display: write!(f, "warning: path '{}' resolved to '{}'", original.display(), resolved.display())
//
// オーケストレーション層（workspace.rs）での出力例:
//   for warning in warnings {
//       eprintln!("{warning}");
//   }
```

### 8.2 パス展開の安全性

**[Stage 4 M2反映]** パス展開はチルダのみ。シェル変数展開やコマンド置換は行わない。

```rust
fn expand_path(path: &str) -> Result<PathBuf, WorkspaceConfigError> {
    // $記号やバッククォートを含むパスは拒否
    if path.contains('$') || path.contains('`') {
        return Err(WorkspaceConfigError::UnsafePath {
            path: path.to_string(),
            reason: "shell variable expansion and command substitution are not allowed".into(),
        });
    }

    if path == "~" {
        // チルダ単体: ホームディレクトリそのもの
        let home = dirs::home_dir().ok_or(WorkspaceConfigError::HomeDirNotFound)?;
        Ok(home)
    } else if let Some(rest) = path.strip_prefix("~/") {
        // ~/... 形式: ホームディレクトリ配下のパス
        let home = dirs::home_dir().ok_or(WorkspaceConfigError::HomeDirNotFound)?;
        Ok(home.join(rest))
    } else if path.starts_with('~') {
        // ~user 形式: サポート外、明確にエラー拒否
        Err(WorkspaceConfigError::UnsafePath {
            path: path.to_string(),
            reason: "~user style path expansion is not supported; use absolute path instead".into(),
        })
    } else {
        Ok(PathBuf::from(path))
    }
}
```

### 8.3 シンボリックリンクチェック（clean.rsパターン適用）

**[Stage 4 M3反映]** workspace設定のpathに対してシンボリックリンクチェックを適用。既存のclean.rsで実装されているパターンを踏襲。

```rust
/// **[Stage 5-7 Should Fix反映]** validate関数からI/O副作用を除去。
/// WorkspaceWarning返却のみとし、出力はオーケストレーション層で行う。
fn validate_symlink(path: &Path) -> Result<Option<WorkspaceWarning>, std::io::Error> {
    // clean.rsのパターンに従い、シンボリックリンクを検出
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        let resolved = std::fs::canonicalize(path)?;
        Ok(Some(WorkspaceWarning::SymlinkDetected {
            path: path.to_path_buf(),
            resolved,
        }))
    } else {
        Ok(None)
    }
}

// WorkspaceWarningにSymlinkDetectedバリアントを追加:
// SymlinkDetected { path: PathBuf, resolved: PathBuf },
// Display: write!(f, "warning: '{}' is a symlink pointing to '{}'", path.display(), resolved.display())
```

---

## 9. エラーハンドリング設計

### Graceful Degradation方針

| ケース | 動作 | 出力 |
|--------|------|------|
| リポパス不在 | スキップ | WorkspaceWarning::RepositoryNotFound（stderr） |
| インデックス未作成 | スキップ | WorkspaceWarning::IndexNotFound（stderr） |
| インデックス破損 | スキップ | 警告メッセージ（stderr） |
| 全リポ利用不可 | エラー終了 | エラーメッセージ |
| update失敗 | スキップ・続行 | エラーサマリー + 非ゼロ終了 |

---

## 10. パフォーマンス設計

### Phase 1: 逐次実行

```
for repo in workspace.repositories {
    println!("[{}/{}] Searching {}...", i, total, repo.alias);
    let ctx = SearchContext::from_path(&repo.path)?;
    let results = run(&ctx, &options, &filters, ...)?;
    all_results.push((repo.alias, results));
}
// rrf_merge_multiple で統合
let all_lists: Vec<Vec<SearchResult>> = all_results.iter().map(|(_, r)| r.clone()).collect();
let merged = rrf_merge_multiple(&all_lists, limit);
// WorkspaceSearchResult に変換
```

### IndexReader管理
- 逐次処理: 各リポのIndexReaderを検索後にdrop（スコープ制御）
- mmapファイルハンドル数を最小化
- 将来: rayon並列化 + セマフォによる同時open数制限

### リポ数上限
- 最大50リポジトリ
- 超過時はWorkspaceConfigError::TooManyRepositories

---

## 11. 出力フォーマット設計

### Human出力（search）

**[Should Fix反映]** スコア非表示の既存形式に合わせる。既存のhuman出力がスコアを表示しない形式の場合、ワークスペースモードでも同様にスコアは表示しない。

```
[backend] src/auth.rs:15
  認証ミドルウェアの実装
  ---
  JWT検証を行い、セッションを...

[frontend] src/login.tsx:42
  ログインコンポーネント
  ---
  認証APIを呼び出し...
```

**[Stage 1 M4反映]** Output層はワークスペースモード時のみWorkspaceSearchResultを扱う分岐を追加。単一リポモード時は既存のSearchResult出力をそのまま使用。

```rust
// src/output/human.rs
pub fn print_workspace_results(results: &[WorkspaceSearchResult], snippet: usize) {
    for wsr in results {
        // [repository] path:line_start 形式で出力
        println!("[{}] {}:{}", wsr.repository, wsr.result.path, wsr.result.line_start);
        // 既存のSnippet出力ロジックを再利用
        print_snippet(&wsr.result, snippet);
    }
}
```

### JSON出力（search）

```json
{"repository":"backend","path":"src/auth.rs","heading":"認証ミドルウェアの実装","body":"...","tags":"","heading_level":2,"line_start":15,"score":0.85}
```

### Human出力（status --workspace）
```
Workspace: my-team (3 repositories)

  frontend  ~/projects/frontend     1,234 files  2026-03-20 15:30  ok
  backend   ~/projects/backend        567 files  2026-03-21 10:00  ok
  docs      ~/projects/docs             -        -                 not_indexed
```

### update進捗メッセージ
```
[1/3] Updating frontend...  done (1,234 files)
[2/3] Updating backend...   done (567 files)
[3/3] Updating docs...      done (89 files)
```

---

## 12. 影響範囲

### 変更ファイル一覧

| ファイル | 変更種別 | 概要 |
|---------|---------|------|
| `src/config/workspace.rs` | **新規** | WorkspaceConfig, WorkspaceConfigError, WorkspaceWarning |
| `src/config/mod.rs` | 変更 | `pub mod workspace;` 追加 |
| `src/cli/workspace.rs` | **新規** | 横断検索オーケストレーション |
| `src/cli/mod.rs` | 変更 | `pub mod workspace;` 追加 |
| `src/main.rs` | 変更 | --workspace/--repo オプション、workspace有無分岐、SearchContext構築、conflicts_with追加 |
| `src/cli/search.rs` | 変更 | SearchContext導入、run()シグネチャ変更（SearchContext受取）、SearchError::Workspace追加 |
| `src/cli/status.rs` | 変更 | ワークスペースstatus対応、--workspaceオプション |
| `src/cli/index.rs` | 変更 | ワークスペースupdate対応、--workspace/--repoオプション、進捗メッセージ |
| `src/search/hybrid.rs` | 変更 | rrf_merge_multiple追加、既存rrf_mergeをラッパー化 |
| `src/output/mod.rs` | 変更 | WorkspaceSearchResult型定義追加 |
| `src/output/human.rs` | 変更 | ワークスペースモード用print_workspace_results追加 |
| `src/output/json.rs` | 変更 | WorkspaceSearchResult出力対応 |
| `src/output/path.rs` | 変更 | WorkspaceSearchResult出力対応 |
| `src/indexer/reader.rs` | **変更なし** | SearchResult構造体は変更しない |
| `Cargo.toml` | 変更 | dirs依存追加 |
| `tests/output_format.rs` | **変更なし** | make_result修正不要（SearchResult変更なし） |
| `tests/cli_args.rs` | 変更 | 新オプションテスト |
| `tests/e2e_workspace.rs` | **新規** | ワークスペースe2eテスト |

**注意**: `src/cli/context.rs` / `src/cli/config.rs` はPhase 1では変更しない（Stage 3 M1）。

---

## 13. 実装変更順序（依存関係グラフ）

**[Should Fix反映]** 実装は依存関係に基づき以下の順序で行う。

```
Step 1: 基盤型定義（依存なし）
  +-- src/config/workspace.rs
  |     WorkspaceConfig, WorkspaceConfigError, WorkspaceWarning
  +-- src/config/mod.rs
        pub mod workspace;

Step 2: 検索基盤変更（Step 1に依存）
  +-- src/search/hybrid.rs
  |     rrf_merge_multiple追加、rrf_mergeラッパー化
  +-- src/cli/search.rs
        SearchContext定義、SearchError::Workspace追加、run()シグネチャ変更

Step 3: オーケストレーション（Step 1, 2に依存）
  +-- src/cli/workspace.rs
  |     WorkspaceSearchResult、横断検索ロジック
  +-- src/cli/mod.rs
        pub mod workspace;

Step 4: 出力層対応（Step 3に依存）
  +-- src/output/human.rs
  +-- src/output/json.rs
  +-- src/output/path.rs

Step 5: CLI統合（Step 2, 3, 4に依存）
  +-- src/main.rs
        --workspace/--repo追加、分岐フロー、conflicts_with更新

Step 6: 追加コマンド対応（Step 1, 5に依存）
  +-- src/cli/status.rs
  +-- src/cli/index.rs

Step 7: テスト（全Stepに依存）
  +-- tests/cli_args.rs
  +-- tests/e2e_workspace.rs

Step 8: 依存追加
  +-- Cargo.toml (dirs)
```

---

## 14. コマンドフロー

### search --workspace ws.toml "query"

```
search --workspace ws.toml "query"
  |
  +-- [main.rs] workspace有無を判定
  |
  +-- [main.rs -> cli/workspace.rs] ワークスペースモード
  |   |
  |   +-- load_workspace_config(ws.toml)  [config/workspace.rs]
  |   |   +-- ファイルサイズチェック（上限1MB）
  |   |   +-- TOMLパース
  |   |   +-- alias/name バリデーション（ASCII英数字+ハイフン+アンダースコア、64文字以内）
  |   |   +-- パス安全性チェック（$やバッククォート拒否）
  |   |   +-- パス解決（チルダ展開 + canonicalize）
  |   |   +-- canonicalize後の許容範囲チェック（stderr警告）
  |   |   +-- シンボリックリンクチェック（clean.rsパターン）
  |   |   +-- エイリアス/パス重複チェック
  |   |   +-- .commandindex/ 存在チェック -> 無ければWorkspaceWarning
  |   |
  |   +-- --repo フィルタ（検索前フィルタ）
  |   |
  |   +-- 各リポジトリで逐次検索
  |   |   +-- SearchContext::from_path(repo.path)  [cli/search.rs]
  |   |   +-- run(&ctx, ...)  BM25検索実行（既存ロジック）
  |   |   +-- 結果をVec<SearchResult>として収集
  |   |
  |   +-- rrf_merge_multiple() で結果統合  [search/hybrid.rs]
  |   |
  |   +-- WorkspaceSearchResult に変換（repository付与）
  |   |
  |   +-- 出力
  |       human: [alias] path:line  (スコア非表示)
  |       json:  +repository フィールド
  |       path:  [alias] path
  |
  +-- [main.rs -> cli/search.rs] 単一リポモード（既存動作）
      +-- SearchContext::from_current_dir()
      +-- run(&ctx, ...)
      +-- 既存出力（変更なし）
```

---

## 15. 設計判断とトレードオフ

### 判断1: 独立インデックス vs 統合インデックス
- **選択**: 独立インデックス（各リポに.commandindex/）
- **理由**: 既存設計との整合性、各リポの独立インデックス更新、シンプルな実装
- **トレードオフ**: リポ間のスコア直接比較不可 -> RRFランク順位ベースマージで対応

### 判断2: 逐次検索 vs 並列検索（Phase 1）
- **選択**: 逐次検索
- **理由**: rayon依存追加なし、実装シンプル、mmapファイルハンドル管理容易
- **トレードオフ**: リポ数増加時のレイテンシ -> 将来のrayon導入で対応

### 判断3: SearchContext構造体 vs 引数追加
- **選択**: SearchContext構造体
- **理由**: 引数爆発防止、将来の拡張容易
- **含めるもの**: base_path（必須）、config
- **含めないもの**: OutputFormat, SnippetConfig（プレゼンテーション層）

### 判断4: WorkspaceConfigError vs ConfigError統合
- **選択**: 分離（独自エラー型）+ Display/Error trait実装
- **理由**: ワークスペース固有のエラー（エイリアス重複、パス重複、リポ数上限）がConfigErrorの責務外。SearchError::Workspaceで統合的にハンドリング。

### 判断5: スコアマージ方式
- **選択**: rrf_merge_multiple汎用関数（RRFスタイル）
- **理由**: 異なるインデックス間のBM25スコアは統計量依存で直接比較不可
- **方式**: 各リポのBM25結果をランク付け -> RRF式(1/(K+rank))でスコア計算 -> キー(path, heading)で統合
- **既存rrf_mergeとの関係**: rrf_mergeはrrf_merge_multipleのラッパーとして再定義（DRY原則）

### 判断6: Composition vs フィールド追加（SearchResult）
- **選択**: Compositionパターン（WorkspaceSearchResult）
- **理由**: SearchResultは検索エンジンの出力型であり、プレゼンテーション関心（リポジトリ帰属）を混入すべきでない（OCP: 拡張に対して開、修正に対して閉）
- **影響**: 既存のSearchResult構築箇所・テストへの変更が不要

---

## 16. 品質基準

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |

---

## 17. テスト戦略

### ユニットテスト
- WorkspaceConfig TOMLパース（正常系・異常系）
- パス解決（絶対/相対/チルダ）
- パス安全性チェック（$やバッククォート拒否）
- alias/name バリデーション（ASCII制限、長さ上限）
- ファイルサイズ上限チェック
- エイリアス/パス重複検出
- rrf_merge_multiple テストケース（**[Stage 5-7 Should Fix反映]** 具体化）:
  - 3リスト以上のマージ: 3つのVec<SearchResult>を入力し、RRFスコア順にソートされた結果を検証
  - 空リストを含むマージ: 入力に空Vecが混在しても正常動作（パニックしない、空リストは無視）
  - 同一キー(path, heading)が複数リストに存在する場合: スコアが加算される（1/(K+rank)の合計）
  - limit超過時の切り捨て: limit=5で10件の結果がある場合、上位5件のみ返却
  - 全リストが空の場合: 空Vecを返却
- rrf_merge がrrf_merge_multipleのラッパーとして正しく動作
- SearchContext生成
- WorkspaceConfigError Display実装

### 統合テスト
- 複数一時ディレクトリでのワークスペース横断検索
- --repo フィルタ動作
- status --workspace出力
- update --workspace（正常/一部失敗）
- 後方互換（--workspace未指定時の動作不変）
- conflicts_with動作（--workspace + --symbol等が排他）

### リグレッションテスト
- 既存e2eテストが--workspace未指定時にパス
- 既存のSearchResult構造体が変更されていないことの確認

---

## 18. レビュー指摘反映サマリー

本設計方針書に反映した全指摘の一覧。

### Stage 1 Must Fix
| ID | 指摘 | 反映箇所 |
|----|------|---------|
| M1(SRP) | WorkspaceConfig/WorkspaceConfigErrorをsrc/config/workspace.rsに分離 | Section 2, 3.1, 3.3, 12 |
| M2(DRY) | main.rsのconfig読込をSearchContext経由に統一 | Section 6.3 |
| M3(DRY/API) | rrf_merge_multiple汎用関数、既存rrf_mergeをラッパー化 | Section 5 |
| M4(OCP) | SearchResult変更なし、WorkspaceSearchResult compositionで定義 | Section 4, 11, 15判断6 |

### Stage 2 Must Fix
| ID | 指摘 | 反映箇所 |
|----|------|---------|
| M1 | run()の新シグネチャ（SearchContext受取）を明記 | Section 6.3 |
| M2 | SearchResult構築箇所の完全一覧 | Section 4.3 |
| M3 | 既存conflicts_with_allへのworkspace追加を網羅的に明記 | Section 6.2 |
| M4 | WorkspaceConfigErrorにDisplay/Error trait実装、SearchError::Workspace追加 | Section 3.3, 3.5 |

### Stage 3 Must Fix
| ID | 指摘 | 反映箇所 |
|----|------|---------|
| M1 | Phase 1ではrun()のみSearchContext化 | Section 6.3 |
| M2 | main.rsにworkspace有無の分岐フロー追加 | Section 6.3, 14 |
| M3 | context.rsを将来Phase対応予定として記載 | Section 3.6 |
| M4 | SearchResult構築をファクトリメソッドに集約検討 | Section 4.3 |
| M5 | json出力でSearchResultにSerialize derive追加を検討 | Section 4.3 |

### Stage 4 Must Fix
| ID | 指摘 | 反映箇所 |
|----|------|---------|
| M1 | canonicalize後の許容範囲チェック（stderr表示警告） | Section 8.1 |
| M2 | パス展開はチルダのみ、$やバッククォートを含むパスは拒否 | Section 8.2 |
| M3 | workspace設定pathのシンボリックリンクチェック（clean.rsパターン） | Section 8.3 |

### Stage 5 Must Fix
| ID | 指摘 | 反映箇所 |
|----|------|---------|
| M1 | expand_path()チルダ展開のoff-by-oneバグ修正（~単体、~/...、~user拒否） | Section 8.2 |
| M2 | run()新シグネチャを現実のコード（SearchOptions, SearchFilters等6引数）と一致させる | Section 6.3 |
| M3 | run_workspace_searchにバラした引数を渡すDRY違反修正（構造化引数で渡す） | Section 6.3 |

### Stage 7 Must Fix
| ID | 指摘 | 反映箇所 |
|----|------|---------|
| M1 | run()シグネチャ（Stage 5 M2と同一） | Section 6.3 |
| M2 | try_hybrid_search内のPath::new(".")3箇所への伝播方法明記（SearchContext渡し） | Section 6.3 |
| M3 | rrf_merge_multipleのキー衝突対策（aliasプレフィックス付与でユニーク化） | Section 5.1 |

### Stage 1-4 主要Should Fix
| 指摘 | 反映箇所 |
|------|---------|
| WorkspaceWarning分離（RepositoryNotFound/IndexNotFound） | Section 3.4 |
| Human出力をスコア非表示の既存形式に合わせる | Section 11 |
| Status/UpdateへのCLIオプション追加 | Section 6.4 |
| 実装変更順序（依存関係グラフ） | Section 13 |
| alias/nameのASCII制限・長さ上限64文字 | Section 3.2 |
| TOMLファイルサイズ上限1MB | Section 3.2, 3.3 |

### Stage 5-7 主要Should Fix
| 指摘 | 反映箇所 |
|------|---------|
| validate関数からI/O副作用(eprintln)除去、WorkspaceWarning返却のみに | Section 3.4, 8.1, 8.3 |
| output層からcli層への逆依存回避（WorkspaceSearchResult型をsrc/output/mod.rsに配置） | Section 4.2, 12 |
| validate_resolved_path()のoriginal!=resolved比較が常にtrueになる問題修正 | Section 8.1 |
| Phase 1のSearchContextからembeddings_db_path()を除外（YAGNI） | Section 3.6 |
| rrf_merge_multipleのテストケース具体化（3リスト以上、空リスト、同一キー加算） | Section 17 |
