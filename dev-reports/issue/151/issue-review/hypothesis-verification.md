# 仮説検証レポート: Issue #151

## 検証日: 2026-03-25

## 仮説1: `why` コマンドが常に空を返す

**判定: Confirmed**

`src/cli/why.rs` の `find_knowledge_related(file_path)` は、knowledge_nodesテーブルで file_path を持つドキュメントノードを起点に、issueノード→兄弟ドキュメントノードを辿る。現在 `file` タイプのノードは存在せず、ソースコードファイルはドキュメントノードとして登録されていないため、常に空結果を返す。

## 仮説2: `before-change` がナレッジグラフを活用できない

**判定: Partially Confirmed**

`src/cli/before_change.rs` は git log からコミットメッセージ中のIssue番号を抽出し（`extract_issues_from_git_log`）、それを基に `find_knowledge_by_issue` でナレッジグラフを検索する。git log にIssue番号が含まれていれば動作するが、modifiesエッジがあれば直接ファイル→Issue紐づけが可能になり精度が向上する。

## 仮説3: git log からIssue番号抽出が可能

**判定: Confirmed**

`src/cli/before_change.rs` (lines 143-213) に既存の実装あり。`ISSUE_RE` 正規表現で `#123`, `(#123)`, `fixes #123`, `refs #123` パターンを抽出。`git log --format=%s%n%b -- {file}` でファイル単位のコミットメッセージを取得。

## 現状のナレッジグラフ構造

### ノードタイプ (`src/indexer/knowledge.rs`)
- `issue`: Issue番号をidentifierとして持つ
- `document`: ファイルパスをidentifier/file_pathとして持つ
- `file`: **未実装**

### エッジタイプ (`KnowledgeRelation` enum)
- `HasDesign`: Issue → 設計ドキュメント
- `HasReview`: Issue → レビュードキュメント
- `HasWorkplan`: Issue → 作業計画ドキュメント
- `modifies`: **未実装**

### 関連クエリメソッド (`src/indexer/symbol_store.rs`)
- `find_knowledge_by_issue()` (lines 936-998): Issue番号 → 関連ドキュメント
- `find_knowledge_related()` (lines 1000-1031): ファイルパス → 同一Issue配下の兄弟ドキュメント

### ナレッジエントリ抽出 (`src/indexer/knowledge.rs`, lines 139-176)
dev-reports/ のパスパターンから自動抽出。ソースコードファイルは対象外。

## 実装に必要な変更箇所

1. `KnowledgeRelation` に `Modifies` バリアント追加
2. ノードタイプに `file` 追加
3. git log / ブランチ名からのmodifiesエッジ抽出ロジック
4. `find_knowledge_related()` クエリの拡張（fileノード経由の検索）
5. `insert_knowledge_entries()` の拡張
