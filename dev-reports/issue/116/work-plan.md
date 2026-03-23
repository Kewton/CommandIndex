# 作業計画: Issue #116 - search/related/impact のLLMプロンプト最適化

## Issue概要
**Issue番号**: #116
**タイトル**: search/related/impact のJSON出力のLLMプロンプト最適化
**サイズ**: M
**優先度**: High
**依存Issue**: #104 (実装済み)

## タスク分解

### Phase 1: 型定義・構造体 (src/output/mod.rs)

- [ ] **Task 1.1**: `LlmFormatOptions` 構造体の定義
  - 成果物: `src/output/mod.rs`
  - 内容: `LlmFormatOptions { max_body_lines: Option<usize> }` + Default実装
  - 依存: なし

- [ ] **Task 1.2**: `format_results` のシグネチャ変更
  - 成果物: `src/output/mod.rs` (L209-220)
  - 内容: `llm_options: &LlmFormatOptions` 引数追加、Llm分岐で `format_llm` に渡す
  - 依存: Task 1.1

- [ ] **Task 1.3**: `format_impact_results` のシグネチャ変更
  - 成果物: `src/output/mod.rs` (L282-293)
  - 内容: `llm_options: &LlmFormatOptions` 引数追加、Llm分岐で `format_impact_llm` に渡す
  - 依存: Task 1.1

### Phase 2: LLMフォーマッタ最適化 (src/output/llm.rs)

- [ ] **Task 2.1**: `format_llm` に `LlmFormatOptions` 引数追加 + 重複除去
  - 成果物: `src/output/llm.rs` (L103-133)
  - 内容:
    - 引数に `llm_options: &LlmFormatOptions` 追加
    - `dedup_results()` 関数追加（path+heading+body完全一致で除外、最初の出現を採用）
    - `group_by_path` 後に各グループで `dedup_results` を適用
  - 依存: Task 1.2

- [ ] **Task 2.2**: bodyトランケーション実装
  - 成果物: `src/output/llm.rs`
  - 内容:
    - `truncate_body_for_llm(body, max_lines) -> (String, bool)` 関数追加
    - `max_lines == 0` は無制限（トランケーションしない）
    - `write_body` 呼び出し前にトランケーション適用
    - トランケーション時に `... (truncated)` マーカー出力
    - コードファイルの場合にコードフェンスの閉じ処理
  - 依存: Task 2.1

- [ ] **Task 2.3**: `format_impact_llm` に impacted_by 省略実装
  - 成果物: `src/output/llm.rs` (L344-377)
  - 内容:
    - 引数に `llm_options: &LlmFormatOptions` 追加
    - `IMPACTED_BY_DISPLAY_LIMIT = 3` 定数追加
    - `format_impacted_by()` ヘルパー関数追加（3件超で `... (+N more)` 表記）
  - 依存: Task 1.3

### Phase 3: CLI層の受け渡し

- [ ] **Task 3.1**: `src/cli/search.rs` の更新
  - 成果物: `src/cli/search.rs` (L209-217, L279)
  - 内容: `run` 関数に `llm_options: &LlmFormatOptions` 引数追加、`format_results` 呼び出しに渡す
  - 依存: Task 1.2

- [ ] **Task 3.2**: `src/cli/impact.rs` の更新
  - 成果物: `src/cli/impact.rs` (L112-118, L170)
  - 内容: `run_impact` 関数に `llm_options: &LlmFormatOptions` 引数追加、`format_impact_results` 呼び出しに渡す
  - 依存: Task 1.3

- [ ] **Task 3.3**: `src/main.rs` の更新
  - 成果物: `src/main.rs`
  - 内容:
    - search サブコマンド: `LlmFormatOptions { max_body_lines: snippet_lines.map(|v| usize::try_from(v).unwrap_or(usize::MAX)) }` 構築、`run()` に渡す
    - impact サブコマンド: `LlmFormatOptions::default()` を構築、`run_impact()` に渡す
  - 依存: Task 3.1, Task 3.2

### Phase 4: テスト

- [ ] **Task 4.1**: 既存テストのコンパイル修正
  - 成果物: `tests/output_format.rs` (format_to_string L21-27, format_impact_to_string L342-347 等)
  - 内容: ヘルパー関数に `&LlmFormatOptions::default()` 引数追加
  - 依存: Task 1.2, Task 1.3

- [ ] **Task 4.2**: トランケーション新規テスト
  - 成果物: `tests/output_format.rs`
  - テストケース:
    - `test_format_llm_truncation` — max_body_lines指定時のbody切り詰め
    - `test_format_llm_truncation_marker` — `... (truncated)` マーカー確認
    - `test_format_llm_truncation_code_fence_close` — コードファイルのフェンス閉じ
    - `test_format_llm_no_truncation_default` — デフォルト(None)で現行動作維持
    - `test_format_llm_truncation_zero_means_unlimited` — Some(0)で無制限
  - 依存: Task 2.2

- [ ] **Task 4.3**: 重複除去新規テスト
  - 成果物: `tests/output_format.rs`
  - テストケース:
    - `test_format_llm_dedup` — 同一エントリの重複除去
    - `test_format_llm_no_dedup_different_body` — bodyが異なる場合は保持
  - 依存: Task 2.1

- [ ] **Task 4.4**: impacted_by省略新規テスト
  - 成果物: `tests/output_format.rs`
  - テストケース:
    - `test_format_impact_llm_impacted_by_truncation` — 4件以上の省略表記
    - `test_format_impact_llm_impacted_by_no_truncation` — 3件以下は全表示
  - 依存: Task 2.3

- [ ] **Task 4.5**: E2Eテスト追加
  - 成果物: `tests/e2e_impact.rs`
  - テストケース:
    - `test_e2e_impact_llm_format` — impact --format llm のE2E
  - 依存: Task 3.3

### Phase 5: 品質チェック

- [ ] **Task 5.1**: 品質チェック実行
  - コマンド: `cargo build && cargo clippy --all-targets -- -D warnings && cargo test --all && cargo fmt --all -- --check`
  - 依存: 全タスク

## 実行順序

```
Task 1.1 (LlmFormatOptions定義)
  ├─→ Task 1.2 (format_results シグネチャ)
  │     ├─→ Task 2.1 (format_llm 重複除去)
  │     │     └─→ Task 2.2 (トランケーション)
  │     └─→ Task 3.1 (cli/search.rs)
  └─→ Task 1.3 (format_impact_results シグネチャ)
        ├─→ Task 2.3 (impacted_by 省略)
        └─→ Task 3.2 (cli/impact.rs)
              └─→ Task 3.3 (main.rs)
                    └─→ Task 4.1 (既存テスト修正)
                          ├─→ Task 4.2-4.5 (新規テスト)
                          └─→ Task 5.1 (品質チェック)
```

## Definition of Done

- [ ] すべてのタスクが完了
- [ ] `cargo test --all` 全パス
- [ ] `cargo clippy --all-targets -- -D warnings` 警告ゼロ
- [ ] `cargo fmt --all -- --check` 差分なし
- [ ] デフォルトオプション（LlmFormatOptions::default()）で現行と同一出力
- [ ] 新規テスト10ケース以上追加
