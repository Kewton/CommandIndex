# 設計方針書 - Issue #117: 複数コマンド併用時の合計トークン量制御

## 1. Issue概要

| 項目 | 内容 |
|------|------|
| Issue番号 | #117 |
| タイトル | 複数コマンド併用時の合計トークン量制御 |
| 優先度 | 中 |
| 関連Issue | #105 (context トークン数制御 - 実装済み) |

### 目的
search / impact の各コマンドに `--max-tokens` オプションを追加し、出力のトークン量を制限する。呼び出し側（anvil等）が各コマンドのトークン配分を制御できるようにする。

---

## 2. システムアーキテクチャ概要

### レイヤー構成と責務

| レイヤー | モジュール | 責務 | 本Issue での変更 |
|---------|-----------|------|-----------------|
| **CLI** | `src/main.rs` | clap サブコマンド定義 | search/impact に `--max-tokens` 引数追加 |
| **CLI Logic** | `src/cli/search.rs` | 検索ロジック実行 | 全モードに max_tokens パラメータ伝達 |
| **CLI Logic** | `src/cli/impact.rs` | 影響範囲分析 | max_tokens パラメータ追加、打ち切りロジック |
| **CLI Logic** | `src/cli/changed_since.rs` | 変更ファイル検出 → impact 委譲 | max_tokens を run_impact に伝達 |
| **CLI Logic** | `src/cli/context.rs` | コンテキストパック構築 | **変更なし**（トークン制限ユーティリティを抽出） |
| **CLI Logic** | `src/cli/help_llm.rs` | LLM向けヘルプ出力 | CommandInfo に新オプション追記 |
| **Output** | `src/output/mod.rs` | 出力型定義、estimate_tokens | **変更なし** |
| **Output** | `src/output/token_budget.rs` | **新規**: トークン打ち切り共通ロジック | 新規作成 |
| **Output** | `src/output/llm.rs` | LLM出力フォーマット | **変更なし** |

---

## 3. 設計方針

### 3.1 全体方針: 各コマンド個別 --max-tokens（方針A）

```
┌─────────────────────────────────────────────────────┐
│  呼び出し側（anvil等）                                │
│                                                     │
│  search --max-tokens 2000 → 2KB分の検索結果          │
│  impact --max-tokens 1500 → 1.5KB分の影響分析        │
│  context --max-tokens 3000 → 3KB分のコンテキスト     │
│                                                     │
│  合計: ≈ 6.5KB（呼び出し側で配分制御）                │
└─────────────────────────────────────────────────────┘
```

**理由**: commandindex は各サブコマンドが独立した CLI コマンドとして実行される設計。コマンド横断の合計制御は呼び出し側の責務とするのが最も自然。

### 3.2 共通トークン打ち切りモジュール

context.rs の打ち切りロジックを `output/token_budget.rs` に抽出し、各コマンドから共通利用する。

**モジュール宣言**: `src/output/mod.rs` に `pub(crate) mod token_budget;` として宣言。

**配置方針**:
- `token_budget.rs` には**汎用関数のみ**を配置: `apply_token_budget`, `tokens_to_char_budget`, `truncate_snippet_for_char_budget`
- context 固有の関数（`estimate_entry_meta_tokens`, `estimate_entry_tokens`）は **context.rs に残す**（KISS原則: 過度な共通化を避ける）
- 各出力型用の `estimate_*_result_tokens` 関数は **各CLIモジュール内で定義**するか、`apply_token_budget` にクロージャとして渡す（God Module化を防止）
- `estimate_tokens` は `output/mod.rs` に `pub(crate)` で定義済み。`token_budget.rs` から `use super::estimate_tokens;` で参照可能

**context.rs のテスト移動**: `tokens_to_char_budget`, `truncate_snippet_for_char_budget` に関する既存ユニットテスト（約10件）も `token_budget.rs` に移動する。

```rust
// src/output/token_budget.rs

/// トークン数から文字数予算に変換（1トークン ≈ 4文字）
pub(crate) fn tokens_to_char_budget(tokens: usize) -> usize {
    tokens * 4
}

/// スニペットを文字数予算内に縮約
/// 先頭(3/5)と末尾(2/5)を保持し中間を "..." で省略
pub(crate) fn truncate_snippet_for_char_budget(
    snippet: &str,
    budget_chars: usize,
) -> String {
    // 既存の context.rs の実装を移動
}

/// 結果リストにトークン予算を適用し、予算内に収まるよう打ち切る
/// - items: スコア順にソート済みの結果リスト
/// - max_tokens: トークン予算（> 0 が前提。0の場合は空Vecを早期リターン）
/// - estimate_fn: 各アイテムのトークン推定関数（クロージャで注入、型依存を排除）
/// - 最初のアイテムは予算超過でも必ず含める
pub(crate) fn apply_token_budget<T, F>(
    items: Vec<T>,
    max_tokens: usize,
    estimate_fn: F,
) -> Vec<T>
where
    F: Fn(&T) -> usize,
{
    if max_tokens == 0 {
        return Vec::new();
    }
    let mut result = Vec::new();
    let mut used_tokens: usize = 0;
    for (i, item) in items.into_iter().enumerate() {
        let item_tokens = estimate_fn(&item);
        if i > 0 && used_tokens.saturating_add(item_tokens) > max_tokens {
            break;
        }
        used_tokens = used_tokens.saturating_add(item_tokens);
        result.push(item);
    }
    result
}
```

> **セキュリティ注記**: `used_tokens` の加算には `saturating_add` を使用し、整数オーバーフローを防止する。

### 3.3 各出力型のトークン推定

各出力型ごとにコンテンツベースのトークン推定クロージャを `apply_token_budget` に渡す。関数は各CLIモジュール内で定義し、`token_budget.rs` が全出力型に依存する God Module 化を防止する。

**型の名前空間**: `SearchResult` / `SearchOptions` / `SearchFilters` は `crate::indexer::reader` で定義。他の型は `crate::output` で定義。

```rust
// search.rs 内で使用（SearchResult は crate::indexer::reader::SearchResult）
// body が支配的
|r: &SearchResult| estimate_tokens(&r.body)

// search.rs 内で使用（SymbolSearchResult は crate::output::SymbolSearchResult）
// 現在の build_symbol_tree は1階層の children のみ取得するため、
// フラットに children を走査する非再帰版で十分。
// 将来的に深いネストが導入された場合は再帰版に変更する。
fn estimate_symbol_result_tokens(r: &SymbolSearchResult) -> usize {
    let text = format!("{} {} {}", r.name, r.kind, r.file_path);
    let children_tokens: usize = r.children.iter()
        .map(|c| {
            let child_text = format!("{} {} {}", c.name, c.kind, c.file_path);
            estimate_tokens(&child_text)
        })
        .sum();
    estimate_tokens(&text) + children_tokens
}

// search.rs 内で使用（RelatedSearchResult は crate::output::RelatedSearchResult）
|r: &RelatedSearchResult| {
    let mut tokens = estimate_tokens(&r.file_path);
    if let Some(ref snippet) = r.snippet {
        tokens = tokens.saturating_add(estimate_tokens(snippet));
    }
    tokens
}

// search.rs 内で使用（SemanticSearchResult は crate::output::SemanticSearchResult）
|r: &SemanticSearchResult| estimate_tokens(&r.body)

// impact.rs 内で使用（ImpactFileResult は crate::output::ImpactFileResult）
|r: &ImpactFileResult| {
    let mut tokens = estimate_tokens(&r.file_path);
    if let Some(ref snippet) = r.snippet {
        tokens = tokens.saturating_add(estimate_tokens(snippet));
    }
    tokens
}
```

### 3.4 オプション間の適用順序

```
search コマンド:
  検索実行 → [hybrid統合] → [rerank] → --limit で件数制限 → --max-tokens で打ち切り → 出力

impact コマンド:
  関連検索 → 集約 → --limit で件数制限 → スニペット付与 → --max-tokens で打ち切り → 出力
```

**重要**: `--max-tokens` は常に最後に適用される。`--limit` や `--rerank` の結果に対してトークン予算を適用する。

---

## 4. 変更対象の詳細設計

### 4.1 src/main.rs — CLI引数定義

```rust
// Commands::Search に追加
/// Limits total estimated tokens in output (approx. 1 token per 4 chars)
#[arg(long, value_parser = clap::value_parser!(u64).range(1..=1_000_000))]
max_tokens: Option<u64>,

// Commands::Impact に追加
/// Limits total estimated tokens in output (approx. 1 token per 4 chars)
#[arg(long, value_parser = clap::value_parser!(u64).range(1..=1_000_000))]
max_tokens: Option<u64>,
```

**パラメータ伝達マップ**:
```
main.rs (Search)
  ├── query あり → run() に max_tokens 伝達
  ├── symbol あり → run_symbol_search() に max_tokens 伝達
  ├── related あり → run_related_search() に max_tokens 伝達
  ├── related_stdin → run_related_search_from_stdin() に max_tokens 伝達
  ├── semantic あり → run_semantic_search() に max_tokens 伝達
  ├── changed_since あり → run_changed_since() に max_tokens 伝達 → run_impact() に伝達
  └── workspace → run_workspace_search() に max_tokens 伝達

main.rs (Impact)
  └── run_impact() に max_tokens 伝達
```

### 4.2 src/cli/search.rs — 関数シグネチャ変更

```rust
// 各関数に max_tokens: Option<usize> パラメータを追加

pub fn run(
    ctx: &SearchContext,
    options: &SearchOptions,
    filters: &SearchFilters,
    format: OutputFormat,
    snippet_config: SnippetConfig,
    rerank: bool,
    rerank_top: Option<usize>,
    max_tokens: Option<usize>,        // ← 追加
) -> Result<(), SearchError>

pub fn run_symbol_search(
    symbol_name: &str, limit: usize, format: OutputFormat,
    ctx: Option<&SearchContext>,
    max_tokens: Option<usize>,        // ← 追加
) -> Result<(), SearchError>

pub fn run_related_search(
    file_paths: &[String], limit: usize, format: OutputFormat,
    ctx: Option<&SearchContext>, snippet_options: SnippetOptions,
    max_tokens: Option<usize>,        // ← 追加
) -> Result<(), SearchError>

pub fn run_related_search_from_stdin(
    limit: usize, format: OutputFormat, snippet_options: SnippetOptions,
    max_tokens: Option<usize>,        // ← 追加
) -> Result<(), SearchError>

pub fn run_semantic_search(
    query: &str, limit: usize, format: OutputFormat, tag: Option<&str>,
    filters: &SearchFilters, ctx: Option<&SearchContext>,
    max_tokens: Option<usize>,        // ← 追加
) -> Result<(), SearchError>
```

**run() 内のトークン打ち切り適用箇所**:
```rust
// rerank 後、Human/non-Human フォーマット分岐の前に適用
let results = /* 検索結果 */;
let results = if let Some(max_tokens) = max_tokens {
    apply_token_budget(results, max_tokens, |r| estimate_tokens(&r.body))
} else {
    results
};
// 既存の Human/non-Human 分岐でフォーマット出力
```

**u64 → usize 変換**: main.rs での伝達時に `max_tokens.map(|t| t as usize)` で変換。value_parser の上限が 1_000_000 のため usize オーバーフローの心配はない（context コマンドの既存パターンと統一）。

### 4.3 src/cli/impact.rs — 関数シグネチャ変更

```rust
pub fn run_impact(
    files: &[String],
    format: OutputFormat,
    limit: Option<usize>,
    index_path: Option<&Path>,
    snippet_options: crate::cli::snippet_helper::SnippetOptions,
    max_tokens: Option<usize>,        // ← 追加
) -> Result<(), ImpactError>
```

**適用箇所**: `enrich_impact_with_snippets` 後、`format_impact_results` 前に打ち切り適用。

```rust
let impacted_files = enrich_impact_with_snippets(impacted_files, &snippet_options);

let impacted_files = if let Some(max_tokens) = max_tokens {
    apply_token_budget(impacted_files, max_tokens, |r| {
        let mut tokens = estimate_tokens(&r.file_path);
        if let Some(ref snippet) = r.snippet {
            tokens = tokens.saturating_add(estimate_tokens(snippet));
        }
        tokens
    })
} else {
    impacted_files
};
// 打ち切り後に total_impacted_files を更新
// ImpactResult 構築時に impacted_files.len() を使用する
```

> **注意**: `ImpactResult.total_impacted_files` は `apply_token_budget` 適用後の件数を反映すること。打ち切り前の件数ではなく、実際に出力されるアイテム数と一致させる。

> **SnippetOptions 伝達**: `changed_since.rs` は `SnippetOptions::default()` を固定で使用する（意図的制限）。main.rs の changed_since 分岐では snippet_options を構築しない現行動作を維持する。

### 4.4 src/cli/changed_since.rs — パラメータ伝達

```rust
pub fn run_changed_since(
    since: &str,
    format: OutputFormat,
    limit: Option<usize>,
    index_path: Option<&Path>,
    max_tokens: Option<usize>,        // ← 追加
) -> Result<(), ChangedSinceError>

// 内部で run_impact() に伝達
run_impact(&changed_files, format, limit, index_path, SnippetOptions::default(), max_tokens)
```

### 4.5 src/output/token_budget.rs — 新規共通モジュール

context.rs から以下の関数を移動:
- `tokens_to_char_budget()`
- `truncate_snippet_for_char_budget()`

新規追加:
- `apply_token_budget<T, F>()`（ジェネリックな打ち切り関数）
- 各出力型用の `estimate_*_result_tokens()` 関数群

context.rs は `token_budget.rs` の関数を呼び出すように変更（既存ロジックの動作変更なし）。

### 4.6 src/cli/workspace.rs — パラメータ伝達

```rust
pub fn run_workspace_search(
    // 既存パラメータ...
    max_tokens: Option<usize>,        // ← 追加
) -> Result<(), SearchError>
```

**適用方式**: 全リポジトリの検索結果を `rrf_merge_multiple` で集約した後、最終結果に対して `apply_token_budget` を一括適用する。リポジトリ毎のトークン分配は Phase 1 では不要（YAGNI原則）。

> **clippy 対応**: `run_workspace_search` は既に `#[allow(clippy::too_many_arguments)]` が付与されている。search.rs の `run()` も8引数になるため同様に `#[allow]` を追加する。将来的に `SearchParams` 構造体へのリファクタリングを検討。

### 4.7 src/cli/help_llm.rs — CommandInfo 更新

search コマンドの `key_options` に追加:
```rust
"--max-tokens <N>  Limit output to estimated token count (1-1000000)"
```

impact コマンドの `key_options` に追加:
```rust
"--max-tokens <N>  Limit output to estimated token count (1-1000000)"
```

### 4.7 src/output/mod.rs — モジュール宣言

```rust
pub(crate) mod token_budget;  // ← 追加（crate 外に公開不要）
```

---

## 5. 影響範囲

### 変更ファイル一覧

| ファイル | 変更種別 | 内容 |
|---------|---------|------|
| `src/main.rs` | 修正 | search/impact に --max-tokens 引数追加、各関数呼び出しに伝達 |
| `src/cli/search.rs` | 修正 | 全関数に max_tokens パラメータ追加、打ち切りロジック適用 |
| `src/cli/impact.rs` | 修正 | run_impact に max_tokens 追加、打ち切りロジック適用 |
| `src/cli/changed_since.rs` | 修正 | run_changed_since に max_tokens 追加、run_impact に伝達 |
| `src/cli/context.rs` | 修正 | トークンユーティリティ関数を token_budget.rs に移動、呼び出し元変更 |
| `src/cli/workspace.rs` | 修正 | run_workspace_search に max_tokens パラメータ追加、集約後に適用 |
| `src/cli/help_llm.rs` | 修正 | search/impact の CommandInfo に --max-tokens 追記 |
| `src/output/mod.rs` | 修正 | token_budget モジュール宣言追加（`pub(crate) mod`） |
| `src/output/token_budget.rs` | **新規** | 共通トークン打ち切りロジック |
| `tests/cli_args.rs` | 既存追記 | --max-tokens パースのバリデーションテスト追加 |
| `tests/e2e_impact.rs` | 既存追記 | impact --max-tokens の E2E テスト追加 |
| `tests/e2e_related_search.rs` | 既存追記 | search --related --max-tokens の E2E テスト追加 |
| `tests/e2e_changed_since.rs` | 既存追記 | changed_since --max-tokens の E2E テスト追加 |
| `tests/e2e_workspace.rs` | 既存追記 | workspace --max-tokens の E2E テスト追加 |

### 既存機能への影響
- `--max-tokens` 未指定時は **全て None** となり、既存の動作を一切変更しない
- context コマンドは **変更なし**（内部ユーティリティの移動のみ）
- suggest コマンドは **Phase 1 スコープ外**

---

## 6. 設計判断とトレードオフ

### 判断1: コンテンツベース推定 vs フォーマット込み推定

**採用**: コンテンツベース推定（フォーマットオーバーヘッドは無視）

| 方式 | メリット | デメリット |
|------|---------|----------|
| コンテンツベース | シンプル、フォーマット非依存 | 実出力が推定より大きくなる |
| フォーマット込み | 実出力に近い推定 | フォーマット毎にロジックが必要、保守コスト大 |

> **日本語推定精度の制約**: 現在の推定式（chars().count() / 4）は英語テキスト向け。日本語テキストでは1文字≒1-2トークンのため、推定値と実値が最大4倍乖離する可能性がある。Phase 1 では許容し、将来的にASCII/非ASCII 係数分離を検討。

> **SearchResult の推定対象**: `body` のみ使用し、`path`/`heading` は省略する。body がトークンの大部分を占め、path/heading は微量なため精度への影響は軽微。

### 判断2: 打ち切り粒度 — アイテム単位 vs 文字列単位

**採用**: アイテム単位（結果アイテムを丸ごと含めるか除外するか）

**理由**: 文字列単位の切り詰めは部分的な検索結果を返すことになり、LLM にとって有害。context では snippet を縮約するが、search/impact では結果アイテム単位の方が情報品質が高い。

> **補足**: search/impact では snippet の文字列単位切り詰めは行わない。context コマンドの `truncate_snippet_for_char_budget` は context 固有の使い方として残すが、search/impact のトークン制限には使用しない。

### 判断3: suggest コマンドの除外

**採用**: Phase 1 スコープから除外

**理由**: suggest の出力は通常小さく（コマンド提案の数行）、途中切断で不完全な戦略をLLMが受け取るリスクがある。

### 判断4: 最初のエントリ例外

**採用**: max_tokens 超過でも最初のエントリは必ず含める（context と同じ設計）

**理由**: max_tokens に極小値を指定した場合でも、少なくとも1件の結果を返すことでユーザーに有用な情報を提供する。

---

## 7. セキュリティ設計

| 脅威 | 対策 | 優先度 |
|------|------|--------|
| 不正な引数値 | clap の value_parser で 1..=1_000_000 の範囲制限 | 高 |
| 巨大な出力によるメモリ消費 | max_tokens で出力量を制限（DoS対策として有効） | 中 |
| 整数オーバーフロー | `saturating_add` を使用してトークン累積の加算を安全に実行 | 高 |
| 再帰的スタックオーバーフロー | `estimate_symbol_result_tokens` に最大再帰深度（10階層）を設定 | 中 |
| unsafe 使用 | 使用禁止 | 高 |
| パストラバーサル | 本機能はファイル内容の読み取りを行わない（出力のみ）ため実害なし。対策不要 | 低 |

---

## 8. テスト戦略

### CLI引数パーステスト（tests/cli_args.rs）
- search --max-tokens 受理テスト
- impact --max-tokens 受理テスト
- --max-tokens 0 拒否テスト（value_parser）
- --max-tokens 1000001 拒否テスト
- --max-tokens と --limit の併用テスト

### E2Eテスト
- search --max-tokens でトークン制限が動作することの検証
- search --related --max-tokens での制限検証
- impact --max-tokens での制限検証
- --max-tokens 未指定時の後方互換性テスト

### ユニットテスト（output/token_budget.rs）
- apply_token_budget: 予算内に収まることの検証
- apply_token_budget: 最初のエントリ例外の検証
- apply_token_budget: max_tokens=1 の動作（実質1件のみ返す）
- apply_token_budget: 空リスト入力で空 Vec が返ること
- truncate_snippet_for_char_budget: 先頭+末尾保持の検証
- context.rs から移動した既存テスト（約10件）

---

## 9. 品質基準

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |

---

## 10. 実装の優先順序（TDDフロー）

各ステップでテストファースト → 実装の順で進める。

1. `output/token_budget.rs` 新規作成 + ユニットテスト（apply_token_budget, tokens_to_char_budget, truncate_snippet_for_char_budget）
2. `context.rs` からのユーティリティ移動 + 既存テスト移動（既存 e2e_context_pack.rs の動作確認）
3. `main.rs` の引数定義（search/impact に --max-tokens）
4. `tests/cli_args.rs` にパーステスト追加
5. `impact.rs` に `--max-tokens` 追加 + E2E テスト
6. `changed_since.rs` に max_tokens 伝達 + E2E テスト
7. `search.rs` に `--max-tokens` 追加（全モード）+ E2E テスト
8. `workspace.rs` に max_tokens 伝達 + E2E テスト
9. `help_llm.rs` 更新
