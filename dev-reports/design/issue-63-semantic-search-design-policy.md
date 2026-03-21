# 設計方針書: Issue #63 Semantic Search（意味検索）

## 1. 概要

| 項目 | 内容 |
|------|------|
| Issue | #63 [Feature] Semantic Search（意味検索） |
| 作成日 | 2026-03-22 |
| ブランチ | `feature/issue-63-semantic-search` |
| 依存Issue | #61 Embedding生成基盤（マージ済み）、#62 Embeddingストレージ（マージ済み） |

`commandindex search --semantic <query>` オプションを実装し、クエリとドキュメントのembedding類似度に基づく意味検索を実現する。

## 2. システムアーキテクチャ

### 2.1 全体フロー

```
CLI (main.rs)
  ↓ --semantic <query>
cli/search.rs::run_semantic_search()
  ↓
embedding/mod.rs::create_provider()  ← config.toml
  ↓
EmbeddingProvider::embed([query])    ← クエリベクトル化
  ↓
indexer/symbol_store.rs::search_similar(query_vec, top_k)
  ↓                                  ← コサイン類似度検索
enrich_with_metadata()               ← tantivyメタデータ取得 + section_heading照合
  ↓
apply_semantic_filters()             ← ポストフィルタ (tag, path, type)
  ↓
output/mod.rs::format_semantic_results()
  ↓
human.rs / json.rs / path.rs        ← 出力
```

### 2.2 レイヤー構成と変更範囲

| レイヤー | モジュール | 変更内容 | 変更種別 |
|---------|-----------|---------|---------|
| **CLI** | `src/main.rs` | `--semantic` オプション追加、matchパターン拡張 | 修正 |
| **CLI** | `src/cli/search.rs` | `run_semantic_search()` + ヘルパー関数追加、SearchError拡張 | 修正 |
| **Indexer** | `src/indexer/symbol_store.rs` | `count_embeddings()` メソッド新規追加 | 修正 |
| **Indexer** | `src/indexer/reader.rs` | `matches_file_type()` を `pub(crate)` に変更 | 修正 |
| **Output** | `src/output/mod.rs` | `SemanticSearchResult` 構造体、`format_semantic_results()` 追加 | 修正 |
| **Output** | `src/output/human.rs` | `format_semantic_human()` 追加 | 修正 |
| **Output** | `src/output/json.rs` | `format_semantic_json()` 追加 | 修正 |
| **Output** | `src/output/path.rs` | `format_semantic_path()` 追加 | 修正 |
| **Embedding** | `src/embedding/mod.rs` | 変更なし（既存API使用） | - |
| **Test** | `tests/cli_args.rs` | 既存テスト更新、新規テスト追加 | 修正 |

## 3. 設計詳細

### 3.1 CLI定義（src/main.rs）

```rust
Search {
    /// 全文検索クエリ
    query: Option<String>,

    /// シンボル名検索
    #[arg(long, conflicts_with_all = ["query", "semantic"])]
    symbol: Option<String>,

    /// 関連ファイル検索
    #[arg(long, conflicts_with_all = ["query", "symbol", "tag", "path", "file_type", "heading", "semantic"])]
    related: Option<String>,

    /// 意味検索（セマンティック検索）
    #[arg(long, conflicts_with_all = ["query", "symbol", "related", "heading"])]
    semantic: Option<String>,

    // ... 既存フィルタオプション（tag, path, file_type, heading, format, limit）
}
```

**排他制御の双方向設定**:
- `--semantic` → `conflicts_with_all = ["query", "symbol", "related", "heading"]`
- `--symbol` → `conflicts_with_all = ["query", "semantic"]`（semantic追加）
- `--related` → `conflicts_with_all = ["query", "symbol", "tag", "path", "file_type", "heading", "semantic"]`（semantic追加）
- `--heading` は tantivy全文検索のフィルタであり、semantic検索はtantivyクエリを使用しないため併用不可

**matchパターン変更:**

```rust
match (query, symbol, related, semantic) {
    (Some(q), None, None, None) => run(options, filters, format),
    (None, Some(s), None, None) => run_symbol_search(&s, limit, format),
    (None, None, Some(f), None) => run_related_search(&f, limit, format),
    (None, None, None, Some(q)) => run_semantic_search(&q, limit, format, tag.as_ref(), filters),
    (None, None, None, None) => {
        // エラーメッセージ: "Either <QUERY>, --symbol <NAME>, --related <FILE>, or --semantic <QUERY> is required"
        Err(SearchError::InvalidArgument(
            "Either <QUERY>, --symbol <NAME>, --related <FILE>, or --semantic <QUERY> is required".to_string()
        ))
    }
    _ => unreachable!(), // clapの排他制御で到達しない
}
```

> **tag引数の受け渡し**: `tag.as_ref()` で `&Option<String>` として渡す。所有権を移動しない。

> **将来改善**: 検索モードが5種類以上に増えた場合、`SearchMode` enum を導入して1変数matchに簡素化を検討する。

### 3.2 セマンティック検索関数（src/cli/search.rs）

**責務分離**: `run_semantic_search()` はオーケストレーションのみ。メタデータ取得とフィルタは独立関数に分離。

```rust
pub fn run_semantic_search(
    query: &str,
    limit: usize,
    format: OutputFormat,
    tag: Option<&String>,
    filters: SearchFilters,
) -> Result<(), SearchError> {
    // 0. config.toml からEmbeddingConfig読み込み
    let config = Config::load(&crate::indexer::commandindex_dir(Path::new(".")))
        .map_err(SearchError::Embedding)?;
    let embedding_config = config
        .and_then(|c| c.embedding)
        .unwrap_or_default();
    let provider = create_provider(&embedding_config)?;

    // 1. SymbolStore 存在確認 + embedding 件数チェック
    let db_path = crate::indexer::symbol_db_path(Path::new("."));
    if !db_path.exists() {
        return Err(SearchError::SymbolDbNotFound);
    }
    let symbol_store = SymbolStore::open(&db_path)?;
    let count = symbol_store.count_embeddings()?;
    if count == 0 {
        return Err(SearchError::NoEmbeddings);
    }

    // 2. クエリベクトル化（安全アクセス）
    let query_embeddings = provider.embed(&[query.to_string()])?;
    let query_vec = query_embeddings.first()
        .ok_or_else(|| SearchError::Embedding(
            EmbeddingError::InvalidResponse("empty embedding result".to_string())
        ))?;

    // 3. コサイン類似度検索（オーバーサンプリング）
    let oversampled_limit = limit * 5;
    let similar_results = symbol_store.search_similar(query_vec, oversampled_limit)?;

    // 4. tantivyインデックス存在確認 + メタデータ取得（独立関数）
    let tantivy_dir = crate::indexer::index_dir(Path::new("."));
    if !tantivy_dir.exists() {
        return Err(SearchError::IndexNotFound);
    }
    let reader = IndexReaderWrapper::open(&tantivy_dir)?;
    let semantic_results = enrich_with_metadata(&similar_results, &reader)?;

    // 5. ポストフィルタ（独立関数）
    let filtered = apply_semantic_filters(semantic_results, &tag, &filters);

    // 6. limit件に切り詰め + 結果0件ハンドリング
    let final_results: Vec<_> = filtered.into_iter().take(limit).collect();
    if final_results.is_empty() {
        eprintln!("No results found.");
        return Ok(());
    }

    // 7. 出力
    format_semantic_results(&final_results, format, &mut std::io::stdout())?;
    Ok(())
}
```

### 3.2.1 メタデータ取得関数

```rust
fn enrich_with_metadata(
    similar_results: &[EmbeddingSimilarityResult],
    reader: &IndexReaderWrapper,
) -> Result<Vec<SemanticSearchResult>, SearchError> {
    let mut results = Vec::new();

    // file_pathでグルーピングしてsearch_by_exact_path呼び出しを最適化
    let mut path_groups: HashMap<&str, Vec<&EmbeddingSimilarityResult>> = HashMap::new();
    for result in similar_results {
        path_groups.entry(&result.file_path).or_default().push(result);
    }

    for (file_path, group) in &path_groups {
        let tantivy_results = reader.search_by_exact_path(file_path)?;

        for result in group {
            let matched = if result.section_heading.is_empty() {
                // 空文字 → ファイル全体のembedding、最初のセクションを使用
                tantivy_results.iter().next()
            } else {
                // 非空 → heading完全一致
                tantivy_results.iter()
                    .find(|r| r.heading == result.section_heading)
            };

            if let Some(meta) = matched {
                results.push(SemanticSearchResult {
                    path: meta.path.clone(),
                    heading: meta.heading.clone(),
                    similarity: result.similarity,
                    body: meta.body.clone(),
                    tags: meta.tags.clone(),
                    heading_level: meta.heading_level,
                });
            }
        }
    }

    // similarity降順でソート（グルーピングで順序が崩れるため）
    results.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap_or(std::cmp::Ordering::Equal));
    Ok(results)
}
```

### 3.3 SearchError 拡張

```rust
pub enum SearchError {
    // 既存バリアント
    IndexNotFound,
    Reader(ReaderError),
    Output(OutputError),
    SymbolStore(SymbolStoreError),
    SymbolDbNotFound,
    InvalidArgument(String),
    SchemaVersionMismatch,
    RelatedSearch(crate::search::related::RelatedSearchError),

    // 新規バリアント
    Embedding(EmbeddingError),
    NoEmbeddings,
}
```

**Display実装:**

```rust
SearchError::Embedding(ref e) => {
    match e {
        EmbeddingError::NetworkError(_) =>
            write!(f, "Failed to connect to embedding provider. If using Ollama, start with `ollama serve`.")
        _ => write!(f, "Embedding error: {}", e)
    }
}
SearchError::NoEmbeddings =>
    write!(f, "No embeddings found. Run `commandindex embed` first.")
```

**source()実装:**

```rust
SearchError::Embedding(ref e) => Some(e),
SearchError::NoEmbeddings => None,
```

**From実装:**

```rust
impl From<EmbeddingError> for SearchError {
    fn from(e: EmbeddingError) -> Self {
        SearchError::Embedding(e)
    }
}
```

### 3.4 SymbolStore拡張（src/indexer/symbol_store.rs）

**新規追加** メソッド:

```rust
pub fn count_embeddings(&self) -> Result<u64, SymbolStoreError> {
    let count: u64 = self.conn.query_row(
        "SELECT COUNT(*) FROM embeddings",
        [],
        |row| row.get(0),
    )?;
    Ok(count)
}
```

### 3.5 reader.rs 可視性変更

`matches_file_type()` を `pub(crate)` に変更し、`cli/search.rs` の `apply_semantic_filters()` から再利用可能にする。

```rust
// 変更前: fn matches_file_type(path: &str, file_type: &str) -> bool
// 変更後:
pub(crate) fn matches_file_type(path: &str, file_type: &str) -> bool
```

### 3.6 出力フォーマット（src/output/mod.rs）

```rust
#[derive(Debug, Clone)]
pub struct SemanticSearchResult {
    pub path: String,
    pub heading: String,
    pub similarity: f32,
    pub body: String,
    pub tags: String,          // スペース区切り（tantivyと同形式）
    pub heading_level: u64,
}

pub fn format_semantic_results(
    results: &[SemanticSearchResult],
    format: OutputFormat,
    writer: &mut dyn Write,
) -> Result<(), OutputError> {
    match format {
        OutputFormat::Human => human::format_semantic_human(results, writer),
        OutputFormat::Json => json::format_semantic_json(results, writer),
        OutputFormat::Path => path::format_semantic_path(results, writer),
    }
}
```

> **NOTE**: format_*関数は4種類目（通常/symbol/related/semantic）。5種類目追加時に trait-based Formatter パターンへのリファクタリングを検討する。

**human形式:**
```
[0.89] docs/auth/design.md > 認証フロー
  ログインフロー...
  Tags: auth, security
```

**json形式（JSONL）:**
```json
{"path":"docs/auth/design.md","heading":"認証フロー","similarity":0.89,"body":"ログインフロー...","tags":["auth","security"],"heading_level":2}
```

**path形式:**
```
docs/auth/design.md
```

### 3.7 ポストフィルタ関数

```rust
fn apply_semantic_filters(
    results: Vec<SemanticSearchResult>,
    tag: &Option<String>,
    filters: &SearchFilters,
) -> Vec<SemanticSearchResult> {
    results.into_iter().filter(|r| {
        // パスプレフィックスフィルタ
        if let Some(prefix) = &filters.path_prefix {
            if !r.path.starts_with(prefix) {
                return false;
            }
        }
        // ファイルタイプフィルタ（reader.rsの関数を再利用）
        if let Some(file_type) = &filters.file_type {
            if !crate::indexer::reader::matches_file_type(&r.path, file_type) {
                return false;
            }
        }
        // タグフィルタ
        if let Some(tag_query) = tag {
            let tags: Vec<&str> = r.tags.split_whitespace().collect();
            if !tags.iter().any(|t| t.eq_ignore_ascii_case(tag_query)) {
                return false;
            }
        }
        true
    }).collect()
}
```

## 4. 設計判断とトレードオフ

### 判断1: SymbolStore vs EmbeddingStore

| 選択肢 | メリット | デメリット |
|--------|---------|-----------|
| **SymbolStore（採用）** | search_similar()実装済み、symbols.dbに統合 | 将来的にEmbeddingStoreとの二重管理 |
| EmbeddingStore | 専用ストア、分離された関心事 | search_similar()未実装、新規開発コスト |

**決定**: SymbolStore を使用。search_similar() が既に実装されており、追加開発コストが最小。

> **データフロー**: `commandindex embed` コマンドは SymbolStore（symbols.db）の embeddings テーブルに書き込む。semantic search はこの同じテーブルから search_similar() で読み取る。EmbeddingStore（embeddings.db）は embed コマンドの中間キャッシュ用であり、semantic search では使用しない。

> **技術的負債**: EmbeddingStore（embeddings.db）とSymbolStore（symbols.db内embeddingsテーブル）の二重管理。将来的に統合を検討するリファクタリングIssueを起票予定。

> **将来改善**: 検索モードが5種類以上に増えた場合、`SearchMode` enum を導入してmain.rsのmatchパターンを1変数に簡素化する。

### 判断2: ブルートフォース vs ANN

| 選択肢 | メリット | デメリット |
|--------|---------|-----------|
| **ブルートフォース（採用）** | 実装済み、正確な結果 | O(n)、大規模時に遅延 |
| ANN（hnsw等） | O(log n)、スケーラブル | 新規ライブラリ追加、近似結果 |

**決定**: Phase 1 はブルートフォース。ローカルナレッジ検索（数千〜数万セクション）では十分。メモリ使用量概算: 1万セクション × 768次元 × 4byte ≈ 30MB。

### 判断3: フィルタ方式

| 選択肢 | メリット | デメリット |
|--------|---------|-----------|
| **ポストフィルタ（採用）** | 実装シンプル、既存search_similar活用 | オーバーサンプリング必要 |
| プレフィルタ | フィルタ後にembedding検索で効率的 | search_similar APIの変更必要 |

**決定**: ポストフィルタ + オーバーサンプリング（limit * 5）。既存APIの変更不要。

### 判断4: 出力構造体

| 選択肢 | メリット | デメリット |
|--------|---------|-----------|
| **専用構造体（採用）** | 型安全、semantic検索固有フィールド | 4種類目の出力構造体 |
| 既存SearchResult拡張 | 構造体数を抑制 | 既存検索への影響、Optional多用 |

**決定**: SemanticSearchResult を新規定義。既存コードへの影響を最小化。

### 判断5: 責務分離

`run_semantic_search()` → オーケストレーションのみ。メタデータ取得は `enrich_with_metadata()`、フィルタは `apply_semantic_filters()` に分離。テスタビリティと単一責任原則を向上。

## 5. エラーハンドリング設計

| エラー状況 | SearchError バリアント | ユーザーメッセージ |
|-----------|----------------------|------------------|
| Embedding生成失敗（ネットワーク） | `Embedding(NetworkError)` | "Failed to connect to embedding provider. If using Ollama, start with `ollama serve`." |
| Embedding生成失敗（その他） | `Embedding(*)` | "Embedding error: {detail}" |
| embeddings 0件 | `NoEmbeddings` | "No embeddings found. Run `commandindex embed` first." |
| SymbolStore操作失敗 | `SymbolStore(*)` | 既存メッセージ |
| tantivy インデックス未作成 | `IndexNotFound` | 既存メッセージ |
| symbols.db 未作成 | `SymbolDbNotFound` | 既存メッセージ |
| 結果0件 | (エラーではない) | "No results found." (stderr, Ok(())返却) |

## 6. セキュリティ設計

| 脅威 | 対策 | 優先度 |
|------|------|--------|
| クエリインジェクション | SQLiteパラメータバインディング（rusqlite標準） | 高 |
| パストラバーサル | search_by_exact_path()でインデックス内のパスのみ使用 | 高 |
| Embedding APIキー漏洩 | config.toml のファイル権限に依存（既存設計踏襲） | 中 |
| unsafe使用 | 使用禁止 | 中 |
| クエリサイズ | EmbeddingProviderの内部truncate_text()で制限（既存設計踏襲） | 低 |

## 7. テスト設計

### 7.1 単体テスト

| テスト対象 | テストケース | 方法 |
|-----------|------------|------|
| `enrich_with_metadata` | section_heading照合（空/非空）、セクション未発見時スキップ | 直接呼び出し |
| `apply_semantic_filters` | tag/path/typeフィルタの適用 | 直接呼び出し |
| `SemanticSearchResult` 出力 | human/json/path各形式 | format_semantic_* 関数テスト |
| `count_embeddings` | 0件/N件のカウント | SymbolStore直接テスト |

### 7.2 CLIテスト（tests/cli_args.rs）

| テストケース | 期待動作 |
|-------------|---------|
| `--semantic` + `--symbol` | 排他エラー |
| `--semantic` + `--related` | 排他エラー |
| `--semantic` + query | 排他エラー |
| `--semantic` + `--heading` | 排他エラー |
| `--semantic` + `--tag` | 成功（併用可能） |
| `--semantic` + `--path` | 成功（併用可能） |
| `--semantic` + `--type` | 成功（併用可能） |
| 引数なし search | エラーメッセージに `--semantic` 含む |

> テスト名 `search_requires_query_or_symbol` → `search_requires_query_or_symbol_or_semantic` に更新

### 7.3 統合テスト

| テストケース | 方法 |
|-------------|------|
| 正常系: 類似度ソート確認 | insert_embeddings() でテストデータ挿入 → search_similar() |
| 異常系: embedding 0件 | 空のSymbolStore → NoEmbeddingsエラー |
| ポストフィルタ | テストデータ + フィルタ条件 |

### 7.4 E2Eテスト

- Ollama依存のため `#[ignore]` 属性付き
- CIではスキップ、ローカルで手動実行

## 8. 品質基準

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |

## 9. 影響範囲まとめ

### 変更ファイル（8ファイル）
1. `src/main.rs` - `--semantic` オプション追加、排他制御双方向設定
2. `src/cli/search.rs` - `run_semantic_search()` + `enrich_with_metadata()` + `apply_semantic_filters()` + SearchError拡張
3. `src/indexer/symbol_store.rs` - `count_embeddings()` 新規追加
4. `src/indexer/reader.rs` - `matches_file_type()` を `pub(crate)` に変更
5. `src/output/mod.rs` - SemanticSearchResult + format_semantic_results()
6. `src/output/human.rs` - format_semantic_human()
7. `src/output/json.rs` - format_semantic_json()
8. `src/output/path.rs` - format_semantic_path()

### テストファイル（2ファイル）
1. `tests/cli_args.rs` - 既存テスト更新 + 新規テスト
2. `tests/semantic_search.rs`（新規） - 統合テスト

### 変更なし（既存API使用のみ）
- `src/embedding/mod.rs` - EmbeddingProvider, create_provider()
- `src/embedding/ollama.rs` - OllamaProvider
- `src/embedding/openai.rs` - OpenAiProvider
- `src/search/` - 変更なし

### 新規クレート追加: なし
