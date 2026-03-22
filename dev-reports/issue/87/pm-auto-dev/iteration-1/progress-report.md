# 進捗レポート: Issue #87 --related の複数ファイル対応

## ステータス: 完了

## 実装サマリー

| 項目 | 内容 |
|------|------|
| Issue | #87 [Feature] --related の複数ファイル対応 |
| ブランチ | feature/issue-87-related-multi |
| イテレーション | 1 |
| 変更ファイル数 | 6 |
| 追加行数 | 283 |
| 削除行数 | 59 |
| 新規テスト数 | 11 (CLI: 5, E2E: 6) |

## 変更ファイル

| ファイル | 変更内容 |
|---------|---------|
| `src/cli/mod.rs` | `validate_file_paths` 共通バリデーション関数追加、`sanitize_path_for_display` ヘルパー追加 |
| `src/cli/context.rs` | インラインバリデーション → `validate_file_paths` 呼び出しに統一、`collect_related_context`/`merge_related_results` を `pub(crate)` に変更 |
| `src/cli/search.rs` | `run_related_search` シグネチャ変更（`&str` → `&[String]`）、マージロジック再利用 |
| `src/main.rs` | clap定義変更（`Option<String>` → `Option<Vec<String>>`、`num_args(1..)` 追加） |
| `tests/cli_args.rs` | 複数ファイルパーステスト 5件追加 |
| `tests/e2e_related_search.rs` | 複数ファイルE2Eテスト 6件追加 |

## 品質チェック結果

| チェック | 結果 |
|---------|------|
| `cargo build` | pass |
| `cargo clippy --all-targets -- -D warnings` | pass (警告0件) |
| `cargo test --all` | pass (全テスト合格、0 failed) |
| `cargo fmt --all -- --check` | pass (差分なし) |

## 受入基準充足状況

| # | 基準 | 状態 |
|---|------|------|
| 1 | `--related` に複数ファイル指定可能 | pass |
| 2 | スコア最大値マージ | pass |
| 3 | 重複ファイル統合 | pass |
| 4 | human/json/path 出力対応 | pass |
| 5 | 後方互換（単一ファイル） | pass |
| 6 | graceful skip（存在しないファイル） | pass |
| 7 | cargo test/clippy/fmt 全パス | pass |

## Codexコードレビュー結果

- **Critical**: 0件
- **Warnings**: 3件
  - 内部limit 1000件 → context.rsと一貫性維持（変更なし）
  - バリデーション後方互換 → context.rsと一貫性維持（変更なし）
  - stderr制御文字サニタイズ → `sanitize_path_for_display` で対応済み

## 設計判断

1. context.rs の既存マージロジック（`collect_related_context`/`merge_related_results`）を `pub(crate)` 化して再利用（DRY）
2. 入力検証を共通関数 `validate_file_paths` に抽出（context.rs と search.rs で統一）
3. `num_args(1..)` によるスペース区切りの自然な複数ファイル指定
