# 開発環境整備計画

## 目的

CommandIndex の開発を開始できる状態を作る。
Anvil と同等の開発体験（TDD、CI/CD、Claude Code 統合、Issue 駆動開発）を実現する。

## 目標

1. モノリシックになる前に、拡張可能なモジュール構成を定義する
2. 品質基盤（clippy ゼロ警告、fmt、テスト）を初日から導入する
3. CI/CD で品質を自動検証できるようにする
4. Claude Code 統合により、AI 支援開発をすぐに開始できるようにする
5. Git ワークフローを確立し、チーム開発の土台を作る

## 成果物一覧

### A. Rust プロジェクト基盤

| 成果物 | 説明 |
|---|---|
| `Cargo.toml` | パッケージ定義・依存クレート |
| `Cargo.lock` | 依存ロックファイル（Git 管理対象） |
| `src/main.rs` | CLI エントリポイント（clap サブコマンド定義） |
| `src/lib.rs` | ライブラリルート（モジュール宣言） |
| `.gitignore` | Git 除外設定 |

### B. 品質基盤

| 成果物 | 説明 |
|---|---|
| `tests/` | 統合テストディレクトリ |
| `tests/common/mod.rs` | テスト共有ユーティリティ |
| `tests/cli_args.rs` | CLIパース確認テスト |

### C. CI/CD

| 成果物 | 説明 |
|---|---|
| `.github/workflows/ci.yml` | CI パイプライン（fmt, clippy, test, build） |
| `.github/workflows/release.yml` | リリースパイプライン（クロスビルド・GitHub Release） |

### D. GitHub テンプレート

| 成果物 | 説明 |
|---|---|
| `.github/PULL_REQUEST_TEMPLATE.md` | PR テンプレート |
| `.github/ISSUE_TEMPLATE/bug_report.md` | バグ報告テンプレート |
| `.github/ISSUE_TEMPLATE/feature_request.md` | 機能要望テンプレート |

### E. ドキュメント

| 成果物 | 説明 |
|---|---|
| `README.md` | プロジェクト概要・ビルド手順 |
| `CLAUDE.md` | AI アシスタント向け開発ガイドライン |
| `COMMANDINDEX.md` | プロジェクトガードレール・テスト方針 |
| `CHANGELOG.md` | 変更履歴 |
| `LICENSE` | ライセンス（MIT） |

### F. Claude Code 統合

| 成果物 | 説明 |
|---|---|
| `.claude/commands/` | カスタムコマンド群 |
| `.claude/agents/` | サブエージェント定義 |
| `.claude/prompts/` | 共有プロンプト |
| `.claude/lib/` | 共有ユーティリティ |

## 制約

- プロダクション機能のコードは含めない（Phase 1 で実装する）
- `src/main.rs` は clap のサブコマンド定義と `todo!()` マクロのみ
- テストは CLI 引数パースの確認のみ（機能テストは Phase 1 で追加）
- 外部サービス（DB、API 等）への依存は持たない

## 実施順序

以下の順序で進める。各ステップの依存関係を考慮した順序である。

### Step 1: Rust プロジェクト初期化（A）

```
作業:
  - Cargo.toml 作成（commandindex, edition 2024）
  - src/main.rs 作成（clap サブコマンド定義）
  - src/lib.rs 作成（モジュール宣言の雛形）
  - .gitignore 作成

確認:
  - cargo build が通ること
  - cargo run -- --help でサブコマンド一覧が出ること

見積: 30分
```

### Step 2: 品質基盤（B）

```
作業:
  - cargo fmt --check が通ることを確認
  - cargo clippy --all-targets -- -D warnings が通ることを確認
  - tests/ ディレクトリ作成
  - tests/common/mod.rs 作成
  - tests/cli_args.rs 作成（サブコマンドパーステスト）
  - Cargo.toml に dev-dependencies 追加（tempfile, assert_cmd）

確認:
  - cargo test が通ること（全テスト PASS）
  - cargo clippy がゼロ警告であること

見積: 30分
```

### Step 3: ドキュメント（E）

```
作業:
  - README.md 作成
  - CLAUDE.md 作成（Anvil の CLAUDE.md をベースにカスタマイズ）
  - COMMANDINDEX.md 作成
  - CHANGELOG.md 作成
  - LICENSE 作成（MIT）

確認:
  - 各ドキュメントの内容が CommandIndex の企画書と整合すること

見積: 1時間
```

### Step 4: 初回コミット・push

```
作業:
  - workspace/ 含め全ファイルをステージ
  - 初回コミット
  - main ブランチに push

確認:
  - GitHub 上でリポジトリの内容が確認できること

見積: 10分
```

### Step 5: CI/CD パイプライン（C）

```
作業:
  - .github/workflows/ci.yml 作成
  - .github/workflows/release.yml 作成
  - develop ブランチ作成・push
  - feature ブランチで CI テスト用の PR を作成

確認:
  - CI の 4 ジョブ（fmt, clippy, test, build）が全て PASS すること
  - PR 上で CI 結果が表示されること

見積: 30分
```

### Step 6: GitHub テンプレート（D）

```
作業:
  - .github/PULL_REQUEST_TEMPLATE.md 作成
  - .github/ISSUE_TEMPLATE/bug_report.md 作成
  - .github/ISSUE_TEMPLATE/feature_request.md 作成

確認:
  - GitHub 上で PR 作成時にテンプレートが表示されること
  - Issue 作成時にテンプレート選択肢が表示されること

見積: 20分
```

### Step 7: Claude Code 統合（F）

```
作業:
  - .claude/commands/ 作成（基本コマンド群）
  - .claude/agents/ 作成（サブエージェント定義）
  - .claude/prompts/ 作成（共有プロンプト）
  - .claude/lib/ 作成（共有ユーティリティ）

確認:
  - Claude Code 上で /work-plan 等のコマンドが認識されること
  - サブエージェントが起動できること

見積: 1時間
```

### Step 8: Git ワークフロー整備・E2E 確認（G）

```
作業:
  - GitHub 上で main ブランチの保護ルール設定
  - develop ブランチの保護ルール設定
  - feature ブランチ → PR → CI → マージの E2E フロー確認

確認:
  - main への直接 push が拒否されること
  - PR 経由でのマージが成功すること
  - CI が必須チェックとして機能すること

見積: 20分
```

## 合計見積

| ステップ | 見積 |
|---|---|
| Step 1: Rust プロジェクト初期化 | 30分 |
| Step 2: 品質基盤 | 30分 |
| Step 3: ドキュメント | 1時間 |
| Step 4: 初回コミット・push | 10分 |
| Step 5: CI/CD パイプライン | 30分 |
| Step 6: GitHub テンプレート | 20分 |
| Step 7: Claude Code 統合 | 1時間 |
| Step 8: Git ワークフロー E2E | 20分 |
| **合計** | **約4時間** |

## 完了条件

以下が全て満たされた状態で、このフェーズは完了とする。

- [ ] `cargo build` がゼロエラーで通る
- [ ] `cargo test` が全テスト PASS する
- [ ] `cargo clippy --all-targets -- -D warnings` がゼロ警告で通る
- [ ] `cargo fmt --check` が差分なしで通る
- [ ] GitHub Actions CI が全ジョブ PASS する
- [ ] feature → develop → main のマージフローが動作する
- [ ] Claude Code のカスタムコマンドが利用可能である
- [ ] README / CLAUDE.md / COMMANDINDEX.md が整備されている

## 次のステップ

このフェーズ完了後、Phase 1（Markdown Knowledge MVP）の実装に着手する。
Phase 1 の作業計画は `workspace/v0.1.0/` に作成する。
