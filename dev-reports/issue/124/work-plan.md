# 作業計画: Issue #124

## Issue: [BUG] search --semantic がembed済みでも「No embeddings found」エラー
**Issue番号**: #124
**サイズ**: M
**優先度**: High
**依存Issue**: なし

## 概要
`search --semantic`と`try_hybrid_search()`が`symbols.db`(SymbolStore)から読み取っているのを`embeddings.db`(EmbeddingStore)に変更する。EmbeddingStoreに`search_similar()`メソッドを追加し、テストを更新する。

---

## Phase 1: EmbeddingStore拡張（コア実装）

### Task 1.1: EmbeddingStoreに型・ヘルパー追加
- **成果物**: `src/embedding/store.rs`
- **依存**: なし
- **内容**:
  - `EmbeddingSimilarityResult`型追加（file_path, section_heading, similarity）
  - `InvalidEmbedding`バリアントを`EmbeddingStoreError`に追加
  - `blob_to_embedding(blob, expected_dimension)`検証関数追加
  - `cosine_similarity(a, b)`関数追加
  - `search_similar(query_embedding, top_k)`メソッド追加
    - 全件取得→BLOBサイズ検証→dimension mismatchスキップ→cosine計算→NaN/Infフィルタ→降順ソート→top_k返却
    - カラム名マッピング: section_path → file_path

### Task 1.2: EmbeddingStoreユニットテスト追加
- **成果物**: `src/embedding/store.rs`（#[cfg(test)]モジュール内）
- **依存**: Task 1.1
- **内容**:
  - `test_search_similar_basic` — 基本的な類似度検索
  - `test_search_similar_top_k` — top_k制限
  - `test_search_similar_dimension_mismatch` — dimension不一致スキップ
  - `test_search_similar_empty_db` — 空DBの場合
  - `test_blob_to_embedding_validation` — BLOBサイズ検証
  - `test_cosine_similarity_nan_filter` — NaN/Inf結果のフィルタリング

## Phase 2: search.rs DB参照先変更（バグ修正本体）

### Task 2.1: SearchErrorにEmbeddingStoreバリアント追加
- **成果物**: `src/cli/search.rs`
- **依存**: Task 1.1
- **内容**:
  - `SearchError::EmbeddingStore(EmbeddingStoreError)`バリアント追加
  - `Display`, `source()`の各match arm追加
  - `From<EmbeddingStoreError> for SearchError`実装
    - `Sqlite`エラーの`no such table`は`NoEmbeddings`に変換（暫定策）
    - `SchemaVersionMismatch`は`EmbeddingStore`バリアントとして保持

### Task 2.2: run_semantic_search()のDB参照先変更
- **成果物**: `src/cli/search.rs`
- **依存**: Task 2.1
- **内容**:
  - `embeddings_db_path`のPath::exists()事前チェック追加
  - `SymbolStore::open()` → `EmbeddingStore::open()`
  - `store.count_embeddings()` → `emb_store.count()`
  - `store.search_similar()` → `emb_store.search_similar()`
  - `enrich_with_metadata()`の引数型を`crate::embedding::store::EmbeddingSimilarityResult`に変更
  - `enrich_semantic_to_search_results()`の引数型も同様に変更

### Task 2.3: try_hybrid_search()のDB参照先変更
- **成果物**: `src/cli/search.rs`
- **依存**: Task 2.1
- **内容**:
  - `symbol_db_path()` → `embeddings_db_path()`
  - `SymbolStore::open()` → `EmbeddingStore::open()`（Path::exists()事前チェック付き）
  - `store.count_embeddings()` → `emb_store.count()`
  - `store.search_similar()` → `emb_store.search_similar()`
  - エラーハンドリング: `EmbeddingStoreError`をgraceful fallback（BM25 only）に変換

### Task 2.4: suggest.rsのDB参照先変更
- **成果物**: `src/cli/suggest.rs`
- **依存**: Task 1.1
- **内容**:
  - `maybe_add_semantic_step()`内の`SymbolStore::count_embeddings()` → `EmbeddingStore::count()`
  - `run_suggest()`/`build_strategy()`のストア初期化変更（必要に応じてシグネチャ変更）

## Phase 3: テスト更新

### Task 3.1: e2e_semantic_hybrid.rsのEmbeddingStore切替
- **成果物**: `tests/e2e_semantic_hybrid.rs`
- **依存**: Task 2.2, 2.3
- **内容**:
  - `SymbolStore::insert_embeddings()` → `EmbeddingStore::upsert_embedding()`に切替
  - 対象テスト:
    1. `test_embedding_insert_and_count`
    2. `test_semantic_search_basic`
    3. `test_semantic_search_top_k`
    4. `test_context_with_embeddings`
    5. `test_hybrid_auto_switch` (ignored)
    6. `test_hybrid_bm25_fallback` (ignored)
  - 不要なSymbolStore import削除

## Phase 4: 品質チェック・最終確認

### Task 4.1: 品質チェック
- **依存**: 全タスク
- **内容**:
  - `cargo build` — エラー0件
  - `cargo clippy --all-targets -- -D warnings` — 警告0件
  - `cargo test --all` — 全テストパス
  - `cargo fmt --all -- --check` — 差分なし

---

## Definition of Done

- [ ] EmbeddingStoreにsearch_similar()メソッドが追加されている
- [ ] run_semantic_search()がembeddings.dbから読み取っている
- [ ] try_hybrid_search()がembeddings.dbから読み取っている
- [ ] suggest.rsがembeddings.dbから読み取っている
- [ ] embeddings.db未存在時の異常系契約が維持されている
- [ ] 全テストパス、clippy警告0件
- [ ] AC-1〜AC-6の受け入れ基準を満たしている
