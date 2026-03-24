# 設計方針書: Issue #127 - suggest のキーワード部分一致によるスコアリング改善

## 1. Issue概要

| 項目 | 内容 |
|------|------|
| Issue番号 | #127 |
| タイトル | [BUG] suggest の英語入力でキーワード部分一致により無関係ファイルを推薦 |
| 深刻度 | 中 |
| スコープ | Phase 1: BM25検索後のポストプロセスでファイル種別に応じたスコア減衰 |

## 2. 現状分析

### 2.1 現在のデータフロー

```
suggest --for "query"
  → validate_input()
  → search_entry_files()
      → reader.search(query, BM25_SEARCH_LIMIT=20)  [heading, body, tags等価検索]
      → deduplicate_by_file(results, DEDUP_FILE_LIMIT=5)  [max score per file]
  → build_strategy() or build_fallback_strategy()
  → format_suggest_results()
```

### 2.2 問題点

1. `reader.search()` が heading/body/tags を等価重みで BM25 検索するため、"display" 等の汎用語がテストファイルの本文にマッチしスコアが高くなる
2. `deduplicate_by_file()` はファイル種別を考慮しない（max scoreのみ）
3. テストファイル/ドキュメントファイルへのスコアペナルティ機構なし

## 3. 設計方針

### 3.1 変更対象の限定

| ファイル | 変更 | 理由 |
|---------|------|------|
| `src/cli/suggest.rs` | **変更** | ポストプロセスにスコア減衰を追加 |
| `src/indexer/reader.rs` | **変更なし** | search/impact/related への影響を防止 |
| `src/indexer/schema.rs` | **変更なし** | スキーマ変更は不要 |
| `src/search/hybrid.rs` | **変更なし** | Phase 2 で検討 |

### 3.2 アーキテクチャ: パイプライン拡張

変更後のデータフロー:

```
suggest --for "query"
  → validate_input()
  → search_entry_files()
      → reader.search(query, BM25_SEARCH_LIMIT=20)
      → deduplicate_by_file(results, BM25_SEARCH_LIMIT)  ← limit拡大（全件dedup）
      → apply_file_type_weight(deduped)                   ← 新規追加（weight適用+再ソート）
      → truncate(DEDUP_FILE_LIMIT)                        ← weight適用後にtruncate
  → build_strategy() or build_fallback_strategy()
  → format_suggest_results()
```

**設計変更（レビュー反映）**: dedup(limit=5)→weight の順序では、BM25上位5件がテストファイルで占有された場合に6位以下のソースファイルが救済されない。そのためdedup時のlimitを拡大し、weight適用後にDEDUP_FILE_LIMITでtruncateする方式に変更。

### 3.3 新規関数設計

#### 3.3.1 `apply_file_type_weight()`

```rust
/// BM25スコアにファイル種別ごとの係数を適用し、再ソート・truncateする
fn apply_file_type_weight(files: Vec<(String, f32)>, limit: usize) -> Vec<(String, f32)> {
    let mut weighted: Vec<(String, f32)> = files
        .into_iter()
        .map(|(path, score)| {
            let factor = file_type_weight_factor(&path);
            (path, score * factor)
        })
        .collect();
    weighted.sort_by(|a, b| b.1.total_cmp(&a.1));
    weighted.truncate(limit);
    weighted
}
```

**命名（レビュー反映）**: 実質「減衰」の処理のため `boost` → `weight` に変更。
**ソート（レビュー反映）**: `partial_cmp` + `unwrap_or` → `total_cmp`（Rust 1.62+ 安定化済み）でNaN安全性を確保。

#### 3.3.2 `file_type_weight_factor()`

```rust
/// ファイルパスからスコア係数を判定する
///
/// - テストファイル: TEST_FILE_WEIGHT (0.3)
/// - ドキュメント/レポート: DOC_FILE_WEIGHT (0.5)
/// - ソースコードファイル: 1.0（調整なし）
fn file_type_weight_factor(path: &str) -> f32 {
    let lower = path.to_lowercase();
    if is_test_file(&lower) {
        TEST_FILE_WEIGHT
    } else if is_doc_file(&lower) {
        DOC_FILE_WEIGHT
    } else {
        1.0
    }
}
```

**最適化（レビュー反映）**: `to_lowercase()` を `file_type_weight_factor` 内で1回のみ実行し、`is_test_file` / `is_doc_file` には小文字化済み文字列を渡す。

#### 3.3.3 `is_test_file()`

```rust
/// テストファイルかどうかをパスベースで判定（小文字化済みパスを受け取る）
///
/// 判定基準（セパレータ付きパターンで誤検知を防止）:
/// - ファイル名が "_test." / ".test." / "_spec." / ".spec." パターンを含む
/// - ファイル名が "test_" で始まる（test_helper等）
/// - パスに "/tests/" または "/__tests__/" ディレクトリを含む
fn is_test_file(lower_path: &str) -> bool {
    let file_name = Path::new(lower_path)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("");

    // セパレータ付きパターンで誤検知防止（contest.rs, latest.rs等を除外）
    file_name.contains("_test.") || file_name.contains(".test.")
        || file_name.contains("_spec.") || file_name.contains(".spec.")
        || file_name.starts_with("test_")
        || lower_path.contains("/tests/") || lower_path.contains("/__tests__/")
}
```

**精度改善（レビュー反映）**: `contains("test")` → セパレータ付きパターン（`_test.`, `.test.`等）に変更。これにより `contest.rs`, `latest.rs`, `test_utils.rs` 等の偽陽性を防止。`test_` プレフィックスは `test_helper.rs` 等のテスト補助ファイルをキャッチするために残す（テスト補助ファイルもテスト関連として減衰対象とする設計判断）。

#### 3.3.4 `is_doc_file()`

```rust
/// ドキュメント/レポートファイルかどうかをパスベースで判定（小文字化済みパスを受け取る）
///
/// 判定基準:
/// - パスに "dev-reports/" を含む（プロジェクト固有の判定基準）
/// - プロジェクトルート直下の定型ドキュメント（readme.md, changelog.md等）
///
/// 注意: src/配下の.mdファイルはナレッジとして有用なため、一律減衰しない
fn is_doc_file(lower_path: &str) -> bool {
    // プロジェクト固有のレポートディレクトリ
    if lower_path.contains("dev-reports/") {
        return true;
    }

    // .mdファイルのうち、docs/配下またはルート直下の定型ドキュメントのみ
    if lower_path.ends_with(".md") {
        let file_name = Path::new(lower_path)
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("");
        // ルート直下の定型ドキュメント
        let is_root_doc = matches!(file_name,
            "readme.md" | "changelog.md" | "contributing.md" | "license.md" | "claude.md"
        );
        // docs/ ディレクトリ配下
        let is_docs_dir = lower_path.contains("/docs/") || lower_path.starts_with("docs/");
        return is_root_doc || is_docs_dir;
    }

    false
}
```

**精度改善（レビュー反映）**: `.md` 一律減衰を廃止。Markdown中心のプロジェクトでsuggest精度が低下する問題を防止。`dev-reports/`, `docs/` 配下、ルート直下の定型ドキュメントのみを対象とする。

### 3.4 定数設計

```rust
/// テストファイルのスコア係数（BM25スコアに乗算、1.0未満は減衰）
const TEST_FILE_WEIGHT: f32 = 0.3;

/// ドキュメント/レポートファイルのスコア係数
const DOC_FILE_WEIGHT: f32 = 0.5;
```

**係数の根拠**:
- テストファイル (0.3): suggestの目的は「タスクに関連するソースコードの推薦」であり、テストファイルは参考情報に留まる。0.3は元スコアの約1/3に減衰させ、同等キーワードマッチのソースファイルより確実に下位に配置する
- ドキュメント (0.5): ドキュメントはソースコードほどの直接的な関連性はないが、テストファイルよりは有用。0.5は中間的な減衰
- 将来的に `.commandindex.toml` 等の設定ファイルで調整可能にすることを検討（Phase 2以降）

### 3.5 `search_entry_files()` の変更

```rust
fn search_entry_files(
    reader: &IndexReaderWrapper,
    query: &str,
) -> Result<Vec<(String, f32)>, SuggestError> {
    let results = reader.search(query, BM25_SEARCH_LIMIT)?;
    let deduped = deduplicate_by_file(results, BM25_SEARCH_LIMIT);
    Ok(apply_file_type_weight(deduped, DEDUP_FILE_LIMIT))
}
```

**設計判断（レビュー反映）**: `deduplicate_by_file()` のlimit引数をBM25_SEARCH_LIMITに拡大（全dedup件を保持）。`apply_file_type_weight()` でweight適用後にDEDUP_FILE_LIMITでtruncate。既存のdedupユニットテスト3件はlimit値が異なるだけなので影響なし。

## 4. テスト設計

### 4.1 ユニットテスト（新規追加）

| テスト | 検証内容 |
|--------|---------|
| `is_test_file_detects_separator_patterns` | `foo_test.ts`, `foo.test.ts`, `foo_spec.py`, `foo.spec.tsx` → true |
| `is_test_file_detects_test_prefix` | `test_helper.rs`, `test_utils.ts` → true |
| `is_test_file_detects_tests_directory` | `tests/unit/foo.rs`, `__tests__/bar.ts` → true |
| `is_test_file_ignores_non_test_files` | `src/auth.rs`, `src/contest.rs`, `src/latest.rs` → false |
| `is_test_file_empty_path` | `""` → false |
| `is_doc_file_detects_dev_reports` | `dev-reports/review.json`, `dev-reports/design/policy.md` → true |
| `is_doc_file_detects_docs_directory` | `docs/guide.md`, `docs/api.md` → true |
| `is_doc_file_detects_root_docs` | `README.md`, `CHANGELOG.md` → true |
| `is_doc_file_ignores_source_markdown` | `src/notes.md`, `src/components/guide.md` → false |
| `is_doc_file_ignores_source_files` | `src/main.rs` → false |
| `file_type_weight_factor_values` | テスト→0.3, ドキュメント→0.5, ソース→1.0 |
| `apply_file_type_weight_reorders` | テストファイル(score=2.0)とソースファイル(score=1.5)で、ソースファイルが上位 |
| `apply_file_type_weight_truncates` | limit=2で3件入力→2件出力 |
| `apply_file_type_weight_empty_input` | 空入力 → 空出力 |

### 4.2 既存テストへの影響

| テスト | 影響 |
|--------|------|
| `dedup_removes_duplicates_keeps_max_score` | **なし** - シグネチャ変更なし |
| `dedup_respects_limit` | **なし** |
| `dedup_empty_input` | **なし** |
| `validate_input_*` | **なし** |
| `shell_quote_*` | **なし** |
| `format_*_output` | **なし** |
| `fallback_strategy_*` | **なし** |
| `e2e_suggest_*` | **要確認** - テスト用リポジトリのファイル構成でweight適用後も安定することを検証 |

## 5. 設計判断とトレードオフ

### 判断1: reader.rs を変更しない

**選択**: suggest.rs 内のポストプロセスとして実装
**却下した案**: reader.rs の search_with_options に file_type ブースト機能を追加
**理由**: reader.rs は search, impact, related, suggest の全コマンドから使用されており、変更の影響範囲が大きい。suggest 固有の要件をreader.rsに持ち込むと責務が混在する。

### 判断2: パスベースのファイル種別判定

**選択**: ファイルパスの文字列パターンマッチで判定
**却下した案**: manifest.rs の FileType を拡張してテスト/ドキュメント判定を追加
**理由**: FileType は拡張子ベースの判定であり、テストファイル判定（ファイル名パターン、ディレクトリ構造）には対応していない。FileType を拡張するとmanifest全体に影響する。
**将来**: Phase 2でsearch等にも適用する場合、`src/util/file_classify.rs` 等の共通モジュールへの移動を検討。

### 判断3: dedup全件保持 → weight適用 → truncate

**選択**: dedup(BM25_SEARCH_LIMIT) → weight → truncate(DEDUP_FILE_LIMIT)
**却下した案（初期設計）**: dedup(limit=5) → weight（5件のみにweight適用）
**理由**: BM25上位5件がテストファイルで占有された場合に6位以下のソースファイルが救済されない。Issue #127の問題がまさにこのケースに該当する。dedup全件にweightを適用後にtruncateすることで、テストファイルのスコアが下がり本来推薦すべきソースファイルが上位に来る。

### 判断4: テストファイル判定のセパレータパターン

**選択**: `_test.`, `.test.`, `_spec.`, `.spec.` のセパレータ付きパターン + `test_` プレフィックス
**却下した案**: `contains("test")` による部分一致
**理由**: `contest.rs`, `latest.rs` 等の誤検知を防止。`test_` プレフィックスはテスト補助ファイル（`test_helper.rs`）をキャッチするために残す。

### 判断5: .md一律減衰を廃止

**選択**: `dev-reports/`, `docs/` 配下、ルート直下の定型ドキュメントのみ減衰
**却下した案**: `.md` 拡張子を全てドキュメント扱い
**理由**: CommandIndexはMarkdownをナレッジベースとしてインデックスする設計。`src/` 配下のMarkdownが一律減衰されるとMarkdown中心プロジェクトでsuggest精度が著しく低下する。

## 6. セキュリティ設計

| 脅威 | 対策 | 優先度 |
|------|------|--------|
| パス操作 | `is_test_file()`, `is_doc_file()` は読み取り専用の文字列パターンマッチのみ。ファイルI/Oなし | 低 |
| パス正規化 | `../` を含むパスで判定が不正確になる可能性あるが、tantivyのインデックスパスは正規化済みのためリスクは極小 | 低 |
| 入力バリデーション | 既存の `validate_input()` で対応済み。変更なし | - |

## 7. 品質基準

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |

## 8. 影響範囲サマリー

```
src/cli/suggest.rs
├── 新規定数: TEST_FILE_WEIGHT, DOC_FILE_WEIGHT
├── 新規関数: apply_file_type_weight()
├── 新規関数: file_type_weight_factor()
├── 新規関数: is_test_file()
├── 新規関数: is_doc_file()
├── 変更関数: search_entry_files() [dedup limit拡大 + apply_file_type_weight呼び出し追加]
└── 新規テスト: 14件
```

変更なし: reader.rs, schema.rs, search.rs, impact.rs, hybrid.rs, related.rs, manifest.rs
