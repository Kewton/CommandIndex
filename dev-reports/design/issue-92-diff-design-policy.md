# 設計方針書 - Issue #92: diff サブコマンド

## 1. Issue概要

| 項目 | 内容 |
|------|------|
| Issue | #92 [Feature] diff サブコマンド（影響範囲の比較・共通ファイル検出） |
| 種別 | 新機能追加 |
| 規模 | 中 |

2つのファイルの影響範囲（関連ファイル）を比較し、共通ファイル（コンフリクトリスク）を検出する `diff` サブコマンドを追加する。

---

## 2. システムアーキテクチャ概要

### レイヤー構成と diff サブコマンドの位置づけ

```
┌────────────────────────────────────────────────────┐
│  CLI Layer (src/main.rs)                           │
│  ┌──────────────────────────────────────────────┐  │
│  │ Commands::Diff { files, format, limit }      │  │ ← 新規追加
│  └──────────────────────────────────────────────┘  │
├────────────────────────────────────────────────────┤
│  CLI Handler Layer (src/cli/)                      │
│  ┌──────────────────────────────────────────────┐  │
│  │ src/cli/diff.rs :: run_diff()                │  │ ← 新規作成
│  │   - バリデーション                            │  │
│  │   - RelatedSearchEngine 呼び出し（2回）       │  │
│  │   - 集合演算（only_a, only_b, overlap）       │  │
│  │   - 出力委譲                                  │  │
│  └──────────────────────────────────────────────┘  │
├────────────────────────────────────────────────────┤
│  Search Layer (src/search/)                        │
│  ┌──────────────────────────────────────────────┐  │
│  │ RelatedSearchEngine::find_related() [既存]    │  │ ← 変更なし
│  │ normalize_path() [既存]                       │  │ ← 変更なし
│  └──────────────────────────────────────────────┘  │
├────────────────────────────────────────────────────┤
│  Output Layer (src/output/)                        │
│  ┌──────────────────────────────────────────────┐  │
│  │ DiffResult 型 [新規]                         │  │ ← 新規追加
│  │ format_diff_results() [新規]                  │  │ ← 新規追加
│  │ human/json/path フォーマッタ [新規]            │  │ ← 新規追加
│  └──────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────┘
```

---

## 3. 設計判断とトレードオフ

### 判断1: 独立サブコマンド vs Search サブオプション

**決定**: 独立サブコマンド（`Commands::Diff`）として実装

**理由**:
- Search サブコマンドは既に `query`, `symbol`, `related`, `semantic` の4モードが `conflicts_with_all` で排他制御されており複雑
- diff は2つのファイルパスを受け取るため、Search の単一 `--related <FILE>` とはインターフェースが異なる
- Context サブコマンド（`files: Vec<String>`）と同様の独立サブコマンドパターンが適切

### 判断2: 集合演算の実装方式

**決定**: `find_related()` を2回呼び出し、ファイルパスの集合演算で only_a / only_b / overlap を算出

**理由**:
- `find_related()` は既に十分なスコアリングを実装済み（6種の関係タイプ）
- context.rs の `collect_related_context()` が同パターンで reader/store を共有する実装済み
- diff 固有のロジックは集合演算のみで、新たなスコアリングは不要

**トレードオフ**:
- `find_related()` 2回実行のコスト → reader/store インスタンス共有で最小化（context.rs L92 パターン踏襲）
- `merge_related_results()` は union 目的であり diff の intersection/difference には直接使えない → diff 固有の集合演算を新規実装

### 判断3: エラーハンドリング方針

**決定**: context.rs の寛容設計（スキップ）ではなく、厳密検証（即エラー終了）

**理由**:
- diff は2ファイルの比較が目的。片方の結果が欠損すると比較結果が無意味
- context.rs は「利用可能な情報を最大限収集」が目的であり設計思想が異なる

### 判断4: JSON出力形式

**決定**: 単一JSONオブジェクト（既存のJSONL形式とは異なる）

**理由**:
- diff 結果は1つの比較結果であり、複数行のストリーミング出力は不要
- Context サブコマンドも単一JSONオブジェクト（ContextPack）を出力する前例あり

---

## 4. 詳細設計

### 4.1 CLI定義（src/main.rs）

`--format` オプションは Search サブコマンドのパターンを踏襲（Context には `--format` がない点に注意）。

```rust
/// Compare related files between two files (conflict detection)
Diff {
    /// Two files to compare
    #[arg(required = true, num_args = 2)]
    files: Vec<String>,

    /// Output format
    #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
    format: OutputFormat,

    /// Maximum related files per input file
    #[arg(long, default_value = "100", value_parser = clap::value_parser!(usize).range(1..=10000))]
    limit: usize,
},
```

main.rs のマッチアーム:
```rust
Commands::Diff { files, format, limit } => {
    commandindex::cli::diff::run_diff(&files, limit, format)
}
```

### 4.2 メインロジック（src/cli/diff.rs）

`normalize_path` は `use crate::search::related::normalize_path;` でインポート。
`RelatedSearchError` → `SearchError` 変換は既存の `From<RelatedSearchError> for SearchError` impl（`?` 演算子）を利用。

```rust
use std::collections::HashSet;
use std::path::Path;

use crate::indexer::IndexReaderWrapper;
use crate::indexer::symbol_store::SymbolStore;
use crate::output::{self, DiffResult, OutputFormat};
use crate::search::related::{normalize_path, RelatedSearchEngine};
use crate::cli::search::SearchError;

pub fn run_diff(
    files: &[String],
    limit: usize,
    format: OutputFormat,
) -> Result<(), SearchError> {
    // 1. バリデーション（context.rs L30-55 と同等のパスバリデーション）
    let file_a = &files[0];
    let file_b = &files[1];

    // パスバリデーション: 空パス、長さ制限は normalize_path() が処理
    // 追加バリデーション: 絶対パス拒否、.. 含有拒否（context.rs パターン踏襲）
    for file in [file_a, file_b] {
        if file.starts_with('/') {
            return Err(SearchError::InvalidArgument(
                format!("Absolute paths are not supported: {file}")
            ));
        }
        if file.contains("..") {
            return Err(SearchError::InvalidArgument(
                format!("Path traversal is not allowed: {file}")
            ));
        }
    }

    // normalize_path() で正規化後に同一ファイルチェック
    let norm_a = normalize_path(file_a)?;
    let norm_b = normalize_path(file_b)?;
    if norm_a == norm_b {
        return Err(SearchError::InvalidArgument(
            format!("Cannot diff a file with itself: {norm_a}")
        ));
    }

    // 2. インデックス存在チェック
    let tantivy_dir = crate::indexer::index_dir(Path::new("."));
    if !tantivy_dir.exists() {
        return Err(SearchError::IndexNotFound);
    }
    let db_path = crate::indexer::symbol_db_path(Path::new("."));
    if !db_path.exists() {
        return Err(SearchError::SymbolDbNotFound);
    }

    // 3. reader/store 共有（context.rs L92 パターン踏襲）
    let reader = IndexReaderWrapper::open(&tantivy_dir)?;
    let store = SymbolStore::open(&db_path)?;
    let engine = RelatedSearchEngine::new(&reader, &store);

    // 4. 各ファイルの関連検索（エラー時は即終了、context.rs のスキップ設計とは異なる）
    // find_related() エラー時、map_err でどちらのファイルか明示
    let results_a = engine.find_related(file_a, limit)
        .map_err(|e| SearchError::InvalidArgument(
            format!("Error analyzing '{}': {}", file_a, e)
        ))?;
    let results_b = engine.find_related(file_b, limit)
        .map_err(|e| SearchError::InvalidArgument(
            format!("Error analyzing '{}': {}", file_b, e)
        ))?;

    // 5. 集合演算（標準ライブラリの sort() を使用、itertools 不要）
    let paths_a: HashSet<String> = results_a.iter().map(|r| r.file_path.clone()).collect();
    let paths_b: HashSet<String> = results_b.iter().map(|r| r.file_path.clone()).collect();

    let mut overlap: Vec<String> = paths_a.intersection(&paths_b).cloned().collect();
    overlap.sort();
    let mut only_a: Vec<String> = paths_a.difference(&paths_b).cloned().collect();
    only_a.sort();
    let mut only_b: Vec<String> = paths_b.difference(&paths_a).cloned().collect();
    only_b.sort();

    // 6. 出力
    let diff_result = DiffResult {
        file_a: norm_a,
        file_b: norm_b,
        only_a,
        only_b,
        overlap,
    };

    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    output::format_diff_results(&diff_result, format, &mut handle)?;
    Ok(())
}
```

### 4.3 出力型（src/output/mod.rs）

`DiffResult` は JSON 出力で `serde_json::json!` マクロを使用するため `Serialize` derive は不要。

> **NOTE**: diff 追加でフォーマット関数が6種類目となり、`output/mod.rs` L177 の NOTE コメント（trait-based リファクタリング検討閾値）を超過する。今回は現行パターンを踏襲し、次回の出力型追加時にリファクタリングを検討する。

```rust
#[derive(Debug, Clone)]
pub struct DiffResult {
    pub file_a: String,
    pub file_b: String,
    pub only_a: Vec<String>,
    pub only_b: Vec<String>,
    pub overlap: Vec<String>,
}

pub fn format_diff_results(
    result: &DiffResult,
    format: OutputFormat,
    writer: &mut dyn Write,
) -> Result<(), OutputError> {
    match format {
        OutputFormat::Human => human::format_diff_human(result, writer),
        OutputFormat::Json => json::format_diff_json(result, writer),
        OutputFormat::Path => path::format_diff_path(result, writer),
    }
}
```

### 4.4 Human形式出力（src/output/human.rs）

```rust
pub fn format_diff_human(
    result: &DiffResult,
    writer: &mut dyn Write,
) -> Result<(), OutputError> {
    writeln!(writer, "=== Diff: {} vs {} ===", result.file_a, result.file_b)?;
    writeln!(writer)?;

    writeln!(writer, "Only in {}:", result.file_a)?;
    if result.only_a.is_empty() {
        writeln!(writer, "  (none)")?;
    } else {
        for path in &result.only_a {
            writeln!(writer, "  {}", path)?;
        }
    }
    writeln!(writer)?;

    writeln!(writer, "Only in {}:", result.file_b)?;
    if result.only_b.is_empty() {
        writeln!(writer, "  (none)")?;
    } else {
        for path in &result.only_b {
            writeln!(writer, "  {}", path)?;
        }
    }
    writeln!(writer)?;

    writeln!(writer, "Overlap ({} files):", result.overlap.len())?;
    if result.overlap.is_empty() {
        writeln!(writer, "  (none)")?;
    } else {
        for path in &result.overlap {
            writeln!(writer, "  {}", path)?;
        }
    }
    Ok(())
}
```

### 4.5 JSON形式出力（src/output/json.rs）

```rust
pub fn format_diff_json(
    result: &DiffResult,
    writer: &mut dyn Write,
) -> Result<(), OutputError> {
    let json_value = serde_json::json!({
        "file_a": result.file_a,
        "file_b": result.file_b,
        "only_a": result.only_a,
        "only_b": result.only_b,
        "overlap": result.overlap,
        "overlap_count": result.overlap.len(),
    });
    serde_json::to_writer_pretty(&mut *writer, &json_value)?;
    writeln!(writer)?;
    Ok(())
}
```

### 4.6 Path形式出力（src/output/path.rs）

```rust
pub fn format_diff_path(
    result: &DiffResult,
    writer: &mut dyn Write,
) -> Result<(), OutputError> {
    for path in &result.overlap {
        writeln!(writer, "{}", path)?;
    }
    Ok(())
}
```

---

## 5. エラーハンドリング設計

| エラーケース | 検出方法 | エラー型 | メッセージ |
|-------------|---------|---------|----------|
| インデックス未作成 | `tantivy_dir.exists()` | `SearchError::IndexNotFound` | 既存メッセージ |
| Symbol DB未作成 | `db_path.exists()` | `SearchError::SymbolDbNotFound` | 既存メッセージ |
| ファイル未インデックス | `find_related()` エラー | `SearchError::RelatedSearch(FileNotIndexed)` | "File not indexed: {path}" |
| 同一ファイル指定 | `normalize_path()` 後に比較 | `SearchError::InvalidArgument` | "Cannot diff a file with itself: {path}" |
| 空パス指定 | `normalize_path()` チェック | `SearchError::RelatedSearch(FileNotFound)` | "File not found: empty path" |

---

## 6. 影響範囲

### 変更対象ファイル

| ファイル | 変更種別 | 変更内容 |
|---------|---------|---------|
| `src/main.rs` | 修正 | Commands enum に Diff バリアント追加、マッチアーム追加 |
| `src/cli/mod.rs` | 修正 | `pub mod diff;` 宣言追加 |
| `src/cli/diff.rs` | **新規** | diff サブコマンドのメインロジック |
| `src/output/mod.rs` | 修正 | `DiffResult` 型と `format_diff_results()` 関数追加 |
| `src/output/human.rs` | 修正 | `format_diff_human()` 追加 |
| `src/output/json.rs` | 修正 | `format_diff_json()` 追加 |
| `src/output/path.rs` | 修正 | `format_diff_path()` 追加 |
| `tests/cli_args.rs` | 修正 | help テストに diff 検証追加 |
| `tests/e2e_diff.rs` | **新規** | E2Eテスト |

### 変更しないファイル

| ファイル | 理由 |
|---------|------|
| `src/search/related.rs` | `find_related()`, `normalize_path()` をそのまま利用 |
| `src/cli/search.rs` | `run_related_search()` は変更不要 |
| `src/cli/context.rs` | `merge_related_results()` は diff 用途には流用しない（union vs intersection の違い） |
| `Cargo.toml` | 新たな外部依存不要 |

---

## 7. テスト戦略

### E2Eテスト（tests/e2e_diff.rs）

| テストケース | 検証内容 |
|-------------|---------|
| `diff_overlap_detected` | 共通関連ファイルが overlap に含まれる |
| `diff_only_a_only_b_correct` | 片方のみの関連ファイルが正しく分類 |
| `diff_no_overlap` | 重複なし（overlap 空） |
| `diff_complete_overlap` | 完全一致（only_a, only_b 空） |
| `diff_self_excluded` | 入力ファイル自身が結果に含まれない |
| `diff_json_format` | JSON出力構造の検証 |
| `diff_human_format` | Human出力形式の検証 |
| `diff_path_format` | Path出力（overlap のみ）の検証 |
| `diff_same_file_error` | 同一ファイル指定時のエラー |
| `diff_no_index_error` | インデックス未作成時のエラー |
| `diff_file_not_indexed_error` | ファイル未インデックス時のエラー |

### テストデータ構成

```
docs/
├── a.md  (tags: auth, security) → links to b.md, c.md
├── b.md  (tags: auth)           → links to a.md
├── c.md  (tags: security)       → links to a.md
└── d.md  (tags: unrelated)      → no links
```

- `diff a.md b.md` → overlap に c.md（共通リンク先）、only_a に固有関連、only_b に固有関連
- `diff a.md d.md` → overlap 空（無関連ファイル同士）

---

## 8. セキュリティ設計

| 脅威 | 対策 |
|------|------|
| パストラバーサル | `..` 含有パスを事前拒否（context.rs パターン踏襲） + `normalize_path()` で除去 |
| 絶対パス | `/` 始まりのパスを事前拒否（context.rs パターン踏襲） |
| 長大パス入力 | 1024文字制限（`normalize_path()` 既存実装） |
| 空パス入力 | `normalize_path()` でバリデーション（既存実装） |
| limit 過大値 | `clap::value_parser` で 1..=10000 に制限 |
| unsafe使用 | なし（Rust標準ライブラリのみ使用） |

---

## 9. 品質基準

| チェック項目 | コマンド | 基準 |
|-------------|---------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |
