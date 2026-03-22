# 仮説検証レポート - Issue #65 Reranking

## 検証日: 2026-03-22

## 検証対象の仮説

### 仮説1: ハイブリッド検索がtop-N件を返せる構造になっている
- **結果: Confirmed**
- `rrf_merge(bm25_results, semantic_results, limit)` が `.take(limit)` で件数制限
- `HYBRID_OVERSAMPLING_FACTOR = 3` でセマンティック側を多めに取得

### 仮説2: Ollama APIを呼び出す基盤が存在する
- **結果: Confirmed**
- `src/embedding/ollama.rs` で `{endpoint}/api/embed` エンドポイント実装済み
- reqwest blocking クライアント使用、タイムアウト設定あり

### 仮説3: searchコマンドに新しいフラグを追加できる構造
- **結果: Confirmed**
- clap derive マクロ使用で容易に拡張可能
- `conflicts_with_all` で排他制御も可能

### 仮説4: config.toml に新セクション追加可能
- **結果: Confirmed**
- `.commandindex/config.toml` から TOML parse
- `Config` 構造体に `Option<RerankConfig>` を追加すれば拡張可能

### 仮説5: 出力フォーマットが拡張可能
- **結果: Confirmed**
- human/json/path 3形式を全結果型で対応済み
- Reranking結果も既存の `SearchResult` 型をそのまま利用可能

### 仮説6: Cross-Encoder方式でOllamaを利用可能
- **結果: Partially Confirmed**
- Ollama API基盤は存在するが、Cross-Encoder（Reranking）用のAPIエンドポイントは `/api/embed` とは異なる可能性あり
- Reranking用のOllama APIは別途調査が必要（生成APIを使った類似度スコアリング等）

## 総合判定

Issue #65 の実装前提条件は概ね揃っている。既存のEmbedding基盤（Ollama/OpenAI）のパターンを踏襲してRerankプロバイダーを実装可能。
