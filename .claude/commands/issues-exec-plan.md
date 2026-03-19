---
model: sonnet
description: "複数Issueの実行計画を策定（優先度・依存関係・スケジュール）"
---

# Issues実行計画スキル

## 概要
複数のIssueに対する実行計画を策定するスキルです。優先度、依存関係、リソース制約を考慮し、最適な実行順序とスケジュールを生成します。

## 使用方法
- `/issues-exec-plan`
- `/issues-exec-plan [ラベルフィルター]`

## 実行内容

### 1. Issue一覧の取得

```bash
gh issue list --repo Kewton/CommandIndex --state open --json number,title,labels,assignees,milestone
```

### 2. Issue分析

| 項目 | 説明 |
|------|------|
| **サイズ** | S (1-2h), M (3-4h), L (5-8h), XL (8h+) |
| **優先度** | P0 (緊急), P1 (高), P2 (中), P3 (低) |
| **種別** | feature, bug, refactor, docs |
| **影響モジュール** | src/xxx/ |
| **依存関係** | 他Issueとの前後関係 |

### 3. 依存関係マッピング

### 4. 実行順序の決定

### 5. 実行計画の策定

Sprint単位で計画を策定。

### 6. リスク評価

### 7. 品質ゲート

各Issue完了時に `cargo build`, `clippy`, `test`, `fmt` を確認。

## 出力先

`dev-reports/issues-exec-plan.md`

## 完了条件

- 全オープンIssueが分析されている
- 依存関係が明確に記載されている
- 優先度・サイズが判定されている
- 実行順序が決定されている

## 関連コマンド

- `/issue-create`: Issue作成
- `/issue-split`: Issue分割
- `/issue-enhance`: Issue内容の補完
- `/work-plan`: 作業計画立案
- `/pm-auto-issue2dev`: Issue補完から開発まで一括実行
