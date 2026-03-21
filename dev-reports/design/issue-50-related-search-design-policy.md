# 設計方針書: Issue #50 --related 検索オプション実装

## 1. Issue情報

| 項目 | 内容 |
|------|------|
| Issue番号 | #50 |
| タイトル | [Feature] --related 検索オプション実装（関連ドキュメント・コード検索） |
| ラベル | enhancement |
| 依存Issue | #9, #36, #37, #51（全てマージ済み） |

## 2. システムアーキテクチャ概要

CommandIndex は Markdown・Code・Git を横断するローカルナレッジ検索CLI。本Issueでは Phase 4 (Context Retrieval) の中核機能として、指定ファイルに関連するドキュメント・コードを検索する `--related` オプションを追加する。

### データフロー

```
                    ┌─────────────────────────┐
                    │     CLI Layer            │
                    │  main.rs (--related)     │
                    └──────────┬──────────────┘
                               │
                    ┌──────────▼──────────────┐
                    │  Orchestration Layer     │
                    │  cli/search.rs           │
                    │  run_related_search()    │
                    └──────────┬──────────────┘
                               │
              ┌────────────────┼────────────────┐
              │                │                │
    ┌─────────▼────┐  ┌───────▼───────┐  ┌────▼──────────┐
    │ search/      │  │ indexer/      │  │ indexer/       │
    │ related.rs   │  │ reader.rs    │  │ symbol_store.rs│
    │ スコアリング  │  │ タグ取得     │  │ リンク・依存   │
    └──────────────┘  └───────────────┘  └────────────────┘
                               │
                    ┌──────────▼──────────────┐
                    │  Output Layer            │
                    │  output/mod.rs           │
                    │  format_related_results()│
                    └─────────────────────────┘
```

## 3. レイヤー構成と責務

| レイヤー | モジュール | 責務 | 変更種別 |
|---------|-----------|------|----------|
| **CLI** | `src/main.rs` | `--related` オプション追加、排他制御、分岐ロジック | 既存変更 |
| **CLI/Search** | `src/cli/search.rs` | `run_related_search()` オーケストレーション | 既存変更 |
| **Search** | `src/search/related.rs` | スコアリングロジック、パス近接性計算 | **新規** |
| **Indexer/Reader** | `src/indexer/reader.rs` | `search_by_exact_path()` タグ取得 | 既存変更 |
| **Indexer/Store** | `src/indexer/symbol_store.rs` | `find_imports_by_source()`, `find_file_links_by_target()` | 既存変更 |
| **Output** | `src/output/mod.rs` | `RelatedSearchResult`, `format_related_results()` | 既存変更 |

## 4. 技術選定

既存技術スタックのみで実装可能。Cargo.toml への新規crate追加は不要。

| 使用技術 | 用途 |
|---------|------|
| tantivy TermQuery | path完全一致でタグ取得 |
| rusqlite | リンク・import逆引きクエリ |
| clap conflicts_with | CLI排他制御 |
| serde_json | JSON出力フォーマット |

## 5. 設計パターン

### 5.1 CLI引数定義

```rust
// src/main.rs - Search サブコマンド
Search {
    query: Option<String>,
    #[arg(long, conflicts_with = "query")]
    symbol: Option<String>,
    #[arg(long, conflicts_with_all = ["query", "symbol", "tag", "path", "file_type", "heading"])]
    related: Option<String>,  // 新規追加（既存フィルタとも排他）
    // ... 既存フィールド
}
```

> `--related` は `--tag`, `--path`, `--type`, `--heading` とも排他にする（サイレント無視ではなくエラー）。

分岐ロジック:
```rust
match (query, symbol, related) {
    (Some(q), None, None) => run(..),              // 全文検索
    (None, Some(s), None) => run_symbol_search(..), // シンボル検索
    (None, None, Some(f)) => run_related_search(..), // 関連検索（新規）
    (None, None, None) => error,
    _ => unreachable!(),  // clap conflicts_with で防止
}
```

### 5.2 関連検索結果型

```rust
// src/output/mod.rs
pub struct RelatedSearchResult {
    pub file_path: String,
    pub score: f32,
    pub relation_types: Vec<RelationType>,
}

pub enum RelationType {
    MarkdownLink,
    ImportDependency,
    TagMatch { matched_tags: Vec<String> },
    PathSimilarity,
    DirectoryProximity,
    // SymbolKeywordMatch は実装時に追加（YAGNI）
}
```

### 5.3 スコアリングエンジン

```rust
// src/search/related.rs
pub struct RelatedSearchEngine<'a> {
    reader: &'a IndexReaderWrapper,
    store: &'a SymbolStore,
}

impl<'a> RelatedSearchEngine<'a> {
    pub fn find_related(
        &self,
        target_path: &str,
        limit: usize,
    ) -> Result<Vec<RelatedSearchResult>, RelatedSearchError>;

    // 各スコアリング要素（pub(crate) で個別テスト可能）
    pub(crate) fn score_markdown_links(&self, target: &str) -> Result<HashMap<String, f32>, ..>;
    pub(crate) fn score_import_deps(&self, target: &str) -> Result<HashMap<String, f32>, ..>;
    pub(crate) fn score_tag_match(&self, target: &str) -> Result<HashMap<String, f32>, ..>;
    pub(crate) fn score_path_proximity(&self, target: &str) -> HashMap<String, f32>;
}

// スコア重み定数
pub const MARKDOWN_LINK_WEIGHT: f32 = 1.0;
pub const IMPORT_DEP_WEIGHT: f32 = 0.9;
pub const TAG_MATCH_WEIGHT: f32 = 0.5;  // × 一致タグ数
pub const PATH_SIMILARITY_WEIGHT: f32 = 0.4;
pub const DIR_PROXIMITY_WEIGHT: f32 = 0.2;
pub const DIR_PROXIMITY_1UP_WEIGHT: f32 = 0.1;
```

### 5.4 エラー型

```rust
// src/search/related.rs
pub enum RelatedSearchError {
    Reader(ReaderError),
    SymbolStore(SymbolStoreError),
    FileNotFound(String),      // 指定ファイルが存在しない
    FileNotIndexed(String),    // 指定ファイルがインデックス未登録
}
```

SearchError に統合（From impl で自動変換）:
```rust
// src/cli/search.rs に追加
RelatedSearch(RelatedSearchError),

impl From<RelatedSearchError> for SearchError {
    fn from(e: RelatedSearchError) -> Self {
        SearchError::RelatedSearch(e)
    }
}
```

### 5.5 SymbolStore 追加メソッド

```rust
// src/indexer/symbol_store.rs

/// 指定ファイルが何をインポートしているかを取得（source_file → target_module の方向）
/// dependencies テーブルの source_file カラムで検索
pub fn find_imports_by_source(
    &self,
    source_file: &str,
) -> Result<Vec<ImportInfo>, SymbolStoreError>;

/// 指定ファイルを参照しているリンク元一覧を取得（target_file → source_file の方向）
/// file_links テーブルの target_file カラムで検索
pub fn find_file_links_by_target(
    &self,
    target_file: &str,
) -> Result<Vec<FileLinkInfo>, SymbolStoreError>;
```

既存インデックス `idx_deps_source` と `idx_file_links_target` を活用。新規テーブル追加不要。

**セマンティクス整理**:
| メソッド | 検索方向 | テーブル | 検索カラム | 用途 |
|---------|---------|---------|-----------|------|
| `find_file_links_by_source()` | source→target | file_links | source_file | 指定ファイルからのリンク先一覧（既存） |
| `find_file_links_by_target()` | target→source | file_links | target_file | 指定ファイルを参照しているリンク元一覧（**新規**） |
| `find_imports_by_target()` | target→source | dependencies | target_module | 指定モジュールをインポートしているファイル一覧（既存） |
| `find_imports_by_source()` | source→target | dependencies | source_file | 指定ファイルのインポート先一覧（**新規**） |

### 5.6 IndexReaderWrapper 追加メソッド

```rust
// src/indexer/reader.rs
pub fn search_by_exact_path(
    &self,
    path: &str,
) -> Result<Vec<SearchResult>, ReaderError>;
```

tantivy TermQuery で path フィールドの完全一致検索を実行。

## 6. セキュリティ設計

| 脅威 | 対策 | 優先度 |
|------|------|--------|
| パストラバーサル | `--related` 入力パスの正規化（`./` 除去、パス区切り統一） | 高 |
| 大量結果によるDoS | `--limit` による結果件数制限（デフォルト20） | 中 |
| SQLインジェクション | rusqlite のパラメータバインド使用（既存パターン踏襲） | 高 |
| unsafe使用 | 原則禁止 | 中 |

### パス正規化

```rust
// src/search/related.rs（将来的に共通ユーティリティに移動を検討）
fn normalize_path(path: &str) -> Result<String, RelatedSearchError> {
    // 入力長バリデーション
    if path.is_empty() {
        return Err(RelatedSearchError::FileNotFound("empty path".to_string()));
    }
    if path.len() > 1024 {
        return Err(RelatedSearchError::FileNotFound("path too long".to_string()));
    }
    // パス正規化
    let path = path.strip_prefix("./").unwrap_or(path);
    let path = path.replace('\\', "/");
    let path = path.trim_end_matches('/');
    // 親ディレクトリ参照の排除
    let components: Vec<&str> = path.split('/').filter(|c| *c != "." && *c != "..").collect();
    Ok(components.join("/"))
}
```

## 7. 設計判断とトレードオフ

### 判断1: スコアリングロジックの分離

**決定**: `src/search/related.rs` に分離
**理由**: cli/search.rs は既に171行。3つのデータソース（tantivy, SQLite, ファイルシステム）を横断するスコアリングロジックを追加すると300行超になる見込み。関心の分離により保守性を維持。
**トレードオフ**: モジュール数の増加。ただし、`src/search/` は CLAUDE.md のモジュール構成で既に想定されている。

### 判断2: タグ取得に tantivy を使用

**決定**: tantivy TermQuery で path 完全一致 → tags フィールド読み出し
**代替案**: SQLite に別テーブル追加
**理由**: 既にtantivyにtagsが格納されている。SQLiteに重複保存する必要なし。TermQuery は O(1) で高速。
**トレードオフ**: tantivy のセクション単位でタグが格納されるため、1ファイルで複数セクションのタグを集約する必要がある。

### 判断3: スコアの単純加算方式

**決定**: 各判定基準のスコアを単純加算
**代替案**: 重み付き幾何平均、max方式
**理由**: 複数の関連性がある場合にスコアが積み上がる方が直感的。初期実装として最もシンプル。チューニングは後続Issueに委譲。
**トレードオフ**: 多くのタグを持つファイルが不当に高スコアになる可能性がある（タグスコア = 0.5 × 一致数）。上限設定を検討。

### 判断4: パス解決の簡易方式

**決定**: target_module の部分文字列がファイルパスに含まれるかで判定
**代替案**: TypeScript/Python固有のモジュール解決アルゴリズム
**理由**: 厳密なモジュール解決は言語依存で複雑。初期実装では部分一致で十分な精度が得られる。
**トレードオフ**: 偽陽性の可能性（例: `utils` が複数ファイルにマッチ）。

### 判断5: 既存フィルタの非適用

**決定**: `--related` 指定時、`--tag`, `--path`, `--type`, `--heading` は適用しない
**理由**: 関連検索は独自のスコアリングロジックで結果を返すため、既存フィルタとの組み合わせは意味的に不整合。将来的に `--related` と query の組み合わせは別Issueで検討。
**トレードオフ**: ユーザーが関連結果をさらに絞り込みたい場合に対応できない。

## 8. 影響範囲

### 変更対象ファイル

| ファイル | 変更内容 | 影響度 |
|---------|---------|--------|
| `src/main.rs` | `--related` オプション追加、分岐ロジック | 低 |
| `src/cli/search.rs` | `run_related_search()` 追加、SearchError拡張 | 中 |
| `src/search/related.rs` | **新規**: スコアリングエンジン | - |
| `src/search/mod.rs` | **新規**: モジュール宣言 | - |
| `src/lib.rs` | `pub mod search;` 追加 | 低 |
| `src/indexer/reader.rs` | `search_by_exact_path()` 追加 | 低 |
| `src/indexer/symbol_store.rs` | 2メソッド追加 | 低 |
| `src/output/mod.rs` | `RelatedSearchResult`, `format_related_results()` | 中 |

### 既存機能への影響

- 全文検索（`search <query>`）: **影響なし**。clap の conflicts_with による排他で既存パスは変更なし
- シンボル検索（`search --symbol`）: **影響なし**
- インデックス構築（`index`）: **影響なし**。既存のインデックス構造は変更しない
- symbols.db スキーマ: **変更なし**。CURRENT_SYMBOL_SCHEMA_VERSION = 2 を維持

### テスト追加計画

| テストファイル | テスト内容 | テスト戦略 |
|--------------|-----------|-----------|
| `tests/cli_args.rs` | `--related` パーステスト、排他制御テスト | CLIパーステスト |
| `tests/e2e_related_search.rs` | **新規**: E2E関連検索テスト（index→search --related） | 統合テスト |
| `src/search/related.rs` | スコアリングユニットテスト（#[cfg(test)]） | ユニットテスト |
| `src/indexer/reader.rs` | `search_by_exact_path()` テスト（`from_index()`使用） | ユニットテスト |
| `src/indexer/symbol_store.rs` | `find_imports_by_source()`, `find_file_links_by_target()` テスト | ユニットテスト |

## 9. 品質基準

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |

## 10. 実装順序（推奨）

1. `src/indexer/symbol_store.rs`: `find_imports_by_source()`, `find_file_links_by_target()` 追加 + テスト
2. `src/indexer/reader.rs`: `search_by_exact_path()` 追加 + テスト
3. `src/search/related.rs`: スコアリングエンジン実装 + ユニットテスト
4. `src/output/mod.rs`: `RelatedSearchResult`, `format_related_results()` 追加
5. `src/main.rs` + `src/cli/search.rs`: CLI統合
6. `tests/`: E2Eテスト
7. 品質チェック（clippy, fmt, test）
