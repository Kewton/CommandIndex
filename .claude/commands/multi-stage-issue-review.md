---
model: sonnet
description: "Issue記載内容の多段階レビュー（通常→影響範囲）×2回と指摘対応を自動実行"
---

# マルチステージIssueレビューコマンド

## 概要
Issueの記載内容を多角的にレビューし、ブラッシュアップするコマンドです。通常レビューと影響範囲レビューを2回ずつ実施し、各段階でレビュー→反映のサイクルを回します。

> **目的**: Issueの品質を段階的に向上させ、実装前に問題点を洗い出す

## 使用方法
```bash
/multi-stage-issue-review [Issue番号]
/multi-stage-issue-review [Issue番号] --skip-stage=5,6,7,8
```

## レビューステージ

| Phase/Stage | レビュー種別 | フォーカス | 目的 |
|-------------|------------|----------|------|
| 0.5 | 仮説検証 | コードベース照合 | Issue内の仮説を実コードで検証 |
| 1 | 通常レビュー（1回目） | 整合性・正確性 | 既存コードとの整合性確認 |
| 2 | 指摘事項反映（1回目） | - | Stage 1の指摘をIssueに反映 |
| 3 | 影響範囲レビュー（1回目） | 影響範囲 | 変更の波及効果分析 |
| 4 | 指摘事項反映（1回目） | - | Stage 3の指摘をIssueに反映 |
| 5 | 通常レビュー（2回目） | 整合性・正確性 | 更新後のIssueを再チェック |
| 6 | 指摘事項反映（2回目） | - | Stage 5の指摘をIssueに反映 |
| 7 | 影響範囲レビュー（2回目） | 影響範囲 | 更新後の影響範囲を再チェック |
| 8 | 指摘事項反映（2回目） | - | Stage 7の指摘をIssueに反映 |

## 実行フェーズ

### Phase 0: 初期設定

```bash
mkdir -p dev-reports/issue/{issue_number}/issue-review
gh issue view {issue_number} --json title,body > dev-reports/issue/{issue_number}/issue-review/original-issue.json
```

### Phase 0.5: 仮説検証

Issue内に記載された仮説・原因分析をコードベースと照合。仮説がない場合はスキップ。

判定：Confirmed / Rejected / Partially Confirmed / Unverifiable

検証レポートを `dev-reports/issue/{issue_number}/issue-review/hypothesis-verification.md` に出力。

### Stage 1-8: レビュー・反映サイクル

**サブエージェントモデル指定**:
| エージェント | モデル | 理由 |
|-------------|--------|------|
| issue-review-agent | **opus** | 品質判断にOpus必要 |
| apply-issue-review-agent | sonnet | Issue更新のみ |

**2回目イテレーション自動スキップ判定**: Stage 4完了後、1回目のMust Fix合計が0件なら Stage 5-8 をスキップ。

### Phase Final: 最終確認と報告

サマリーレポートを `dev-reports/issue/{issue_number}/issue-review/summary-report.md` に出力。

## ファイル構造

```
dev-reports/issue/{issue_number}/
└── issue-review/
    ├── original-issue.json
    ├── hypothesis-verification.md
    ├── stage1-review-context.json ~ stage8-apply-result.json
    └── summary-report.md
```

## 完了条件

- 仮説検証完了（仮説がない場合はスキップ記録）
- 全8ステージ完了（またはスキップ指定分を除く）
- 各ステージのMust Fix指摘が対応済み
- GitHubのIssueが更新されている
- サマリーレポート作成完了

## 関連コマンド

- `/design-policy`: 設計方針策定
- `/architecture-review`: アーキテクチャレビュー
- `/pm-auto-dev`: 自動開発フロー
- `/tdd-impl`: TDD実装
