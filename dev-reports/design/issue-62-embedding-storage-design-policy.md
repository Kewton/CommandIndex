# 設計方針書: Issue #62 Embeddingストレージ（SQLite ベクトル格納）

## 1. Issue概要

| 項目 | 内容 |
|------|------|
| Issue番号 | #62 |
| タイトル | [Feature] Embeddingストレージ（SQLite ベクトル格納） |
| 種別 | enhancement |
| 目的 | Semantic Search実現のためのEmbeddingベクトル永続化・類似度検索基盤 |

## 2. システムアーキテクチャ上の位置づけ

```
┌─────────────┐    ┌──────────────┐    ┌──────────────────────────┐
│  CLI Layer   │───▶│ Indexer Layer │───▶│ SymbolStore (SQLite)     │
│ src/cli/     │    │ src/indexer/  │    │ symbols.db               │
└─────────────┘    └──────────────┘    │  ├── symbols              │
                                       │  ├── dependencies          │
                                       │  ├── file_links            │
                                       │  ├── embeddings  ← NEW    │
                                       │  └── schema_meta           │
                                       └──────────────────────────┘
```

本Issue は **Indexer Layer** に新テーブル `embeddings` を追加する変更。既存の `symbol_store.rs` 内のテーブル定義・スキーマバージョン管理・delete_by_file()を拡張しつつ、Embedding固有のCRUD・検索ロジックは `symbol_store.rs` 内に追加する。

> **SRP考慮**: レビューでEmbeddingStoreの分離が提案されたが、現時点では既存SymbolStoreへの追加とする。理由: (1) 同一SQLiteファイル・同一Connection共有のため分離のメリットが薄い、(2) delete_by_fileのトランザクション一貫性を保つため同一構造体が有利、(3) YAGNIの観点で責務が明確に膨らんだ段階でリファクタリングする。

## 3. レイヤー構成と責務

| レイヤー | 変更対象 | 変更内容 |
|---------|---------|---------|
| **Indexer** | `src/indexer/symbol_store.rs` | embeddingsテーブル追加、CRUD操作、コサイン類似度検索 |
| CLI | なし | 本Issueでは変更なし（将来のsemantic searchコマンドで利用） |
| Parser | なし | Embedding生成は別Issue |
| Search | なし | 将来のsemantic search統合で利用 |

## 4. 技術選定

| 項目 | 選定 | 理由 |
|------|------|------|
| ベクトル格納 | SQLite BLOB | 既存のsymbols.dbを拡張。追加依存なし |
| BLOB変換 | `f32::to_le_bytes()` / `f32::from_le_bytes()` | 標準ライブラリのみ。byteorder crate不要 |
| エンディアン | リトルエンディアン固定 | クロスプラットフォーム互換性 |
| 類似度検索 | ブルートフォース（コサイン類似度） | 想定規模（数千ベクトル）では十分。ANN不要 |
| タイムスタンプ | `chrono::Utc::now().to_rfc3339()` | 既存依存を利用。ISO 8601形式 |
| 新規crate依存 | なし | 標準ライブラリ + 既存rusqlite/chronoのみ |

## 5. データモデル設計

### 5.1 スキーマ

```sql
CREATE TABLE IF NOT EXISTS embeddings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_path TEXT NOT NULL,
    section_heading TEXT NOT NULL DEFAULT '',  -- 空文字 = ファイル全体
    embedding BLOB NOT NULL,                   -- f32配列をLE BLOBで格納
    dimension INTEGER NOT NULL,                -- ベクトル次元数
    model_name TEXT NOT NULL,                  -- 生成に使用したモデル名
    file_hash TEXT NOT NULL,                   -- キャッシュ用ハッシュ
    created_at TEXT NOT NULL                   -- デバッグ・診断用
);

CREATE INDEX IF NOT EXISTS idx_embeddings_path ON embeddings(file_path);
CREATE INDEX IF NOT EXISTS idx_embeddings_hash ON embeddings(file_hash);
CREATE UNIQUE INDEX IF NOT EXISTS idx_embeddings_path_section ON embeddings(file_path, section_heading);
```

**設計判断**:
- `section_heading` は `NOT NULL DEFAULT ''`（NULL不使用）: SQLiteのユニーク制約でNULL同士が等しいとみなされないため
- 複合ユニーク制約 `(file_path, section_heading)`: 重複防止の安全弁（delete-before-insertに加え）
- `created_at`: デバッグ・診断用。TTLや自動無効化には使用しない

### 5.2 Rust構造体

```rust
/// Embedding格納用の情報構造体
#[derive(Debug, Clone, PartialEq)]
pub struct EmbeddingInfo {
    pub id: Option<i64>,            // DB上のID（挿入時はNone）
    pub file_path: String,
    pub section_heading: String,    // 空文字 = ファイル全体
    pub embedding: Vec<f32>,
    pub model_name: String,
    pub file_hash: String,
}

/// コサイン類似度検索の結果構造体
#[derive(Debug, Clone, PartialEq)]
pub struct EmbeddingSimilarityResult {
    pub file_path: String,
    pub section_heading: String,
    pub similarity: f32,
}
```

### 5.3 BLOB変換

```rust
/// Vec<f32> → BLOB (LE bytes)
fn embedding_to_blob(embedding: &[f32]) -> Vec<u8> {
    let mut blob = Vec::with_capacity(embedding.len() * 4);
    for &val in embedding {
        blob.extend_from_slice(&val.to_le_bytes());
    }
    blob
}

/// BLOB → Vec<f32>（バリデーション付き）
fn blob_to_embedding(blob: &[u8], expected_dimension: usize) -> Result<Vec<f32>, SymbolStoreError> {
    if blob.len() != expected_dimension * 4 {
        return Err(SymbolStoreError::InvalidEmbedding {
            reason: format!("BLOB size {} != expected {}", blob.len(), expected_dimension * 4),
        });
    }
    Ok(blob.chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect())
}
```

## 6. API設計

### 6.1 インサート

```rust
/// バルクインサート（既存パターン踏襲）
/// 注意: 呼び出し前にdelete_by_file()で既存レコードを削除する前提。
/// UNIQUE制約(file_path, section_heading)があるため、削除せずに呼ぶとINSERT OR REPLACEで上書きされる。
pub fn insert_embeddings(&self, embeddings: &[EmbeddingInfo]) -> Result<(), SymbolStoreError>
```

- トランザクション内でバルクインサート（`INSERT OR REPLACE`を使用）
- 入力バリデーション: 空ベクトル・NaN/Infinity値を含むベクトルはエラー
- dimensionはembedding.len()から自動計算
- created_atは挿入時にchrono::Utc::now().to_rfc3339()を使用（ISO 8601形式）

### 6.2 検索

```rust
/// コサイン類似度によるtop-k検索
pub fn search_similar(
    &self,
    query_embedding: &[f32],
    top_k: usize,
) -> Result<Vec<EmbeddingSimilarityResult>, SymbolStoreError>
```

- 全ベクトルをSQLiteからロード（想定上限: ~10,000ベクトル、384次元で約15MB）
- BLOB読み出し後にblob.len() == dimension * 4のバリデーション実施。不正レコードはスキップ+warn
- dimension不一致のベクトルは結果から除外（0.0で返すのではなくフィルタ）+warnログ
- Rust側でコサイン類似度を計算
- 上位top_k件をソートして返却

### 6.3 削除

```rust
/// 既存のdelete_by_file()を拡張
pub fn delete_by_file(&self, file_path: &str) -> Result<(), SymbolStoreError>
// → DELETE FROM embeddings WHERE file_path = ?1 を追加
```

### 6.4 コサイン類似度計算

```rust
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    // 前提: a.len() == b.len() （呼び出し側でフィルタ済み）
    // dot(a, b) / (||a|| * ||b||)
    // ゼロベクトル時は 0.0 を返す
}
```

## 7. エッジケース対応

| ケース | 対応 |
|--------|------|
| ゼロベクトル（ノルム=0） | 類似度 0.0 を返す |
| dimension不一致 | 結果から除外 + warnログ出力 |
| BLOBサイズ不正 | blob_to_embedding()でResult::Errを返しスキップ + warnログ |
| 空のembeddings配列 | 空のVecを返す |
| NaN/Infinity入力 | insert_embeddings()でバリデーションエラー |
| 空ベクトル入力 | insert_embeddings()でバリデーションエラー |

## 8. スキーマバージョン管理

| 項目 | 変更 |
|------|------|
| `CURRENT_SYMBOL_SCHEMA_VERSION` | 2 → 3 |
| 移行方式 | clean → index で再構築（マイグレーション不要） |
| 既存DB対応 | SchemaVersionMismatchエラー → ユーザーにclean提案 |

## 9. 影響範囲

### 直接変更ファイル

| ファイル | 変更内容 |
|---------|---------|
| `src/indexer/symbol_store.rs` | embeddingsテーブル、構造体、CRUD、検索メソッド追加 |

### 間接影響（変更不要）

| ファイル | 影響 | 対応 |
|---------|------|------|
| `src/cli/index.rs` | delete_by_fileの呼び出し元 | API互換のため変更不要 |
| `src/cli/status.rs` | SymbolStore::open()を使用 | SchemaVersionMismatch処理は既存で対応済み |
| `tests/e2e_*.rs` | スキーマバージョン変更 | run_index()が新DBを作成するため壊れない |
| `tests/cli_index.rs` | state.json schema_version=1をアサート | symbols.db側のみ変更のため影響なし |
| `tests/e2e_phase3_integration.rs` | SymbolStore::open()直接呼び出し | run_index()で新規v3 DB作成後のため問題なし |
| `Cargo.toml` | 依存追加 | 変更不要（既存依存のみ使用） |

**incremental index時の影響**: 既存v2のsymbols.dbに対してv3でopenするとSchemaVersionMismatchが発生 → IndexError経由でユーザーにclean提案メッセージが表示。既存設計通りの動作であり追加対応不要。

## 10. テスト戦略

### 単体テスト（symbol_store.rs内）

| テスト | 内容 |
|--------|------|
| `test_create_embeddings_table` | テーブル作成の確認 |
| `test_insert_and_search_embeddings` | BLOB格納→読み出し→コサイン類似度検索 |
| `test_embedding_blob_roundtrip` | Vec<f32> ↔ BLOB変換の正確性 |
| `test_delete_by_file_removes_embeddings` | delete_by_fileでembeddings削除 |
| `test_cosine_similarity_zero_vector` | ゼロベクトルのエッジケース |
| `test_cosine_similarity_dimension_mismatch` | dimension不一致のスキップ |
| `test_embedding_unique_constraint` | file_path+section_headingの重複防止 |
| `test_schema_version_incremented` | v3のバージョン確認 |
| `test_insert_embedding_validation` | 空ベクトル・NaN/Infinityのバリデーション |
| `test_blob_to_embedding_invalid_size` | 不正BLOBサイズのエラー |
| `test_delete_by_file_cascade_with_embeddings` | 4テーブル一括削除の統合テスト |

### 既存テストの影響

- 既存テストはすべて壊れない見込み（create_tables()のIF NOT EXISTS、open_in_memory()パターン踏襲）
- tests/cli_index.rs: state.json側のschema_version=1は変更しないため影響なし
- tests/e2e_phase3_integration.rs: run_index()がv3 DBを新規作成するため問題なし

## 11. セキュリティ設計

| 脅威 | 対策 | 優先度 |
|------|------|--------|
| SQLインジェクション | パラメータバインディング使用（params!マクロ） | 高 |
| 不正なBLOBデータ | BLOBサイズバリデーション（読み出し時にdimension*4チェック） | 高 |
| 不正な入力値 | insert時にNaN/Infinity/空ベクトルをバリデーション | 高 |
| unsafe使用 | 禁止（標準ライブラリのみ使用） | 高 |

## 12. 設計判断とトレードオフ

| 判断 | 選択肢 | 決定 | 理由 |
|------|--------|------|------|
| ベクトル格納方式 | BLOB vs JSON | BLOB | パフォーマンス（BLOBは4byte/float、JSONは数倍） |
| 類似度検索方式 | ブルートフォース vs ANN | ブルートフォース | 想定規模（数千ベクトル）では十分。ANN移行コストは低い |
| 依存追加 | byteorder vs 標準ライブラリ | 標準ライブラリ | 依存最小化。f32::to_le_bytes()で十分 |
| NULL vs 空文字 | section_heading NULL許可 vs NOT NULL | NOT NULL DEFAULT '' | ユニーク制約の一貫性 |
| エラー型 | 既存enum拡張 vs 新エラー型 | 既存SymbolStoreError + InvalidEmbeddingバリアント追加 | 入力バリデーション・BLOBバリデーション用 |
| UPSERT戦略 | DELETE→INSERT vs INSERT OR REPLACE | INSERT OR REPLACE | ユニーク制約との整合性。delete_by_fileとの併用も安全 |
| SRP分離 | EmbeddingStore分離 vs SymbolStore拡張 | SymbolStore拡張 | 同一Connection共有・トランザクション一貫性のため。肥大化時にリファクタリング |

## 13. 品質基準

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |
