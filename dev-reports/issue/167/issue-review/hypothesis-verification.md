# 仮説検証レポート: Issue #167

## 検証対象の仮説

suggestコマンドがナレッジグラフからIssue関連の全ファイルを1件ずつcontextコマンドに展開するため、提案数が80件に膨らむ。

## 検証結果: **Confirmed**

### 根拠

1. **フィルタリングなしの展開**: `query_knowledge_graph()` (suggest.rs:221-246) がIssueに紐づく全ドキュメントを返却
2. **リレーション種別フィルタなし**: has_progress, modifies を含む全リレーションが展開対象
3. **doc_subtypeフィルタなし**: JSON成果物、stage別レビュー等も個別展開
4. **件数制限なし**: `prepend_knowledge_steps()` (suggest.rs:249-276) が全ドキュメントを1件1コマンドで展開

### 参照: before_changeコマンドの既存実装

`before_change.rs` では以下の制御が既に実装済み:
- `relation_priority()` による優先度付け (has_design=0, has_workplan=1, has_review=2, has_progress=3, modifies=4)
- `MAX_DOCS_PER_ISSUE = 2` によるIssue単位の件数制限
- `modifies` リレーションの除外フィルタ

### 改善案の妥当性

Issue記載の改善案（ドキュメント種別で優先度をつけてフィルタリング）は、既存の `before_change.rs` の実装パターンと整合しており、妥当。

### 関連コード

| コンポーネント | ファイル | 行 | 関数 |
|---|---|---|---|
| KG展開（問題箇所） | suggest.rs | 249-276 | `prepend_knowledge_steps()` |
| KGクエリ | suggest.rs | 221-246 | `query_knowledge_graph()` |
| リレーション定義 | knowledge.rs | 80-88 | `KnowledgeRelation` |
| 参照実装 | before_change.rs | 331-340 | `relation_priority()` |
| 参照実装 | before_change.rs | 349-381 | Issue単位グルーピング |
