---
model: sonnet
description: "Issueレビューから実装完了まで完全自動化（Issueレビュー→設計→設計レビュー→作業計画→TDD実装）"
---

# PM自動 Issue→開発スキル

## 概要
Issueレビューから実装完了までの全工程（Issueレビュー → 設計方針策定 → 設計レビュー → 作業計画立案 → TDD実装）を**完全自動化**するプロジェクトマネージャースキルです。

## 使用方法
- `/pm-auto-issue2dev [Issue番号]`

## 実行フェーズ

### Phase 0: 初期設定とTodoリスト作成

```
- [ ] Phase 1: マルチステージIssueレビュー
- [ ] Phase 2: 設計方針書確認・作成
- [ ] Phase 3: マルチステージ設計レビュー
- [ ] Phase 4: 作業計画立案
- [ ] Phase 5: TDD自動開発
- [ ] Phase 6: 完了報告
```

### Phase 1: マルチステージIssueレビュー

`/multi-stage-issue-review {issue_number}` を実行。

### Phase 2: 設計方針書の確認・作成

設計方針書が存在しない場合は `/design-policy {issue_number}` を実行。

### Phase 3: マルチステージ設計レビュー

`/multi-stage-design-review {issue_number}` を実行。

### Phase 4: 作業計画立案

`/work-plan {issue_number}` を実行。

### Phase 5: TDD自動開発

`/pm-auto-dev {issue_number}` を実行。

### Phase 6: 完了報告

最終検証（cargo build, clippy, test, fmt）と成果物サマリーを報告。

## ファイル構造

```
dev-reports/
├── design/
│   └── issue-{issue_number}-*-design-policy.md
└── issue/{issue_number}/
    ├── issue-review/
    │   ├── original-issue.json
    │   ├── hypothesis-verification.md
    │   ├── stage1-*.json ~ stage8-*.json
    │   └── summary-report.md
    ├── multi-stage-design-review/
    │   ├── stage1-*.json ~ stage4-*.json
    │   └── summary-report.md
    ├── work-plan.md
    └── pm-auto-dev/
        └── iteration-1/
            ├── tdd-*.json
            ├── acceptance-*.json
            ├── refactor-*.json
            └── progress-report.md
```

## 完了条件

- Phase 1: マルチステージIssueレビュー完了（Issue本文が更新されている）
- Phase 2: 設計方針書が存在する
- Phase 3: マルチステージ設計レビュー完了
- Phase 4: 作業計画書が作成されている
- Phase 5: TDD自動開発完了（テスト全パス、clippy警告0件）
- Phase 6: 完了報告

## 関連コマンド

- `/multi-stage-issue-review`: マルチステージIssueレビュー
- `/design-policy`: 設計方針書作成
- `/multi-stage-design-review`: マルチステージ設計レビュー
- `/work-plan`: 作業計画立案
- `/pm-auto-dev`: TDD自動開発
- `/create-pr`: PR作成
- `/pm-auto-design2dev`: 設計レビューから実装完了まで（Issueレビューなし版）
