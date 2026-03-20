# 設計方針書 - Issue #8: 検索結果出力フォーマッター

## 1. Issue情報

| 項目 | 値 |
|------|-----|
| Issue番号 | #8 |
| タイトル | [Feature] 検索結果出力フォーマッター（human / json / path） |
| ラベル | enhancement |
| 設計書作成日 | 2026-03-19 |

## 2. システムアーキテクチャ概要

CommandIndexは、Markdown・Code・Gitを横断するローカルナレッジ検索CLIです。本Issueでは、検索結果の出力レイヤー（`src/output/`）を新規追加します。

```
┌─────────────────────────────────────────┐
│              CLI Layer (main.rs)         │
│  clap: index | search | update | ...    │
│              --format flag               │
└─────────────┬───────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────┐
│          Indexer Layer (indexer/)        │
│  reader.rs → Vec<SearchResult>          │
└─────────────┬───────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────┐
│        Output Layer (output/) ← NEW     │
│  human.rs | json.rs | path.rs           │
│  OutputFormat enum | OutputError enum   │
└─────────────────────────────────────────┘
```

## 3. レイヤー構成と責務

| レイヤー | モジュール | 責務 | 本Issue関連 |
|---------|-----------|------|------------|
| **CLI** | `src/main.rs` | エントリポイント、clapサブコマンド定義 | `--format` フラグをSearchサブコマンドに追加 |
| **Parser** | `src/parser/` | Markdown・ソースコード解析 | 変更なし |
| **Indexer** | `src/indexer/` | tantivyインデックス操作・検索 | 変更なし |
| **Output** | `src/output/` | 出力フォーマット（human/json/path） | **新規作成** |

## 4. 技術選定

### 本Issue固有の技術選定

| カテゴリ | 選定技術 | バージョン | 選定理由 |
|---------|---------|-----------|---------|
| 色付き出力 | `colored` | 2 | 軽量・NO_COLOR対応・Rust標準的クレート |
| JSONシリアライズ | `serde_json` | 1（既存） | 既にCargo.tomlに含まれる。`json!()` マクロで構築 |
| CLI引数パース | `clap` ValueEnum | 4（既存） | enum→CLI値の自動変換 |

### 不採用の選択肢

| 技術 | 不採用理由 |
|------|-----------|
| `owo-colors` | colored に比べてエコシステムが小さい |
| `termcolor` | APIが冗長、colored の方がシンプル |
| ANSI直書き | 環境変数対応が手動になる、保守性低い |
| 中間構造体 `JsonSearchResult` | `serde_json::json!()` マクロで十分。不要な構造体定義を避ける |

## 5. 設計パターン

### 5.1 モジュール構成

```
src/output/
├── mod.rs        # OutputFormat enum, OutputError enum, format_results()
│                 # parse_tags(), truncate_body() 共通ヘルパー
├── human.rs      # format_human() 関数
├── json.rs       # format_json() 関数
└── path.rs       # format_path() 関数
```

### 5.2 OutputFormat enum

```rust
use clap::ValueEnum;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum OutputFormat {
    Human,
    Json,
    Path,
}
```

### 5.3 OutputError enum

```rust
use std::fmt;

#[derive(Debug)]
pub enum OutputError {
    Io(std::io::Error),
    Json(serde_json::Error),
}

impl fmt::Display for OutputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OutputError::Io(e) => write!(f, "IO error: {e}"),
            OutputError::Json(e) => write!(f, "JSON serialization error: {e}"),
        }
    }
}

impl std::error::Error for OutputError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            OutputError::Io(e) => Some(e),
            OutputError::Json(e) => Some(e),
        }
    }
}

impl From<std::io::Error> for OutputError {
    fn from(e: std::io::Error) -> Self { OutputError::Io(e) }
}

impl From<serde_json::Error> for OutputError {
    fn from(e: serde_json::Error) -> Self { OutputError::Json(e) }
}
```

### 5.4 メインディスパッチ関数

```rust
use crate::indexer::reader::SearchResult;
use std::io::Write;

/// 検索結果を指定フォーマットで出力する
pub fn format_results(
    results: &[SearchResult],
    format: OutputFormat,
    writer: &mut dyn Write,
) -> Result<(), OutputError> {
    // NOTE: フォーマットが5種類以上に増えた場合、trait-based Formatterパターンへのリファクタリングを検討
    match format {
        OutputFormat::Human => human::format_human(results, writer),
        OutputFormat::Json => json::format_json(results, writer),
        OutputFormat::Path => path::format_path(results, writer),
    }
}
```

**設計判断**: `&mut dyn Write` をパラメータとすることで、テスト時に `Vec<u8>` に書き込み可能にする（stdout非依存テスト）。動的ディスパッチのオーバーヘッドはI/O出力では無視できるレベル。

### 5.5 共通ヘルパー関数（mod.rs 内）

```rust
/// tags文字列をパースしてVec<&str>に変換する
/// SearchResult.tagsはスペース区切り文字列（例: "auth security"）
pub(crate) fn parse_tags(tags: &str) -> Vec<&str> {
    tags.split_whitespace().collect()
}

/// 本文を指定行数で切り詰める（マルチバイト文字安全）
pub(crate) fn truncate_body(body: &str, max_lines: usize, max_chars: usize) -> String {
    let lines: Vec<&str> = body.lines().collect();
    if lines.len() > 1 {
        let taken: Vec<&str> = lines.iter().take(max_lines).copied().collect();
        let mut result = taken.join("\n");
        if lines.len() > max_lines {
            result.push_str("...");
        }
        result
    } else {
        let chars: String = body.chars().take(max_chars).collect();
        if body.chars().count() > max_chars {
            format!("{chars}...")
        } else {
            chars
        }
    }
}

/// 制御文字をストリッピングする（ANSIインジェクション対策）
/// 改行は保持し、それ以外の制御文字（0x00-0x1F, 0x7F）を除去
pub(crate) fn strip_control_chars(s: &str) -> String {
    s.chars().filter(|c| !c.is_control() || *c == '\n').collect()
}
```

**設計判断**:
- `truncate_body()` と `parse_tags()` は `mod.rs` に配置。human.rs/json.rs 両方から参照される共通ロジックであり、特定フォーマッターに依存しない。
- `strip_control_chars()` は human 形式出力前に適用し、ファイル内容由来のANSIエスケープシーケンスによるターミナル操作を防止する（セキュリティ対策）。JSON形式は `serde_json` が自動エスケープするため不要。

### 5.5.1 空結果時の挙動

| フォーマット | 空結果時の挙動 |
|-------------|--------------|
| human | 何も出力しない（呼び出し元で「No results found」メッセージをstderrに表示） |
| json | 何も出力しない（JSONL形式のため、0行 = 空出力が正しい） |
| path | 何も出力しない |

**注意**: 空結果メッセージの表示責務は `format_results()` ではなく CLI レイヤー（main.rs）に持たせる。フォーマッターは純粋な出力変換のみを担当する。

### 5.6 各フォーマッター関数シグネチャ

```rust
// human.rs
pub fn format_human(results: &[SearchResult], writer: &mut dyn Write) -> Result<(), OutputError>;

// json.rs
pub fn format_json(results: &[SearchResult], writer: &mut dyn Write) -> Result<(), OutputError>;

// path.rs
pub fn format_path(results: &[SearchResult], writer: &mut dyn Write) -> Result<(), OutputError>;
```

### 5.7 JSON出力方式

中間構造体を使わず、`serde_json::json!()` マクロでインライン構築する:

```rust
// json.rs 内
for result in results {
    let tags = parse_tags(&result.tags);
    let json_value = serde_json::json!({
        "path": result.path,
        "heading": result.heading,
        "heading_level": result.heading_level,
        "body": result.body,
        "tags": tags,
        "line_start": result.line_start,
        "score": result.score,
    });
    serde_json::to_writer(&mut *writer, &json_value)?;
    writeln!(writer)?;
}
```

**設計判断**: `serde_json::json!()` マクロにより中間構造体 `JsonSearchResult` が不要になる。KISSの原則に従い、不要な型定義を排除。

### 5.8 CLI統合

```rust
// main.rs の Search サブコマンド
Search {
    /// Search query
    query: String,
    /// Output format (human, json, path)
    #[arg(long, value_enum, default_value_t = commandindex::output::OutputFormat::Human)]
    format: commandindex::output::OutputFormat,
},
```

**注意**: `--format` は Search サブコマンド固有のオプション（グローバルオプションではない）。

## 6. セキュリティ設計

| 脅威 | 対策 | 優先度 |
|------|------|--------|
| JSONインジェクション | `serde_json` による適切なエスケープ（自動） | 高 |
| ANSIインジェクション | human形式出力前に `strip_control_chars()` で制御文字を除去 | 高 |
| リソース枯渇 | JSON形式でもbodyに `truncate_body()` 適用（巨大セクション対策） | 中 |
| パストラバーサル | 出力のみのモジュールのため、ファイル操作なし | 低（対象外） |
| unsafe使用 | 原則禁止 | 高 |
| BrokenPipe | パイプ先プロセス終了時のグレースフル処理（エラーを静かに無視） | 低 |

## 7. 設計判断とトレードオフ

### 7.1 関数ベース vs trait ベース

| 選択肢 | メリット | デメリット |
|--------|---------|-----------|
| **関数ベース（採用）** | シンプル、YAGNI準拠 | フォーマット追加時にmatch分岐増加 |
| traitベース | OCP準拠、拡張性高い | 3種類では過剰設計 |

**決定**: 関数ベースを採用。3種類のフォーマットに対してtrait抽象化は過剰。フォーマットが5種類以上になった場合にリファクタリングする（OCP trade-off）。

### 7.2 serde_json::json!() vs 中間構造体

| 選択肢 | メリット | デメリット |
|--------|---------|-----------|
| **json!()マクロ（採用）** | 型定義不要、KISS準拠 | フィールド名のtypoがコンパイル時検出不可 |
| 中間構造体 | コンパイル時型安全 | tags変換のための不要な型定義 |

**決定**: `json!()` マクロを採用。テストで出力内容を検証するためtypoリスクは低い。

### 7.3 Writerパラメータ vs stdout直接

| 選択肢 | メリット | デメリット |
|--------|---------|-----------|
| **&mut dyn Write（採用）** | テスト容易、柔軟 | 動的ディスパッチ（I/Oでは無視可） |
| stdout直接出力 | シンプル | テスト困難 |

**決定**: `&mut dyn Write` パラメータを採用。

### 7.4 共通ヘルパーの配置

| 選択肢 | メリット | デメリット |
|--------|---------|-----------|
| **mod.rsに配置（採用）** | 単一参照元、DRY準拠 | mod.rsがやや肥大化 |
| 各フォーマッターに分散 | モジュール独立性 | DRY違反 |

**決定**: `parse_tags()` と `truncate_body()` を `mod.rs` に配置。tags解析ロジックが変更された場合に1箇所の修正で済む。

## 8. 影響範囲

### 新規ファイル（5ファイル）

| ファイル | 責務 | 行数見積もり |
|---------|------|------------|
| `src/output/mod.rs` | モジュール宣言・OutputFormat・OutputError・共通ヘルパー・format_results() | ~100行 |
| `src/output/human.rs` | Human形式フォーマッター | ~40行 |
| `src/output/json.rs` | JSONL形式フォーマッター | ~25行 |
| `src/output/path.rs` | Path形式フォーマッター（重複除去） | ~20行 |
| `tests/output_format.rs` | 全フォーマットの統合テスト | ~150行 |

### 既存ファイル変更（3ファイル）

| ファイル | 変更内容 | 影響度 |
|---------|---------|--------|
| `src/main.rs` | Search サブコマンドに `--format` オプション追加 | 低（追加のみ） |
| `src/lib.rs` | `pub mod output;` アンコメント | 低（1行） |
| `Cargo.toml` | `colored = "2"` 追加 | 低（1行） |

### 変更しないファイル

| ファイル | 理由 |
|---------|------|
| `src/indexer/reader.rs` | **SearchResult は変更しない**（Serialize追加も不要。json!()マクロで対応し関心の分離を維持） |
| `src/parser/` | 本Issue の対象外 |

## 9. テスト戦略

### テスト配置
- `tests/output_format.rs` に統合テストとして配置（既存テストパターンに準拠）
- `commandindex::output` モジュールの公開APIを通じてテスト
- SearchResult はテスト内で直接構築（tantivy不要、テスト高速）

### テストケース一覧

| テスト名 | 対象 | 検証内容 |
|---------|------|---------|
| `test_human_format_basic` | human | 基本的なhuman形式出力 |
| `test_human_format_with_tags` | human | タグ付き出力（カンマ区切り） |
| `test_human_format_no_tags` | human | タグなし時にTags行が非表示 |
| `test_human_format_snippet_truncation` | human | 2行超過時の切り詰め+「...」 |
| `test_human_format_long_single_line` | human | 120文字超過の単一行切り詰め |
| `test_json_format_basic` | json | JSONL形式出力 |
| `test_json_format_tags_array` | json | tags が配列に変換される |
| `test_json_format_empty_tags` | json | 空tagsが空配列になる |
| `test_json_format_score` | json | scoreフィールドが含まれる |
| `test_path_format_basic` | path | パスのみ出力 |
| `test_path_format_dedup` | path | 重複パスの除去 |
| `test_format_empty_results` | 全体 | 空結果のハンドリング |

### テスト環境設定
- テスト実行前に `colored::control::set_override(false)` でANSIカラーを無効化（CI/非TTY環境での安定性確保）

### テスト手法
```rust
// テストパターン例
fn make_result(path: &str, heading: &str, body: &str, tags: &str) -> SearchResult {
    SearchResult { path: path.to_string(), heading: heading.to_string(), body: body.to_string(),
                   tags: tags.to_string(), heading_level: 2, line_start: 1, score: 1.0 }
}

#[test]
fn test_json_format_basic() {
    let results = vec![make_result("test.md", "Title", "Body text", "tag1 tag2")];
    let mut buf = Vec::new();
    format_results(&results, OutputFormat::Json, &mut buf).unwrap();
    let output = String::from_utf8(buf).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(parsed["path"], "test.md");
    assert_eq!(parsed["tags"], serde_json::json!(["tag1", "tag2"]));
}
```

## 10. 品質基準

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |
