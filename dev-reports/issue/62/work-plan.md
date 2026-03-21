# 作業計画書: Issue #62 Embeddingストレージ

## Issue概要
- **Issue番号**: #62
- **タイトル**: [Feature] Embeddingストレージ（SQLite ベクトル格納）
- **サイズ**: M
- **優先度**: High
- **依存Issue**: なし（Phase 4完了が前提）
- **変更ファイル**: `src/indexer/symbol_store.rs` のみ

---

## 実装タスク

### Phase 1: データモデル・型定義

#### Task 1.1: エラー型拡張
- **成果物**: `src/indexer/symbol_store.rs` - SymbolStoreError enum
- **内容**:
  - `InvalidEmbedding { reason: String }` バリアントをSymbolStoreErrorに追加
  - Display, Error, source()の実装を拡張
- **依存**: なし
- **テスト**: コンパイル確認

#### Task 1.2: データ構造体定義
- **成果物**: `src/indexer/symbol_store.rs` - 構造体定義
- **内容**:
  - `EmbeddingInfo` 構造体（id, file_path, section_heading, embedding, model_name, file_hash）
  - `EmbeddingSimilarityResult` 構造体（file_path, section_heading, similarity）
  - `embedding_from_row()` ヘルパー関数（既存パターン踏襲）
- **依存**: Task 1.1
- **テスト**: コンパイル確認

#### Task 1.3: BLOB変換関数
- **成果物**: `src/indexer/symbol_store.rs` - ヘルパー関数
- **内容**:
  - `embedding_to_blob(embedding: &[f32]) -> Vec<u8>` （LE変換）
  - `blob_to_embedding(blob: &[u8], expected_dimension: usize) -> Result<Vec<f32>, SymbolStoreError>` （バリデーション付き）
  - `cosine_similarity(a: &[f32], b: &[f32]) -> f32` （ゼロベクトル対応）
- **依存**: Task 1.1
- **テスト**:
  - `test_embedding_blob_roundtrip`
  - `test_blob_to_embedding_invalid_size`
  - `test_cosine_similarity_zero_vector`
  - `test_cosine_similarity_dimension_mismatch`（同次元前提だが呼び出し側テスト）

### Phase 2: スキーマ・テーブル定義

#### Task 2.1: スキーマバージョンインクリメント
- **成果物**: `src/indexer/symbol_store.rs` - 定数変更
- **内容**:
  - `CURRENT_SYMBOL_SCHEMA_VERSION` を 2 → 3 に変更
- **依存**: なし
- **テスト**: `test_schema_version_incremented`

#### Task 2.2: embeddingsテーブルCREATE追加
- **成果物**: `src/indexer/symbol_store.rs` - create_tables()拡張
- **内容**:
  - CREATE TABLE IF NOT EXISTS embeddings (...)
  - CREATE INDEX IF NOT EXISTS idx_embeddings_path
  - CREATE INDEX IF NOT EXISTS idx_embeddings_hash
  - CREATE UNIQUE INDEX IF NOT EXISTS idx_embeddings_path_section
- **依存**: Task 2.1
- **テスト**: `test_create_embeddings_table`

### Phase 3: CRUD操作

#### Task 3.1: insert_embeddings
- **成果物**: `src/indexer/symbol_store.rs` - メソッド追加
- **内容**:
  - 入力バリデーション（空ベクトル、NaN/Infinity検査）
  - トランザクション内でINSERT OR REPLACE
  - dimensionはembedding.len()から自動計算
  - created_atはchrono::Utc::now().to_rfc3339()
- **依存**: Task 1.2, 1.3, 2.2
- **テスト**:
  - `test_insert_and_search_embeddings`（Phase 4と合わせて）
  - `test_insert_embedding_validation`
  - `test_embedding_unique_constraint`

#### Task 3.2: delete_by_file拡張
- **成果物**: `src/indexer/symbol_store.rs` - 既存メソッド拡張
- **内容**:
  - 既存トランザクション内にDELETE FROM embeddings WHERE file_path = ?1を追加
- **依存**: Task 2.2
- **テスト**:
  - `test_delete_by_file_removes_embeddings`
  - `test_delete_by_file_cascade_with_embeddings`

### Phase 4: 検索機能

#### Task 4.1: search_similar
- **成果物**: `src/indexer/symbol_store.rs` - メソッド追加
- **内容**:
  - 全embeddingsをSQLiteからロード
  - BLOBバリデーション（不正レコードスキップ+warn）
  - dimension不一致フィルタ+warn
  - コサイン類似度計算
  - top_k件をソートして返却
- **依存**: Task 1.3, 3.1
- **テスト**: `test_insert_and_search_embeddings`

### Phase 5: 品質チェック・既存テスト確認

#### Task 5.1: 全テスト実行・品質チェック
- **内容**:
  - `cargo build` → エラー0件
  - `cargo clippy --all-targets -- -D warnings` → 警告0件
  - `cargo test --all` → 全テスト（既存+新規）パス
  - `cargo fmt --all -- --check` → 差分なし
- **依存**: Phase 1-4 すべて

---

## TDD実装順序

設計方針書のテスト戦略に基づき、以下の順序でRed→Green→Refactorサイクルを回す:

1. **BLOB変換テスト** → BLOB変換関数実装
2. **コサイン類似度テスト** → cosine_similarity実装
3. **エラー型テスト** → InvalidEmbeddingバリアント追加
4. **スキーマバージョンテスト** → バージョンインクリメント + テーブル定義
5. **テーブル作成テスト** → create_tables()拡張
6. **インサート+バリデーションテスト** → insert_embeddings実装
7. **ユニーク制約テスト** → INSERT OR REPLACE確認
8. **削除テスト** → delete_by_file拡張
9. **検索テスト** → search_similar実装
10. **統合削除テスト** → 4テーブル一括削除確認

---

## Definition of Done

- [ ] すべてのPhase 1-5タスクが完了
- [ ] 11件の新規テストが全パス
- [ ] 既存テストが全パス（リグレッションなし）
- [ ] cargo clippy 警告ゼロ
- [ ] cargo fmt 差分なし
- [ ] `CURRENT_SYMBOL_SCHEMA_VERSION` が 3 に更新されている

---

## 次のアクション

1. 現在のブランチ `feature/issue-62-embedding-storage` で実装
2. `/pm-auto-dev 62` でTDD自動開発を実行
3. 完了後 `/create-pr` でPR作成
