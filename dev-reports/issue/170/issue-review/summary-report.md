# Issue #170 マルチステージレビュー サマリーレポート

## 概要
- **Issue**: #170 why/issueのJSON出力に日付情報を付与する
- **実施日**: 2026-03-25
- **ステージ**: Stage 1-4 完了、Stage 5-8 スキップ（CommandMateサーバー停止）

## レビュー結果

### Stage 0.5: 仮説検証

| 仮説 | 判定 |
|------|------|
| ファイル名から日付抽出 | Partially Confirmed（Stage Reviewのみ） |
| パスからgit logの最終コミット日 | Confirmed（未実装） |
| git log由来の日付 | Confirmed（未実装） |
| --timelineオプション | Confirmed（未実装） |

### Stage 1: 通常レビュー（1回目）
- **Must Fix**: 3件 → 全て反映済み
  - M1: 現状例のファイル名が不正確
  - M2: 受け入れ基準が不明確
  - M3: 日付抽出可能範囲の記載と実態の乖離
- **Should Fix**: 4件 → 全て反映済み
- **Nice to Have**: 2件 → 1件反映済み

### Stage 3: 影響範囲レビュー（1回目）
- **Must Fix**: 4件 → 全て反映済み
  - M1: WhyDocumentEntry変更
  - M2: IssueDocumentEntry変更
  - M3: issue JSON破壊的変更
  - M4: 日付取得ユーティリティ新規実装
- **Should Fix**: 4件 → 全て反映済み
- **Nice to Have**: 4件 → 反映済み

### Stage 5-8: スキップ
- CommandMateサーバー停止のためCodexレビューをスキップ

## Issue更新内容

1. 受け入れ基準セクション追加
2. 影響範囲セクション追加（構造体一覧、破壊的変更明記）
3. 実装方針セクション追加（日付取得ユーティリティ設計）
4. マイグレーション戦略セクション追加
5. 日付取得優先順位を2段階に簡素化
6. --timelineオプションは別Issue分離と注記
7. ISO 8601日付フォーマット明記

## 結論

Issue #170 は1回目のレビューサイクル（Stage 1-4）で大幅にブラッシュアップされた。受け入れ基準、影響範囲、実装方針が明確化され、実装に進められる状態。
