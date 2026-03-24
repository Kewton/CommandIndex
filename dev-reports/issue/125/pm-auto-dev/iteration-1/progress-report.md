# 進捗レポート: Issue #125 - rerankフォールバック通知改善

## ステータス: 実装完了

## 変更ファイル一覧

| ファイル | 変更内容 |
|---|---|
| `src/rerank/mod.rs` | `RerankError::PartialTimeout` バリアント追加、`RerankResult` に `#[derive(Debug)]` 追加 |
| `src/rerank/ollama.rs` | タイムアウト時 `eprintln!` 削除→ `PartialTimeout` エラー返却 |
| `src/cli/search.rs` | `RerankStatus` enum、ヘルパー関数群、`try_rerank()` シグネチャ変更、`run()` 出力制御統合 |
| `tests/common/mod.rs` | `parse_search_jsonl()`, `parse_jsonl_metadata()` 追加 |
| `tests/e2e_semantic_hybrid.rs` | E2Eテスト3件追加 |

## 品質チェック結果

| チェック | 結果 |
|---|---|
| `cargo build` | 成功 |
| `cargo clippy --all-targets -- -D warnings` | 警告 0 件 |
| `cargo test --all` | 12 passed, 1 failed (既存), 2 ignored |
| `cargo fmt --all -- --check` | 差分なし |

## Codexコードレビュー結果

| 深刻度 | 件数 | 対応 |
|---|---|---|
| Critical | 1件 | 修正済み（`-->` エスケープ追加） |
| Warning | 2件 | 修正済み（unwrap削除、stderrサニタイズ） |

## テスト追加

- 単体テスト: 13件（search.rs 12件 + mod.rs 1件）
- E2Eテスト: 3件（stderr, json metadata, llm comment）
- テストユーティリティ: 2関数追加
