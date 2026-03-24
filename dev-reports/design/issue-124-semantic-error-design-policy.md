# 設計方針書: Issue #124 - search --semantic がembed済みでも「No embeddings found」エラー

## 1. Issue概要

| 項目 | 内容 |
|------|------|
| Issue番号 | #124 |
| 種別 | BUG |
| 深刻度 | 高 |
| タイトル | search --semantic がembed済みでも「No embeddings found」エラー |

### 根本原因
`embed`コマンドは`EmbeddingStore`経由で`embeddings.db`にembeddingを書き込むが、`search --semantic`と`try_hybrid_search()`は`SymbolStore`経由で`symbols.db`のembeddingsテーブルを参照している。`symbols.db`のembeddingsテーブルにはデータが投入されないため、常に0件となりエラーが発生する。

## 2. システムアーキテクチャ概要

### 現行のembedding参照関係（バグ状態）

```
embed command ──write──> embeddings.db (EmbeddingStore)
status command ──read──> embeddings.db (EmbeddingStore) ✓ 正常
search --semantic ──read──> symbols.db (SymbolStore) ✗ 空テーブル
search (hybrid) ──read──> symbols.db (SymbolStore) ✗ 空テーブル
```

### 修正後のembedding参照関係

```
embed command ──write──> embeddings.db (EmbeddingStore)
status command ──read──> embeddings.db (EmbeddingStore)
search --semantic ──read──> embeddings.db (EmbeddingStore) ✓
search (hybrid) ──read──> embeddings.db (EmbeddingStore) ✓
enrich_with_metadata ──read──> symbols.db (SymbolStore) ※メタデータ補完のみ
```

## 3. レイヤー構成と変更対象

| レイヤー | モジュール | 変更 | 責務 |
|---------|-----------|------|------|
| **Embedding Store** | `src/embedding/store.rs` | **変更** | `search_similar()`, `EmbeddingSimilarityResult`型追加 |
| **CLI Search** | `src/cli/search.rs` | **変更** | DB参照先変更、エラー型変換追加 |
| **CLI Suggest** | `src/cli/suggest.rs` | **変更** | `maybe_add_semantic_step()`のDB参照先変更 |
| **Indexer** | `src/indexer/symbol_store.rs` | 変更なし | embedding関連メソッドは残置（後続Issueで対応） |
| **CLI Embed** | `src/cli/embed.rs` | 変更なし | |
| **CLI Status** | `src/cli/status/mod.rs` | 変更なし | |
| **Tests** | `tests/e2e_semantic_hybrid.rs` | **変更** | EmbeddingStore使用に切替 |

## 4. 設計判断とトレードオフ

### 判断1: searchの読み取り先をEmbeddingStoreに変更する（推奨案A）

**選択**: `search`コマンドの読み取り先を`symbols.db`から`embeddings.db`に変更する

**理由**:
- 最小変更で済む（embedコマンド側は変更不要）
- EmbeddingStoreに必要なAPIを追加するだけで実現可能
- statusコマンドと同じデータソースを参照するため、一貫性が向上

**却下した代替案**: embedコマンドの書き込み先をsymbols.dbに統合する（案B）
- 影響範囲が大きい（embed, index, statusすべてに影響）
- 別Issueでリファクタリングとして対応

### 判断2: EmbeddingSimilarityResult型の配置

**選択**: `src/embedding/store.rs`に新規定義する

**理由**:
- SymbolStoreの`EmbeddingSimilarityResult`と同等のフィールドを持つが、カラム名が異なる（`section_path` vs `file_path`）
- 検索結果として返すため`file_path`フィールド名を使用（SymbolStore側と同じインターフェース）
- SymbolStore側の型は残置（後方互換性）

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct EmbeddingSimilarityResult {
    pub file_path: String,       // section_pathカラムから取得
    pub section_heading: String,
    pub similarity: f32,
}
```

### 判断3: 異常系契約の維持

**選択**: embeddings.db未存在・テーブル未作成時の挙動を既存契約と同等に維持

| 関数 | embeddings.db未存在時 | embeddings.db存在・0件時 |
|------|---------------------|------------------------|
| `run_semantic_search()` | `SearchError::NoEmbeddings` | `SearchError::NoEmbeddings` |
| `try_hybrid_search()` | BM25 fallback（graceful） | BM25 fallback（graceful） |

**実装**:
- `embeddings.db`未存在は`Path::exists()`で事前判定し、`SearchError::NoEmbeddings`を返す（`EmbeddingStore::open()`はDB未存在時に新規作成で成功するため、open()の失敗に頼れない）
- DB存在・テーブル未作成時は`count()`/`search_similar()`の`no such table`エラーを`SearchError::NoEmbeddings`に変換
- `SchemaVersionMismatch`はデータ再構築が必要な異常であり、`NoEmbeddings`とは分離して専用メッセージ（"Embedding database schema version mismatch. Please re-run `embed` command."）を返す

### 判断4: 既存count()の活用

**選択**: `EmbeddingStore`の既存`count()`メソッドを直接使用し、`count_embeddings()`は新設しない

**理由**: 同義APIの増加を避ける。SymbolStoreの`count_embeddings()`と名前は異なるが機能は同一。

### 判断5: SymbolStore embedding関連メソッドの扱い

**選択**: 本Issue内では呼び出し元の切替のみ行い、SymbolStore側のembedding関連メソッドはそのまま残置する。`#[deprecated]`付与と削除は後続の専用リファクタリングIssueで対応する

**理由**:
- バグ修正スコープでは最小変更を優先（判断1との整合性維持）
- `#[deprecated]`付与はテストや残存呼び出しに警告影響が出るため、本Issueの必須スコープを超える
- 後続Issueとして「SymbolStoreからembedding関連コード削除」を起票する

### 判断6: BLOBデシリアライズのバリデーション

**選択**: `EmbeddingStore`に`blob_to_embedding(blob, expected_dimension)`関数を追加し、BLOBサイズ検証を行う

**理由**:
- 既存の`bytes_to_f32_vec()`は`chunks_exact(4)`を使用しており、末尾バイトを暗黙にドロップする
- SymbolStoreの`blob_to_embedding()`は`blob.len() == dimension * 4`を事前チェックしている
- 破損・切り詰めBLOBによる不正な類似度スコアを防止
- `EmbeddingStoreError`に`InvalidEmbedding`バリアントを追加

### 判断7: cosine_similarity/BLOB変換の重複管理

**選択**: 本Issue内ではEmbeddingStore側に新規実装し、SymbolStore側はそのまま残置する。共通モジュールへの抽出は別Issueで対応

**理由**:
- バグ修正スコープでは最小変更を優先
- SymbolStore側のembedding関連コード全体が将来削除対象のため、共通化の投資対効果が低い

## 5. 詳細設計

### 5.1 EmbeddingStore拡張 (`src/embedding/store.rs`)

#### 新規追加: EmbeddingSimilarityResult型

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct EmbeddingSimilarityResult {
    pub file_path: String,
    pub section_heading: String,
    pub similarity: f32,
}
```

#### 新規追加: cosine_similarity関数

```rust
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let mut dot = 0.0_f32;
    let mut norm_a = 0.0_f32;
    let mut norm_b = 0.0_f32;
    for (&ai, &bi) in a.iter().zip(b.iter()) {
        dot += ai * bi;
        norm_a += ai * ai;
        norm_b += bi * bi;
    }
    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom == 0.0 { 0.0 } else { dot / denom }
}
```

#### 新規追加: blob_to_embedding検証関数

```rust
fn blob_to_embedding(blob: &[u8], expected_dimension: usize) -> Result<Vec<f32>, EmbeddingStoreError> {
    if blob.len() != expected_dimension * 4 {
        return Err(EmbeddingStoreError::InvalidEmbedding {
            expected_bytes: expected_dimension * 4,
            actual_bytes: blob.len(),
        });
    }
    Ok(bytes_to_f32_vec(blob))
}
```

#### EmbeddingStoreErrorにInvalidEmbeddingバリアント追加

```rust
pub enum EmbeddingStoreError {
    Sqlite(rusqlite::Error),
    Io(std::io::Error),
    SchemaVersionMismatch { expected: u32, found: u32 },
    InvalidEmbedding { expected_bytes: usize, actual_bytes: usize },  // 新規
}
```

#### 新規追加: search_similar()メソッド

```rust
impl EmbeddingStore {
    pub fn search_similar(
        &self,
        query_embedding: &[f32],
        top_k: usize,
    ) -> Result<Vec<EmbeddingSimilarityResult>, EmbeddingStoreError> {
        // 1. embeddings テーブルから全件取得（section_path, section_heading, embedding, dimension）
        // 2. blob_to_embedding()でBLOBサイズ検証付きデシリアライズ
        //    - InvalidEmbeddingの場合はスキップ（ログ出力）
        // 3. dimension mismatch はスキップ（クエリとのdim不一致）
        // 4. cosine_similarity で類似度計算
        // 5. NaN/Inf結果をフィルタリング（破損BLOBからのNaN伝播防止）
        // 6. 降順ソート、top_k件返却
        // カラム名マッピング: section_path → file_path
    }
}
```

### 5.2 search.rs変更

#### run_semantic_search()のDB参照先変更

```rust
// Before:
let db_path = c.symbol_db_path();
let store = SymbolStore::open(&db_path)?;
if store.count_embeddings()? == 0 {
    return Err(SearchError::NoEmbeddings);
}

// After:
let emb_db_path = c.embeddings_db_path();
if !emb_db_path.exists() {
    return Err(SearchError::NoEmbeddings);
}
let emb_store = EmbeddingStore::open(&emb_db_path)?;
if emb_store.count()? == 0 {
    return Err(SearchError::NoEmbeddings);
}
```

#### try_hybrid_search()のDB参照先変更

SymbolStoreからEmbeddingStoreに変更。具体的な変更箇所:
1. `crate::indexer::symbol_db_path(commandindex_dir)` → `crate::indexer::embeddings_db_path(commandindex_dir)`
2. `SymbolStore::open(&db_path)` → `EmbeddingStore::open(&emb_db_path)`
3. `store.count_embeddings()` → `emb_store.count()`
4. `store.search_similar()` → `emb_store.search_similar()`
5. エラーハンドリング: `SymbolStoreError` → `EmbeddingStoreError`、SchemaVersionMismatch時はBM25 fallback維持

#### suggest.rsのDB参照先変更

`maybe_add_semantic_step()`が`SymbolStore::count_embeddings()`でsemantic検索ステップの有無を判定している。`EmbeddingStore::count()`に変更。

#### enrich関数の引数型変更

- `enrich_with_metadata()`: 引数型を`crate::indexer::symbol_store::EmbeddingSimilarityResult` → `crate::embedding::store::EmbeddingSimilarityResult`に変更
- `enrich_semantic_to_search_results()`: 同上

#### エラー型変換追加

```rust
impl From<EmbeddingStoreError> for SearchError {
    fn from(e: EmbeddingStoreError) -> Self {
        match e {
            EmbeddingStoreError::Sqlite(ref sql_err) => {
                // テーブル未作成時の "no such table" はNoEmbeddingsに変換
                if sql_err.to_string().contains("no such table") {
                    SearchError::NoEmbeddings
                } else {
                    SearchError::EmbeddingStore(e)
                }
            }
            // SchemaVersionMismatchはNoEmbeddingsとは分離（再構築が必要な異常）
            _ => SearchError::EmbeddingStore(e),
        }
    }
}
```

SearchError enumに`EmbeddingStore`バリアントを追加（Display, source()の各match armも追加）:

> 注: `no such table`文字列一致によるエラー判定は暫定策であり、SQLiteのエラーメッセージ変更に脆弱。将来的にはSQLiteエラーコードベースの構造化判定に移行すべき。
```rust
pub enum SearchError {
    // 既存バリアント...
    EmbeddingStore(EmbeddingStoreError),
}
```

### 5.3 テスト変更 (`tests/e2e_semantic_hybrid.rs`)

SymbolStoreの`insert_embeddings()`でembeddingsを挿入しているテストを、EmbeddingStoreの`upsert_embedding()`に切り替える。

**変更対象テスト**:
1. `test_embedding_insert_and_count`
2. `test_semantic_search_basic`
3. `test_semantic_search_top_k`
4. `test_context_with_embeddings`
5. `test_hybrid_auto_switch` (ignored)
6. `test_hybrid_bm25_fallback` (ignored)

**新規テスト**（`src/embedding/store.rs`内のユニットテスト）:
- `test_search_similar_basic` — 基本的な類似度検索
- `test_search_similar_top_k` — top_k制限
- `test_search_similar_dimension_mismatch` — dimension不一致のスキップ
- `test_search_similar_empty_db` — 空DBの場合

## 6. セキュリティ設計

| 脅威 | 対策 | 優先度 |
|------|------|--------|
| パストラバーサル | `embeddings_db_path()`はcommandindex_dirからの相対パスで構成。既存の安全性を維持 | 中 |
| SQLインジェクション | rusqliteのパラメータバインディングを使用（既存パターン踏襲） | 高 |
| unsafe使用 | 使用しない | 高 |
| BLOB破損・切り詰め | `blob_to_embedding()`でBLOBサイズ検証を実施。不正データはスキップ | 高 |
| NaN/Inf伝播 | cosine_similarity結果がNaNの場合はフィルタリング | 中 |

## 7. 影響範囲

### 変更ファイル一覧

| ファイル | 変更種別 | 変更概要 |
|---------|---------|---------|
| `src/embedding/store.rs` | 機能追加 | `EmbeddingSimilarityResult`, `search_similar()`, `cosine_similarity()`, `blob_to_embedding()`, `InvalidEmbedding`エラー |
| `src/cli/search.rs` | バグ修正 | DB参照先変更、エラー型変換追加、enrich関数の引数型変更 |
| `src/cli/suggest.rs` | バグ修正 | `maybe_add_semantic_step()`のDB参照先変更 |
| `src/indexer/symbol_store.rs` | 変更なし | embedding関連メソッドは残置（後続Issueで対応） |
| `tests/e2e_semantic_hybrid.rs` | テスト修正 | EmbeddingStore使用に切替 |

### 変更しないファイル
- `src/cli/embed.rs` — 書き込み先は正しい
- `src/cli/status/mod.rs` — 参照先は正しい

### 新規依存関係
- `cli/search` → `embedding/store`（新規モジュール依存）
- `cli/suggest` → `embedding/store`（新規モジュール依存）
- 外部crate追加なし

### 責務境界（本Issue後）
- **EmbeddingStore** (`embeddings.db`): embedding検索（search_similar）、embedding数取得（count）
- **SymbolStore** (`symbols.db`): メタデータ補完（enrich_with_metadata）、シンボル・依存関係管理

## 8. パフォーマンス考慮

- semantic/hybrid検索は引き続きbrute-force全件走査（SymbolStore実装と同等のアルゴリズム）
- データソースが変わるだけで計算量は同一
- 大規模DB（5万件超）での性能は退行測定対象
- ANN（近似最近傍）導入は将来の最適化として別Issue管理

## 9. 品質基準

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |

## 10. 受け入れ基準

- AC-1: `embed`コマンド実行後、`search --semantic`がエラーなく検索結果を返す
- AC-2: `search`コマンド（hybridモード）で「No embeddings found」警告が出ず、BM25+embeddingのRRF統合結果が返る
- AC-3: 既存の`embeddings.db`を持つ環境で再indexなしに検索が動作する（後方互換性）
- AC-4: `embeddings.db`が存在しない環境では従来通りのエラーメッセージ/フォールバックが表示される
- AC-5: `cargo test --all`が全パス、`cargo clippy --all-targets`が警告0件
- AC-6: 検索結果の見出し・本文・タグ補完（`enrich_with_metadata()`）が従来通り維持される
