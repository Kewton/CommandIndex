---
model: opus
description: "Issue内容を分析し、ユーザーに質問しながら不足情報を補完・更新"
---

# Issue補完コマンド

## 概要

Issueの記載内容を分析し、不足しているセクションを特定し、ユーザーに質問して情報を収集した上でIssue本文を補完・更新するコマンドです。

> **目的**: タイトルのみ・本文が不十分なIssueを、ユーザーとの対話を通じて実装可能な品質まで引き上げる

## 使用方法

```bash
/issue-enhance [Issue番号]
```

## 実行内容

あなたはIssue補完の専門家です。以下の4フェーズでIssue内容を分析・補完します。

### Phase 1: Issue読み込みと分析

```bash
gh issue view {issue_number} --json title,body,labels
```

Issue種別を判定し、必須セクションの充足状況を確認する。

### Phase 2: コードベース調査

Issue内で言及されているファイル、モジュール、機能名を抽出し、Explore agentで関連コードを調査する。

CommandIndexのモジュール構成を参考にする：
- `src/cli/` - CLIサブコマンド
- `src/parser/` - Markdown / ソースコード解析
- `src/indexer/` - tantivy / SQLite インデックス操作
- `src/search/` - 検索ロジック
- `src/output/` - 出力フォーマット

### Phase 3: ユーザーへの質問

Phase 1で特定した不足セクションについてユーザーに質問する。AskUserQuestionの制限（1回あたり最大4問）を考慮し、必須セクションから優先的に質問する。

### Phase 4: Issue本文生成・更新

既存のIssue内容、コードベース調査結果、ユーザーの回答を統合してIssue本文を生成し、ユーザー確認後にGitHub Issueを更新する。

```bash
gh issue edit {issue_number} --body "$(cat <<'ISSUE_BODY'
{生成したIssue本文}
ISSUE_BODY
)"
```

## 完了条件

- Issue種別が判定されている
- コードベース調査が完了している
- ユーザーへの質問が完了している
- 生成したIssue本文をユーザーが確認している
- GitHub Issueが更新されている
