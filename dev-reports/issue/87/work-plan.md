# 作業計画書: Issue #87 --related の複数ファイル対応

## Issue: [Feature] --related の複数ファイル対応
**Issue番号**: #87
**サイズ**: S
**優先度**: Medium
**依存Issue**: なし
**ブランチ**: `feature/issue-87-related-multi`（作成済み）

---

## 詳細タスク分解

### Phase 1: 入力検証の共通化

- [ ] **Task 1.1**: `validate_file_paths` 関数を `cli/mod.rs` に追加
  - 成果物: `src/cli/mod.rs`
  - 内容:
    - 空スライスチェック
    - ファイル数上限チェック（max 100）
    - 各パス: 空文字、長さ（max 1024）、`..` 禁止、絶対パス禁止、`\` 禁止
  - context.rs L20-56 のバリデーションロジックを参考に抽出
  - 依存: なし

- [ ] **Task 1.2**: `context.rs` の `run_context` を `validate_file_paths` 呼び出しに置き換え
  - 成果物: `src/cli/context.rs`
  - 内容: L19-56 のインラインバリデーションを `validate_file_paths()` 呼び出しに統一
  - 依存: Task 1.1
  - **注意**: 既存テストが全パスすることを確認

### Phase 2: コア実装

- [ ] **Task 2.1**: `context.rs` の可視性変更
  - 成果物: `src/cli/context.rs`
  - 内容:
    - `collect_related_context` → `pub(crate)`、docコメント追加（validate_file_paths必須）
    - `merge_related_results` → `pub(crate)`、docコメント追加
  - 依存: Task 1.2

- [ ] **Task 2.2**: `main.rs` の clap 定義変更
  - 成果物: `src/main.rs`
  - 内容:
    - `related: Option<String>` → `Option<Vec<String>>`
    - `#[arg(long, num_args(1..), conflicts_with_all = [...])]` 追加
    - パターンマッチ: `Some(f)` → `Some(ref files)` に変更
  - 依存: Task 2.3

- [ ] **Task 2.3**: `run_related_search` シグネチャ・ロジック変更
  - 成果物: `src/cli/search.rs`
  - 内容:
    - シグネチャ: `file_path: &str` → `file_paths: &[String]`
    - 入力検証: `validate_file_paths(file_paths, 100)` 呼び出し
    - コアロジック: `collect_related_context(file_paths, &reader, &store)` 呼び出し
    - limit切り詰め: マージ結果を `effective_limit` で truncate
    - 空結果時メッセージ: 対象ファイル一覧表示
  - 依存: Task 1.1, Task 2.1

### Phase 3: テスト

- [ ] **Task 3.1**: CLIパーステスト追加（cli_args.rs）
  - 成果物: `tests/cli_args.rs`
  - テストケース:
    - `--related file1.rs file2.rs` → Vec パース確認
    - `--related file.rs` → 後方互換（単一値）
    - `--related file1.rs file2.rs --format json` → パース境界
    - `--related file1.rs file2.rs --limit 5` → 正常パース
    - `--related a.rs b.rs --symbol foo` → 排他制約
  - 依存: Task 2.2

- [ ] **Task 3.2**: E2Eテスト追加（e2e_related_search.rs）
  - 成果物: `tests/e2e_related_search.rs`
  - テストケース:
    - `related_search_multiple_files_merged` — 複数ファイルでunionマージ
    - `related_search_multiple_files_max_score` — スコア最大値統合
    - `related_search_single_file_backward_compat` — 単一ファイル後方互換
    - `related_search_partial_missing_graceful` — 一部ファイル不在でgraceful skip
    - `related_search_all_missing` — 全ファイル不在時のメッセージ
    - `related_search_multiple_files_json_format` — JSON出力確認
    - `related_search_multiple_files_path_format` — path出力確認
  - 依存: Task 2.2, Task 2.3

- [ ] **Task 3.3**: 既存テスト全パス確認
  - コマンド: `cargo test --all`
  - 依存: Task 3.1, Task 3.2

### Phase 4: 品質チェック

- [ ] **Task 4.1**: 品質チェック全項目パス
  - `cargo build` — エラー0件
  - `cargo clippy --all-targets -- -D warnings` — 警告0件
  - `cargo test --all` — 全テストパス
  - `cargo fmt --all -- --check` — 差分なし
  - 依存: Task 3.3

---

## 実装順序（TDD）

```
Task 1.1 (validate_file_paths)
  ↓ テスト先行: バリデーション関数のユニットテスト
Task 1.2 (context.rs バリデーション統一)
  ↓ 既存テスト全パス確認
Task 2.1 (context.rs 可視性変更)
  ↓ 既存テスト全パス確認
Task 3.1 (CLIパーステスト追加) ← テスト先行
Task 2.2 (main.rs clap定義変更)
  ↓ Task 3.1 のテストがパス
Task 3.2 (E2Eテスト追加) ← テスト先行
Task 2.3 (run_related_search ロジック変更)
  ↓ Task 3.2 のテストがパス
Task 3.3 + Task 4.1 (全テスト + 品質チェック)
```

**重要**: Task 1.1 + 1.2 + 2.1 + 2.2 + 2.3 は同一コミットで行い、バリデーション未適用の中間状態を防ぐ。

---

## Definition of Done

- [ ] すべてのタスクが完了
- [ ] `cargo test --all` 全テストパス
- [ ] `cargo clippy --all-targets -- -D warnings` 警告ゼロ
- [ ] `cargo fmt --all -- --check` 差分なし
- [ ] 既存テスト（15件のE2E + 5件のCLI引数）が全パス（後方互換）
- [ ] 新規テスト（7件のE2E + 5件のCLI引数）が全パス

---

## 次のアクション

1. `/pm-auto-dev 87` でTDD自動開発を実行
2. `/create-pr` でPR作成
