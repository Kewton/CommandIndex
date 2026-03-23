# 作業計画 - Issue #117: 複数コマンド併用時の合計トークン量制御

## Issue概要
**Issue番号**: #117
**タイトル**: 複数コマンド併用時の合計トークン量制御
**サイズ**: L（12ファイル変更、新規モジュール1件）
**優先度**: Medium
**依存Issue**: #105（context トークン数制御 - 実装済み）
**設計方針書**: `dev-reports/design/issue-117-total-token-control-design-policy.md`

---

## タスク分解

### Phase 1: 共通基盤（token_budget.rs）

#### Task 1.1: output/token_budget.rs 新規作成 + ユニットテスト
- **成果物**: `src/output/token_budget.rs`
- **依存**: なし
- **内容**:
  - `apply_token_budget<T, F>()` 関数実装
    - max_tokens==0 で空Vec早期リターン
    - saturating_add で整数オーバーフロー防止
    - 最初のエントリは予算超過でも必ず含める
  - `tokens_to_char_budget()` 関数実装
  - `truncate_snippet_for_char_budget()` 関数実装
  - `src/output/mod.rs` に `pub(crate) mod token_budget;` 追加
  - ユニットテスト:
    - apply_token_budget: 予算内に収まる
    - apply_token_budget: 最初のエントリ例外
    - apply_token_budget: max_tokens=1
    - apply_token_budget: 空リスト → 空Vec
    - apply_token_budget: max_tokens=0 → 空Vec
    - apply_token_budget: 全アイテムが予算内 → 全件返す
    - tokens_to_char_budget: 変換検証
    - truncate_snippet_for_char_budget: 先頭+末尾保持

#### Task 1.2: context.rs からのユーティリティ移動
- **成果物**: `src/cli/context.rs`（修正）、`src/output/token_budget.rs`（追記）
- **依存**: Task 1.1
- **内容**:
  - context.rs から `tokens_to_char_budget`, `truncate_snippet_for_char_budget` を token_budget.rs に移動
  - context.rs は `use crate::output::token_budget::*;` で呼び出し
  - context.rs 内の関連ユニットテスト（約10件）を token_budget.rs に移動
  - `estimate_entry_meta_tokens`, `estimate_entry_tokens` は context.rs に残す
- **検証**: `cargo test` で既存テスト（特に e2e_context_pack.rs）が全パスすること

### Phase 2: CLI引数定義 + パーステスト

#### Task 2.1: main.rs に --max-tokens 引数追加
- **成果物**: `src/main.rs`
- **依存**: Task 1.1
- **内容**:
  - Commands::Search に `max_tokens: Option<u64>` 追加（value_parser 1..=1_000_000）
  - Commands::Impact に `max_tokens: Option<u64>` 追加（同上）
  - 各サブコマンド分岐で `max_tokens.map(|t| t as usize)` 変換して各run_*関数に伝達
  - **伝達先**: run(), run_symbol_search(), run_related_search(), run_related_search_from_stdin(), run_semantic_search(), run_changed_since(), run_workspace_search(), run_impact()

#### Task 2.2: CLI引数パーステスト
- **成果物**: `tests/cli_args.rs`（追記）
- **依存**: Task 2.1
- **内容**:
  - search --max-tokens 100 受理テスト
  - impact --max-tokens 100 受理テスト
  - --max-tokens 0 拒否テスト
  - --max-tokens 1000001 拒否テスト
  - --max-tokens と --limit の併用受理テスト

### Phase 3: impact コマンド実装

#### Task 3.1: impact.rs に --max-tokens 追加
- **成果物**: `src/cli/impact.rs`
- **依存**: Task 1.1, Task 2.1
- **内容**:
  - `run_impact()` に `max_tokens: Option<usize>` パラメータ追加
  - `enrich_impact_with_snippets` 後、`format_impact_results` 前に `apply_token_budget` 適用
  - estimate クロージャ: `|r| estimate_tokens(&r.file_path) + r.snippet.as_ref().map_or(0, |s| estimate_tokens(s))`
  - ImpactResult.total_impacted_files を打ち切り後の件数に更新

#### Task 3.2: changed_since.rs に max_tokens 伝達
- **成果物**: `src/cli/changed_since.rs`
- **依存**: Task 3.1
- **内容**:
  - `run_changed_since()` に `max_tokens: Option<usize>` パラメータ追加
  - `run_impact()` 呼び出しに max_tokens を伝達
  - SnippetOptions::default() は現状維持（意図的制限）

#### Task 3.3: impact E2E テスト
- **成果物**: `tests/e2e_impact.rs`（追記）、`tests/e2e_changed_since.rs`（追記）
- **依存**: Task 3.1, Task 3.2
- **内容**:
  - impact --max-tokens でトークン制限が動作することの検証
  - impact --max-tokens 未指定時の後方互換性テスト
  - changed_since --max-tokens の伝達検証

### Phase 4: search コマンド実装

#### Task 4.1: search.rs の全モードに --max-tokens 追加
- **成果物**: `src/cli/search.rs`
- **依存**: Task 1.1, Task 2.1
- **内容**:
  - `run()` に max_tokens 追加 + `#[allow(clippy::too_many_arguments)]`
    - rerank 後、Human/non-Human 分岐前に apply_token_budget 適用
    - estimate: `|r| estimate_tokens(&r.body)`
  - `run_symbol_search()` に max_tokens 追加
    - estimate: 非再帰版 estimate_symbol_result_tokens
  - `run_related_search()` に max_tokens 追加
    - estimate: `|r| estimate_tokens(&r.file_path) + r.snippet.as_ref().map_or(0, |s| estimate_tokens(s))`
  - `run_related_search_from_stdin()` に max_tokens 追加
  - `run_semantic_search()` に max_tokens 追加
    - estimate: `|r| estimate_tokens(&r.body)`

#### Task 4.2: workspace.rs に max_tokens 伝達
- **成果物**: `src/cli/workspace.rs`
- **依存**: Task 4.1
- **内容**:
  - `run_workspace_search()` に max_tokens 追加
  - rrf_merge_multiple 後の集約結果に apply_token_budget 一括適用

#### Task 4.3: search E2E テスト
- **成果物**: `tests/e2e_related_search.rs`（追記）、`tests/e2e_workspace.rs`（追記）
- **依存**: Task 4.1, Task 4.2
- **内容**:
  - search --max-tokens でトークン制限が動作する検証
  - search --related --max-tokens の制限検証
  - search --symbol --max-tokens の制限検証
  - --max-tokens 未指定時の後方互換性テスト

### Phase 5: help_llm + 最終検証

#### Task 5.1: help_llm.rs 更新
- **成果物**: `src/cli/help_llm.rs`
- **依存**: Task 3.1, Task 4.1
- **内容**:
  - search CommandInfo の key_options に `--max-tokens <N>` 追加
  - impact CommandInfo の key_options に `--max-tokens <N>` 追加

#### Task 5.2: 最終品質チェック
- **依存**: 全タスク
- **内容**:
  - `cargo build` エラー0件
  - `cargo clippy --all-targets -- -D warnings` 警告0件
  - `cargo test --all` 全テストパス
  - `cargo fmt --all -- --check` 差分なし

---

## タスク依存関係

```
Task 1.1 (token_budget.rs 新規)
  ├── Task 1.2 (context.rs 移動)
  ├── Task 2.1 (main.rs 引数定義)
  │     ├── Task 2.2 (CLI パーステスト)
  │     ├── Task 3.1 (impact.rs)
  │     │     ├── Task 3.2 (changed_since.rs)
  │     │     └── Task 3.3 (impact E2E テスト)
  │     ├── Task 4.1 (search.rs)
  │     │     ├── Task 4.2 (workspace.rs)
  │     │     └── Task 4.3 (search E2E テスト)
  │     └── Task 5.1 (help_llm.rs)
  └── Task 5.2 (最終品質チェック)
```

---

## 変更ファイル一覧

| ファイル | 変更種別 | Phase |
|---------|---------|-------|
| `src/output/token_budget.rs` | **新規** | 1 |
| `src/output/mod.rs` | 修正（モジュール宣言追加） | 1 |
| `src/cli/context.rs` | 修正（ユーティリティ移動） | 1 |
| `src/main.rs` | 修正（引数追加+伝達） | 2 |
| `tests/cli_args.rs` | 追記（パーステスト） | 2 |
| `src/cli/impact.rs` | 修正（max_tokens追加） | 3 |
| `src/cli/changed_since.rs` | 修正（max_tokens伝達） | 3 |
| `tests/e2e_impact.rs` | 追記（E2Eテスト） | 3 |
| `tests/e2e_changed_since.rs` | 追記（E2Eテスト） | 3 |
| `src/cli/search.rs` | 修正（全モードmax_tokens追加） | 4 |
| `src/cli/workspace.rs` | 修正（max_tokens伝達） | 4 |
| `tests/e2e_related_search.rs` | 追記（E2Eテスト） | 4 |
| `tests/e2e_workspace.rs` | 追記（E2Eテスト） | 4 |
| `src/cli/help_llm.rs` | 修正（CommandInfo更新） | 5 |

---

## 品質チェック項目

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |

---

## Definition of Done

- [ ] 全14ファイルの変更・追加が完了
- [ ] output/token_budget.rs のユニットテスト全パス
- [ ] CLI引数パーステスト全パス
- [ ] E2E テスト（impact, changed_since, search, workspace）全パス
- [ ] --max-tokens 未指定時の後方互換性テスト全パス
- [ ] cargo build / clippy / test / fmt 全パス
- [ ] help_llm 出力に新オプション情報含む
