# 進捗レポート - Issue #92: diff サブコマンド

## 完了日: 2026-03-23

## ステータス: 完了

## 成果物

### 新規作成ファイル
| ファイル | 説明 |
|---------|------|
| `src/cli/diff.rs` | diff サブコマンドのメインロジック（86行） |
| `tests/e2e_diff.rs` | E2Eテスト 5件 |

### 修正ファイル
| ファイル | 変更内容 |
|---------|---------|
| `src/main.rs` | Commands::Diff バリアント追加、マッチアーム追加 |
| `src/cli/mod.rs` | `pub mod diff;` 宣言追加 |
| `src/output/mod.rs` | `DiffResult` 型、`format_diff_results()` 追加 |
| `src/output/human.rs` | `format_diff_human()` 追加（strip_control_chars適用） |
| `src/output/json.rs` | `format_diff_json()` 追加（単一JSON、overlap_count付き） |
| `src/output/path.rs` | `format_diff_path()` 追加（overlapのみ出力） |
| `tests/cli_args.rs` | help テストに diff 検証追加 |
| `README.md` | diff コマンド使用例追加 |

### 変更統計
- 追加: 約120行
- ファイル数: 新規2、修正8

## 品質チェック結果

| チェック | 結果 |
|---------|------|
| cargo build | PASS |
| cargo clippy --all-targets -- -D warnings | PASS (警告0件) |
| cargo test --all | PASS (全33スイート、0 failed) |
| cargo fmt --all -- --check | PASS (差分なし) |

## E2Eテスト結果

| テスト | 結果 |
|--------|------|
| diff_json_format_correct | PASS |
| diff_human_format_correct | PASS |
| diff_path_format_outputs_overlap_only | PASS |
| diff_same_file_error | PASS |
| diff_no_index_error | PASS |

## Codexコードレビュー結果

| 重要度 | 件数 | 対応 |
|--------|------|------|
| Critical | 0 | - |
| Warning | 2 | 修正済み |

### 修正した warnings:
1. `run_diff()` にスライス長チェック追加（panic防止）
2. `format_diff_human()` に `strip_control_chars()` 適用（制御文字サニタイズ）

## 受け入れ基準達成状況

- [x] 2ファイルの影響範囲を比較できる
- [x] only_a, only_b, overlap が正しく分類される
- [x] human / json / path 出力形式に対応
- [x] エラーケースが適切にハンドリングされる
- [x] cargo test / clippy / fmt 全パス
