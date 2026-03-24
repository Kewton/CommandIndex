# 設計方針書: Issue #144 - suggest の英語クエリ精度改善

## 1. Issue概要

| 項目 | 内容 |
|------|------|
| Issue番号 | #144 |
| タイトル | suggest の英語クエリ精度改善 |
| 目的 | suggestコマンドのBM25フォールバック方式をRRFハイブリッド方式に移行し、英語クエリの推薦精度を改善 |
| 関連Issue | #127 (suggest英語推薦精度), #134 (BGE-M3, 完了済み) |

## 2. システムアーキテクチャ概要

### 現在のアーキテクチャ

```
[CLI Layer]          [Search Layer]        [Data Layer]

suggest.rs ──BM25──> indexer/reader.rs ──> tantivy index
    │
    └─fallback──> embedding/store.rs ───> embeddings.db (SQLite)
                  embedding/mod.rs ────> Ollama / OpenAI

search.rs ──hybrid──> search/hybrid.rs ──rrf──> (BM25 + semantic結果を統合)
    │
    └──> search.rs::try_hybrid_search()
         search.rs::enrich_semantic_to_search_results()
```

### 変更後のアーキテクチャ

```
[CLI Layer]          [Search Layer]                  [Data Layer]

suggest.rs ──────> search/hybrid.rs (ファイルRRF) ──> tantivy index
    │              search/semantic.rs ──────────────> embeddings.db (SQLite)
    │              (セマンティック結果変換・集約)       > Ollama / OpenAI
    │              search/ranking.rs
    │              (ファイル集約・重み付け)
    │
search.rs ──────> search/hybrid.rs (section RRF)
    │              search/semantic.rs
    │              (セマンティック結果変換)
```

## 3. レイヤー構成と責務

| レイヤー | モジュール | 責務 | 変更有無 |
|---------|-----------|------|---------|
| **CLI** | `src/cli/suggest.rs` | suggestコマンド、オーケストレーション | **主要変更**（検索統合は委譲） |
| **CLI** | `src/cli/search.rs` | searchコマンド、SearchContext | リファクタ（共通化） |
| **Search** | `src/search/hybrid.rs` | RRF統合ロジック（section-level + **ファイル単位**） | **追加**: `rrf_merge_by_key` |
| **Search** | `src/search/semantic.rs` (**新規**) | セマンティック結果変換（enrich）、セマンティッククエリ実行 | **新規作成** |
| **Search** | `src/search/ranking.rs` (**新規**) | ファイル単位集約、ファイルタイプ重み付け | **新規作成** |
| **Embedding** | `src/embedding/store.rs` | SQLite embeddings操作 | 変更なし |
| **Embedding** | `src/embedding/mod.rs` | Provider trait, factory | 変更なし |
| **Indexer** | `src/indexer/reader.rs` | BM25検索、SearchResult型 | 変更なし |

## 4. 設計判断とトレードオフ

### DJ-1: ファイル単位RRF vs section-level RRF

**決定**: ファイル単位でRRF統合する

**理由**:
- suggestが必要とするのはファイル単位のランキング（`DEDUP_FILE_LIMIT=5`件のファイル候補）
- 既存の`rrf_merge`は`(path, heading)`キーで動作する → section-level
- section-level RRFをそのまま使うと、同一ファイルの複数セクションがRRFスコアに過剰寄与

**トレードオフ**:
- (+) ファイル候補ランキングが公平
- (+) 既存のfile_type_weight体系と自然に統合
- (-) セクション粒度の情報が失われる（suggestでは不要）

### DJ-2: RRFロジックのDRY化

**決定**: `hybrid.rs`にファイル単位RRF関数`rrf_merge_files`を追加。内部で非公開の共通ヘルパーを使いDRY化

**理由**:
- RRFスコア計算(`1/(K+rank)`)はsection-levelもファイル単位も同一アルゴリズム
- suggest.rsにRRFロジックをコピーするとDRY違反でK値変更時に両方修正が必要
- `hybrid.rs`は「RRFアルゴリズムの置き場」として責務が明確

**実装方針**:
```rust
// src/search/hybrid.rs

// K値は非公開定数のまま維持（外部からの直接依存を避け疎結合を保つ）
const RRF_K: f32 = 60.0;

// 非公開の共通ヘルパー（KISSに従い最小限の共通化）
fn compute_rrf_scores(ranked_lists: &[&[(String, f32)]], k: f32) -> HashMap<String, f32> {
    // 各リストのランクに基づいてRRFスコアを計算
}

/// 既存: section-level RRF統合（SearchResult型、search.rsから利用）
pub fn rrf_merge(bm25: &[SearchResult], semantic: &[SearchResult], limit: usize) -> Vec<SearchResult>

/// 新規: ファイル単位のRRF統合（suggestから利用）
pub fn rrf_merge_files(
    bm25_files: &[(String, f32)],
    semantic_files: &[(String, f32)],
    limit: usize,
) -> Vec<(String, f32)> {
    // compute_rrf_scoresで共通ロジック再利用
}
```

**公開APIは2本のみ**: `rrf_merge`（section-level、既存）と `rrf_merge_files`（ファイル単位、新規）。過度な汎化（ジェネリックAPI）は避ける（KISS）

### DJ-3: セマンティック検索パイプラインの共通化

**決定**: `src/search/semantic.rs`にセマンティック関連のSearch層関数を配置。エラー方針はResult返却に統一。

**理由**:
- suggest.rsの`try_semantic_fallback`、search.rsの`try_hybrid_search`で同一パイプラインが重複
- Search層はResultで失敗を返し、graceful degradationの判断はCLI層（suggest.rs/search.rs）で行う

**エラー方針**: semantic.rsの全公開関数はResult型で失敗を返す。graceful degradation（None/BM25フォールバック）の判断は**CLI層のみ**で行う。

**実装方針**:
```rust
// src/search/semantic.rs
// 責務: セマンティッククエリの実行（I/O）と結果の型変換（pure）

/// [I/O] セマンティッククエリを実行し、類似結果を返す
/// エラーはResult::Errで返す（graceful degradationはCLI層で判断）
/// embeddings.db不在時はOk(None)を返す（エラーではない）
pub fn query_semantic(
    embeddings_db_path: &Path,
    config: &AppConfig,
    query: &str,
    limit: usize,
) -> Result<Option<Vec<EmbeddingSimilarityResult>>, SemanticError> {
    // 1. embeddings.db存在確認 → 不在ならOk(None)（ログなし）
    // 2. EmbeddingStore::open()
    // 3. count() > 0 確認 → 0件ならOk(None)
    // 4. create_provider()
    // 5. provider.embed(&[query])
    // 6. store.search_similar(embedding, limit)
    // → Ok(Some(results))
}

/// [pure] EmbeddingSimilarityResultをSearchResult型に変換する（search.rs用）
/// 既存のsearch.rs::enrich_semantic_to_search_resultsを移動
pub fn enrich_semantic_to_search_results(
    semantic_results: &[EmbeddingSimilarityResult],
    reader: &IndexReaderWrapper,
) -> Result<Vec<SearchResult>, ReaderError> {
    // CLI層のSearchErrorへの変換はsearch.rs側でFrom implで行う
}

/// SemanticError: セマンティック検索固有のエラー型
pub enum SemanticError {
    StoreError(EmbeddingStoreError),
    ProviderError(EmbeddingError),
}
```

**CLI層でのgraceful degradation**:
```rust
// suggest.rs
let semantic_results = match query_semantic(&db_path, &config, &query, limit) {
    Ok(Some(results)) => Some(results),
    Ok(None) => None,           // DB不在・空 → セマンティックなし
    Err(e) => {
        eprintln!("[suggest] semantic search failed: {e}");
        None  // graceful degradation
    }
};
```

### DJ-4: ファイル集約・重み付けの共通化

**決定**: `src/search/ranking.rs`にファイル集約と重み付けロジックを配置。集約責務はranking.rsに一本化。

**理由**:
- suggest.rsは既に900行近いモジュール。RRF/セマンティック/集約/重み付けを全て追加するとSRP違反
- `deduplicate_by_file`と`aggregate_by_file`は同一ロジック（既存関数を移動・統合）
- `apply_file_type_weight`、`is_test_file`、`is_doc_file`もsearch層の責務
- セマンティック結果のファイル集約（旧`aggregate_semantic_by_file`）もranking.rsに統合（DRY化）

**実装方針**:
```rust
// src/search/ranking.rs
// 責務: ファイル単位集約（pure）、ファイルタイプ重み付け（pure）

/// [pure] BM25結果をファイル単位に集約（既存deduplicate_by_fileを移動）
pub fn aggregate_by_file(results: &[SearchResult]) -> Vec<(String, f32)>

/// [pure] セマンティック結果をファイル単位に集約（既存deduplicate_by_file_pairsを統合）
/// semantic.rsではなくranking.rsに配置（集約責務の一本化）
pub fn aggregate_similarity_by_file(results: &[EmbeddingSimilarityResult]) -> Vec<(String, f32)>

/// [pure] ファイルタイプ重み付けを適用し再ソート（既存apply_file_type_weightを移動）
pub fn apply_file_type_weight(files: Vec<(String, f32)>, limit: usize) -> Vec<(String, f32)>

/// [pure] テストファイル判定（既存is_test_fileを移動）
pub fn is_test_file(lower_path: &str) -> bool

/// [pure] ドキュメントファイル判定（既存is_doc_fileを移動）
pub fn is_doc_file(lower_path: &str) -> bool

/// [pure] ファイルタイプ重み係数（既存file_type_weight_factorを移動）
pub fn file_type_weight_factor(path: &str) -> f32
```

### DJ-5: file_type_weightの適用タイミング

**決定**: RRF統合**前**に各結果（BM25/セマンティック）に適用し再ソート

**理由**:
- RRFはランク順位ベース（`1/(K+rank)`）のため、スコア値の変更だけでは効果なし
- 重み付け→再ソートすることでRRFの入力ランクに反映される
- BM25側・セマンティック側両方に適用（テストファイルが両方に出現する場合、RRFで両方からペナルティ適用済みランクが使われる）

**二重減衰の考慮**: テストファイルがBM25・セマンティック両方で上位に来た場合、両方のランクが降格されるためRRFスコアが大きく下がる。これは意図した動作（テストファイルを強く抑制）だが、テストで検証する。

### DJ-6: maybe_add_semantic_stepの扱い

**決定**: 維持する

**理由**:
- suggestの出力は「次に叩くコマンド戦略」であり、検索結果そのものではない
- 内部でハイブリッド検索を使っても、ユーザーが`search --semantic`で別観点の探索を行う価値はある

### DJ-7: graceful degradationパターン

**決定**: `search.rs`の`try_hybrid_search`パターンを踏襲

**理由**:
- 実績のあるパターン（全エラーポイントでBM25フォールバック）
- 既存のembedding provider timeout（Ollama: 60秒、OpenAI: 30秒）をそのまま利用
- 新規タイムアウト機構は追加しない（YAGNI）

**エラーハンドリングフロー**:
```
query_semantic()  [src/search/semantic.rs]
  ├─ embeddings.db不在 → None（ログなし）
  ├─ EmbeddingStore::open()失敗 → None
  ├─ count() = 0 → None
  ├─ create_provider()失敗 → None
  ├─ provider.embed()失敗 → None（timeout含む）
  ├─ store.search_similar()失敗 → None
  └─ 成功 → Some(results)
```

### DJ-8: 既存関数の扱い

**決定**: 既存関数をリファクタリング（移動+統合）

| 既存関数 | 移動先 | 備考 |
|---------|-------|------|
| `suggest.rs::deduplicate_by_file` | `search/ranking.rs::aggregate_by_file` | リネーム+移動 |
| `suggest.rs::deduplicate_by_file_pairs` | **削除** | `semantic.rs::aggregate_semantic_by_file`に統合 |
| `suggest.rs::apply_file_type_weight` | `search/ranking.rs` | 移動 |
| `suggest.rs::is_test_file` | `search/ranking.rs` | 移動 |
| `suggest.rs::is_doc_file` | `search/ranking.rs` | 移動 |
| `suggest.rs::file_type_weight_factor` | `search/ranking.rs` | 移動 |
| `suggest.rs::try_semantic_fallback` | **削除** | `semantic.rs::query_semantic`に統合 |
| `suggest.rs::search_entry_files` | **変更なし** | BM25検索部分は現行維持 |
| `search.rs::enrich_semantic_to_search_results` | `search/semantic.rs` | 移動、エラー型をReaderErrorに変更 |

**search_entry_filesの扱い**: `search_entry_files`は変更しない。内部でBM25検索+`deduplicate_by_file`+`apply_file_type_weight`を一括実行する現行ロジックを維持。ハイブリッド統合は`run_suggest`のオーケストレーション層で行う。

**注意**: `search_entry_files`は内部で`DEDUP_FILE_LIMIT=5`件にtruncateするため、RRFに渡すBM25候補が不足する。新しいフローではBM25結果の取得を`search_entry_files`の内部ロジック（`reader.search` → `aggregate_by_file` → `apply_file_type_weight`）を直接呼び出す形に変更し、truncateは`DEDUP_FILE_LIMIT * 3`（RRF用にやや多め）にする。`search_entry_files`自体は既存のBM25-only パスで使用を維持。

## 5. データフロー設計

### 変更後のrun_suggest()フロー

```
run_suggest(for_task, format, index_path)
  │
  ├─ validate_input(for_task)
  ├─ SearchContext::new()
  ├─ IndexReaderWrapper::open()
  │
  ├─ BM25検索: reader.search(query, BM25_SEARCH_LIMIT)
  ├─ ファイル集約: ranking::aggregate_by_file(bm25_results)
  ├─ weight適用: ranking::apply_file_type_weight(bm25_files, DEDUP_FILE_LIMIT * 3)
  │
  ├─ セマンティック検索:
  │   semantic::query_semantic(db_path, config, query, SEMANTIC_FALLBACK_LIMIT)
  │   ├─ None → セマンティック結果なし
  │   └─ Some(results):
  │       ├─ semantic::aggregate_semantic_by_file(results)
  │       └─ ranking::apply_file_type_weight(semantic_files, DEDUP_FILE_LIMIT * 3)
  │
  ├─ 結果統合:
  │   ├─ セマンティック結果あり → hybrid::rrf_merge_files(bm25, semantic, DEDUP_FILE_LIMIT)
  │   └─ セマンティック結果なし → bm25_files.truncate(DEDUP_FILE_LIMIT)
  │
  ├─ build_strategy(emb_store, entry_files, query)
  │   └─ maybe_add_semantic_step() [維持]
  │
  └─ output::format_suggest_results()
```

### 既存run_suggestからの具体的な分岐変更

**現在のコード（L467-476）**:
```rust
if entry_files.is_empty() {
    // BM25結果0件 → セマンティックフォールバック試行
    if let Some(semantic_files) = try_semantic_fallback(&ctx, &query) {
        // セマンティック結果で戦略生成
    } else {
        // フォールバック戦略
    }
} else {
    // BM25結果あり → そのまま戦略生成
}
```

**変更後**:
```rust
// BM25検索（ファイル集約・weight適用済み）
let bm25_files = /* ... */;

// 常にセマンティック検索を試行（BM25結果の有無に関わらず）
let semantic_files = query_semantic(&ctx.embeddings_db_path(), &ctx.config, &query, SEMANTIC_FALLBACK_LIMIT);

// 結果統合
let entry_files = match (bm25_files.is_empty(), semantic_files) {
    (false, Some(sem)) => rrf_merge_files(&bm25_files, &sem, DEDUP_FILE_LIMIT),
    (false, None) => { bm25_files.truncate(DEDUP_FILE_LIMIT); bm25_files },
    (true, Some(sem)) => { sem.truncate(DEDUP_FILE_LIMIT); sem },
    (true, None) => vec![], // フォールバック戦略
};

if entry_files.is_empty() {
    build_fallback_strategy(has_embeddings)
} else {
    build_strategy(emb_store.as_ref(), &entry_files, &query)
}
```

## 6. 影響範囲

### 変更ファイル一覧

| ファイル | 変更内容 | 影響度 |
|---------|---------|--------|
| `src/cli/suggest.rs` | フォールバック→ハイブリッド化、関数移動後の呼び出し変更 | **高** |
| `src/search/mod.rs` | `pub mod semantic; pub mod ranking;` 追加 | 低 |
| `src/search/semantic.rs` | **新規**: query_semantic, aggregate_semantic_by_file, enrich_semantic_to_search_results | 中 |
| `src/search/ranking.rs` | **新規**: aggregate_by_file, apply_file_type_weight, is_test_file, is_doc_file等を移動 | 中 |
| `src/search/hybrid.rs` | `pub const RRF_K`, `rrf_merge_files`追加 | 低 |
| `src/cli/search.rs` | enrich呼び出しをsearch::semantic経由に変更、SearchErrorにFrom<ReaderError>追加 | 低 |
| `tests/e2e_suggest.rs` | 新規テスト追加 | 中 |

### 変更しないファイル

| ファイル | 理由 |
|---------|------|
| `src/embedding/store.rs` | API変更なし |
| `src/embedding/mod.rs` | API変更なし |
| `src/indexer/reader.rs` | API変更なし |
| `Cargo.toml` | 外部依存追加なし |

### 出力互換性

- SuggestResult / SuggestStep の構造体フィールドは変更なし
- ランキング順序は改善されるが出力スキーマは完全互換
- embedding未構築環境では従来と同一の挙動

## 7. セキュリティ設計

| 脅威 | 対策 | 優先度 |
|------|------|--------|
| パストラバーサル | 既存のSearchContext::newでベースディレクトリチェック済み。変更なし | - |
| APIキー漏洩 | 既存のEmbeddingConfig::resolve_api_keyで環境変数から取得。ログ出力にキーを含めない | 中 |
| unsafe使用 | 禁止（本変更でunsafe不要） | - |
| validate_input | 既存のMAX_INPUT_LENGTH=500はバイト数制限（マルチバイト文字ではより少ない文字数に制限）。セキュリティ上はバイト数のほうが安全 | - |

## 8. テスト戦略

### 単体テスト

| テスト | 配置 | 内容 |
|-------|------|------|
| `test_rrf_merge_files_basic` | `search/hybrid.rs` | 2リストのRRF統合が正しいランキングを返す |
| `test_rrf_merge_files_disjoint` | `search/hybrid.rs` | 共通ファイルなしの場合の統合 |
| `test_rrf_merge_files_single_source` | `search/hybrid.rs` | 片方のみの場合（BM25のみ/セマンティックのみ） |
| `test_aggregate_by_file_basic` | `search/ranking.rs` | BM25結果のファイル単位集約 |
| `test_aggregate_by_file_empty` | `search/ranking.rs` | 空入力のハンドリング |
| `test_aggregate_semantic_by_file` | `search/semantic.rs` | セマンティック結果のファイル単位集約 |
| `test_file_type_weight_with_rrf` | `suggest.rs` | weight適用→再ソート→RRFの一連フロー |
| `test_double_weight_penalty` | `suggest.rs` | テストファイルがBM25+semantic両方に出現する場合のRRF結果 |

### 統合テスト（tests/e2e_suggest.rs）

| テスト | 内容 |
|-------|------|
| 既存テスト（6件） | embedding未構築環境での既存動作を維持（**回帰テスト**） |
| `test_suggest_bm25_only_graceful_degradation` | embedding DB不在時にBM25のみで動作 |
| `test_suggest_provider_failure_fallback` | 存在しないOllamaホストを設定してタイムアウト/接続エラー時にBM25フォールバック |

### テスト実行方法

- provider障害テスト: 存在しないホスト（例: `http://localhost:99999`）をembedding configに設定してエラーを発生させる
- 既存のsearch.rs #[cfg(test)]内テスト（rerank関連7件）: 今回の変更で影響なし（確認済み）

## 9. 品質基準

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |
