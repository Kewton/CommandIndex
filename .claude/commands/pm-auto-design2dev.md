---
model: sonnet
description: "設計レビューから実装完了まで完全自動化（設計→設計レビュー→作業計画→TDD実装）"
---

# PM自動 設計→開発スキル

## 概要
設計レビューから実装完了までの全工程（設計方針策定 → 設計レビュー → 作業計画立案 → TDD実装）を**完全自動化**するプロジェクトマネージャースキルです。

Issue内容が十分に整備されている場合に使用します。Issueレビューから始める場合は `/pm-auto-issue2dev` を使用してください。

## 使用方法
- `/pm-auto-design2dev [Issue番号]`

## 実行フェーズ

### Phase 0: 初期設定とTodoリスト作成

```
- [ ] Phase 1: 設計方針書確認・作成
- [ ] Phase 2: マルチステージ設計レビュー
- [ ] Phase 3: 作業計画立案
- [ ] Phase 4: TDD自動開発
- [ ] Phase 5: 完了報告
```

### Phase 1: 設計方針書の確認・作成

設計方針書が存在しない場合は `/design-policy {issue_number}` を実行。

### Phase 2: マルチステージ設計レビュー

`/multi-stage-design-review {issue_number}` を実行。

### Phase 3: 作業計画立案

`/work-plan {issue_number}` を実行。

### Phase 4: TDD自動開発

`/pm-auto-dev {issue_number}` を実行。

### Phase 5: 完了報告

最終検証（cargo build, clippy, test, fmt）と成果物サマリーを報告。

## 完了条件

- Phase 1: 設計方針書が存在する
- Phase 2: マルチステージ設計レビュー完了
- Phase 3: 作業計画書が作成されている
- Phase 4: TDD自動開発完了（テスト全パス、clippy警告0件）
- Phase 5: 完了報告

## 関連コマンド

- `/design-policy`: 設計方針書作成
- `/multi-stage-design-review`: マルチステージ設計レビュー
- `/work-plan`: 作業計画立案
- `/pm-auto-dev`: TDD自動開発
- `/create-pr`: PR作成
- `/pm-auto-issue2dev`: Issueレビューから開発まで一括実行
