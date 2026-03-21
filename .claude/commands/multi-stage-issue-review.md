---
model: sonnet
description: "Issue記載内容の多段階レビュー（通常→影響範囲）×2回と指摘対応を自動実行"
---

# マルチステージIssueレビューコマンド

## 概要
Issueの記載内容を多角的にレビューし、ブラッシュアップするコマンドです。通常レビューと影響範囲レビューを2回ずつ実施し、各段階でレビュー→反映のサイクルを回します。

> **目的**: Issueの品質を段階的に向上させ、実装前に問題点を洗い出す
> **2回目レビュー**: Stage 5, 7 は commandmatedev 経由で Codex に委託し、異なるモデルによる多角的レビューを実現する

## 使用方法
```bash
/multi-stage-issue-review [Issue番号]
/multi-stage-issue-review [Issue番号] --skip-stage=5,6,7,8
```

## レビューステージ

| Phase/Stage | レビュー種別 | 実行エージェント | フォーカス | 目的 |
|-------------|------------|----------------|----------|------|
| 0.5 | 仮説検証 | Claude | コードベース照合 | Issue内の仮説を実コードで検証 |
| 1 | 通常レビュー（1回目） | Claude (opus) | 整合性・正確性 | 既存コードとの整合性確認 |
| 2 | 指摘事項反映（1回目） | Claude (sonnet) | - | Stage 1の指摘をIssueに反映 |
| 3 | 影響範囲レビュー（1回目） | Claude (opus) | 影響範囲 | 変更の波及効果分析 |
| 4 | 指摘事項反映（1回目） | Claude (sonnet) | - | Stage 3の指摘をIssueに反映 |
| 5 | 通常レビュー（2回目） | **Codex** (commandmatedev) | 整合性・正確性 | 異なるモデルで再チェック |
| 6 | 指摘事項反映（2回目） | Claude (sonnet) | - | Stage 5の指摘をIssueに反映 |
| 7 | 影響範囲レビュー（2回目） | **Codex** (commandmatedev) | 影響範囲 | 異なるモデルで影響範囲を再チェック |
| 8 | 指摘事項反映（2回目） | Claude (sonnet) | - | Stage 7の指摘をIssueに反映 |

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

### Stage 1-4: 1回目レビュー・反映サイクル（Claude）

**サブエージェントモデル指定**:
| エージェント | モデル | 理由 |
|-------------|--------|------|
| issue-review-agent | **opus** | 品質判断にOpus必要 |
| apply-issue-review-agent | sonnet | Issue更新のみ |

**2回目イテレーション自動スキップ判定**: Stage 4完了後、1回目のMust Fix合計が0件なら Stage 5-8 をスキップ。

### Stage 5, 7: 2回目レビュー（Codex via commandmatedev）

2回目のレビュー（Stage 5: 通常レビュー、Stage 7: 影響範囲レビュー）は、commandmatedev 経由で Codex に委託する。

**実行手順**:

1. **Worktree ID を取得**:
```bash
WORKTREE_ID=$(commandmatedev ls --quiet --branch "$(git branch --show-current)" 2>/dev/null | head -1)
```

2. **レビュープロンプトを構築**: Issue内容と前ステージの結果を含むレビュー指示を作成

3. **Codex にレビューを送信**:
```bash
commandmatedev send "$WORKTREE_ID" "$REVIEW_PROMPT" --agent codex --auto-yes --duration 1h
```

4. **完了を待機**:
```bash
commandmatedev wait "$WORKTREE_ID" --timeout 3600
```

5. **結果を取得**:
```bash
commandmatedev capture "$WORKTREE_ID" --agent codex --json
```

6. **結果をレビューコンテキストファイルに保存**: `stage{N}-review-context.json`

**Stage 5 プロンプトテンプレート**:
```
以下のGitHub Issueの内容をレビューしてください。1回目のレビューで指摘された内容は既に反映済みです。
2回目のレビューとして、以下の観点で再チェックしてください:
- 整合性・正確性: 既存コードベースとの整合性
- 受け入れ基準の網羅性と明確性
- 実装方針の妥当性

Issue内容:
{issue_body}

1回目レビュー結果:
{stage1_review_context}

結果はJSON形式で出力してください:
{"must_fix": [...], "should_fix": [...], "nice_to_have": [...], "summary": "..."}
```

**Stage 7 プロンプトテンプレート**:
```
以下のGitHub Issueの変更による影響範囲をレビューしてください。
- 既存機能への影響
- テストへの影響
- パフォーマンスへの影響
- 依存関係への影響

Issue内容:
{issue_body}

前回の影響範囲レビュー結果:
{stage3_review_context}

結果はJSON形式で出力してください:
{"must_fix": [...], "should_fix": [...], "nice_to_have": [...], "summary": "..."}
```

### Stage 6, 8: 指摘事項反映（Claude）

Codex のレビュー結果（stage5/stage7-review-context.json）を読み取り、Claude (sonnet) が Issue に反映する。

### Phase Final: 最終確認と報告

サマリーレポートを `dev-reports/issue/{issue_number}/issue-review/summary-report.md` に出力。

## ファイル構造

```
dev-reports/issue/{issue_number}/
└── issue-review/
    ├── original-issue.json
    ├── hypothesis-verification.md
    ├── stage1-review-context.json   (Claude opus)
    ├── stage2-apply-result.json     (Claude sonnet)
    ├── stage3-review-context.json   (Claude opus)
    ├── stage4-apply-result.json     (Claude sonnet)
    ├── stage5-review-context.json   (Codex via commandmatedev)
    ├── stage6-apply-result.json     (Claude sonnet)
    ├── stage7-review-context.json   (Codex via commandmatedev)
    ├── stage8-apply-result.json     (Claude sonnet)
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
