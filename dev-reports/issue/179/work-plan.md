# 作業計画 - Issue #179

## Issue: セマンティック検索結果にスニペット（本文抜粋）を追加
**Issue番号**: #179
**サイズ**: M（中規模）
**優先度**: High
**ブランチ**: `fix/issue-179-semantic-snippet`（作成済み）

---

## 詳細タスク分解

### Phase 1: テスト作成（TDD - Red）

- [ ] **Task 1.1**: format_semantic_human() + SnippetConfig テスト
  - 成果物: `tests/output_format.rs` に追加
  - 検証: SnippetConfig { lines: 3, chars: 80 } で正しくトランケーションされること
  - 依存: なし

- [ ] **Task 1.2**: format_semantic_human() + lines=0/chars=0 テスト
  - 成果物: `tests/output_format.rs` に追加
  - 検証: 0指定で全文表示されること
  - 依存: なし

- [ ] **Task 1.3**: format_semantic_llm() + LlmFormatOptions テスト
  - 成果物: `tests/output_format.rs` に追加
  - 検証: max_body_lines: Some(3) で正しくトランケーションされること
  - 依存: なし

- [ ] **Task 1.4**: format_semantic_llm() + bodyが正しく出力されるテスト
  - 成果物: `tests/output_format.rs` に追加
  - 検証: bodyが空でないSemanticSearchResultでestimated tokensが0でないこと
  - 依存: なし

### Phase 2: 出力フォーマッタ実装（TDD - Green）

- [ ] **Task 2.1**: format_semantic_results() シグネチャ変更
  - 成果物: `src/output/mod.rs`
  - 変更: SnippetConfig, &LlmFormatOptions パラメータ追加
  - 依存: なし

- [ ] **Task 2.2**: format_semantic_human() にSnippetConfig追加
  - 成果物: `src/output/human.rs`
  - 変更: SnippetConfig引数追加、ハードコード(2, 120)をconfig参照に変更、lines=0/chars=0ガード追加
  - 依存: Task 2.1

- [ ] **Task 2.3**: format_semantic_llm() にLlmFormatOptions追加
  - 成果物: `src/output/llm.rs`
  - 変更: LlmFormatOptions引数追加、truncate_body_for_llm()適用、was_truncated分岐
  - 依存: Task 2.1

### Phase 3: 検索ロジック修正

- [ ] **Task 3.1**: run_semantic_search() シグネチャ変更
  - 成果物: `src/cli/search.rs`
  - 変更: snippet_config: SnippetConfig, llm_options: &LlmFormatOptions パラメータ追加、format_semantic_results()に伝播
  - 依存: Task 2.1

- [ ] **Task 3.2**: enrich_with_metadata() fallback改善
  - 成果物: `src/cli/search.rs`
  - 変更: heading不一致時にsections.first()のbodyを使用
  - 依存: なし

### Phase 4: CLI統合

- [ ] **Task 4.1**: main.rs セマンティック検索呼び出し更新
  - 成果物: `src/main.rs`
  - 変更: セマンティック分岐内でLlmFormatOptions構築、run_semantic_search()にsnippet_config/llm_optionsを渡す
  - 依存: Task 3.1

### Phase 5: 既存テスト修正・追加テスト

- [ ] **Task 5.1**: 既存テスト（test_format_semantic_llm）のシグネチャ追従
  - 成果物: `tests/output_format.rs`
  - 変更: format_semantic_results()呼び出しにSnippetConfig::default(), &LlmFormatOptions::default() を追加
  - 依存: Task 2.1

- [ ] **Task 5.2**: enrich_with_metadata fallbackテスト追加
  - 成果物: `src/cli/search.rs` のテストモジュールまたは `tests/` 配下
  - 検証: heading不一致時にsections.first()のbodyが使用されること
  - 依存: Task 3.2

### Phase 6: 品質チェック

- [ ] **Task 6.1**: cargo build / clippy / test / fmt 全パス確認
  - 依存: 全タスク完了後

---

## タスク依存関係

```
Task 1.1-1.4 (テスト作成)
    ↓
Task 2.1 (format_semantic_results シグネチャ)
    ├→ Task 2.2 (format_semantic_human)
    ├→ Task 2.3 (format_semantic_llm)
    └→ Task 3.1 (run_semantic_search シグネチャ)
         └→ Task 4.1 (main.rs 統合)
Task 3.2 (enrich fallback) ← 独立
Task 5.1, 5.2 (テスト修正) ← 実装完了後
Task 6.1 (品質チェック) ← 全タスク完了後
```

---

## 実装順序（推奨）

TDDアプローチ: テスト先行で進めるが、シグネチャ変更が先に必要なため以下の順序:

1. **Task 2.1** → format_semantic_results() シグネチャ変更（コンパイル通すため）
2. **Task 2.2** → format_semantic_human() + SnippetConfig
3. **Task 2.3** → format_semantic_llm() + LlmFormatOptions
4. **Task 3.1** → run_semantic_search() シグネチャ変更
5. **Task 3.2** → enrich_with_metadata() fallback改善
6. **Task 4.1** → main.rs 統合
7. **Task 5.1** → 既存テスト修正
8. **Task 1.1-1.4, 5.2** → テスト追加
9. **Task 6.1** → 品質チェック

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

- [ ] すべてのタスクが完了
- [ ] `cargo test --all` 全パス
- [ ] `cargo clippy --all-targets -- -D warnings` 警告0件
- [ ] `cargo fmt --all -- --check` 差分なし
- [ ] 受け入れ基準5項目をすべて満たす
