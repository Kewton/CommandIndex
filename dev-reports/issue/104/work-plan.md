# 作業計画: Issue #104 LLM向け出力フォーマットの追加

## Issue: LLM向け出力フォーマットの追加 (--format llm)
**Issue番号**: #104
**サイズ**: M
**優先度**: High
**依存Issue**: なし
**ブランチ**: `feature/issue-104-llm-format`（作成済み）

---

## Phase 1: 基盤変更

### Task 1.1: estimate_tokensの移動
- **成果物**: `src/output/mod.rs`, `src/cli/context.rs`
- **依存**: なし
- **作業内容**:
  1. `src/output/mod.rs` に `pub(crate) fn estimate_tokens(text: &str) -> usize { text.len() / 4 }` を追加
  2. `src/cli/context.rs` L375の `fn estimate_tokens` 定義を削除
  3. `src/cli/context.rs` に `use crate::output::estimate_tokens;` を追加
  4. `cargo build` で既存コードが壊れないことを確認

### Task 1.2: OutputFormat enum拡張
- **成果物**: `src/output/mod.rs`
- **依存**: なし
- **作業内容**:
  1. `OutputFormat` enumに `Llm` バリアントを追加
  2. `pub mod llm;` を追加
  3. 空の `src/output/llm.rs` を作成（コンパイルを通すため）
  4. 7つの `format_*` 関数に `OutputFormat::Llm` アームを追加（仮実装: Pathフォーマットにフォールバック）
  5. `cargo build` でコンパイル通ること確認

## Phase 2: LLMフォーマット関数実装（TDD）

### Task 2.1: format_llm（全文検索）+ テスト
- **成果物**: `src/output/llm.rs`, `tests/output_format.rs`
- **依存**: Task 1.2
- **作業内容**:
  1. テストを先に書く: `test_format_llm_basic`, `test_format_llm_empty`, `test_format_llm_grouping`, `test_format_llm_code_fence`, `test_format_llm_markdown_no_fence`, `test_format_llm_estimated_tokens`, `test_format_llm_strip_control_chars`
  2. ヘルパー関数実装: `detect_language()`, `is_code_file()`, `group_by_path()`, `fence_backticks()`
  3. `format_llm()` を実装
  4. mod.rsの `format_results` のLlmアームを正式実装に差し替え

### Task 2.2: format_semantic_llm + format_workspace_llm + テスト
- **成果物**: `src/output/llm.rs`, `tests/output_format.rs`
- **依存**: Task 2.1（ヘルパー関数を再利用）
- **作業内容**:
  1. テスト: `test_format_semantic_llm`, `test_format_workspace_llm`
  2. `format_semantic_llm()`, `format_workspace_llm()` を実装
  3. mod.rsのLlmアームを差し替え

### Task 2.3: format_symbol_llm + format_related_llm + テスト
- **成果物**: `src/output/llm.rs`, `tests/output_format.rs`
- **依存**: Task 1.2
- **作業内容**:
  1. テスト: `test_format_symbol_llm`, `test_format_related_llm`
  2. `format_symbol_llm()`, `format_related_llm()` を実装
  3. mod.rsのLlmアームを差し替え

### Task 2.4: format_diff_llm + format_impact_llm + テスト
- **成果物**: `src/output/llm.rs`, `tests/output_format.rs`
- **依存**: Task 1.2
- **作業内容**:
  1. テスト: `test_format_diff_llm`, `test_format_impact_llm`
  2. `format_diff_llm()`, `format_impact_llm()` を実装
  3. mod.rsのLlmアームを差し替え

## Phase 3: 既存テスト・ヘルプ更新

### Task 3.1: 既存テスト更新
- **成果物**: `tests/output_format.rs`, `tests/e2e_integration.rs`
- **依存**: Phase 2完了
- **作業内容**:
  1. `test_format_empty_results` の列挙に `OutputFormat::Llm` を追加
  2. `e2e_output_formats` テストにllmフォーマット検証を追加
  3. `cargo test --all` で全テストパス確認

### Task 3.2: CLI引数ヘルプ更新
- **成果物**: `src/main.rs`
- **依存**: Task 1.2
- **作業内容**:
  1. L52のヘルプコメント更新: `"Output format (human, json, path, llm)"`
  2. L149のDiffコマンドヘルプ更新
  3. L205のImpactコマンドヘルプ更新

### Task 3.3: help-llm更新
- **成果物**: `src/cli/help_llm.rs`
- **依存**: Task 1.2
- **作業内容**:
  1. search(L286)の `output_formats` に `"llm"` を追加
  2. diff(L381)の `output_formats` を `["human", "json", "path", "llm"]` に修正（path欠落も修正）
  3. impact(L476)の `output_formats` に `"llm"` を追加
  4. 各コマンドの `key_options` 説明文を `"human, json, path, llm"` に更新

## Phase 4: 品質チェック

### Task 4.1: 最終品質チェック
- **成果物**: なし（チェックのみ）
- **依存**: Phase 3完了
- **作業内容**:
  1. `cargo build` — エラー0件
  2. `cargo clippy --all-targets -- -D warnings` — 警告0件
  3. `cargo test --all` — 全テストパス
  4. `cargo fmt --all -- --check` — 差分なし

---

## Definition of Done

- [x] `OutputFormat::Llm` が追加され `--format llm` で選択可能
- [ ] 全7サブコマンドでLLM出力が動作する
- [ ] search結果がMarkdown形式でpath + スニペットのみ出力される
- [ ] 出力ヘッダーに estimated tokens が含まれる
- [ ] `help-llm` コマンドの `output_formats` に `llm` が追加される
- [ ] 既存テストが全パス
- [ ] 新規テストが追加される
- [ ] `cargo clippy --all-targets -- -D warnings` で警告0件

## タスク依存関係

```
Task 1.1 (estimate_tokens移動) ──┐
                                  ├→ Task 2.1 (format_llm) → Task 2.2 (semantic/workspace)
Task 1.2 (enum拡張) ─────────────┤→ Task 2.3 (symbol/related)
                                  ├→ Task 2.4 (diff/impact)
                                  ├→ Task 3.2 (main.rsヘルプ)
                                  └→ Task 3.3 (help-llm更新)

Phase 2完了 → Task 3.1 (既存テスト更新) → Task 4.1 (品質チェック)
```
