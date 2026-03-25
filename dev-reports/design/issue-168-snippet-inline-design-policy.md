# 設計方針書: Issue #168 - issue/before-changeの出力に判断理由のスニペットを付与する

## 1. 概要

### 目的
`issue` と `before-change` コマンドの出力に、各文書の判断理由のスニペットをインライン表示する機能を追加する。

### 背景
現状、両コマンドはファイルパスのリストのみを返す。「過去の判断を取り出す」というプロダクトコアを実現するには、判断理由が直接読めて初めて「次の意思決定に接続」できる。

### スコープ
- Phase 1（本Issue）: 既存 `snippet_helper::fetch_snippet()` を活用した基本スニペット付与
- Phase 2（別Issue）: セクション優先抽出（heading ベースのフィルタリング）

## 2. アーキテクチャ設計

### レイヤー構成と責務

```
┌─────────────────────────────────────────────────┐
│ CLI Layer (main.rs)                              │
│  --with-snippet / --snippet-lines / --snippet-chars │
│  → SnippetOptions 構築 → コマンド関数に渡す        │
├─────────────────────────────────────────────────┤
│ Command Layer (cli/issue.rs, cli/before_change.rs)│
│  1. 既存ロジック（SQLite/Git）で結果取得           │
│  2. enrich_*_with_snippets() でスニペット付与      │
│  3. format 関数で出力                             │
├─────────────────────────────────────────────────┤
│ Snippet Helper (cli/snippet_helper.rs)            │
│  enrich_issue_documents_with_snippets()           │
│  enrich_before_change_with_snippets()             │
│  → fetch_snippet() → IndexReaderWrapper           │
├─────────────────────────────────────────────────┤
│ Output Layer (output/human.rs, llm.rs, json.rs)  │
│  snippet フィールドの条件付き表示                  │
├─────────────────────────────────────────────────┤
│ Index Layer (indexer/reader.rs)                   │
│  tantivy IndexReaderWrapper                       │
│  search_by_exact_path() → body フィールド取得      │
└─────────────────────────────────────────────────┘
```

### データフロー

```
CLI引数 → SnippetOptions
              ↓
issue/before-change の結果取得（SQLite/Git）
              ↓
enrich_*_with_snippets(results, reader, options, format)
  ├── enabled=false → スキップ（snippet=None のまま）
  ├── format=Path → スキップ
  └── enabled=true → fetch_snippet(reader, doc_path, config)
        ├── 成功 & 非空 → Some(text)
        └── 失敗/空 → None
              ↓
format_*() で出力（snippet の有無で条件分岐）
```

## 3. 設計判断とトレードオフ

### 判断1: snippet 未取得時の契約

**採用方針**: `Option<String>` に統一。`Some(non-empty)` / `None` のみ。`Some("")` は禁止。

**理由**:
- JSON 出力で `null` と `""` の区別が消費者にとって曖昧
- 既存 `fetch_snippet()` は空文字列を返すが、enrich 関数内で空→None に変換
- 一貫したAPI契約により、フォーマッタ側の条件分岐がシンプルに

**トレードオフ**: fetch_snippet() の戻り値を直接使えず変換が必要だが、enrich 関数内で吸収可能。

**既存関数との統一**: 既存の `enrich_impact_with_snippets()` / `enrich_related_with_snippets()` は `Some(fetch_snippet(...))` で空文字列を `Some("")` として設定している。本Issue のスコープ内で、既存の enrich 関数も空→None 変換に統一するリファクタリングを行う。既存フォーマッタは `!snippet.is_empty()` チェックを既に行っているため影響は軽微。

### 判断2: --with-snippet フラグ（デフォルトオフ）

**採用方針**: 既存の impact/search と同じパターンで `--with-snippet` フラグを追加。デフォルトオフ。

**理由**:
- 後方互換性の維持（既存の出力に影響なし）
- tantivy IndexReader のオープンコストを必要時のみ発生させる
- issue JSON の条件付きスキーマ問題を回避（--with-snippet 未指定時は現行 string[] 維持）

### 判断3: issue JSON のスキーマ変更

**採用方針**: `--with-snippet` 未指定時は現行 `string[]` を維持。指定時のみオブジェクト配列に拡張。

```rust
// --with-snippet 未指定時: 現行互換
{ "documents": { "設計": ["path/to/design.md"] } }

// --with-snippet 指定時: オブジェクト配列
{ "documents": { "設計": [{"file_path": "path/to/design.md", "snippet": "..."}] } }
```

**理由**: JSON 出力を常時変更すると breaking change になり、既存の CI/CD パイプラインや LLM 連携が壊れる。

### 判断4: before-change JSON の snippet フィールド

**採用方針**: `--with-snippet` 指定時のみ `snippet` フィールドを出力。None 時は `null` で出力。

**理由**:
- 既存の impact JSON パターンに準拠
- JSON consumer が snippet フィールドの有無で --with-snippet 指定を判別可能

### 判断5: SnippetConfig のデフォルト値

**採用方針**: `lines=3, chars=200`（既存の `lines=2, chars=120` とは異なる）

**理由**:
- issue/before-change は判断理由の要約が主目的で、コードスニペットより長めの文脈が必要
- 150-200文字の要件を満たすデフォルト
- `--snippet-lines` / `--snippet-chars` で調整可能

**デフォルト値の注入箇所**: main.rs の CLI 引数処理で `snippet_lines.unwrap_or(3)`, `snippet_chars.unwrap_or(200)` とする。定数は `const KNOWLEDGE_SNIPPET_LINES: usize = 3; const KNOWLEDGE_SNIPPET_CHARS: usize = 200;` として main.rs に定義。

### 判断6: tantivy 未存在時のフォールバック

**採用方針**: IndexReaderWrapper のオープンに失敗した場合は snippet: None でフォールバック（エラーにしない）

**理由**:
- `commandindex index` 未実行でも issue/before-change は SQLite ベースで動作する
- スニペットは付加情報であり、取得失敗でコマンド全体が失敗すべきではない

### 判断7: IssueDocumentEntry への snippet 追加

**採用方針**: `IssueDocumentEntry` に直接 `snippet: Option<String>` を追加。別DTOは作成しない。

**理由**:
- 既存の ImpactFileResult, RelatedSearchResult と同じパターン
- 今回のスコープでは表示専用DTOを分離するほどの複雑性はない
- YAGNI: 将来必要になった時にリファクタリングすれば良い

## 4. 型定義の変更

### BeforeChangeFinding

```rust
#[derive(Debug, Clone, Serialize)]
pub struct BeforeChangeFinding {
    pub issue_number: String,
    pub relation: String,
    pub doc_path: String,
    pub doc_title: Option<String>,
    pub similarity: Option<f32>,
    pub snippet: Option<String>,  // 追加
}
```

### IssueDocumentEntry

```rust
#[derive(Debug, Clone, Serialize)]
pub struct IssueDocumentEntry {
    pub file_path: String,
    pub relation: KnowledgeRelation,
    pub doc_subtype: DocSubtype,
    pub snippet: Option<String>,  // 追加
}
```

## 5. CLI引数の追加

### before-change コマンド

```rust
BeforeChange {
    // ... existing fields ...

    /// Enable snippet output for findings
    #[arg(long)]
    with_snippet: bool,

    /// Number of snippet lines (default: 3)
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..=100))]
    snippet_lines: Option<u64>,

    /// Number of snippet characters for single-line body (default: 200)
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..=10000))]
    snippet_chars: Option<u64>,
}
```

### issue コマンド

```rust
Issue {
    // ... existing fields ...

    /// Enable snippet output for documents
    #[arg(long)]
    with_snippet: bool,

    /// Number of snippet lines (default: 3)
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..=100))]
    snippet_lines: Option<u64>,

    /// Number of snippet characters for single-line body (default: 200)
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..=10000))]
    snippet_chars: Option<u64>,
}
```

## 6. snippet_helper.rs の追加関数

```rust
pub(crate) fn enrich_before_change_with_snippets(
    findings: &mut [crate::output::BeforeChangeFinding],
    reader: &IndexReaderWrapper,
    snippet_options: &SnippetOptions,
    format: crate::output::OutputFormat,
) {
    if !snippet_options.enabled || matches!(format, crate::output::OutputFormat::Path) {
        return;
    }
    for finding in findings.iter_mut() {
        let snippet = fetch_snippet(reader, &finding.doc_path, snippet_options.config);
        finding.snippet = if snippet.is_empty() { None } else { Some(snippet) };
    }
}

pub(crate) fn enrich_issue_documents_with_snippets(
    documents: &mut [IssueDocumentEntry],
    reader: &IndexReaderWrapper,
    snippet_options: &SnippetOptions,
    format: crate::output::OutputFormat,
) {
    if !snippet_options.enabled || matches!(format, crate::output::OutputFormat::Path) {
        return;
    }
    for doc in documents.iter_mut() {
        let snippet = fetch_snippet(reader, &doc.file_path, snippet_options.config);
        doc.snippet = if snippet.is_empty() { None } else { Some(snippet) };
    }
}
```

## 7. 出力フォーマット仕様

### before-change human形式

```
path/to/design.md (similarity: 0.85) [#299, has_design]
  設計方針書タイトル
  > z-index指定方式をinline style方式で統一。Z_INDEX定数を直接参照...
```

### before-change llm形式

```
- path/to/design.md [0.85] (#299, has_design) - 設計方針書タイトル
  > z-index指定方式をinline style方式で統一。Z_INDEX定数を直接参照...
```

### before-change json形式（--with-snippet指定時）

```json
{
  "findings": [
    {
      "issue_number": "299",
      "relation": "has_design",
      "doc_path": "path/to/design.md",
      "doc_title": "設計方針書タイトル",
      "similarity": 0.85,
      "snippet": "z-index指定方式をinline style方式で統一..."
    }
  ]
}
```

### issue human形式

```
# Issue #299 関連ドキュメント

## 設計
  path/to/design.md
  > z-index指定方式をinline style方式で統一...
```

### issue llm形式

```markdown
# Issue #299 関連ドキュメント

## 設計
- path/to/design.md
  > z-index指定方式をinline style方式で統一...
```

### issue json形式（--with-snippet指定時）

```json
{
  "issue_number": "299",
  "documents": {
    "設計": [{"file_path": "path/to/design.md", "snippet": "z-index指定方式を..."}]
  }
}
```

### issue json形式（--with-snippet未指定時 = 現行互換）

```json
{
  "issue_number": "299",
  "documents": {
    "設計": ["path/to/design.md"]
  }
}
```

## 8. run_before_change() の変更

### 関数シグネチャ変更

```rust
pub fn run_before_change(
    file: &str,
    format: OutputFormat,
    index_path: Option<&Path>,
    limit: usize,
    max_commits: usize,
    snippet_options: SnippetOptions,  // 追加
) -> Result<(), BeforeChangeError>
```

### スニペット付与タイミング

```rust
// group_and_limit_by_issue() 後、format 出力前
let limited = group_and_limit_by_issue(findings, limit);

// tantivy reader のオープン（snippet 有効時のみ）
if snippet_options.enabled {
    if let Ok(reader) = IndexReaderWrapper::open(&commandindex_dir) {
        enrich_before_change_with_snippets(
            &mut limited,
            &reader,
            &snippet_options,
            format,
        );
    }
    // reader オープン失敗時は snippet: None のまま継続
}
```

## 9. issue::run() の変更

### 関数シグネチャ変更

```rust
pub fn run(
    issue_number: u64,
    format: OutputFormat,
    commandindex_dir: &Path,
    snippet_options: SnippetOptions,  // 追加
) -> Result<(), IssueCommandError>
```

### スニペット付与タイミング

```rust
let mut result = IssueDocumentsResult { ... };

// tantivy reader のオープン（snippet 有効時のみ）
if snippet_options.enabled {
    if let Ok(reader) = IndexReaderWrapper::open(commandindex_dir) {
        enrich_issue_documents_with_snippets(
            &mut result.documents,
            &reader,
            &snippet_options,
            format,
        );
    }
}
```

## 10. セキュリティ設計

| 脅威 | 対策 | 優先度 |
|------|------|--------|
| パストラバーサル | doc_path は tantivy インデックスから取得済みの正規化パス。strip_control_chars() で制御文字除去 | 中 |
| 大量データ | SnippetConfig の lines/chars で出力量制限（上限: lines=100, chars=10000）。issue は LIMIT 100、before-change は limit で制限 | 低 |
| unsafe | 使用しない | 高 |

## 11. 影響範囲

### 変更対象ファイル（15件）

| ファイル | 変更種別 | 変更内容 |
|---|---|---|
| `src/output/mod.rs` | 型追加 | BeforeChangeFinding.snippet |
| `src/indexer/knowledge.rs` | 型追加 | IssueDocumentEntry.snippet（定義は knowledge.rs L179） |
| `src/indexer/symbol_store.rs` | 初期値 | find_documents_by_issue() で snippet: None 設定 |
| `src/cli/snippet_helper.rs` | リファクタリング | 既存 enrich_impact/enrich_related の空→None 変換統一 |
| `src/cli/before_change.rs` | 機能追加 | enrich 呼び出し + テスト更新 |
| `src/cli/issue.rs` | 機能追加 | enrich 呼び出し + フォーマッタ更新 + テスト更新 |
| `src/cli/snippet_helper.rs` | 機能追加 | enrich 関数2つ追加 |
| `src/output/human.rs` | 表示追加 | before-change snippet 表示 |
| `src/output/llm.rs` | 表示追加 | before-change snippet 表示 |
| `src/output/json.rs` | フィールド追加 | before-change snippet |
| `src/main.rs` | CLI引数追加 | --with-snippet 等 3引数 × 2コマンド |
| `src/cli/help_llm.rs` | ドキュメント | コマンド説明更新 |
| `tests/cli_args.rs` | テスト追加 | 新オプション検証 |
| `tests/e2e_issue.rs` | テスト更新 | JSON スキーマ + snippet |
| `tests/e2e_before_change.rs` | テスト追加 | snippet 検証 |
| `tests/output_format.rs` | テスト追加 | フォーマッタ snippet テスト |

### 影響なし

- search, impact, why, suggest 等の他コマンド
- parser, embedding モジュール

## 12. 品質基準

| チェック項目 | コマンド | 基準 |
|---|---|---|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |
