# 仮説検証レポート: Issue #171

## 検証対象の仮説

Issue本文の主張: 「`context`コマンドはimport依存のみで関連ファイルを収集する。ナレッジグラフの設計制約・レビュー知見は含まれない。」

## 検証結果: **Rejected（否定）**

### 根拠

コードベースを確認した結果、ナレッジグラフのcontext統合は**既にmainブランチに実装済み**です。

#### 1. related.rs: ナレッジグラフスコアリング

- **L10-16**: `KNOWLEDGE_GRAPH_WEIGHT: 0.8` が定義済み
- **L255**: `find_related()` 内で `score_knowledge_graph()` が呼び出される
- **L456-474**: `score_knowledge_graph()` メソッドが `store.find_knowledge_related(target)` を呼び、結果を `KnowledgeGraph` リレーションとしてスコアに追加

#### 2. context.rs: ナレッジグラフエントリのエンリッチメント

- **L283-285**: `has_knowledge_graph` フラグで `RelationType::KnowledgeGraph` を検出
- **L292**: knowledge_graph エントリに heading と snippet を付与
- **L388-391**: `relation_to_string()` で "knowledge_graph" 文字列に変換

#### 3. symbol_store.rs: ナレッジグラフDB

- `knowledge_nodes` / `knowledge_edges` テーブルが定義済み
- `find_knowledge_related()` メソッドがファイルからIssue経由で関連ドキュメントを検索

### 潜在的な改善点

1. `relation_to_string()` で `KnowledgeGraph` の優先度が最低（他のリレーションに隠れる可能性）
2. `KNOWLEDGE_GRAPH_WEIGHT: 0.8` はIssue期待値（3.0）と乖離
3. スニペットの内容がIssue期待の「判断理由の要約」と異なる可能性（現在はbodyの先頭を切り詰めるのみ）

## 結論

基本的な統合は完了済み。Issueは既存実装の改善・拡張を意図している可能性がある。Issue本文の更新が必要。
