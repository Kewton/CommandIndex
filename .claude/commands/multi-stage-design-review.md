---
model: sonnet
description: "設計書の4段階レビュー（通常→整合性→影響分析→セキュリティ）と指摘対応を自動実行"
---

# マルチステージ設計レビューコマンド

## 概要
4段階のアーキテクチャレビューとその指摘事項対応を自動で実行するコマンドです。各段階でレビュー→対応のサイクルを回し、**設計方針書の品質**を段階的に向上させます。

> **重要**: このコマンドは**設計方針書のレビューと改善**を目的としています。ソースコードの実装は行いません。

## 使用方法
```bash
/multi-stage-design-review [Issue番号]
/multi-stage-design-review [Issue番号] --skip-stage=3,4
```

## レビューステージ

| Stage | レビュー種別 | フォーカス | 目的 |
|-------|------------|----------|------|
| 1 | 通常レビュー | 設計原則 | SOLID/KISS/YAGNI/DRY準拠確認 |
| 2 | 整合性レビュー | 整合性 | 設計書と実装の整合性確認 |
| 3 | 影響分析レビュー | 影響範囲 | 変更の波及効果分析 |
| 4 | セキュリティレビュー | セキュリティ | unsafe使用・パストラバーサル確認 |

## 実行フェーズ

### Phase 0: 初期設定

```bash
mkdir -p dev-reports/issue/{issue_number}/multi-stage-design-review
```

### Stage 1-4: 各ステージ実行

各ステージでサブエージェントによるレビュー → 指摘事項を設計方針書に反映。

**サブエージェントモデル指定**:
| エージェント | モデル | 理由 |
|-------------|--------|------|
| architecture-review-agent | **opus** | 品質判断にOpus必要 |
| apply-review-agent | sonnet | 設計方針書更新のみ |

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
    ├── stage1-review-context.json ~ stage4-apply-result.json
    └── summary-report.md
```

## 完了条件

- 全4ステージのレビュー完了
- 各ステージの指摘事項が設計方針書に反映完了
- サマリーレポート作成完了

## 関連コマンド

- `/architecture-review`: 単体アーキテクチャレビュー
- `/apply-review`: レビュー指摘事項の反映
- `/pm-auto-dev`: 自動開発フロー
- `/create-pr`: PR作成
