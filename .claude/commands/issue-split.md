---
model: opus
description: "大規模Issueを実装可能な粒度に分割"
---

# Issue分割スキル

## 概要
大規模なIssueを実装可能な適切な粒度に分割するスキルです。依存関係を考慮し、Phase分けされた実行計画を生成します。

## 使用方法
- `/issue-split [Issue番号]`

## 前提条件
- 対象Issueが存在すること
- Issue内容が十分に記述されていること（不足時は `/issue-enhance` を先に実行）

## 実行内容

### 1. Issue情報の取得

```bash
gh issue view {issue_number} --json number,title,body,labels,assignees
```

### 2. 分割基準

| 基準 | 閾値 | 判定 |
|------|------|------|
| 変更ファイル数 | 5ファイル以上 | 分割推奨 |
| 変更モジュール数 | 3モジュール以上 | 分割推奨 |
| テストケース数 | 10ケース以上 | 分割推奨 |
| 要件数 | 5つ以上 | 分割推奨 |

### 3. 分割方針（レイヤー別）

| レイヤー | モジュール | 分割単位 |
|---------|-----------|---------|
| パーサー | `src/parser/` | Markdown / ソースコード解析 |
| インデクサー | `src/indexer/` | tantivy / SQLite操作 |
| 検索 | `src/search/` | 検索ロジック |
| 出力 | `src/output/` | フォーマット処理 |
| CLI | `src/main.rs` | サブコマンド定義 |

### 4. Phase分け

```
Phase 1: 基盤（依存なし）- 型定義・trait追加
Phase 2: コアロジック（Phase 1に依存）
Phase 3: 統合（Phase 2に依存）
Phase 4: テスト・ドキュメント
```

### 5. Sub-issue作成

```bash
gh issue create \
  --repo Kewton/CommandIndex \
  --title "feat: {sub-issue title} (part of #{issue_number})" \
  --body "{sub-issue body}" \
  --label "feature"
```

親Issueに分割結果をコメントとして記録。

## 完了条件

- Sub-issueがすべてGitHubに作成されている
- 依存関係が明確に記載されている
- 各Sub-issueが独立してビルド・テスト可能な粒度である
- 親Issueに分割結果が記録されている

## 関連コマンド

- `/issue-create`: Issue作成
- `/issue-enhance`: Issue内容の補完
- `/work-plan`: 作業計画立案
- `/pm-auto-issue2dev`: Issue補完から開発まで一括実行
