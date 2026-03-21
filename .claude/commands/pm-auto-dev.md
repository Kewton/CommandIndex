---
model: sonnet
description: "Issue開発を完全自動化（TDD→Codexレビュー→テスト→リファクタリング→報告）"
---

# PM自動開発スキル

## 概要
Issue開発（TDD実装 → Codexコードレビュー → 受入テスト → リファクタリング → 進捗報告）を**完全自動化**するプロジェクトマネージャースキルです。

**アーキテクチャ**: サブエージェント方式を採用し、各フェーズを専門エージェントに委譲します。
TDD実装後に commandmatedev 経由で Codex による「潜在バグ・セキュリティ脆弱性」レビューを挟み、品質を向上させます。

## 使用方法
- `/pm-auto-dev [Issue番号]`
- `/pm-auto-dev [Issue番号] --max-iterations=5`

## 実行内容

あなたはプロジェクトマネージャーとして、Issue開発を統括します。

### パラメータ
- **issue_number**: 開発対象のIssue番号（必須）
- **max_iterations**: 最大イテレーション回数（デフォルト: 3）

### サブエージェントモデル指定

| エージェント | モデル | 理由 |
|-------------|--------|------|
| tdd-impl-agent | **opus** | コード生成にOpus必要 |
| Codex (commandmatedev) | **codex** | 潜在バグ・セキュリティレビュー |
| acceptance-test-agent | **opus** | テスト品質にOpus必要 |
| refactoring-agent | **opus** | コード改善にOpus必要 |
| progress-report-agent | sonnet | テンプレート埋め込み程度 |

---

## 実行フェーズ

### Phase 0: 初期設定

TodoWriteツールで作業計画を作成：

```
- [ ] Phase 1: Issue情報収集
- [ ] Phase 2: TDD実装 (イテレーション 0/3)
- [ ] Phase 2.5: Codexコードレビュー（潜在バグ・セキュリティ）
- [ ] Phase 3: 受入テスト
- [ ] Phase 4: リファクタリング
- [ ] Phase 5: ドキュメント最新化
- [ ] Phase 6: 進捗報告
```

### Phase 1: Issue情報収集

```bash
gh issue view {issue_number} --json number,title,body,labels,assignees
```

ディレクトリ構造作成：
```bash
BRANCH=$(git branch --show-current)
ISSUE_NUM=$(echo "$BRANCH" | grep -oE '[0-9]+')
BASE_DIR="dev-reports/issue/${ISSUE_NUM}/pm-auto-dev/iteration-1"
mkdir -p "$BASE_DIR"
```

### Phase 2: TDD実装（イテレーション可能）

TDDコンテキストファイル作成後、サブエージェント呼び出し：
```
Use tdd-impl-agent (model: opus) to implement Issue #{issue_number} with TDD approach.
Context file: dev-reports/issue/{issue_number}/pm-auto-dev/iteration-1/tdd-context.json
Output file: dev-reports/issue/{issue_number}/pm-auto-dev/iteration-1/tdd-result.json
```

### Phase 2.5: Codexコードレビュー（潜在バグ・セキュリティ脆弱性）

TDD実装完了後、commandmatedev 経由で Codex に「潜在バグとセキュリティ脆弱性」の観点でコードレビューを依頼する。

**実行手順**:

1. **Worktree ID を取得**:
```bash
WORKTREE_ID=$(commandmatedev ls --quiet --branch "$(git branch --show-current)" 2>/dev/null | head -1)
```

2. **変更ファイル一覧を取得**:
```bash
CHANGED_FILES=$(git diff develop --name-only -- '*.rs' | tr '\n' ' ')
```

3. **レビュープロンプトを構築**:
```bash
REVIEW_PROMPT="以下の変更ファイルについて「潜在バグ」と「セキュリティ脆弱性」の観点でコードレビューを実施してください。

## レビュー観点
1. **潜在バグ**: パニック可能性（unwrap/expect）、整数オーバーフロー、off-by-oneエラー、未処理エラー、競合状態、リソースリーク
2. **セキュリティ脆弱性**: パストラバーサル、コマンドインジェクション、SQLインジェクション、unsafe使用、入力バリデーション不足、機密情報露出

## 変更ファイル
${CHANGED_FILES}

## 出力形式
結果を以下のJSON形式でファイル dev-reports/issue/{issue_number}/pm-auto-dev/iteration-1/codex-review-result.json に保存してください:
{
  \"critical\": [{\"file\": \"...\", \"line\": N, \"severity\": \"critical|high|medium|low\", \"category\": \"bug|security\", \"description\": \"...\", \"suggestion\": \"...\"}],
  \"warnings\": [{\"file\": \"...\", \"line\": N, \"severity\": \"...\", \"category\": \"...\", \"description\": \"...\", \"suggestion\": \"...\"}],
  \"summary\": \"...\",
  \"requires_fix\": true/false
}"
```

4. **Codex にレビューを送信**:
```bash
commandmatedev send "$WORKTREE_ID" "$REVIEW_PROMPT" --agent codex --auto-yes --duration 1h
```

5. **完了を待機**:
```bash
commandmatedev wait "$WORKTREE_ID" --timeout 3600
```

6. **結果を取得**:
```bash
commandmatedev capture "$WORKTREE_ID" --agent codex --json
```

7. **結果ファイルを確認**:
```bash
cat dev-reports/issue/{issue_number}/pm-auto-dev/iteration-1/codex-review-result.json
```

**レビュー結果の判定**:
- **critical が 1件以上**: 修正必須。Phase 2 に戻って修正後、再度 Phase 2.5 を実行（最大3回）
- **critical が 0件**: Phase 3（受入テスト）に進む
- **warnings**: Phase 4（リファクタリング）で対応

### Phase 3: 受入テスト

```
Use acceptance-test-agent (model: opus) to verify Issue #{issue_number}.
Context file: dev-reports/issue/{issue_number}/pm-auto-dev/iteration-1/acceptance-context.json
Output file: dev-reports/issue/{issue_number}/pm-auto-dev/iteration-1/acceptance-result.json
```

### Phase 4: リファクタリング

```
Use refactoring-agent (model: opus) to improve code quality for Issue #{issue_number}.
Context file: dev-reports/issue/{issue_number}/pm-auto-dev/iteration-1/refactor-context.json
Output file: dev-reports/issue/{issue_number}/pm-auto-dev/iteration-1/refactor-result.json
```

Phase 2.5 で検出された warnings がある場合、リファクタリング時に合わせて対応する。

### Phase 5: ドキュメント最新化

- **README.md**: 機能一覧の更新
- **CLAUDE.md**: モジュール構成の更新

### Phase 6: 進捗報告

```
Use progress-report-agent to generate progress report for Issue #{issue_number}.
Context file: dev-reports/issue/{issue_number}/pm-auto-dev/iteration-1/progress-context.json
Output file: dev-reports/issue/{issue_number}/pm-auto-dev/iteration-1/progress-report.md
```

進捗レポートには Codex コードレビューの結果サマリーも含める。

## ファイル構造

```
dev-reports/issue/{issue_number}/
├── work-plan.md
└── pm-auto-dev/
    └── iteration-1/
        ├── tdd-context.json
        ├── tdd-result.json
        ├── codex-review-result.json    ← Codex レビュー結果
        ├── acceptance-context.json
        ├── acceptance-result.json
        ├── refactor-context.json
        ├── refactor-result.json
        ├── progress-context.json
        └── progress-report.md
```

## 完了条件

- Phase 2: TDD実装成功（全テストパス、clippy警告0件）
- Phase 2.5: Codexコードレビュー完了（critical 0件）
- Phase 3: 受入テスト成功
- Phase 4: リファクタリング完了
- Phase 5: ドキュメント最新化完了
- Phase 6: 進捗レポート作成完了
