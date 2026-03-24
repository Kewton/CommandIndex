# 仮説検証レポート: Issue #142 `before-change` コマンド

## 検証サマリー

| # | 仮説 | 判定 | 補足 |
|---|------|------|------|
| 1 | knowledge_nodes/edges 実装済み | ✅ CONFIRMED | symbol_store.rs に完全実装 |
| 2 | BGE-M3 セマンティック検索動作 | ✅ CONFIRMED | ollama.rs で対応済み（#134） |
| 3 | find_issues_for_file メソッド | ❌ REJECTED | 代わりに `find_knowledge_related()` が実装済み |
| 4 | get_embedding メソッド | ❌ REJECTED | 代わりに `find_by_path()` + `search_similar()` |
| 5 | cosine_similarity 関数 | ✅ CONFIRMED | store.rs:140, symbol_store.rs:211 |
| 6 | CLI サブコマンド構造 | ⚠️ PARTIALLY CONFIRMED | 15個実装済み、before-change は未実装 |
| 7 | 出力フォーマット | ✅ CONFIRMED | human/json/path/llm すべて実装 |

## 詳細

### 仮説1: ナレッジグラフ
- `src/indexer/symbol_store.rs:335-361` にテーブル定義
- メソッド: `upsert_knowledge_node()`, `upsert_knowledge_edge()`, `find_knowledge_related()` 等

### 仮説2: BGE-M3
- `src/embedding/ollama.rs:76` で `qllama/bge-m3:q8_0` 対応

### 仮説3: グラフ走査メソッド
- Issue記載の `find_issues_for_file`, `find_docs_for_issues` は存在しない
- 代わりに `find_knowledge_related(file_path)` が2ホップ走査を一括実行

### 仮説4: Embedding取得
- `get_embedding` は存在しない
- `find_by_path(path)` で embedding レコード取得可能
- `search_similar(query_embedding, top_k)` でコサイン類似度検索可能

### 仮説5: cosine_similarity
- `embedding/store.rs:140-151` と `indexer/symbol_store.rs:211-220` の2箇所に実装

### 仮説6: CLIサブコマンド
- 15個のサブコマンドが実装済み（Index, Search, Update, Status, Clean, Diff, Context, Embed, Config, Export, Impact, Import, HelpLlm, Suggest, Watch）
- `before-change` は未実装

### 仮説7: 出力フォーマット
- `OutputFormat` enum: Human, Json, Path, Llm
- 各フォーマットに対応する出力モジュール実装済み

## Issueへの修正提案

Issue内の擬似コードでは存在しないメソッド名が使われているため、実際のAPIに合わせて修正が必要:
- `KnowledgeGraph::open()` → `SymbolStore::open()`
- `graph.find_issues_for_file()` → `symbol_store.find_knowledge_related()`
- `store.get_embedding()` → `store.find_by_path()`
