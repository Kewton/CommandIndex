---
model: opus
description: "Issueの受入テストをdevelopブランチ上でCLIを実際に起動して実施し、HTMLレポートを生成"
---

# ユーザー受入テスト（UAT）

## 概要
developブランチでIssueの受入テストを実施します。CommandIndexを実際に起動して動作確認を行い、結果をHTMLレポートとして出力します。複数Issueの一括テストに対応しています。

**重要**: コンテキストを綺麗に保つため、テスト実行はサブエージェントで行います。

## 使用方法
```
/uat [Issue番号]              # 単一Issue
/uat [Issue番号1] [Issue番号2] ...  # 複数Issue（スペース区切り）
```

## 実行手順

### 1. 事前チェック

```bash
current_branch=$(git branch --show-current)
if [ "$current_branch" != "develop" ]; then
  echo "ERROR: developブランチで実行してください（現在: $current_branch）"
  exit 1
fi
git status --porcelain
```

### 2. 引数の解析

`$ARGUMENTS` をスペースで分割し、Issue番号のリストを生成する。各Issue番号に対して `gh issue view` で存在を確認する。

```bash
for issue_num in $ARGUMENTS; do
  gh issue view "$issue_num" --repo Kewton/CommandIndex --json title -q '.title' 2>/dev/null
done
```

### 3. Issue情報の取得（Issueごと）

```bash
gh issue view $issue_num --json title,body,labels
```

Issue本文から以下を抽出：
- **受け入れ基準**（「受け入れ基準」「Acceptance Criteria」セクション）
- **提案する解決策**（期待される機能の概要）
- **関連するファイル・モジュール**

### 4. 受入テスト計画の作成

Issueの受け入れ基準に基づき、**具体的なテスト計画**を作成する。

**重要**: 各テスト項目には必ず **CommandIndexバイナリを実際に起動する具体的なコマンド** を含めること。

#### テスト計画のフォーマット

```
AT-${ISSUE_NUM}-${N}: ${テスト項目タイトル}
  対応する受け入れ基準: ${Issueの受け入れ基準の該当項目}
  前提条件:
    - ${セットアップ手順}
  実行コマンド:
    - ${前提条件のセットアップコマンド}
    - ./target/release/commandindex ${サブコマンド} ${引数} 2>"$RUN_DIR/AT-${N}/stderr.log" >"$RUN_DIR/AT-${N}/stdout.log"
  期待結果:
    - ${具体的な期待結果}
  PASS/FAIL判定基準:
    - PASS: ${具体的な条件}
    - FAIL: ${具体的な条件}
  ログ出力先:
    - stdout: $RUN_DIR/AT-${N}/stdout.log
    - stderr: $RUN_DIR/AT-${N}/stderr.log
```

### 5. テスト計画のレビュー（第1回）

サブエージェント（general-purpose）でレビュー。

**観点A: UAT方針への準拠**
- 全テスト項目にCommandIndexバイナリ起動コマンドが含まれているか
- PASS/FAIL判定基準が明確か

**観点B: Issue記載内容の網羅性**
- 受け入れ基準の全項目にテスト項目が対応しているか
- 正常系・異常系がカバーされているか

### 6-8. テスト計画の修正・再レビュー

2回のレビューサイクルを実施。

### 9. テスト計画のユーザー確認

最終テスト計画をユーザーに確認。

### 10. 作業環境の準備（ラン管理）

```bash
DATE=$(date +%Y-%m-%d)
for issue_num in $ISSUE_LIST; do
  EXISTING=$(ls -d "./sandbox/${issue_num}/${DATE}_"* 2>/dev/null | wc -l | tr -d ' ')
  SEQ=$(printf "%03d" $((EXISTING + 1)))
  RUN_DIR="./sandbox/${issue_num}/${DATE}_${SEQ}"
  mkdir -p "$RUN_DIR"
  ln -sfn "${DATE}_${SEQ}" "./sandbox/${issue_num}/latest"
done
```

### 11. サブエージェントによるテスト実行

各Issueのテスト項目をサブエージェント（general-purpose）で実行。独立したIssueは並列実行。

サブエージェントには以下を必ず含める：
- CommandIndexバイナリを実際に起動すること
- stdout.log / stderr.log を保存すること
- result.json を保存すること

### 12. HTMLレポート生成

各ランのテスト結果を `$RUN_DIR/report.html` に生成。
履歴レポートを `./sandbox/${ISSUE_NUM}/history.html` に生成・更新。
複数Issue時は全体サマリーを `./sandbox/uat-summary.html` に生成。

### 13. 結果報告

全Issueの結果サマリーをユーザーに報告。

### 14. テスト結果のIssueへの記録

```bash
gh issue comment $ISSUE_NUM --repo Kewton/CommandIndex --body "{UAT結果}"
```

2回目以降は既存UATコメントを更新。

## 完了条件

- [ ] テスト計画が2回のレビューサイクルを経て確定している
- [ ] 全Issueの全テスト項目が実行されている
- [ ] 各テスト項目で stdout.log / stderr.log が作成されている
- [ ] HTMLレポートが生成されている
- [ ] GitHub Issueコメントにテスト結果が記録されている

## エラーハンドリング

| エラーケース | 対応 |
|-------------|------|
| developブランチでない | エラー表示し中断 |
| `cargo build` 失敗 | エラー表示し中断 |
| Issue番号が無効 | 該当Issueをスキップ |
| テスト途中でエラー | 該当テストをFAILとし、残りを続行 |
