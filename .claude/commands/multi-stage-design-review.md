---
model: sonnet
description: "設計書の4段階レビュー（通常→整合性→影響分析→セキュリティ）×2回と指摘対応を自動実行"
---

# マルチステージ設計レビューコマンド

## 概要
アーキテクチャレビューとその指摘事項対応を自動で実行するコマンドです。各段階でレビュー→対応のサイクルを回し、**設計方針書の品質**を段階的に向上させます。

> **重要**: このコマンドは**設計方針書のレビューと改善**を目的としています。ソースコードの実装は行いません。
> **2回目レビュー**: Stage 5-8 は commandmatedev 経由で Codex に委託し、異なるモデルによる多角的レビューを実現する

## 使用方法
```bash
/multi-stage-design-review [Issue番号]
/multi-stage-design-review [Issue番号] --skip-stage=5,6,7,8
```

## レビューステージ

| Stage | レビュー種別 | 実行エージェント | フォーカス | 目的 |
|-------|------------|----------------|----------|------|
| 1 | 通常レビュー（1回目） | Claude (opus) | 設計原則 | SOLID/KISS/YAGNI/DRY準拠確認 |
| 2 | 整合性レビュー（1回目） | Claude (opus) | 整合性 | 設計書と実装の整合性確認 |
| 3 | 影響分析レビュー（1回目） | Claude (opus) | 影響範囲 | 変更の波及効果分析 |
| 4 | セキュリティレビュー（1回目） | Claude (opus) | セキュリティ | unsafe使用・パストラバーサル確認 |
| 5 | 通常レビュー（2回目） | **Codex** (commandmatedev) | 設計原則 | 異なるモデルで設計原則を再チェック |
| 6 | 指摘事項反映（2回目） | Claude (sonnet) | - | Stage 5の指摘を設計方針書に反映 |
| 7 | 整合性・影響分析レビュー（2回目） | **Codex** (commandmatedev) | 整合性・影響範囲 | 異なるモデルで整合性・影響範囲を再チェック |
| 8 | 指摘事項反映（2回目） | Claude (sonnet) | - | Stage 7の指摘を設計方針書に反映 |

## 実行フェーズ

### Phase 0: 初期設定

```bash
mkdir -p dev-reports/issue/{issue_number}/multi-stage-design-review
```

### Stage 1-4: 1回目レビュー（Claude）

各ステージでサブエージェントによるレビュー → 指摘事項を設計方針書に反映。

**サブエージェントモデル指定**:
| エージェント | モデル | 理由 |
|-------------|--------|------|
| architecture-review-agent | **opus** | 品質判断にOpus必要 |
| apply-review-agent | sonnet | 設計方針書更新のみ |

**2回目イテレーション自動スキップ判定**: Stage 4完了後、1回目のMust Fix合計が0件なら Stage 5-8 をスキップ。

### Stage 5, 7: 2回目レビュー（Codex via commandmatedev）

2回目のレビューは commandmatedev 経由で Codex に委託する。

**実行手順**:

1. **Worktree ID を取得**:
```bash
WORKTREE_ID=$(commandmatedev ls --quiet --branch "$(git branch --show-current)" 2>/dev/null | head -1)
```

2. **レビュープロンプトを構築**: 設計方針書の内容と前ステージの結果を含むレビュー指示を作成

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
以下の設計方針書をレビューしてください。1回目のレビュー（4段階）で指摘された内容は既に反映済みです。
2回目のレビューとして、以下の観点で再チェックしてください:
- SOLID/KISS/YAGNI/DRY 原則への準拠
- API設計の妥当性
- エラーハンドリング方針の網羅性

設計方針書:
{design_policy_content}

1回目レビュー結果サマリー:
{stage1_to_4_summary}

結果はJSON形式で出力してください:
{"must_fix": [...], "should_fix": [...], "nice_to_have": [...], "summary": "..."}
```

**Stage 7 プロンプトテンプレート**:
```
以下の設計方針書の整合性と影響範囲をレビューしてください。
- 設計書と既存コードベースの整合性
- 変更の波及効果（既存テスト・モジュールへの影響）
- セキュリティ上の懸念

設計方針書:
{design_policy_content}

前回の整合性・影響分析レビュー結果:
{stage2_3_review_context}

結果はJSON形式で出力してください:
{"must_fix": [...], "should_fix": [...], "nice_to_have": [...], "summary": "..."}
```

### Stage 6, 8: 指摘事項反映（Claude）

Codex のレビュー結果（stage5/stage7-review-context.json）を読み取り、Claude (sonnet) が設計方針書に反映する。

### Phase Final: 最終確認と報告

```bash
cargo build
cargo clippy --all-targets -- -D warnings
cargo test --all
cargo fmt --all -- --check
```

サマリーレポートを `dev-reports/issue/{issue_number}/multi-stage-design-review/summary-report.md` に出力。

## ファイル構造

```
dev-reports/issue/{issue_number}/
└── multi-stage-design-review/
    ├── stage1-review-context.json   (Claude opus)
    ├── stage1-apply-result.json     (Claude sonnet)
    ├── stage2-review-context.json   (Claude opus)
    ├── stage2-apply-result.json     (Claude sonnet)
    ├── stage3-review-context.json   (Claude opus)
    ├── stage3-apply-result.json     (Claude sonnet)
    ├── stage4-review-context.json   (Claude opus)
    ├── stage4-apply-result.json     (Claude sonnet)
    ├── stage5-review-context.json   (Codex via commandmatedev)
    ├── stage6-apply-result.json     (Claude sonnet)
    ├── stage7-review-context.json   (Codex via commandmatedev)
    ├── stage8-apply-result.json     (Claude sonnet)
    └── summary-report.md
```

## 完了条件

- 全8ステージのレビュー完了（またはスキップ指定分を除く）
- 各ステージの指摘事項が設計方針書に反映完了
- サマリーレポート作成完了

## 関連コマンド

- `/architecture-review`: 単体アーキテクチャレビュー
- `/apply-review`: レビュー指摘事項の反映
- `/pm-auto-dev`: 自動開発フロー
- `/create-pr`: PR作成
