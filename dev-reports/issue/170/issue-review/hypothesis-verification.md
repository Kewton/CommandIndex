# 仮説検証レポート: Issue #170

## 検証結果サマリー

| 仮説 | 判定 | 詳細 |
|------|------|------|
| ファイル名から日付抽出 | **Partially Confirmed** | Stage Reviewファイルのみ `YYYY-MM-DD` プレフィックスあり。他のファイルにはなし |
| パスからgit logの最終コミット日 | **Confirmed (未実装)** | 実装可能だが現在メカニズムなし |
| git log由来の日付 | **Confirmed (未実装)** | `--format=%ai` 追加で実現可能 |
| `--timeline` オプション | **Confirmed (未実装)** | 日付フィールド追加後に実装可能 |

## 詳細検証

### 仮説1: ファイル名からの日付抽出

- **Stage Reviewファイルのみ** 日付プレフィックスあり: `dev-reports/review/YYYY-MM-DD-issueN-*.md`
- パターン定義: `src/indexer/knowledge.rs:407-413`
- 他のファイル（design-policy, work-plan, progress-report等）には日付プレフィックスなし

### 仮説2-3: git log由来の日付

- 現在の git log 処理: `src/indexer/knowledge.rs:237-357` (`extract_file_modifies_from_git_log()`)
- `--format` に `%ai` がなく、日付情報は取得していない
- 追加で実装可能

### 仮説4: timeline オプション

- 現在未実装。日付フィールド追加後に実装可能

## コア問題の特定

1. **メタデータに日付なし**: `knowledge_edges.metadata` に `doc_subtype` のみ (`src/indexer/symbol_store.rs:819-820`)
2. **JSON出力構造体に日付フィールドなし**: `WhyDocumentEntry`, `IssueDocumentEntry` に日付フィールドなし (`src/output/mod.rs:410-416`)
3. **日付取得メカニズムなし**: ファイル名パース時に日付抽出ロジックがない

## 主要コード箇所

| 箇所 | ファイル | 行 |
|------|---------|-----|
| WhyDocumentEntry | src/output/mod.rs | 410-416 |
| IssueDocumentEntry | src/cli/issue.rs | 56-60 |
| メタデータ設定 | src/indexer/symbol_store.rs | 819-820 |
| メタデータ取得(issue) | src/indexer/symbol_store.rs | 859-916 |
| メタデータ取得(why) | src/indexer/symbol_store.rs | 1060-1105 |
| パス解析 | src/indexer/knowledge.rs | 422-440 |
| ファイルパターン定義 | src/indexer/knowledge.rs | 372-413 |
| git log処理 | src/indexer/knowledge.rs | 237-357 |
