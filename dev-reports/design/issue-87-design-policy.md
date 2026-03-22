# 設計方針書: Issue #87 --related の複数ファイル対応

## 1. Issue概要

| 項目 | 内容 |
|------|------|
| Issue番号 | #87 |
| タイトル | [Feature] --related の複数ファイル対応 |
| ラベル | enhancement |
| 実装規模 | 小 |

`search --related` オプションで複数ファイルを指定し、それぞれの影響範囲をスコア最大値でマージして返す機能を追加する。

## 2. システムアーキテクチャ概要

### 現在の呼び出しフロー（単一ファイル）

```
main.rs (CLI arg parsing)
  ↓ related: Option<String>
cli/search.rs::run_related_search(file_path: &str, limit, format)
  ↓
search/related.rs::RelatedSearchEngine::find_related(target_path: &str, limit)
  ├→ score_markdown_links()
  ├→ score_import_deps()
  ├→ score_tag_match()
  └→ score_path_proximity()
  ↓
output/mod.rs::format_related_results(&[RelatedSearchResult], format)
```

### 変更後の呼び出しフロー（複数ファイル）

```
main.rs (CLI arg parsing)
  ↓ related: Option<Vec<String>>
cli/search.rs::run_related_search(file_paths: &[String], limit, format)
  ↓
cli/context.rs::collect_related_context(files, reader, store)  ← 既存関数を再利用
  ├→ for file in files:
  │     search/related.rs::find_related(file, 1000)
  │     ├→ FileNotFound/FileNotIndexed → graceful skip
  │     └→ 他のエラー → 伝播
  └→ merge_related_results(results_per_file, files)  ← 既存関数を再利用
       ├→ union マージ（HashMap）
       ├→ スコア最大値採用
       ├→ RelationType union
       └→ target_files 除外
  ↓
output/mod.rs::format_related_results(&[RelatedSearchResult], format)  ← 変更不要
```

## 3. レイヤー構成と責務

| レイヤー | モジュール | 今回の変更 |
|---------|-----------|-----------|
| **CLI** | `src/main.rs` | clap引数定義変更（`Option<String>` → `Option<Vec<String>>`） |
| **Handler** | `src/cli/search.rs` | `run_related_search` シグネチャ変更、複数ファイル対応ロジック |
| **Handler** | `src/cli/context.rs` | `collect_related_context`, `merge_related_results` の可視性変更（`pub(crate)`） |
| **Engine** | `src/search/related.rs` | **変更不要** |
| **Output** | `src/output/mod.rs` | **変更不要** |

## 4. 設計判断とトレードオフ

### 判断1: マージロジックの再利用方式

**選択**: `context.rs` の `collect_related_context` と `merge_related_results` を `pub(crate)` に変更して再利用

**代替案**:
- A) `search/related.rs` にマージロジックを移動 → ドメイン的には適切だが、context.rs側も修正が必要で変更量が増える
- B) マージロジックをコピー → DRY違反

**理由**: 最小限の変更（可視性修正のみ）で目的を達成でき、context.rs のテスト済みロジックをそのまま活用できる。

### 判断2: clap 引数定義方式

**選択**: `num_args(1..)` を使用

```rust
#[arg(long, num_args(1..), conflicts_with_all = [...])]
related: Option<Vec<String>>,
```

**代替案**:
- A) `action = Append` で `--related a.rs --related b.rs` 形式 → 冗長だがパース境界問題なし
- B) カンマ区切り `value_delimiter = ','` → 冗長さなしだがファイル名にカンマを含む場合問題

**理由**: `num_args(1..)` はスペース区切りで自然に複数値を受け付けられる。`--format` 等のオプションフラグ（`--` prefix）で自動的にパース境界が決まるため安全。後方互換性あり（単一値もそのまま動作）。

### 判断3: 内部 limit の統一

**選択**: context.rs と同じ `limit=1000` のハードコード方式

**理由**: find_related の limit を大きくとってマージ後に effective_limit で切り詰める方式が、context コマンドと一貫性があり、マージ時の精度低下を防げる。

### 判断4: エラーハンドリング方式

**選択**: context.rs の graceful skip パターンを採用

```rust
match engine.find_related(file, 1000) {
    Ok(results) => results_per_file.push(results),
    Err(FileNotFound(_)) | Err(FileNotIndexed(_)) => {
        results_per_file.push(Vec::new());  // スキップ
    }
    Err(e) => return Err(SearchError::RelatedSearch(e)),
}
```

**理由**: 複数ファイル指定時に1ファイルのエラーで全体が失敗するのはUXが悪い。有効なファイルの結果だけ返すのが自然。

## 5. 詳細設計

### 5.1 main.rs の変更

```rust
// Before
#[arg(long, conflicts_with_all = ["query", "symbol", "semantic", "tag", "path", "file_type", "heading", "workspace"])]
related: Option<String>,

// After
/// Search for related files
#[arg(long, num_args(1..), conflicts_with_all = ["query", "symbol", "semantic", "tag", "path", "file_type", "heading", "workspace"])]
related: Option<Vec<String>>,
```

パターンマッチ:
```rust
// Before
(None, None, Some(f), None) => {
    commandindex::cli::search::run_related_search(&f, effective_limit, format)
}

// After
(None, None, Some(ref files), None) => {
    commandindex::cli::search::run_related_search(files, effective_limit, format)
}
```

### 5.2 cli/search.rs の変更

```rust
// Before
pub fn run_related_search(
    file_path: &str,
    limit: usize,
    format: OutputFormat,
) -> Result<(), SearchError>

// After
pub fn run_related_search(
    file_paths: &[String],
    limit: usize,
    format: OutputFormat,
) -> Result<(), SearchError>
```

処理フロー:
1. 入力検証（空スライス、各パスの空文字・長さチェック、パストラバーサル防止、ファイル数上限）
2. インデックス存在確認
3. IndexReaderWrapper と SymbolStore をオープン
4. `context::collect_related_context(file_paths, &reader, &store)` を呼び出し
5. マージ済み結果を `limit` で切り詰め
6. 結果が空なら stderr メッセージ表示
7. `format_related_results()` で出力

### 5.4 入力検証の共通化

context.rs の `run_context` (L41-55) と同等のパストラバーサル防止検証を `run_related_search` にも適用する。DRY原則に従い、共通のバリデーション関数を抽出する。

```rust
/// ファイルパスリストの共通バリデーション
/// - 空スライスチェック
/// - ファイル数上限チェック（max 100）
/// - 各パス: 空文字チェック、長さチェック、".." 禁止、絶対パス禁止、バックスラッシュ禁止
fn validate_file_paths(file_paths: &[String], max_files: usize) -> Result<(), SearchError> {
    if file_paths.is_empty() {
        return Err(SearchError::InvalidArgument("No files specified".to_string()));
    }
    if file_paths.len() > max_files {
        return Err(SearchError::InvalidArgument(
            format!("Too many files specified: {} (max: {})", file_paths.len(), max_files)
        ));
    }
    for path in file_paths {
        if path.is_empty() || path.len() > 1024 {
            return Err(SearchError::InvalidArgument(...));
        }
        if path.contains("..") || path.starts_with('/') || path.contains('\\') {
            return Err(SearchError::InvalidArgument(...));
        }
    }
    Ok(())
}
```

**設計判断**: この関数を `cli/mod.rs` に `pub(crate)` で配置し、`run_context` の既存インラインバリデーションも置き換えて統一する。実装時は `validate_file_paths` 導入とシグネチャ変更を同一コミットで行い、バリデーション未適用の中間状態を防ぐ。

### 5.3 cli/context.rs の変更

可視性のみ変更:
```rust
// Before
fn collect_related_context(...) -> Result<Vec<RelatedSearchResult>, SearchError>
fn merge_related_results(...) -> Vec<RelatedSearchResult>

// After
/// Collects related context for multiple files.
/// Caller must validate file_paths before calling (see validate_file_paths).
pub(crate) fn collect_related_context(...) -> Result<Vec<RelatedSearchResult>, SearchError>
/// Merges related search results from multiple files using union + max score strategy.
pub(crate) fn merge_related_results(...) -> Vec<RelatedSearchResult>
```

**注意**: pub(crate) 化により検証なし呼び出しが可能になるため、docコメントで `validate_file_paths` による事前検証が必須であることを明記する。`normalize_path` は `..` セグメントを除去するのみでエラーにしないため、パストラバーサル防止は `validate_file_paths` が担う（防御の多層化）。

## 6. 型定義（変更なし）

```rust
// output/mod.rs - そのまま利用
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
}
```

## 7. エラー型（変更なし）

```rust
// search/related.rs
pub enum RelatedSearchError {
    Reader(ReaderError),
    SymbolStore(SymbolStoreError),
    FileNotFound(String),
    FileNotIndexed(String),
}

// cli/search.rs
pub enum SearchError {
    ...
    RelatedSearch(RelatedSearchError),
    ...
}
```

## 8. セキュリティ設計

| 脅威 | 対策 | 状態 |
|------|------|------|
| パストラバーサル | 共通バリデーション関数で `..` 禁止・絶対パス禁止・`\` 禁止 + `normalize_path` による正規化 | 対応予定（5.4参照） |
| 大量ファイル指定によるDoS | ファイル数上限チェック（max 100、context.rs と統一） | 対応予定（5.4参照） |
| unsafe使用 | 使用しない | - |

## 9. 影響範囲

| ファイル | 変更内容 | リスク |
|---------|---------|-------|
| `src/main.rs` | clap定義変更（2箇所） | 低: 後方互換あり |
| `src/cli/search.rs` | シグネチャ変更、ロジック変更 | 低: テストで検証 |
| `src/cli/context.rs` | 可視性変更のみ（`pub(crate)`） | 極低: 動作変更なし |
| `tests/cli_args.rs` | 複数ファイル用テスト追加 | なし: 追加のみ |
| `tests/e2e_related_search.rs` | 複数ファイルE2Eテスト追加 | なし: 追加のみ |

**変更不要ファイル**: `src/search/related.rs`, `src/output/mod.rs`, `Cargo.toml`

## 10. テスト戦略

### CLIパーステスト（cli_args.rs）
- `--related file1.rs file2.rs` が `Vec!["file1.rs", "file2.rs"]` にパースされること
- `--related file.rs` の後方互換（単一値が `Vec!["file.rs"]`）
- `--related file1.rs file2.rs --format json` のパース境界
- `--related file1.rs --symbol foo` の排他制約

### E2Eテスト（e2e_related_search.rs）
- 複数ファイル指定で union マージされること
- 重複ファイルのスコアが最大値で統合されること
- 単一ファイル指定時の既存動作が変わらないこと
- 存在しないファイルを含む複数指定で graceful skip
- human / json / path 各出力形式

## 11. 品質基準

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |
