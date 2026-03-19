---
model: sonnet
description: "アーキテクチャレビュー結果を設計方針書に反映"
---

# レビュー結果反映スキル

## 概要
アーキテクチャレビュー（`/architecture-review`）の指摘事項を設計方針書（`/design-policy`）に反映するスキルです。

## 使用方法
- `/apply-review [Issue番号]`

## 前提条件
- `/architecture-review` が実行済みであること
- `/design-policy` で設計方針書が作成済みであること

## 実行内容

### 1. レビュー結果の確認

`dev-reports/issue/{issue_number}/architecture-review.md` を読み込み。

### 2. 設計方針書の確認

`dev-reports/design/issue-{issue_number}-design-policy.md` を読み込み。

### 3. 反映項目の分類

| カテゴリ | 説明 | 優先度 |
|---------|------|--------|
| 設計原則違反 | SOLID/KISS/YAGNI/DRY違反 | 高 |
| セキュリティ懸念 | パストラバーサル等 | 高 |
| メモリ安全性 | unsafe使用、ライフタイム問題 | 高 |
| 構造改善 | モジュール構成、trait設計 | 中 |
| パフォーマンス | 不要なclone、アロケーション | 低 |

### 4. 設計方針書への反映

各指摘事項について、設計方針書の該当セクションを更新。変更履歴を記録。

### 5. 品質チェック

- 全セクションが矛盾なく更新されている
- 変更履歴が正しく記録されている
- モジュール構成と整合している

### 6. 反映サマリー出力

## 出力先

`dev-reports/design/issue-{issue_number}-design-policy.md`（更新）

## 完了条件

- レビュー指摘事項がすべて分類されている
- 設計方針書が更新されている
- 変更履歴が記録されている
- 反映サマリーが出力されている

## 関連コマンド

- `/architecture-review`: アーキテクチャレビュー実行
- `/design-policy`: 設計方針書作成
- `/multi-stage-design-review`: マルチステージ設計レビュー
- `/work-plan`: 作業計画立案
