# 作業計画: Issue #125 - rerankフォールバック通知改善

## Issue概要
**Issue番号**: #125
**タイトル**: [BUG] --rerank がモデル未検出時にサイレントフォールバックし結果が変わらない
**サイズ**: M
**優先度**: Medium
**依存Issue**: なし
**ブランチ**: `fix/issue-125-rerank-fallback`（作成済み）

## 詳細タスク分解

### Phase 1: データモデル・型定義

#### Task 1.1: RerankError::PartialTimeout バリアント追加
- **対象**: `src/rerank/mod.rs`
- **内容**:
  - `RerankError` enum に `PartialTimeout { results: Vec<RerankResult>, scored: usize, total: usize }` バリアントを追加
  - `Display` 実装に `PartialTimeout` の表示を追加（"Timeout reached after scoring N of M candidates"）
  - 既存の `Display` 実装はエラー事実のみ維持（ヒントは含めない）
- **依存**: なし
- **テスト**: `test_rerank_error_partial_timeout_display`

#### Task 1.2: RerankStatus enum 定義
- **対象**: `src/cli/search.rs`（private enum）
- **内容**:
  ```rust
  #[derive(Debug, Clone, PartialEq)]
  enum RerankStatus {
      Applied,
      AppliedPartially { warning: String },
      Skipped { reason: String },
  }
  ```
- **依存**: なし
- **テスト**: 単体テストは不要（シンプルなデータ型）

### Phase 2: Provider層の変更

#### Task 2.1: ollama.rs のタイムアウト処理変更
- **対象**: `src/rerank/ollama.rs`
- **内容**:
  - L73-79 のデッドライン超過処理を変更
  - `eprintln!` を削除
  - `break` → `return Err(RerankError::PartialTimeout { results, scored: i, total: documents.len() })`
  - `results.sort_by()` を `PartialTimeout` 返却前に実行
- **依存**: Task 1.1
- **テスト**: 既存テストの動作確認

### Phase 3: CLI層の変更

#### Task 3.1: rerank_error_hint() ヘルパー関数追加
- **対象**: `src/cli/search.rs`
- **内容**:
  ```rust
  fn rerank_error_hint(err: &RerankError) -> &'static str {
      match err {
          RerankError::ModelNotFound(_) => "Run `ollama pull <model>` to install, or set rerank.model in config.",
          RerankError::NetworkError(_) => "Is Ollama running? Try `ollama serve`.",
          RerankError::Timeout => "Check Ollama server load.",
          RerankError::ApiError { .. } => "Check Ollama logs.",
          RerankError::InvalidResponse(_) => "Check model compatibility.",
          RerankError::ConfigError(_) => "Check rerank settings in commandindex.toml.",
          RerankError::PartialTimeout { .. } => "Some candidates were not scored due to timeout.",
      }
  }
  ```
- **依存**: Task 1.1
- **テスト**: `test_rerank_error_hint_all_variants`

#### Task 3.2: try_rerank() シグネチャ変更
- **対象**: `src/cli/search.rs`
- **内容**:
  - 戻り値を `Vec<SearchResult>` → `(Vec<SearchResult>, RerankStatus)` に変更
  - Provider 生成失敗時: `(results, RerankStatus::Skipped { reason })` を返す
  - `eprintln!` を全て削除
  - `PartialTimeout` のハンドリング:
    - 空結果 → `Skipped` にフォールバック
    - 部分結果 → `AppliedPartially` で返す
  - 通常の `Err` → `Skipped` で返す
  - 成功 → `Applied` で返す
- **依存**: Task 1.1, 1.2, 2.1
- **テスト**: E2Eテストで検証

#### Task 3.3: build_rerank_stdout_prefix() / build_rerank_stderr_message() 追加
- **対象**: `src/cli/search.rs`
- **内容**:
  - `build_rerank_stdout_prefix()`: JSON メタデータ行、LLM コメントを生成
  - `build_rerank_stderr_message()`: human/path 向け stderr 警告を生成
  - JSON metadata スキーマ: `{"type":"metadata","rerank_status":"skipped"|"partial","rerank_warnings":["reason"]}`
  - reason のサニタイズ: ApiError はステータスコードのみ。制御文字除去
- **依存**: Task 1.2
- **テスト**: `test_build_rerank_stdout_prefix_json`, `test_build_rerank_stdout_prefix_llm`, `test_build_rerank_stderr_message`

#### Task 3.4: run() 関数の出力制御統合
- **対象**: `src/cli/search.rs`
- **内容**:
  - `try_rerank()` 呼び出しを `(reranked, status)` で受け取り
  - `format_results()` 呼び出し前に `build_rerank_stdout_prefix()` で prefix 出力
  - `format_results()` 呼び出し後に `build_rerank_stderr_message()` で stderr 出力
- **依存**: Task 3.2, 3.3
- **テスト**: E2Eテストで検証

### Phase 4: テスト

#### Task 4.1: 単体テスト追加
- **対象**: `src/rerank/mod.rs` (#[cfg(test)])、`src/cli/search.rs` (#[cfg(test)])
- **内容**:
  - `test_rerank_error_partial_timeout_display`: PartialTimeout の Display
  - `test_rerank_error_hint_all_variants`: 全バリアントのヒント存在確認
  - `test_build_rerank_stdout_prefix_json`: JSON Skipped/Partial メタデータ行
  - `test_build_rerank_stdout_prefix_llm`: LLM Skipped/Partial コメント
  - `test_build_rerank_stderr_message`: stderr 警告メッセージ
- **依存**: Task 3.3

#### Task 4.2: テストユーティリティ更新
- **対象**: `tests/common/mod.rs`
- **内容**:
  - `parse_search_jsonl()`: metadata 行を除外して検索結果のみ返す
  - `parse_jsonl_metadata()`: metadata 行を取得
- **依存**: なし

#### Task 4.3: E2Eテスト拡充
- **対象**: `tests/e2e_semantic_hybrid.rs`
- **前提**: Ollama 未起動状態でテスト実行
- **内容**:
  - `test_rerank_fallback_stderr_message`: stderr に `[rerank] Reranking skipped:` とモデル名
  - `test_rerank_fallback_json_metadata`: JSON 先頭行に `"type":"metadata"`, `"rerank_status":"skipped"`
  - `test_rerank_fallback_llm_comment`: llm 出力に `<!-- rerank skipped:` コメント
  - `test_rerank_fallback_exit_code_zero`: exitコード 0 維持
  - 既存の `test_rerank_fallback_via_cli` の更新（必要に応じて）
- **依存**: Task 3.4, 4.2

### Phase 5: 品質チェック・最終確認

#### Task 5.1: 品質チェック実行
- **コマンド**:
  ```bash
  cargo build
  cargo clippy --all-targets -- -D warnings
  cargo test --all
  cargo fmt --all -- --check
  ```
- **依存**: 全タスク完了後

## タスク依存関係

```
Task 1.1 (PartialTimeout) ──┬── Task 2.1 (ollama.rs)
                             ├── Task 3.1 (hint関数)
                             └── Task 3.2 (try_rerank) ──── Task 3.4 (run統合)
Task 1.2 (RerankStatus)  ──┬── Task 3.2 (try_rerank)
                            └── Task 3.3 (出力ヘルパー) ── Task 3.4 (run統合)
Task 4.2 (テストutil) ────── Task 4.3 (E2Eテスト)
Task 3.4 (run統合) ────────── Task 4.3 (E2Eテスト)
全タスク ──────────────────── Task 5.1 (品質チェック)
```

## 実装順序（推奨）

1. Task 1.1 + Task 1.2（並列可）
2. Task 2.1 + Task 3.1（並列可）
3. Task 3.2
4. Task 3.3
5. Task 3.4
6. Task 4.1 + Task 4.2（並列可）
7. Task 4.3
8. Task 5.1

## Definition of Done

- [ ] `RerankError::PartialTimeout` バリアント追加済み
- [ ] `RerankStatus` enum 定義済み
- [ ] `ollama.rs` の `eprintln!` 削除・`PartialTimeout` 返却
- [ ] `try_rerank()` が `(Vec<SearchResult>, RerankStatus)` を返す
- [ ] `try_rerank()` 内の全 `eprintln!` 削除
- [ ] `build_rerank_stdout_prefix()` / `build_rerank_stderr_message()` 実装
- [ ] `run()` 関数の出力制御統合
- [ ] 全フォーマットでフォールバック情報が出力される
- [ ] `cargo clippy --all-targets -- -D warnings` 警告 0 件
- [ ] `cargo test --all` 全テストパス
- [ ] `cargo fmt --all -- --check` 差分なし
