# v0.0.0 Work Plan — 開発環境整備

## Step 1: Rust プロジェクト初期化

- [x] `Cargo.toml` 作成（name: commandindex, edition: 2024）
- [x] 初期依存クレート追加（clap, serde, serde_json, tracing, tracing-subscriber, tracing-appender）
- [x] `src/main.rs` 作成（clap サブコマンド定義: index, search, update, status, clean）
- [x] `src/lib.rs` 作成（モジュール宣言の雛形）
- [x] `.gitignore` 作成（target/, .commandindex/, .DS_Store 等）
- [x] `cargo build` がゼロエラーで通ることを確認
- [x] `cargo run -- --help` でサブコマンド一覧が表示されることを確認
- [x] 各サブコマンド実行時に「未実装」メッセージが表示されることを確認

## Step 2: 品質基盤

- [x] `cargo fmt --all -- --check` が差分なしで通ることを確認
- [x] `cargo clippy --all-targets -- -D warnings` がゼロ警告で通ることを確認
- [x] `tests/common/mod.rs` 作成（テスト共有ユーティリティ）
- [x] `tests/cli_args.rs` 作成（サブコマンドパーステスト）
- [x] `Cargo.toml` に dev-dependencies 追加（tempfile, assert_cmd, predicates）
- [x] `cargo test --all` が全テスト PASS することを確認

## Step 3: ドキュメント

- [x] `README.md` 作成（概要、ビルド手順、品質チェックコマンド一覧）
- [x] `CLAUDE.md` 作成（技術スタック、ブランチ戦略、コミット規約、コーディング規約、モジュール構成、品質チェック）
- [x] `COMMANDINDEX.md` 作成（TDD ポリシー、テスト方針、設計原則）
- [x] `CHANGELOG.md` 作成（v0.0.0 初期エントリ）
- [x] `LICENSE` 作成（MIT）

## Step 4: 初回コミット・push

- [x] 全ファイルをステージし初回コミット
- [x] `main` ブランチに push
- [x] GitHub 上でリポジトリの内容が確認できることを確認

## Step 5: CI/CD パイプライン

- [x] `.github/workflows/ci.yml` 作成（fmt, clippy, test, build の 4 ジョブ）
- [x] `.github/workflows/release.yml` 作成（4 ターゲットクロスビルド、GitHub Release 自動作成）
- [x] `develop` ブランチ作成・push
- [x] CI テスト用の feature ブランチで PR を作成（PR #1）
- [x] CI の 4 ジョブ（fmt, clippy, test, build）が全て PASS することを確認
- [x] PR 上で CI 結果が表示されることを確認

## Step 6: GitHub テンプレート

- [x] `.github/PULL_REQUEST_TEMPLATE.md` 作成（Summary, Changes, Type/Test チェックリスト）
- [x] `.github/ISSUE_TEMPLATE/bug_report.md` 作成（再現手順、期待/実際の動作、環境情報）
- [x] `.github/ISSUE_TEMPLATE/feature_request.md` 作成（概要、動機、提案、受け入れ条件）
- [x] GitHub 上で PR 作成時にテンプレートが表示されることを確認（PR #1, #2 で確認）
- [x] GitHub 上で Issue 作成時にテンプレート選択肢が表示されることを確認

## Step 7: Claude Code 統合

- [x] `.claude/commands/work-plan.md` 作成（Issue 単位の作業計画）
- [x] `.claude/commands/tdd-impl.md` 作成（TDD 実装）
- [x] `.claude/commands/pm-auto-dev.md` 作成（Issue 完全自動化）
- [x] `.claude/commands/bug-fix.md` 作成（バグ調査・修正）
- [x] `.claude/commands/create-pr.md` 作成（PR 自動生成）
- [x] `.claude/commands/worktree-setup.md` 作成（Git worktree セットアップ）
- [x] `.claude/commands/worktree-cleanup.md` 作成（worktree クリーンアップ）
- [x] `.claude/commands/progress-report.md` 作成（進捗レポート）
- [x] `.claude/commands/refactoring.md` 作成（リファクタリング）
- [x] `.claude/commands/acceptance-test.md` 作成（受け入れテスト検証）
- [x] `.claude/commands/issue-enhance.md` 作成（Issue 拡充）
- [x] `.claude/agents/tdd-impl-agent.md` 作成
- [x] `.claude/agents/acceptance-test-agent.md` 作成
- [x] `.claude/agents/refactoring-agent.md` 作成
- [x] `.claude/agents/progress-report-agent.md` 作成
- [x] `.claude/agents/investigation-agent.md` 作成
- [x] `.claude/prompts/tdd-impl-core.md` 作成
- [x] `.claude/prompts/acceptance-test-core.md` 作成
- [x] `.claude/prompts/refactoring-core.md` 作成
- [x] `.claude/prompts/progress-report-core.md` 作成
- [x] `.claude/lib/validators.sh` 作成
- [x] Claude Code 上でカスタムコマンドが認識されることを確認

## Step 8: Git ワークフロー E2E 確認

- [x] GitHub 上で `main` ブランチの保護ルール設定（direct push 禁止、CI 必須、レビュー必須）
- [x] GitHub 上で `develop` ブランチの保護ルール設定（direct push 禁止、CI 必須）
- [x] feature ブランチ → develop PR → CI PASS → マージの確認（PR #1）
- [x] develop → main PR → CI PASS → マージの確認（PR #2）
- [x] `main` への直接 push が拒否されることを確認（保護ルール設定済み）

## 完了条件

- [x] `cargo build` がゼロエラー
- [x] `cargo test --all` が全テスト PASS
- [x] `cargo clippy --all-targets -- -D warnings` がゼロ警告
- [x] `cargo fmt --all -- --check` が差分なし
- [x] GitHub Actions CI 全ジョブ PASS
- [x] feature → develop → main のマージフローが動作
- [x] Claude Code カスタムコマンドが利用可能
- [x] README / CLAUDE.md / COMMANDINDEX.md が整備済み
