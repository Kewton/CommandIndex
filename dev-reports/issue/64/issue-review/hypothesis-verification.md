# Issue #64 仮説検証レポート

## 検証対象
Issue #64「Hybrid Retrieval（BM25 + Semantic統合検索）」

## 検証結果サマリー

| 仮説 | 状態 | 信頼度 |
|-----|------|--------|
| BM25検索実装 | ✅ Confirmed | 100% |
| Semantic Search実装 | ✅ Confirmed | 100% |
| Embedding検出メカニズム | ✅ Confirmed | 100% |
| CLIオプション構造 | ✅ Confirmed | 100% |
| 出力フォーマット(human/json/path) | ✅ Confirmed | 100% |
| searchモジュール構造 | ✅ Confirmed | 100% |
| RRF実装 | ❌ Not Found | 100% |
| ハイブリッド用CLIオプション | ❌ Not Found | 100% |
| 自動フォールバック | ⚠️ Partially Confirmed | 80% |

## 詳細

### BM25検索
- `src/indexer/reader.rs` の `IndexReaderWrapper::search_with_options()` で実装済み
- tantivy BM25スコアリング、`score: f32` を保持

### Semantic Search (#63)
- `src/cli/search.rs` の `run_semantic_search()` で実装済み
- `SymbolStore::search_similar()` でコサイン類似度検索
- `--semantic` オプションで明示指定が必要

### Embedding存在確認
- `SymbolStore::count_embeddings()` で確認可能
- 不在時は `SearchError::NoEmbeddings` を返す
- ストレージ: `.commandindex/embeddings.db` (SQLite)

### CLI構造
- 4つの相互排他的検索モード: query, --symbol, --related, --semantic
- 共通オプション: format, tag, path, file_type, heading, limit
- `--no-semantic`, `--bm25-weight`, `--semantic-weight` は未定義

### スコア型の相違
- BM25: `score: f32` (0〜任意値)
- Semantic: `similarity: f32` (0〜1, コサイン類似度)
- RRFではランクベースなのでスコア正規化不要

### 実装可能性: ✅ 十分に実行可能
- BM25/Semantic両方が完全実装済み
- CLIオプション追加基盤あり
- 出力フォーマット拡張は既存パターン踏襲可能
