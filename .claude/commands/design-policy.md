---
model: opus
description: "Issue単位の設計方針書を作成"
---

# 設計方針書作成スキル

## 概要
Issue単位での設計方針書を作成するスキルです。CommandIndexプロジェクトのアーキテクチャに沿った設計判断を文書化し、実装前の合意形成を支援します。

## 使用方法
- `/design-policy [Issue番号]`

## 前提条件
- 対象Issueの内容が明確であること
- GitHubリポジトリにアクセス可能

## 実行内容

あなたはソフトウェアアーキテクトとして、以下の設計方針書を作成します。

### 1. Issue情報の取得

```bash
gh issue view {issue_number} --json number,title,body,labels,assignees
```

### 2. システムアーキテクチャ概要

CommandIndexの全体アーキテクチャを踏まえた設計を行います。

### 3. レイヤー構成と責務

| レイヤー | モジュール | 責務 |
|---------|-----------|------|
| **CLI** | `src/main.rs` | エントリポイント、clapサブコマンド定義 |
| **Parser** | `src/parser/` | Markdown・ソースコード解析 |
| **Indexer** | `src/indexer/` | tantivy/SQLiteインデックス操作 |
| **Search** | `src/search/` | 検索ロジック |
| **Output** | `src/output/` | 出力フォーマット（human/json/path） |

### 4. 技術選定

| カテゴリ | 選定技術 | 選定理由 |
|---------|---------|---------|
| 言語 | Rust (Edition 2024) | メモリ安全性、パフォーマンス |
| ビルド | Cargo | Rust標準ビルドシステム |
| 全文検索 | tantivy | 高速な全文検索エンジン |
| 日本語トークナイズ | lindera | 日本語形態素解析 |
| コード解析 | tree-sitter | 構文解析 |
| 補助ストア | SQLite / rusqlite | メタデータ管理 |
| テスト | cargo test | 統合テスト中心 |

### 5. 設計パターン

エラー型は構造化enum、unsafeは使用禁止。

### 6. セキュリティ設計

| 脅威 | 対策 | 優先度 |
|------|------|--------|
| パストラバーサル | ファイル操作時の正規化とベースディレクトリチェック | 高 |
| unsafe使用 | 原則禁止 | 中 |

### 7. 設計判断とトレードオフ

Issue #{issue_number} に関する設計判断を記録。

### 8. 影響範囲

変更対象のモジュールと影響範囲を明記。

### 9. 品質基準

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --check` | 差分なし |

## 出力先

`dev-reports/design/issue-{issue_number}-design-policy.md`

## 完了条件

- レイヤー構成と責務が明確である
- 設計パターンが具体的なRustコードで示されている
- セキュリティ要件が記載されている
- 設計判断とトレードオフが記録されている
- 影響範囲が明確である

## 関連コマンド

- `/architecture-review`: アーキテクチャレビュー実行
- `/apply-review`: レビュー結果を設計方針書に反映
- `/multi-stage-design-review`: マルチステージ設計レビュー
- `/work-plan`: 作業計画立案
- `/pm-auto-design2dev`: 設計から開発まで一括実行
