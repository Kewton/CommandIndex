# 設計方針書 - Issue #179: セマンティック検索結果にスニペット（本文抜粋）を追加

## 1. Issue概要

| 項目 | 内容 |
|------|------|
| Issue番号 | #179 |
| タイトル | セマンティック検索結果にスニペット（本文抜粋）が含まれない |
| 種別 | バグ修正 / 機能改善 |
| 優先度 | 高 |

### 問題
`search --semantic` の結果にスニペット（本文抜粋）が含まれず、見出しのみが返る。`estimated tokens: ~0` となりAIエージェントが判断を取り出せない。

### ゴール
セマンティック検索結果にBM25検索と同等のスニペット機能を提供する。`--snippet-lines`/`--snippet-chars`/`--format llm`のmax_body_linesがセマンティック検索でも機能すること。

---

## 2. システムアーキテクチャ概要

### 現在のデータフロー

```
[CLI] main.rs
  ├─ BM25検索パス ─→ search::run() ─→ format_results()
  │     SnippetConfig ✅       LlmFormatOptions ✅
  │
  └─ セマンティック検索パス ─→ search::run_semantic_search() ─→ format_semantic_results()
        SnippetConfig ❌       LlmFormatOptions ❌
```

### 修正後のデータフロー

```
[CLI] main.rs
  ├─ BM25検索パス ─→ search::run() ─→ format_results()
  │     SnippetConfig ✅       LlmFormatOptions ✅
  │
  └─ セマンティック検索パス ─→ search::run_semantic_search() ─→ format_semantic_results()
        SnippetConfig ✅       LlmFormatOptions ✅
```

---

## 3. レイヤー構成と責務

| レイヤー | モジュール | 本Issue での役割 |
|---------|-----------|-----------------|
| **CLI** | `src/main.rs` | SnippetConfig/LlmFormatOptionsをセマンティック検索に渡す |
| **Search** | `src/cli/search.rs` | run_semantic_search()のシグネチャ拡張、enrich_with_metadata()のfallback改善 |
| **Output** | `src/output/mod.rs` | format_semantic_results()のシグネチャ拡張 |
| **Output/Human** | `src/output/human.rs` | format_semantic_human()にSnippetConfig適用 |
| **Output/LLM** | `src/output/llm.rs` | format_semantic_llm()にLlmFormatOptions適用 |
| **Output/JSON** | `src/output/json.rs` | 変更なし（bodyフィールドは全文のまま） |

---

## 4. 設計判断とトレードオフ

### 判断1: bodyフィールドの扱い

**選択**: bodyフィールドをフォーマッタ側でトランケーションする（BM25と同じパターン）

**代替案**: SemanticSearchResultにsnippetフィールドを追加してbodyと分離する

**理由**: BM25検索のformat_human()/format_llm()と同じパターンを踏襲することで一貫性を保つ。JSON出力ではbody全文を返すため、snippet分離は不要（YAGNI）。将来的にsnippetフィールドが必要になった場合は追加対応する。

### 判断2: enrich_with_metadata()のfallback改善

**選択**: heading不一致時にsections.first()のbodyを使用。sectionsが空の場合は現行と同じ空bodyを返す（意図的な設計判断）。

**代替案A**: trim比較や部分一致マッチング
**代替案B**: fallbackなし（空文字のまま）

**理由**: heading不一致は通常、embeddingsの再生成忘れに起因する。最初のセクションのbodyは「何もないより良い」最低限のfallback。完全一致を維持しつつ、ユーザーに空結果を返さない配慮。sectionsが空の場合はtantivyにファイルが未登録の状態であり、空bodyは妥当。

### 判断3: format_semantic_json()の扱い

**選択**: 変更なし（bodyフィールドは全文のまま出力）

**理由**: JSON出力はプログラム的に消費されるため、全文を返す方がユースケースに適合。BM25のformat_json()も同様にbody全文を出力している（OCP準拠）。将来的にJSON出力にもmax_bodyオプションが必要になった場合は、LlmFormatOptionsの拡張ではなく別のJsonFormatOptionsを検討すること。

### 判断4: format_semantic_path()の扱い

**選択**: 変更なし

**理由**: path形式はファイルパスのみの出力であり、スニペットは関係ない。

### 判断5: パラメータ膨張への対応

**選択**: 既存run()と同パターンの引数追加（9パラメータ）

**代替案**: SemanticSearchParams構造体の導入

**理由**: 既存のrun()も9パラメータで同じ構造を持つ。本Issueスコープでは既存パターンの踏襲を優先し、将来のリファクタリングIssueでSemanticSearchOptions構造体の導入を検討する。

### 判断6: ISP（Interface Segregation）への対応

**選択**: format_semantic_results()にsnippet_config/llm_optionsを両方渡す

**理由**: format_results()（BM25用）も同じパターンでllm_optionsをJson/Pathで無視している。内部のmatch文で各フォーマッタに必要なパラメータだけを渡す構造のため、実害は限定的。Json/Pathブランチではsnippet_config/llm_optionsは使用しない。

---

## 5. 詳細設計

### 5.1 型定義の変更

**変更なし**。既存の型をそのまま使用する。

```rust
// src/output/mod.rs - 既存のまま
// SnippetConfigは #[derive(Debug, Clone, Copy)] でCopy trait実装済み
pub struct SnippetConfig {
    pub lines: usize,
    pub chars: usize,
}

pub struct LlmFormatOptions {
    pub max_body_lines: Option<usize>,
}

pub struct SemanticSearchResult {
    pub path: String,
    pub heading: String,
    pub similarity: f32,
    pub body: String,
    pub tags: String,
    pub heading_level: u64,
}
```

### 5.2 run_semantic_search() シグネチャ変更

```rust
// src/cli/search.rs
// Before:
pub fn run_semantic_search(
    query: &str, limit: usize, format: OutputFormat,
    tag: Option<&str>, filters: &SearchFilters,
    ctx: Option<&SearchContext>, max_tokens: Option<usize>,
) -> Result<(), SearchError>

// After:
pub fn run_semantic_search(
    query: &str, limit: usize, format: OutputFormat,
    tag: Option<&str>, filters: &SearchFilters,
    ctx: Option<&SearchContext>, max_tokens: Option<usize>,
    snippet_config: SnippetConfig, llm_options: &LlmFormatOptions,
) -> Result<(), SearchError>
```

内部でformat_semantic_results()にsnippet_config/llm_optionsを伝播する。

### 5.3 format_semantic_results() シグネチャ変更

```rust
// src/output/mod.rs
// Before:
pub fn format_semantic_results(
    results: &[SemanticSearchResult], format: OutputFormat,
    writer: &mut dyn Write,
) -> Result<(), OutputError>

// After:
pub fn format_semantic_results(
    results: &[SemanticSearchResult], format: OutputFormat,
    writer: &mut dyn Write,
    snippet_config: SnippetConfig, llm_options: &LlmFormatOptions,
) -> Result<(), OutputError>
```

内部のmatch文:
- Human → `format_semantic_human(results, writer, snippet_config)`
- Llm → `format_semantic_llm(results, writer, llm_options)`
- Json → `format_semantic_json(results, writer)` （変更なし）
- Path → `format_semantic_path(results, writer)` （変更なし）

### 5.4 format_semantic_human() 変更

```rust
// src/output/human.rs
// Before:
pub fn format_semantic_human(
    results: &[SemanticSearchResult], writer: &mut dyn Write,
) -> Result<(), OutputError>

// After:
pub fn format_semantic_human(
    results: &[SemanticSearchResult], writer: &mut dyn Write,
    snippet_config: SnippetConfig,
) -> Result<(), OutputError>
```

内部変更（format_human()と同じパターンを適用）:
```rust
// lines=0/chars=0の場合は全文表示（format_human()と同じガード）
let effective_lines = if snippet_config.lines == 0 { usize::MAX } else { snippet_config.lines };
let effective_chars = if snippet_config.chars == 0 { usize::MAX } else { snippet_config.chars };
let body_display = truncate_body(&strip_control_chars(&result.body), effective_lines, effective_chars);
```

### 5.5 format_semantic_llm() 変更

```rust
// src/output/llm.rs
// Before:
pub fn format_semantic_llm(
    results: &[SemanticSearchResult], writer: &mut dyn Write,
) -> Result<(), OutputError>

// After:
pub fn format_semantic_llm(
    results: &[SemanticSearchResult], writer: &mut dyn Write,
    llm_options: &LlmFormatOptions,
) -> Result<(), OutputError>
```

内部変更（format_llm()と同じtruncation+was_truncated分岐パターン）:
```rust
// write_body()前にtruncate_body_for_llm()を適用
let (truncated_body, was_truncated) =
    truncate_body_for_llm(&result.body, llm_options.max_body_lines);
// was_truncated時は "... (truncated)" を追加（format_llm()と同パターン）
```

### 5.6 main.rs 呼び出し変更

セマンティック検索ブランチ内でLlmFormatOptionsを構築し、run_semantic_search()に渡す。

```rust
// セマンティック検索ブランチ内:
// snippet_configはmatch文外（L483）で定義済み、Copy traitなのでclone()不要
let llm_options = commandindex::output::LlmFormatOptions {
    max_body_lines: snippet_lines,
};
run_semantic_search(&q, effective_limit, format, tag.as_deref(),
    &filters, ctx_for_semantic.as_ref(), max_tokens,
    snippet_config, &llm_options)
```

> **Note**: `snippet_config` はCopy trait実装済みのため、`clone()` は不要（clippy `clone_on_copy` 警告回避）。`llm_options` はBM25ブランチのローカル変数のため、セマンティック検索ブランチ内で別途構築する。

### 5.7 enrich_with_metadata() fallback改善

```rust
// Before (heading不一致時):
SemanticSearchResult {
    path: item.file_path.clone(),
    heading: item.section_heading.clone(),
    similarity: item.similarity,
    body: String::new(),  // 空文字
    tags: String::new(),
    heading_level: 0,
}

// After:
let fallback = sections.first();
SemanticSearchResult {
    path: item.file_path.clone(),
    heading: item.section_heading.clone(),
    similarity: item.similarity,
    body: fallback.map(|s| s.body.clone()).unwrap_or_default(),
    tags: fallback.map(|s| s.tags.clone()).unwrap_or_default(),
    heading_level: fallback.map(|s| s.heading_level).unwrap_or(0),
}
```

> **Note**: `sections.first()` をローカル変数 `fallback` にバインドし、3フィールド分の冗長呼び出しを回避。sectionsが空の場合は `unwrap_or_default()` により空文字/0を返す（意図的な設計判断）。

---

## 6. 影響範囲

### 変更対象ファイル

| ファイル | 変更種別 | 変更内容 |
|---------|---------|---------|
| `src/main.rs` | 呼び出し変更 | セマンティック分岐内でLlmFormatOptions構築、run_semantic_search()にsnippet_config/llm_optionsを渡す |
| `src/cli/search.rs` | シグネチャ変更 + ロジック変更 | run_semantic_search()パラメータ追加、enrich_with_metadata()のfallback改善 |
| `src/output/mod.rs` | シグネチャ変更 | format_semantic_results()パラメータ追加、内部matchで各フォーマッタに必要なパラメータを伝播 |
| `src/output/human.rs` | シグネチャ変更 + ロジック変更 | format_semantic_human()にSnippetConfig追加、lines=0/chars=0ガード適用 |
| `src/output/llm.rs` | シグネチャ変更 + ロジック変更 | format_semantic_llm()にLlmFormatOptions追加、truncate_body_for_llm + was_truncated分岐適用 |
| `tests/output_format.rs` | テスト更新 | 新シグネチャへの追従 + 新テスト追加 |

### 影響なし

- `src/output/json.rs`: bodyフィールドは全文のまま
- `src/output/path.rs`: パスのみ出力
- `src/cli/snippet_helper.rs`: セマンティック検索ではenrich_with_metadataでbodyを取得済みのため不使用
- **ハイブリッド検索**: `try_hybrid_search()` は `enrich_semantic_to_search_results()` でSearchResult型に変換し、`format_results()` 経由で出力するため、`format_semantic_results()` の変更には影響されない
- BM25検索のコードパス: 変更なし
- CLIインターフェース: 破壊的変更なし（既存オプションが有効になるだけ）
- 外部クレート依存: 追加なし

### デフォルト挙動の変更

`SnippetConfig::default()` の変更がセマンティック検索のデフォルト表示にも反映されるようになる（BM25と一貫した挙動）。現在のデフォルトは `lines: 2, chars: 120` で、ハードコード値と同一のため実質的な挙動変更はない。

---

## 7. セキュリティ設計

| 脅威 | 対策 | 優先度 |
|------|------|--------|
| 大量body出力によるメモリ消費 | truncate_body/truncate_body_for_llmでサイズ制限 | 中 |
| lines=0/chars=0指定時の全文展開 | format_human()と同じusize::MAXガード。ローカルCLIのためリスク限定的 | 低 |
| unsafe使用 | なし（原則禁止に準拠） | - |

---

## 8. テスト方針

### ユニットテスト

| テスト | 対象 | 検証内容 |
|--------|------|---------|
| format_semantic_human + SnippetConfig | human.rs | snippet_config.lines/charsに従ったトランケーション |
| format_semantic_human + lines=0/chars=0 | human.rs | 0指定で全文表示（format_human()と同挙動） |
| format_semantic_llm + LlmFormatOptions | llm.rs | max_body_linesに従ったトランケーション |
| format_semantic_llm + was_truncated | llm.rs | truncation時に"... (truncated)"が表示されること |
| format_semantic_llm body出力 | llm.rs | bodyが空でないSemanticSearchResultの正常出力 |
| enrich_with_metadata fallback | search.rs | heading不一致時にsections.first()のbodyが使用されること |
| enrich_with_metadata sections空 | search.rs | sections空の場合にbodyが空文字になること |

### 既存テスト更新

| テスト | ファイル | 変更内容 |
|--------|---------|---------|
| test_format_semantic_llm | tests/output_format.rs | `format_semantic_results(&results, OutputFormat::Llm, &mut buf, SnippetConfig::default(), &LlmFormatOptions { max_body_lines: None })` に更新 |

### 品質基準

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |
