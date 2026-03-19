# Claude Code 統合計画

## 概要

Anvil と同等の Claude Code 統合を行い、AI 支援による Issue 駆動・TDD 開発を可能にする。

## ディレクトリ構成

```
.claude/
├── commands/                   # カスタムスラッシュコマンド
│   ├── work-plan.md            # Issue 単位の作業計画
│   ├── tdd-impl.md             # TDD 実装（Red-Green-Refactor）
│   ├── pm-auto-dev.md          # Issue 完全自動化（TDD → テスト → レポート）
│   ├── bug-fix.md              # バグ調査・修正
│   ├── create-pr.md            # PR 自動生成
│   ├── worktree-setup.md       # Git worktree セットアップ
│   ├── worktree-cleanup.md     # worktree クリーンアップ
│   ├── progress-report.md      # 進捗レポート
│   ├── refactoring.md          # リファクタリング
│   ├── acceptance-test.md      # 受け入れテスト検証
│   └── issue-enhance.md        # Issue 拡充
│
├── agents/                     # サブエージェント定義
│   ├── tdd-impl-agent.md       # TDD 実装エージェント
│   ├── acceptance-test-agent.md # 受け入れテストエージェント
│   ├── refactoring-agent.md    # リファクタリングエージェント
│   ├── progress-report-agent.md # 進捗レポートエージェント
│   └── investigation-agent.md  # バグ調査エージェント
│
├── prompts/                    # 共有プロンプト
│   ├── tdd-impl-core.md        # TDD 手法
│   ├── acceptance-test-core.md # テスト検証手法
│   ├── refactoring-core.md     # リファクタリング手法
│   └── progress-report-core.md # レポート生成手法
│
└── lib/                        # 共有ユーティリティ
    └── validators.sh           # バリデーションスクリプト
```

## サブエージェント構成

| エージェント | モデル | 役割 |
|---|---|---|
| tdd-impl-agent | opus | TDD 実装（Red → Green → Refactor） |
| acceptance-test-agent | opus | 受け入れテストの検証・実行 |
| refactoring-agent | opus | コード品質改善 |
| progress-report-agent | sonnet | 進捗レポート作成 |
| investigation-agent | opus | バグ調査・原因分析 |

## 主要コマンドの役割

### /work-plan

Issue を読み込み、作業計画を立案する。
タスク分解、実装順序、テスト方針を出力する。

### /tdd-impl

TDD のサイクル（Red → Green → Refactor）に従って実装する。
1. 失敗するテストを書く
2. テストを通す最小限の実装をする
3. リファクタリングする
4. cargo clippy / cargo fmt を実行して品質を確認する

### /pm-auto-dev

Issue の完全自動化。以下を一気通貫で実行する。
1. Issue の読み込み・分析
2. TDD による実装
3. テスト実行・品質チェック
4. 進捗レポート生成

### /create-pr

現在のブランチの変更から PR を自動生成する。
タイトル、本文、関連 Issue の紐付けを行う。

### /bug-fix

バグの調査と修正。
1. 再現手順の確認
2. 原因の特定
3. 修正の実装（TDD）
4. 修正確認テスト

## カスタマイズ方針

Anvil のコマンド・エージェント・プロンプトをベースに、以下を CommandIndex 固有にカスタマイズする。

- CLAUDE.md のプロジェクト情報（技術スタック、モジュール構成）
- 品質チェックコマンド（Anvil と同一だが、プロジェクト名を変更）
- テスト方針（CommandIndex のテスト戦略に合わせる）

Anvil と共通で使えるもの（TDD 手法、Git ワークフロー等）はそのまま流用する。
