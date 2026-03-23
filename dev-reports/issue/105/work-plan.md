# 作業計画: Issue #105 — context コマンドのトークン数制御の実効化

## Issue概要
**Issue番号**: #105
**タイトル**: context コマンドのトークン数制御の実効化
**サイズ**: M
**優先度**: 高
**依存Issue**: なし
**ブランチ**: `feature/issue-105-context-token`（既存）

---

## 詳細タスク分解

### Phase 1: コアロジック実装（src/cli/context.rs）

- [ ] **Task 1.1**: `estimate_tokens` 関数の改修
  - 成果物: `src/cli/context.rs`
  - 内容: `text.len() / 4` → `text.chars().count() / 4`（最低1トークン）に変更
  - 依存: なし
  - テスト: 単体テスト（ASCII、日本語、空文字列、混合テキスト）

- [ ] **Task 1.2**: `tokens_to_char_budget` ヘルパー関数新設
  - 成果物: `src/cli/context.rs`
  - 内容: トークン数→文字数変換（`tokens * 4`）
  - 依存: なし
  - テスト: 単体テスト

- [ ] **Task 1.3**: `estimate_entry_meta_tokens` / `estimate_entry_tokens` 新設
  - 成果物: `src/cli/context.rs`
  - 内容: ContextEntry全フィールド（path, relation, score, heading, symbols）のメタデータトークン推定 + snippet合算
  - 依存: Task 1.1
  - テスト: 単体テスト（全フィールドあり/なしパターン）

- [ ] **Task 1.4**: `truncate_snippet_for_char_budget` 新設
  - 成果物: `src/cli/context.rs`
  - 内容: 文字数予算に基づく先頭60%+末尾40%切り詰め（アンダーフロー対策済み）
  - 依存: なし
  - テスト: 単体テスト（予算内/超過/0予算/短文/境界値budget_chars=1〜5）

- [ ] **Task 1.5**: `build_context_pack` 改修
  - 成果物: `src/cli/context.rs`
  - 内容:
    - 全エントリ統一縮約ロジック（KISS原則）
    - `estimate_entry_meta_tokens` でメタデータトークン算出
    - 残予算内で `truncate_snippet_for_char_budget` による動的snippet縮約
    - 空snippet→None正規化
    - break→continue変更
    - `Ok(...).map(...)` パターン廃止、included直接計算
    - estimated_tokensのtoken_total再利用
  - 依存: Task 1.1, 1.2, 1.3, 1.4

### Phase 2: CLIヘルプ・バリデーション更新

- [ ] **Task 2.1**: `--max-tokens` ヘルプテキスト・バリデーション更新
  - 成果物: `src/main.rs`
  - 内容: ヘルプを `Estimated token limit (approx. 1 token per 4 chars)` に変更、`value_parser` で `1..=1_000_000` 制約追加
  - 依存: なし

- [ ] **Task 2.2**: `--max-files` バリデーション追加
  - 成果物: `src/main.rs`
  - 内容: `value_parser` で `1..=1000` 制約追加
  - 依存: なし

- [ ] **Task 2.3**: `CONTEXT_AFTER_HELP` 更新
  - 成果物: `src/cli/context.rs`
  - 内容: 推定方式の説明を追加
  - 依存: なし

- [ ] **Task 2.4**: `help_llm.rs` 更新（必要に応じて）
  - 成果物: `src/cli/help_llm.rs`
  - 内容: `--max-tokens` の key_options 説明・例文の更新
  - 依存: なし

### Phase 3: テスト

- [ ] **Task 3.1**: 単体テスト追加
  - 成果物: `src/cli/context.rs`（`#[cfg(test)]` モジュール）
  - 内容:
    - `estimate_tokens`: ASCII/日本語/空/混合
    - `estimate_entry_meta_tokens`: 全フィールドあり/なし
    - `estimate_entry_tokens`: meta + snippet合算
    - `truncate_snippet_for_char_budget`: 正常系/境界値/空
    - `tokens_to_char_budget`: 基本変換
  - 依存: Phase 1完了

- [ ] **Task 3.2**: E2Eテスト追加・改修
  - 成果物: `tests/e2e_context_pack.rs`
  - 内容:
    - 十分に長いfixtureでincluded減少/snippet縮約が発生するテスト
    - 最初のエントリ例外（メタデータ超過時）テスト
    - 既存 `context_pack_max_tokens_limits_output` の改修
    - value_parser範囲制約テスト
  - 依存: Phase 1, Phase 2完了

### Phase 4: 品質チェック・最終検証

- [ ] **Task 4.1**: 品質チェック実行
  - `cargo build`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test --all`
  - `cargo fmt --all -- --check`
  - 依存: Phase 1-3完了

---

## TDD実装順序

設計方針書に基づき、以下の順序でTDD実装を行う:

```
1. estimate_tokens テスト作成 → 実装改修
2. tokens_to_char_budget テスト作成 → 実装
3. estimate_entry_meta_tokens テスト作成 → 実装
4. estimate_entry_tokens テスト作成 → 実装
5. truncate_snippet_for_char_budget テスト作成 → 実装
6. build_context_pack 統合テスト作成 → 実装改修
7. CLIヘルプ・バリデーション更新
8. E2Eテスト追加・改修
9. 品質チェック
```

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

- [ ] すべてのタスク（Phase 1-4）が完了
- [ ] `cargo test --all` 全パス
- [ ] `cargo clippy --all-targets -- -D warnings` 警告0件
- [ ] `cargo fmt --all -- --check` 差分なし
- [ ] Issue #105 の受け入れ基準すべて達成
