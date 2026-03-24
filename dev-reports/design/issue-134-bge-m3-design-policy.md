# 設計方針書 - Issue #134: 多言語embeddingモデル対応 (BGE-M3)

## 1. Issue概要

| 項目 | 内容 |
|------|------|
| Issue番号 | #134 |
| タイトル | 多言語embeddingモデル対応 (BGE-M3) |
| 種別 | 機能追加 + 品質改善 |
| 主目的 | 日本語ナレッジ検索の精度向上 |

## 2. システムアーキテクチャ概要

### 影響を受けるレイヤー

```
┌─────────────────────────────────────────────┐
│ CLI Layer                                    │
│  embed.rs / index.rs / search.rs / suggest.rs│  ← T1.5, T2.5 で変更
├─────────────────────────────────────────────┤
│ Embedding Layer                              │
│  mod.rs (trait) / ollama.rs / store.rs      │  ← T1, T1.5, T2.5 で変更
├─────────────────────────────────────────────┤
│ Search Layer                                 │
│  hybrid.rs / related.rs                     │  ← 変更なし
├─────────────────────────────────────────────┤
│ Storage Layer                                │
│  SQLite (embeddings.db)                     │  ← スキーマ変更なし
└─────────────────────────────────────────────┘
```

## 3. 設計方針

### 3.1 T1: known_dimension追加

**方針:** 最小変更。既存のパターンマッチに1行追加。

```rust
// src/embedding/ollama.rs
fn known_dimension(model: &str) -> Option<usize> {
    match model {
        "nomic-embed-text" => Some(768),
        "all-minilm" => Some(384),
        "mxbai-embed-large" => Some(1024),
        "qllama/bge-m3:q8_0" => Some(1024),  // 追加
        _ => None,
    }
}
```

**テスト:** `test_dimension_known_models` にBGE-M3アサーション追加。

### 3.2 T1.5: モデル変更時のキャッシュ無効化

**問題の根本原因:**
- `has_current_embedding(path, file_hash)` がmodel列を見ていない
- モデル変更後もファイル未変更ならキャッシュヒット→旧embeddingが残存
- search_similar()で次元不一致により全レコードがスキップされ検索結果0件

**設計方針:**

#### 3.2.1 has_current_embedding() のシグネチャ変更

```rust
// 変更前
pub fn has_current_embedding(&self, path: &str, file_hash: &str) -> Result<bool, EmbeddingStoreError>

// 変更後
pub fn has_current_embedding(&self, path: &str, file_hash: &str, model: &str) -> Result<bool, EmbeddingStoreError>
```

SQL:
```sql
-- 変更前
SELECT COUNT(*) FROM embeddings WHERE section_path = ?1 AND file_hash = ?2

-- 変更後
SELECT COUNT(*) FROM embeddings WHERE section_path = ?1 AND file_hash = ?2 AND model = ?3
```

**トレードオフ:**
- 既存の呼び出し元（embed.rs, index.rs）全てにmodel引数を追加する必要がある
- ただし、provider.model_name()で取得できるため追加コストは低い

#### 3.2.2 旧モデルレコードの自動削除

**方針:** embedding生成開始時にDBから現在のモデルと異なるレコードを一括削除。

```rust
// src/embedding/store.rs に新メソッド追加
pub fn delete_stale_model_embeddings(&self, current_model: &str) -> Result<usize, EmbeddingStoreError> {
    if current_model.is_empty() {
        return Err(EmbeddingStoreError::InvalidEmbedding("model name cannot be empty".into()));
    }
    let deleted = self.conn.execute(
        "DELETE FROM embeddings WHERE model != ?1",
        params![current_model],
    )?;
    Ok(deleted)
}
```

**呼び出し箇所:** embed.rs と index.rs の embedding生成開始前（create_tables()直後、ループ開始前に1回のみ）。

**冪等性:** 二重呼び出しは安全（対象レコードが既に削除済みなら0件削除で返る）。

**データ消失リスク対策:**
- `current_model` が空文字列の場合はエラーを返す（Fail Fast原則。設定異常を正常系として扱わない）
- 削除件数が0より大きい場合、情報メッセージを表示: `Info: Deleted N stale embeddings from previous model.`

**トレードオフ:**
- Pros: ユーザーが意識せずとも旧モデルデータが自動削除される
- Cons: 複数モデルのembeddingを同時に保持できなくなる
- 判断: 現行アーキテクチャでは同一DBに複数モデルを持つユースケースがないため、自動削除が適切

#### 3.2.3 影響を受ける全経路

| 経路 | ファイル | 対応 |
|------|---------|------|
| `commandindex embed` | src/cli/embed.rs (L152) | has_current_embedding()にmodel追加 |
| `commandindex index --with-embedding` | src/cli/index.rs (generate_embeddings_for_manifest) | 同上 |
| `commandindex update --with-embedding` | src/cli/index.rs (run_incremental→generate_embeddings_for_manifest) | 同上 |

### 3.3 T2.5: 次元不一致時の警告メッセージ

**設計方針:** search_similar()の返り値にメタ情報を付加する新構造体を導入。

```rust
// src/embedding/store.rs

/// セマンティック検索結果（メタ情報付き）
pub struct SimilaritySearchOutput {
    pub results: Vec<EmbeddingSimilarityResult>,
    pub total_records: usize,
    pub skipped_dimension_mismatch: usize,
}
```

**search_similar()の返り値変更:**
```rust
// 変更前
pub fn search_similar(&self, query_embedding: &[f32], top_k: usize)
    -> Result<Vec<EmbeddingSimilarityResult>, EmbeddingStoreError>

// 変更後
pub fn search_similar(&self, query_embedding: &[f32], top_k: usize)
    -> Result<SimilaritySearchOutput, EmbeddingStoreError>
```

**警告判定メソッド（DRY原則 + SRP準拠）:**

SimilaritySearchOutputは純粋なデータ構造に留め、副作用なしの判定メソッドのみ持つ。
実際の警告表示（eprintln!）はCLI層（search.rs, suggest.rs）で行う。

```rust
// src/embedding/store.rs
impl SimilaritySearchOutput {
    /// total_records の定義: DBから取得した全行数（BLOB不正含む）
    /// skipped_dimension_mismatch の定義: query_dim と stored_embedding.len() の不一致でスキップした数のみ
    pub fn should_warn_dimension_mismatch(&self) -> bool {
        self.total_records > 0 && self.skipped_dimension_mismatch > self.total_records / 2
    }
}

// CLI層での使用例（search.rs, suggest.rs）
let output = emb_store.search_similar(query_embedding, top_k)?;
if output.should_warn_dimension_mismatch() {
    eprintln!(
        "Warning: {}/{} embeddings were skipped due to dimension mismatch. \
         Consider re-running 'commandindex embed' after model change.",
        output.skipped_dimension_mismatch, output.total_records
    );
}
let results = output.results;
```

**API変更方針:** search_similar()はプロジェクト内部APIのため、breaking changeを許容する。全呼び出し元を一括で新返り値に移行する。
```

**影響を受ける呼び出し元:**

| 経路 | ファイル | 変更内容 |
|------|---------|---------|
| search --semantic | src/cli/search.rs (run_semantic_search) | 返り値をSimilaritySearchOutputに変更、should_warn_dimension_mismatch()呼び出し |
| search（hybrid） | src/cli/search.rs (try_hybrid_search) | 同上 |
| suggest（semantic fallback） | src/cli/suggest.rs (try_semantic_fallback, L281) | 返り値をSimilaritySearchOutputに変更、output.results取得、should_warn_dimension_mismatch()呼び出し |

**注記:** src/indexer/symbol_store.rs にも同名の search_similar() メソッドが存在するが、SymbolStore の独立メソッドであり EmbeddingStore とは別の型定義のため今回の変更対象外。

**テスト影響:**
- src/embedding/store.rs のユニットテスト（5件）: search_similar()の返り値型変更に対応
  - test_search_similar_basic, test_search_similar_top_k, test_search_similar_dimension_mismatch, test_search_similar_empty_db, cosine_similarity関連
- src/embedding/store.rs のユニットテスト（3件）: has_current_embedding()のmodel引数追加に対応
  - test_has_current_embedding_true, test_has_current_embedding_false_different_hash, test_has_current_embedding_false_no_record
- tests/e2e_semantic_hybrid.rs（2箇所）: search_similar返り値のoutput.resultsアクセスに変更

### 3.4 T4: ドキュメント整備

README.mdに「Embedding」セクションを新設。内容:
- 対応モデル一覧
- Ollamaモデルの事前pull手順
- モデル変更手順
- 注意事項

## 4. データフロー

### Embedding生成フロー（変更後）

```
1. Config読み込み → EmbeddingConfig { model: "qllama/bge-m3:q8_0" }
2. create_provider() → OllamaProvider
3. delete_stale_model_embeddings(model)  ← 新規追加
4. セクションごとのループ:
   a. has_current_embedding(path, hash, model)  ← model引数追加
   b. キャッシュミスなら provider.embed(texts)
   c. store.upsert_embedding(path, heading, embedding, 1024, model, hash)
```

### セマンティック検索フロー（変更後）

```
1. provider.embed(&[query]) → query_embedding (1024次元)
2. store.search_similar(query_embedding, top_k)
   → SimilaritySearchOutput { results, total_records, skipped }  ← 返り値変更
3. skipped > total/2 なら警告表示  ← 新規追加
4. results をフィルタ・ランキング
```

## 5. エラーハンドリング

| エラーケース | 現状 | 変更後 |
|-------------|------|--------|
| known_dimension未登録モデル | dimension()が0を返す→初回embed時にcached_dimension設定 | 変更なし（フォールバック動作を維持） |
| モデル変更後にclean未実施 | サイレントに検索結果0件 | 警告メッセージ表示 + embed時に旧モデル自動削除 |
| Ollamaモデル未pull | embedエラー | 変更なし |

## 6. セキュリティ設計

| 脅威 | 対策 | 判定 |
|------|------|------|
| SQLインジェクション | rusqliteのparams!マクロによるパラメータバインディング使用（新規DELETE文含む） | 問題なし |
| unsafe使用 | なし（テストコード内のenv操作のみ。新規テストではConfig経由で注入） | 問題なし |
| 外部API通信 | OllamaローカルサーバーへのHTTP通信のみ。APIキー不要 | 問題なし |
| 設定ミスによるデータ喪失 | delete_stale_model_embeddings()の空文字列エラー（Fail Fast） | 対策済み |
| リモートendpoint | endpointがlocalhost以外でHTTPの場合はリスクあり（スコープ外） | 注記のみ |

## 7. 設計判断とトレードオフ

### 判断1: 旧モデルの自動削除 vs 手動clean要求

- **選択:** 自動削除
- **理由:** ユーザーがcleanを忘れた場合のサイレント障害が最大のリスク。自動削除で確実に回避
- **トレードオフ:** 複数モデルの同時保持が不可能になるが、現行アーキテクチャではユースケースなし

### 判断2: search_similar()の返り値拡張 vs 別メソッド追加

- **選択:** 返り値をSimilaritySearchOutputに拡張
- **理由:** 全呼び出し元で一貫した警告処理が必要。別メソッドだと呼び出し忘れのリスク
- **トレードオフ:** 既存テストの修正が必要だが、型安全性により修正漏れをコンパイラが検出

### 判断3: has_current_embedding()のシグネチャ変更 vs DB側での暗黙的処理

- **選択:** シグネチャ変更（model引数追加）
- **理由:** 明示的なAPIが暗黙的な処理より安全。コンパイラが呼び出し元の更新漏れを検出
- **トレードオフ:** 呼び出し元の変更が必要だが2箇所のみ

## 8. 影響範囲

### 変更対象ファイル

| ファイル | 変更種別 | 変更量 |
|---------|---------|--------|
| src/embedding/ollama.rs | 修正 | S（1行追加 + テスト1行追加） |
| src/embedding/store.rs | 修正 | M（has_current_embedding変更、新メソッド追加、search_similar返り値変更） |
| src/cli/embed.rs | 修正 | S（has_current_embedding呼び出し変更、delete_stale呼び出し追加） |
| src/cli/index.rs | 修正 | S（has_current_embedding呼び出し変更、delete_stale呼び出し追加） |
| src/cli/search.rs | 修正 | S（search_similar返り値変更、警告表示追加） |
| src/cli/suggest.rs | 修正 | S（search_similar返り値変更、警告表示追加） |
| tests/e2e_semantic_hybrid.rs | 修正 | S（返り値型変更対応、2箇所） |
| README.md | 修正 | S（Embeddingセクション新設） |

### 変更なしのファイル

| ファイル | 理由 |
|---------|------|
| src/embedding/mod.rs | EmbeddingProviderトレイト変更なし |
| src/embedding/openai.rs | OpenAI側は変更不要 |
| src/search/hybrid.rs | RRFロジック変更なし |
| src/config/mod.rs | 設定構造体変更なし |
| src/indexer/symbol_store.rs | 独立したsearch_similar()メソッド。EmbeddingStoreとは別の型定義 |
| Cargo.toml | 新規依存追加なし |

## 9. テスト戦略

| テスト種別 | 対象 | 内容 |
|-----------|------|------|
| ユニットテスト（新規） | ollama.rs | known_dimension()にBGE-M3が含まれること |
| ユニットテスト（新規） | store.rs | has_current_embedding()がmodel条件で正しくフィルタすること |
| ユニットテスト（新規） | store.rs | has_current_embedding()でmodel不一致時にfalseを返すこと |
| ユニットテスト（新規） | store.rs | delete_stale_model_embeddings()が旧モデルのみ削除すること |
| ユニットテスト（新規） | store.rs | delete_stale_model_embeddings()が空文字列でInvalidEmbeddingエラーを返すこと |
| ユニットテスト（新規） | store.rs | search_similar()がSimilaritySearchOutputを返し、skipped数が正しいこと |
| ユニットテスト（既存修正） | store.rs | has_current_embedding既存3テストにmodel引数追加 |
| ユニットテスト（既存修正） | store.rs | search_similar既存5テストのSimilaritySearchOutput対応 |
| 統合テスト（既存修正） | e2e_semantic_hybrid.rs | search_similar返り値の2箇所をoutput.resultsに変更 |
| 統合テスト（新規） | e2e | モデル変更後にembedを再実行し全セクションが再生成されること |
| 手動評価 | T3 | BGE-M3での日本語・英語検索精度比較 |

## 10. 品質基準

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |
