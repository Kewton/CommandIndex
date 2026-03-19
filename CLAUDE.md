# CLAUDE.md

このドキュメントはClaude Code向けのプロジェクトガイドラインです。

---

## プロジェクト概要

### 基本情報
- **プロジェクト名**: CommandIndex
- **説明**: Markdown・Code・Git を横断するローカルナレッジ検索CLI
- **リポジトリ**: https://github.com/Kewton/CommandIndex

### 技術スタック
| カテゴリ | 技術 |
|---|---|
| **言語** | Rust (Edition 2024) |
| **ビルド** | Cargo |
| **全文検索** | tantivy（Phase 1〜） |
| **日本語トークナイズ** | lindera（Phase 1〜） |
| **コード解析** | tree-sitter（Phase 3〜） |
| **補助ストア** | SQLite / rusqlite（Phase 3〜） |
| **テスト** | cargo test（統合テスト中心） |

---

## ブランチ構成

### ブランチ戦略
```
main (本番) <- PRマージのみ
  |
develop (受け入れ・動作確認)
  |
feature/*, fix/*, hotfix/* (作業ブランチ)
```

### 命名規則
| ブランチ種類 | パターン | 例 |
|---|---|---|
| 機能追加 | `feature/<issue-number>-<description>` | `feature/123-add-markdown-parser` |
| バグ修正 | `fix/<issue-number>-<description>` | `fix/456-fix-index-corruption` |
| 緊急修正 | `hotfix/<description>` | `hotfix/critical-search-fix` |
| ドキュメント | `docs/<description>` | `docs/update-readme` |

---

## 標準マージフロー

### 通常フロー
```
feature/* --PR--> develop --PR--> main
fix/*     --PR--> develop --PR--> main
hotfix/*  --PR--> main (緊急時のみ)
```

### PRルール
1. **PRタイトル**: `<type>: <description>` 形式
   - 例: `feat: add markdown heading parser`
   - 例: `fix: resolve index corruption on update`
2. **PRラベル**: 種類に応じたラベルを付与
   - `feature`, `bug`, `documentation`, `refactor`
3. **レビュー**: 1名以上の承認必須（main向けPR）
4. **CI/CD**: 全チェックパス必須

### コミットメッセージ規約
```
<type>(<scope>): <subject>

<body>

<footer>
```

| type | 説明 |
|---|---|
| `feat` | 新機能 |
| `fix` | バグ修正 |
| `docs` | ドキュメント |
| `style` | フォーマット（機能変更なし） |
| `refactor` | リファクタリング |
| `test` | テスト追加・修正 |
| `chore` | ビルド・設定変更 |
| `ci` | CI/CD設定 |
| `perf` | パフォーマンス改善 |

---

## コーディング規約

### Rust
- `cargo clippy --all-targets` で警告ゼロを維持
- `cargo test` で全テスト通過を維持
- `unsafe` は使用禁止（明確な理由がない限り）
- エラー型は構造化（`String` ではなく専用enum）を推奨

### モジュール構成（v0.0.0 時点）
```
src/
├── main.rs              # エントリポイント（clap サブコマンド定義）
└── lib.rs               # モジュール宣言

tests/
├── common/mod.rs        # テスト共有ユーティリティ
└── cli_args.rs          # CLIパーステスト
```

### モジュール構成（Phase 1 以降の想定）
```
src/
├── main.rs              # エントリポイント
├── lib.rs               # モジュール宣言
├── cli/                 # CLI サブコマンド
├── parser/              # Markdown / ソースコード解析
├── indexer/             # tantivy / SQLite インデックス操作
├── search/              # 検索ロジック
└── output/              # 出力フォーマット（human / json / path）
```

---

## 品質チェック

| チェック項目 | コマンド | 基準 |
|---|---|---|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |

---

## スラッシュコマンド（Claude Code用）

### Issue管理

| コマンド | 説明 |
|---|---|
| `/issue-create` | GitHub Issue作成（テンプレート準拠） |
| `/issue-enhance` | Issue内容補完・質問ベース更新 |
| `/issue-split` | 大規模Issueを実装可能な粒度に分割 |
| `/issues-exec-plan` | 複数Issueの実行計画策定 |

### 設計・レビュー

| コマンド | 説明 |
|---|---|
| `/design-policy` | Issue単位の設計方針書作成 |
| `/architecture-review` | SOLID/KISS/YAGNI/DRY・セキュリティ・リスク評価 |
| `/apply-review` | レビュー結果を設計方針書に反映 |
| `/multi-stage-issue-review` | Issue多段階レビュー（通常→影響範囲）×2回 |
| `/multi-stage-design-review` | 設計書4段階レビュー（通常→整合性→影響分析→セキュリティ） |

### 計画・開発

| コマンド | 説明 |
|---|---|
| `/work-plan` | Issue単位の作業計画立案 |
| `/tdd-impl` | テスト駆動開発で実装 |
| `/pm-auto-dev` | Issue開発を完全自動化（TDD→テスト→リファクタリング→報告） |
| `/pm-auto-design2dev` | 設計→開発完全自動化（設計→レビュー→計画→TDD） |
| `/pm-auto-issue2dev` | Issue→開発完全自動化（Issueレビュー→設計→レビュー→計画→TDD） |

### テスト・品質

| コマンド | 説明 |
|---|---|
| `/acceptance-test` | 受け入れテスト作成・実行 |
| `/bug-fix` | バグ調査から修正・テストまで自動実行 |
| `/refactoring` | コード品質改善リファクタリング |
| `/uat` | ユーザー受入テスト（HTMLレポート生成） |
| `/uat-fix-loop` | UAT不合格→修正→再PR→再UATのループ |

### PR・マージ・オーケストレーション

| コマンド | 説明 |
|---|---|
| `/create-pr` | PR自動作成（CIチェック含む） |
| `/pr-merge-pipeline` | 複数Issue PR→CI→マージ→統合検証 |
| `/orchestrate` | 複数Issue並列オーケストレーション（準備→開発→PR→マージ→UAT→完了） |

### Git Worktree・レポート

| コマンド | 説明 |
|---|---|
| `/worktree-setup` | Issue用Git Worktree環境構築 |
| `/worktree-cleanup` | Worktree環境のクリーンアップ |
| `/progress-report` | 開発進捗レポート作成 |

---

## サブエージェント

| エージェント | モデル | 役割 |
|---|---|---|
| tdd-impl-agent | opus | TDD実装スペシャリスト |
| acceptance-test-agent | opus | 受入テスト検証 |
| refactoring-agent | opus | コード品質改善 |
| progress-report-agent | sonnet | 進捗レポート作成 |
| investigation-agent | opus | バグ原因調査 |

---

## 禁止事項

- `main` への直接プッシュ禁止
- `force push` 禁止（自分のブランチを除く）
- `unsafe` コード禁止（明確な理由なし）
- テストなしのマージ禁止
- clippy警告の放置禁止
