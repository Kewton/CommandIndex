# 仮説検証レポート - Issue #135

## 検証日: 2026-03-24

## 検証結果サマリー

| # | 仮説 | ファイル | 行 | 判定 |
|---|------|---------|-----|------|
| 1 | Ollama BATCH_SIZE=10 | `src/embedding/ollama.rs` | 12 | **Confirmed** |
| 2 | embed.rs 183-199行でembedding格納 | `src/cli/embed.rs` | 183-199 | **Confirmed** |
| 3 | store.rsでSQLite操作 | `src/embedding/store.rs` | 全体 | **Confirmed** |
| 4 | OpenAI BATCH_SIZE=100 | `src/embedding/openai.rs` | 12 | **Confirmed** |
| 5 | SQLite書き込みがautocommit | `src/embedding/store.rs` | 243-267 | **Confirmed** |

## 詳細

### 仮説1: Ollama BATCH_SIZE=10
- **実コード**: `const BATCH_SIZE: usize = 10;` (ollama.rs:12)
- **判定**: Confirmed

### 仮説2: embed.rs 183-199行でembedding格納
- **実コード**: `store.upsert_embedding()` をループ内で個別呼び出し
- **判定**: Confirmed

### 仮説3: store.rsでSQLite操作
- **実コード**: `EmbeddingStore` 構造体、`upsert_embedding()`, `find_by_path()`, `search_similar()` 等
- **判定**: Confirmed

### 仮説4: OpenAI BATCH_SIZE=100
- **実コード**: `const BATCH_SIZE: usize = 100;` (openai.rs:12)
- **判定**: Confirmed

### 仮説5: SQLite autocommit
- **実コード**: `upsert_embedding()` は `self.conn.execute()` を直接呼び出し、トランザクション管理なし
- **判定**: Confirmed

## 結論

Issue内の仮説は全て正確。実装方針は妥当。
