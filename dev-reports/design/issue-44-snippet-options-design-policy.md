# 設計方針書: Issue #44 — searchコマンドのスニペット表示行数・文字数をCLIオプションで動的に調整可能にする

## 1. 概要

`commandindex search` の human 形式出力において、スニペット表示の行数・文字数が `src/output/human.rs:28` でハードコード（2行・120文字）されている。CLIオプション `--snippet-lines` / `--snippet-chars` を追加し、ユーザーが動的に調整できるようにする。

## 2. システムアーキテクチャ概要

本変更は Output レイヤーと CLI レイヤーに閉じた変更であり、Parser・Indexer・Search レイヤーへの影響はない。

```
CLI (main.rs)  →  cli/search.rs  →  output/human.rs
  ↑ オプション追加      ↑ 引数追加       ↑ SnippetConfig受取・動的値利用

output/mod.rs: SnippetConfig 構造体定義（format_results シグネチャ不変）
```

## 3. レイヤー構成と責務

| レイヤー | モジュール | 本Issue での責務 |
|---------|-----------|-----------------|
| **CLI** | `src/main.rs` | `--snippet-lines`, `--snippet-chars` オプション追加、`SnippetConfig` 構築 |
| **CLI/Search** | `src/cli/search.rs` | `run()` に `SnippetConfig` を受け渡し、`format_human` を直接呼び出し |
| **Output** | `src/output/mod.rs` | `SnippetConfig` 構造体定義（`format_results()` シグネチャは不変） |
| **Output/Human** | `src/output/human.rs` | `format_human()` が `SnippetConfig` を受け取り、0=無制限の判定・動的値で `truncate_body` を呼び出す |

## 4. 設計詳細

### 4.1 SnippetConfig 構造体

**定義場所**: `src/output/mod.rs`
**公開パス**: `commandindex::output::SnippetConfig`

```rust
/// スニペット表示設定（human形式のみ）
#[derive(Debug, Clone, Copy)]
pub struct SnippetConfig {
    /// 最大表示行数（0 = 無制限）
    pub lines: usize,
    /// 最大表示文字数（0 = 無制限）
    pub chars: usize,
}

impl Default for SnippetConfig {
    fn default() -> Self {
        Self {
            lines: 2,
            chars: 120,
        }
    }
}
```

**設計判断**:
- `Copy` を derive: 小さな値型のため値コピーで十分
- `Default` を手動実装: `lines=2, chars=120` でハードコード値と同じデフォルト（後方互換性維持）
- `Option<SnippetConfig>` ではなく直接 `SnippetConfig` を渡す: `None` のフォールバックロジックを各所に分散させない

### 4.2 format_results() — シグネチャ不変

```rust
// シグネチャは変更しない（ISP原則: Human専用の設定を全フォーマット共通APIに追加しない）
pub fn format_results(
    results: &[SearchResult],
    format: OutputFormat,
    writer: &mut dyn Write,
) -> Result<(), OutputError>
```

**設計判断**: SnippetConfig は Human 形式のみに関係するため、`format_results()` のシグネチャには追加しない。代わりに `cli/search.rs` の `run()` 内で Human 形式の場合に `format_human()` を直接呼び出し、それ以外は従来通り `format_results()` を使用する。

### 4.3 format_human() シグネチャ変更

```rust
// Before
pub fn format_human(results: &[SearchResult], writer: &mut dyn Write) -> Result<(), OutputError>

// After
pub fn format_human(
    results: &[SearchResult],
    writer: &mut dyn Write,
    snippet_config: SnippetConfig,
) -> Result<(), OutputError>
```

**import追加**: `use crate::output::SnippetConfig;`（または `use super::SnippetConfig;`）

**変更箇所**: 28行目で 0=無制限の判定を行い、truncate_body を呼び出す

```rust
// Before
let snippet = truncate_body(&strip_control_chars(&result.body), 2, 120);

// After（0=無制限の制御を format_human 側で行う — SRP維持）
let body_cleaned = strip_control_chars(&result.body);
let snippet = if snippet_config.lines == 0 && snippet_config.chars == 0 {
    body_cleaned
} else {
    truncate_body(&body_cleaned, snippet_config.lines, snippet_config.chars)
};
```

### 4.4 truncate_body() — シグネチャ・ロジック不変

**truncate_body は変更しない**。0=無制限のセマンティクスは呼び出し側（format_human）で制御する。

理由:
- SRP: truncate_body は純粋な切り詰め関数として維持
- context.rs からの既存呼び出し `truncate_body(body, 10, 500)` への影響完全回避
- 0 の特殊セマンティクスを関数内部に持たせない

ただし、`lines == 0` のみや `chars == 0` のみの場合も format_human 側でハンドリングする:

```rust
let body_cleaned = strip_control_chars(&result.body);
let snippet = if snippet_config.lines == 0 && snippet_config.chars == 0 {
    // 両方0: 全文表示
    body_cleaned
} else if snippet_config.lines == 0 {
    // 行数無制限、文字数制限あり（単一行のみ有効）
    truncate_body(&body_cleaned, usize::MAX, snippet_config.chars)
} else if snippet_config.chars == 0 {
    // 行数制限あり、文字数無制限
    truncate_body(&body_cleaned, snippet_config.lines, usize::MAX)
} else {
    truncate_body(&body_cleaned, snippet_config.lines, snippet_config.chars)
};
```

### 4.5 cli/search.rs run() シグネチャ変更

```rust
// Before
pub fn run(options: &SearchOptions, filters: &SearchFilters, format: OutputFormat) -> Result<(), SearchError>

// After
pub fn run(
    options: &SearchOptions,
    filters: &SearchFilters,
    format: OutputFormat,
    snippet_config: SnippetConfig,
) -> Result<(), SearchError>
```

**import追加**: `use crate::output::SnippetConfig;` を既存の use 文に追加

**format_results 呼び出し変更**:
```rust
// Before
output::format_results(&results, format, &mut handle)?;

// After — Human形式のみ SnippetConfig を渡す
match format {
    OutputFormat::Human => {
        output::human::format_human(&results, &mut handle, snippet_config)?;
    }
    _ => {
        output::format_results(&results, format, &mut handle)?;
    }
}
```

### 4.6 main.rs CLIオプション追加

**Search enum にフィールド追加**:
```rust
Search {
    // ... existing fields ...
    /// Maximum number of snippet lines in human output (0 = unlimited)
    #[arg(long, default_value_t = 2)]
    snippet_lines: usize,
    /// Maximum number of snippet characters in human output (0 = unlimited)
    #[arg(long, default_value_t = 120)]
    snippet_chars: usize,
}
```

**destructuring パターン更新** (main.rs:114):
```rust
Commands::Search {
    query,
    symbol,
    related,
    format,
    tag,
    path,
    file_type,
    heading,
    limit,
    snippet_lines,   // 追加
    snippet_chars,    // 追加
} => {
```

**SnippetConfig 構築と呼び出し** (import追加: `use commandindex::output::SnippetConfig;`):
```rust
(Some(q), None, None) => {
    let options = SearchOptions { query: q, tag, heading, limit: limit.min(1000) };
    let filters = SearchFilters { path_prefix: path, file_type };
    let snippet_config = SnippetConfig { lines: snippet_lines, chars: snippet_chars };
    commandindex::cli::search::run(&options, &filters, format, snippet_config)
}
```

**`--symbol` / `--related` モード**: `snippet_lines` / `snippet_chars` はサイレントに無視される（run_symbol_search / run_related_search には渡さない）。将来的に symbol 検索にスニペットを追加する可能性を残すため、clap の `conflicts_with` は設定しない。

## 5. テスト計画

### 5.1 既存テスト修正

`tests/output_format.rs`:
- **import変更**: `use commandindex::output::{OutputFormat, SnippetConfig, format_results};`
- **format_to_string() ヘルパー**: シグネチャ不変（format_results のシグネチャが不変のため修正不要）
- **format_human 直接呼び出しテスト用ヘルパー追加**:

```rust
fn format_human_to_string(results: &[SearchResult], snippet_config: SnippetConfig) -> String {
    colored::control::set_override(false);
    let mut buf = Vec::new();
    commandindex::output::human::format_human(results, &mut buf, snippet_config).unwrap();
    String::from_utf8(buf).unwrap()
}
```

### 5.2 新規テスト

| テストケース | 検証内容 |
|-------------|---------|
| `test_snippet_custom_lines` | `lines=5` で5行分表示 |
| `test_snippet_custom_chars` | `chars=50` で50文字切り詰め |
| `test_snippet_lines_zero_unlimited` | `lines=0, chars=0` で全行表示 |
| `test_snippet_chars_zero_unlimited` | `lines=2, chars=0` で文字数無制限（単一行） |
| `test_snippet_default_unchanged` | デフォルト値で既存動作と同一 |

## 6. 影響範囲

### 変更対象

| ファイル | 変更内容 |
|---------|---------|
| `src/main.rs` | `Search` に `snippet_lines`, `snippet_chars` フィールド追加、destructuring更新、`SnippetConfig` 構築、import追加 |
| `src/cli/search.rs` | `run()` に `SnippetConfig` 引数追加、Human 分岐で format_human 直接呼び出し、import追加 |
| `src/output/mod.rs` | `SnippetConfig` 構造体定義（`format_results()` シグネチャ不変） |
| `src/output/human.rs` | `format_human()` に `SnippetConfig` 引数追加、0=無制限制御、import追加 |
| `tests/output_format.rs` | 新規テスト追加、format_human 用ヘルパー追加、import追加 |

### 影響なし

| ファイル/機能 | 理由 |
|-------------|------|
| `format_results()` | シグネチャ不変 |
| `format_symbol_results` / `format_related_results` | シグネチャ不変 |
| `src/cli/context.rs` | `truncate_body` 不変 |
| `--symbol` / `--related` モード | スニペットオプション不使用 |
| `json` / `path` フォーマット | `SnippetConfig` 無関係 |
| `src/output/json.rs` / `src/output/path.rs` | 変更なし |
| 既存テスト（tests/output_format.rs の format_results 呼び出し） | format_results シグネチャ不変のため修正不要 |

## 7. セキュリティ設計

| 脅威 | 対策 | 優先度 |
|------|------|--------|
| 大量テキスト全文表示によるメモリ消費 | 検索結果のbodyサイズは検索エンジン側で制限済み。追加制限は不要 | 低 |
| 入力値不正 | `usize` 型により負の値はclap が自動で弾く | 低 |

## 8. 設計判断とトレードオフ

| 判断 | 選択肢 | 採用理由 |
|------|--------|---------|
| `format_results` シグネチャ不変 | format_results に追加 vs format_human のみ変更 | ISP原則: Human専用設定を全フォーマット共通APIに追加しない |
| 0=無制限ロジックの配置 | truncate_body 内 vs format_human 側 | SRP: truncate_body は純粋な切り詰め関数として維持 |
| `SnippetConfig` vs `Option<SnippetConfig>` | `SnippetConfig` 直接渡し | フォールバックロジック分散を防ぐ |
| `truncate_body` シグネチャ不変 | シグネチャ変更 vs 不変 | `context.rs` への影響完全回避 |
| 排他的分岐の維持 vs 両方適用 | 維持 | 現行動作の後方互換性を優先 |
| 入力上限値バリデーション | 不要 | YAGNI。巨大値でも実質無害（body サイズは検索エンジン側で制限） |
| symbol/related での snippet オプション | サイレント無視 | KISS。将来のスニペット追加可能性を残す |

## 9. 品質基準

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |
