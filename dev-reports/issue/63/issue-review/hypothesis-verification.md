# 仮説検証レポート（Issue #63: Semantic Search）

## 検証結果サマリー

| 仮説 | 判定 | 説明 |
|------|------|------|
| 1. CLIオプション排他性 | Partially Confirmed | `--semantic`未実装。排他性パターンは既存(`--related`)で実績あり |
| 2a. EmbeddingProvider | Confirmed | Ollama/OpenAIプロバイダー実装済み |
| 2b. コサイン類似度検索 | Confirmed | `search_similar()`でSQLite embeddingsテーブルからtop-k検索可能 |
| 2c. Tantivyメタデータ | Confirmed | IndexReaderWrapperでメタデータ取得可能 |
| 3. フィルタ機能 | Confirmed | `--tag`, `--path`, `--type`実装済み |
| 4. 出力フォーマット | Confirmed | human/json/path形式対応済み |
| 5. 依存Issue | Confirmed | #61・#62マージ済み、基盤整備完了 |

## 詳細

### 1. CLIオプション排他性
- `src/main.rs` (line 27-59): `--semantic`オプションは未実装
- `--related`の排他性パターン（`conflicts_with_all`）が参考になる
- 新規追加時は`query`, `symbol`, `related`と排他にする

### 2. 検索フロー

#### 2a. EmbeddingProvider
- `src/embedding/mod.rs`: EmbeddingProviderトレイト定義済み
- `embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError>`
- Ollama/OpenAIプロバイダー実装済み

#### 2b. コサイン類似度検索
- `src/indexer/symbol_store.rs` (line 211): `cosine_similarity()`関数
- `search_similar()` (line 602): top-k類似度検索
- `EmbeddingSimilarityResult`: file_path, section_heading, similarity

#### 2c. Tantivyメタデータ取得
- `src/indexer/reader.rs`: SearchResult構造体（path, heading, body, tags等）
- `search_by_exact_path()`でファイルパスからメタデータ取得可能

### 3. フィルタ機能
- `SearchFilters`: path_prefix, file_type
- `SearchOptions`: query, tag, heading, limit
- Post-filterパターンで適用

### 4. 出力フォーマット
- `src/output/mod.rs`: OutputFormat enum (Human, Json, Path)
- 各形式実装済み（human.rs, json.rs, path.rs）

### 5. 依存Issue
- #61 (commit 80397b8): Embedding生成基盤追加
- #62 (commit 935dc28): Embeddingストレージ追加
- 両方マージ済み（commit d85a1b3）

## 未実装項目

1. `--semantic` CLIオプション追加
2. セマンティック検索関数実装
3. フィルタ対応（search_similar後のpost-filter）
4. セマンティック検索結果の出力フォーマット対応
