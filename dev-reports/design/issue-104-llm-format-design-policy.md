# 設計方針書: Issue #104 LLM向け出力フォーマットの追加 (--format llm)

## 1. 概要

### 1.1 目的
LLMプロンプトに最適化された出力フォーマット `--format llm` を追加し、検索結果をMarkdown形式で簡潔に出力する。

### 1.2 背景
- `--format json` はLLMプロンプトとして冗長（スコア、見出しレベル、タグ等が不要）
- LLMが必要とするのは「ファイルパス + 関連コードスニペット」のみ
- `context` コマンドは存在するがファイル指定ベースであり、検索結果のLLM向け出力が不足

## 2. システムアーキテクチャにおける位置づけ

### 2.1 レイヤー構成と本機能の影響範囲

| レイヤー | モジュール | 変更 |
|---------|-----------|------|
| **CLI** | `src/main.rs` | ヘルプコメント更新（L52, L149, L205の3箇所） |
| **CLI** | `src/cli/help_llm.rs` | output_formatsフィールド更新 |
| **CLI** | `src/cli/context.rs` | estimate_tokens関数削除 + use文追加 |
| **Output** | `src/output/mod.rs` | OutputFormat enum拡張 + 各format_*関数にLlmアーム追加 + `pub mod llm;` 追加 + `pub(crate) fn estimate_tokens` 追加 |
| **Output** | `src/output/llm.rs` | **新規作成** |
| **CLI** | `src/cli/search.rs` | 変更不要（Llmはformat_results経由で処理。Human専用分岐は不要） |
| **CLI** | `src/cli/status.rs` | **対象外**（独自StatusFormat enum使用） |
| **CLI** | `src/cli/changed_since.rs` | 変更不要（format_impact_resultsへの委譲で自動対応） |
| Parser, Indexer, Search | - | **変更なし** |

### 2.2 既存フォーマットとの比較

| フォーマット | 用途 | 出力形式 | 情報量 |
|-------------|------|---------|--------|
| `human` | ターミナル表示 | カラー付きテキスト | フル（スコア、タグ、スニペット） |
| `json` | ツール連携 | JSONL | フル（全フィールド） |
| `path` | パイプライン | パスのみ | 最小 |
| **`llm`** | **LLMプロンプト** | **Markdown** | **中（パス + スニペット + 見出し）** |

## 3. 設計詳細

### 3.1 OutputFormat enum拡張

```rust
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum OutputFormat {
    Human,
    Json,
    Path,
    Llm,  // 新規追加
}
```

### 3.2 新規モジュール: `src/output/llm.rs`

以下の7関数を実装する。各関数のシグネチャは既存のjson.rs/path.rsと同一パターンに従う。

#### 3.2.1 format_llm（全文検索結果）

```rust
pub fn format_llm(results: &[SearchResult], writer: &mut dyn Write) -> Result<(), OutputError>
```

**出力形式:**
```markdown
<!-- estimated tokens: ~1234 -->
## path/to/file.md
### Heading Name

body content here...

## path/to/another.rs
```rust
code snippet here
```
```

**設計判断:**
- 同一ファイルパスの結果をグループ化して重複除去
- `heading` 情報を `###` として含める（LLMがファイル構造を理解するため）
- `tags`, `score`, `heading_level`, `line_start` は除外（冗長性削減）
- `body` はstrip_control_chars適用後にそのまま出力（切り詰めなし）
- コードフェンスの言語指定はファイル拡張子から推定
- コードフェンスのバッククォート数は動的に決定（body内の ``` 連続数+1）

#### 3.2.2 format_workspace_llm（ワークスペース横断検索）

```rust
pub fn format_workspace_llm(
    results: &[WorkspaceSearchResult],
    writer: &mut dyn Write,
) -> Result<(), OutputError>
```

**出力形式:**
```markdown
<!-- estimated tokens: ~1234 -->
## [repo-name] path/to/file.md
### Heading Name

body content...
```

#### 3.2.3 format_semantic_llm（セマンティック検索）

```rust
pub fn format_semantic_llm(
    results: &[SemanticSearchResult],
    writer: &mut dyn Write,
) -> Result<(), OutputError>
```

**出力形式:** format_llmと同じMarkdown構造。`similarity` スコアは除外。

#### 3.2.4 format_symbol_llm（シンボル検索）

```rust
pub fn format_symbol_llm(
    results: &[SymbolSearchResult],
    writer: &mut dyn Write,
) -> Result<(), OutputError>
```

**出力形式:**
```markdown
<!-- estimated tokens: ~500 -->
## path/to/file.ts
- `function` **functionName** (L10-L25)
  - `method` **childMethod** (L12-L20)
```

#### 3.2.5 format_related_llm（関連検索）

```rust
pub fn format_related_llm(
    results: &[RelatedSearchResult],
    writer: &mut dyn Write,
) -> Result<(), OutputError>
```

**出力形式:**
```markdown
<!-- estimated tokens: ~300 -->
- path/to/file.ts (link, import)
- path/to/other.rs (tags, path)
```

#### 3.2.6 format_diff_llm（Diff結果）

```rust
pub fn format_diff_llm(result: &DiffResult, writer: &mut dyn Write) -> Result<(), OutputError>
```

**出力形式:**
```markdown
<!-- estimated tokens: ~200 -->
## Diff: file_a vs file_b

### Only in file_a
- path1
- path2

### Only in file_b
- path3

### Overlap (N files)
- path4
- path5
```

#### 3.2.7 format_impact_llm（Impact分析）

```rust
pub fn format_impact_llm(
    result: &ImpactResult,
    writer: &mut dyn Write,
) -> Result<(), OutputError>
```

**出力形式:**
```markdown
<!-- estimated tokens: ~400 -->
## Impact: N input file(s) → M impacted file(s)

- path/to/impacted.rs (link, import) ← source1.rs, source2.rs
- path/to/another.ts (tags) ← source1.rs
```

### 3.3 トークン数推定

既存の `estimate_tokens` 関数（`src/cli/context.rs`）のロジックを再利用:

```rust
fn estimate_tokens(text: &str) -> usize {
    text.len() / 4
}
```

**設計判断:** この関数は現在 `src/cli/context.rs` にプライベートで定義されている。llm.rsでも同じロジックが必要なため、以下の2つのアプローチを検討:

| アプローチ | メリット | デメリット |
|-----------|---------|-----------|
| A: llm.rs内に同じ関数をコピー | シンプル、最小変更 | DRY違反 |
| B: 共通ユーティリティに移動 | DRY準拠 | context.rsの変更が必要 |

**選択: アプローチB** — `src/output/mod.rs` に `pub(crate) fn estimate_tokens` を定義し、context.rsとllm.rsの両方から参照する。これにより将来の推定ロジック変更時に1箇所の修正で済む。

**移動手順:**
1. `src/cli/context.rs` L375のfn estimate_tokens定義を削除
2. `src/cli/context.rs` の先頭use文に `use crate::output::estimate_tokens;` を追加
3. `src/output/mod.rs` に `pub(crate) fn estimate_tokens(text: &str) -> usize { text.len() / 4 }` を追加
4. L218, L234の呼び出しはそのまま動作する（関数名は同じ）

**制限事項:** `text.len() / 4` はバイト数ベースの概算であり、日本語テキスト（UTF-8で1文字3バイト）では実際のLLMトークン数より少なく見積もられる可能性がある。精度向上は別Issueで対応する。

### 3.4 言語判定（コードフェンス用）

ファイル拡張子からコードフェンスの言語指定を推定する関数を `llm.rs` に定義:

```rust
fn detect_language(path: &str) -> &'static str {
    match Path::new(path).extension().and_then(|e| e.to_str()) {
        Some("rs") => "rust",
        Some("ts") | Some("tsx") => "typescript",
        Some("js") | Some("jsx") => "javascript",
        Some("py") => "python",
        Some("go") => "go",
        Some("java") => "java",
        Some("rb") => "ruby",
        Some("sh") | Some("bash") => "bash",
        Some("sql") => "sql",
        Some("yaml") | Some("yml") => "yaml",
        Some("toml") => "toml",
        Some("json") => "json",
        Some("md") => "markdown",
        Some("html") | Some("htm") => "html",
        Some("css") => "css",
        _ => "",
    }
}
```

**設計判断:** Markdownファイルの場合はコードフェンスで囲まない（そのまま出力）。コードファイルの場合のみ言語付きコードフェンスで囲む。

### 3.5 ファイルグループ化

同一パスの結果をグループ化する設計。`HashMap` で集約後に元の出現順でソートする（新規crate依存を避けつつO(1)ルックアップ）。

```rust
fn group_by_path<'a>(results: &'a [SearchResult]) -> Vec<(&'a str, Vec<&'a SearchResult>)> {
    let mut map: std::collections::HashMap<&str, Vec<&SearchResult>> =
        std::collections::HashMap::new();
    let mut order: Vec<&str> = Vec::new();
    for result in results {
        if !map.contains_key(result.path.as_str()) {
            order.push(&result.path);
        }
        map.entry(&result.path).or_default().push(result);
    }
    order.into_iter().map(|p| (p, map.remove(p).unwrap())).collect()
}
```

### 3.6 Markdown/非コードファイルの出力判定

ファイルの種類によって出力形式を切り替える:

| ファイル種別 | 出力方式 |
|-------------|---------|
| Markdownファイル (`.md`) | bodyをそのまま出力（コードフェンスなし） |
| ソースコード (`.rs`, `.ts` 等) | 言語付きコードフェンスで囲む |
| その他 | コードフェンスなし（プレーンテキストとして出力） |

判定関数:
```rust
fn is_code_file(path: &str) -> bool {
    let lang = detect_language(path);
    !lang.is_empty() && lang != "markdown"
}
```

## 4. mod.rs の各format_*関数への統合

7つのformat_*関数すべてにLlmアームを追加:

```rust
// format_results
OutputFormat::Llm => llm::format_llm(results, writer),

// format_symbol_results
OutputFormat::Llm => llm::format_symbol_llm(results, writer),

// format_related_results
OutputFormat::Llm => llm::format_related_llm(results, writer),

// format_semantic_results
OutputFormat::Llm => llm::format_semantic_llm(results, writer),

// format_workspace_results
OutputFormat::Llm => llm::format_workspace_llm(results, writer),

// format_diff_results
OutputFormat::Llm => llm::format_diff_llm(result, writer),

// format_impact_results
OutputFormat::Llm => llm::format_impact_llm(result, writer),
```

**注意:**
- `format_workspace_results` のHumanアームは `snippet_config` を受け取るが、LlmフォーマットではJson/Pathと同様にsnippet_configは使用しない（切り詰めなし）
- `src/cli/search.rs` のrun()関数では、Humanのみ直接分岐し、それ以外（Json/Path/Llm）はformat_results()経由で処理される。Llmに特別な分岐は不要

## 5. help-llm更新

`src/cli/help_llm.rs` の以下箇所を更新:

1. `build_commands()` 内の `output_formats` フィールド
   - search: `["human", "json", "path"]` → `["human", "json", "path", "llm"]`
   - diff: `["human", "json"]` → `["human", "json", "path", "llm"]`（※現状pathが欠落しているため併せて追加）
   - impact: `["human", "json", "path"]` → `["human", "json", "path", "llm"]`
   - workspace search: searchのCommandInfoに含まれるため個別対応不要

2. `key_options` の説明文更新
   - `"Output format: human, json, path"` → `"Output format: human, json, path, llm"`

## 6. セキュリティ設計

| 脅威 | 対策 | 実装箇所 |
|------|------|---------|
| ANSIインジェクション | `strip_control_chars()` を全出力に適用 | llm.rs 各関数 |
| Markdownインジェクション（コードフェンス） | body内のバッククォート連続数を検査し、それより1つ多い数でフェンスを構成 | llm.rs の `fence_backticks()` 関数 |
| パストラバーサル | 出力専用モジュールのためリスクなし | - |
| プロンプトインジェクション | body内容はユーザー管理のローカルファイルのため対策不要 | - |
| Unicode BiDi制御文字 | フォローアップIssueで対応（strip_control_charsの改善） | - |

## 7. 設計判断とトレードオフ

| 判断 | 選択 | 理由 | 代替案 |
|------|------|------|--------|
| フォーマット方式 | Markdown | LLMが最も理解しやすい形式 | XML, YAML |
| リファクタリング | enum match維持 | 4種類目で閾値（5）未満 | trait-based Formatter |
| トークン推定 | bytes/4 | 既存ContextPackと統一 | tiktoken等の正確な推定 |
| 新規crate | 不要 | 標準ライブラリのみで実装可能 | indexmap等 |
| estimate_tokens配置 | output/mod.rsに移動 | DRY準拠、将来の変更が1箇所 | コピー |
| スニペット切り詰め | なし（全body出力） | LLMには完全な情報が有用 | max_lines指定 |

## 8. テスト方針

### 8.1 更新が必要な既存テスト
- `tests/output_format.rs` の `test_format_empty_results`: OutputFormat::Llm を列挙に追加
- `tests/output_format.rs` の全format列挙箇所（workspace/impactテスト含む）にOutputFormat::Llmを追加
- `tests/e2e_integration.rs` の `e2e_output_formats` テストにllmフォーマットのE2E検証を追加

### 8.3 空結果時の挙動
空結果の場合は何も出力しない（トークンコメントも出力しない）。これにより既存のtest_format_empty_resultsのis_empty()アサーションと整合する。

### 8.2 新規テスト

| テスト名 | 検証内容 |
|---------|---------|
| `test_format_llm_basic` | 基本的なsearch結果のMarkdown出力 |
| `test_format_llm_empty` | 空結果のLLM出力（ヘッダーのみ） |
| `test_format_llm_grouping` | 同一ファイルのグループ化 |
| `test_format_llm_code_fence` | コードファイルのフェンス付き出力 |
| `test_format_llm_markdown_no_fence` | Markdownファイルのフェンスなし出力 |
| `test_format_llm_estimated_tokens` | トークン数推定の正確性 |
| `test_format_llm_strip_control_chars` | 制御文字除去 |
| `test_format_symbol_llm` | シンボル検索のLLM出力 |
| `test_format_related_llm` | 関連検索のLLM出力 |
| `test_format_semantic_llm` | セマンティック検索のLLM出力 |
| `test_format_workspace_llm` | ワークスペース検索のLLM出力 |
| `test_format_diff_llm` | Diff結果のLLM出力 |
| `test_format_impact_llm` | Impact結果のLLM出力 |

## 9. 品質基準

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |
