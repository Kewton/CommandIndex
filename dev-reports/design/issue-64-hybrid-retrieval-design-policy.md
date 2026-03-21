# 設計方針書: Issue #64 Hybrid Retrieval（BM25 + Semantic統合検索）

## 1. 概要

BM25（全文検索）とSemantic（意味検索）のスコアをReciprocal Rank Fusion (RRF) で統合し、ハイブリッド検索を実装する。`query` 引数使用時にembeddingが存在すれば自動的にハイブリッドモードになる。

## 2. システムアーキテクチャ概要

### レイヤー構成と責務

| レイヤー | モジュール | 今回の変更 |
|---------|-----------|-----------|
| **CLI** | `src/main.rs` | `--no-semantic` オプション追加、パターンマッチ更新 |
| **CLI/Search** | `src/cli/search.rs` | `run()` にハイブリッド統合ロジック追加 |
| **Search** | `src/search/hybrid.rs` | **新規**: RRFスコア統合アルゴリズム（純粋関数） |
| **Search** | `src/search/mod.rs` | `pub mod hybrid;` 追加 |
| **Indexer** | `src/indexer/reader.rs` | コード変更なし（新規呼び出しパスあり: セマンティックのみヒット時のメタデータ取得） |
| **Indexer** | `src/indexer/symbol_store.rs` | 変更なし（embedding検索） |
| **Output** | `src/output/` | 変更なし（SearchResult型再利用） |
| **Embedding** | `src/embedding/` | 変更なし |

### データフロー

```
                     query引数
                         │
                    ┌────▼────┐
                    │ main.rs │  CLIオプション解析
                    └────┬────┘
                         │
                    ┌────▼────────┐
                    │ cli/search  │  オーケストレーション
                    │   run()     │
                    └─┬────────┬──┘
                      │        │
              ┌───────▼──┐  ┌──▼──────────┐
              │ tantivy   │  │ embedding   │
              │ BM25検索  │  │ provider    │
              │ (reader)  │  │ + SQLite    │
              └───────┬──┘  └──┬──────────┘
                      │        │
                 ランキングA  ランキングB
                      │        │
                      │   ┌────▼─────────┐
                      │   │ enrich_with  │  メタデータ付与
                      │   │ _metadata()  │  (SearchResult化)
                      │   └────┬─────────┘
                      │        │
                    ┌──▼────────▼──┐
                    │ search/hybrid │  RRF統合（純粋関数）
                    │  rrf_merge()  │
                    └──────┬───────┘
                           │
                    ┌──────▼──────┐
                    │ output/     │  フォーマット出力
                    │ format_results()
                    └─────────────┘
```

### 責務分離

| モジュール | 責務 |
|-----------|------|
| `src/cli/search.rs` (`run()`) | オーケストレーション: ハイブリッド判定、BM25実行、セマンティック実行、メタデータエンリッチ、フォールバック |
| `src/search/hybrid.rs` (`rrf_merge()`) | 純粋関数: RRFスコア計算と結果統合のみ。副作用なし、I/Oなし |

## 3. 詳細設計

### 3.1 CLIオプション設計（src/main.rs）

```rust
Search {
    query: Option<String>,
    #[arg(long, conflicts_with_all = ["query", "semantic"])]
    symbol: Option<String>,
    #[arg(long, conflicts_with_all = ["query", "symbol", "semantic", "tag", "path", "file_type", "heading"])]
    related: Option<String>,
    #[arg(long, conflicts_with_all = ["query", "symbol", "related", "heading"])]
    semantic: Option<String>,

    // 新規追加: queryモード専用、semantic/symbol/relatedと競合
    #[arg(long, conflicts_with_all = ["semantic", "symbol", "related"])]
    no_semantic: bool,

    // 既存オプション（変更なし）
    format: OutputFormat,
    tag: Option<String>,
    path: Option<String>,
    file_type: Option<String>,
    heading: Option<String>,
    limit: usize,
}
```

**パターンマッチ更新**:
```rust
// Full-text search (query引数)
(Some(q), None, None, None) => {
    let options = SearchOptions { query: q, tag, heading, limit, no_semantic };
    let filters = SearchFilters { path_prefix: path, file_type };
    commandindex::cli::search::run(&options, &filters, format)
}
```

### 3.2 SearchOptions拡張（src/indexer/reader.rs）

```rust
pub struct SearchOptions {
    pub query: String,
    pub tag: Option<String>,
    pub heading: Option<String>,
    pub limit: usize,
    pub no_semantic: bool,  // 新規追加
}
```

`no_semantic` を `SearchOptions` に統合することで、`run()` のシグネチャを変更せずに済む。`heading` フィールドの有無からハイブリッド判定も `run()` 内部で行える。

### 3.3 ハイブリッド検索オーケストレーション（src/cli/search.rs）

`run()` 関数のシグネチャは**変更なし**:

```rust
pub fn run(
    options: &SearchOptions,
    filters: &SearchFilters,
    format: OutputFormat,
) -> Result<(), SearchError>
```

**ハイブリッド判定ロジック**（run()内部）:

```rust
// ハイブリッド検索の使用判定
// - no_semanticフラグがfalse
// - heading フィルタなし（headingはBM25固有機能）
let use_hybrid = !options.no_semantic && options.heading.is_none();
```

**try_hybrid_search()** - セマンティック検索 + エンリッチ + RRF統合:

```rust
/// BM25結果にセマンティック結果を統合してRRFスコアを計算する。
/// セマンティック側の全エラーはキャッチしてBM25結果のみを返す（後方互換性）。
fn try_hybrid_search(
    bm25_results: Vec<SearchResult>,
    options: &SearchOptions,
    filters: &SearchFilters,
) -> Vec<SearchResult> {
    // 1. SymbolStore::open(symbol_db_path) → 不在なら BM25のみ + stderr情報
    // 2. store.count_embeddings() == 0 チェック → BM25のみ + stderr情報
    // 3. EmbeddingConfig読み込み → provider生成（失敗→BM25のみ + stderr警告）
    // 4. provider.embed(query) → クエリ埋め込み生成（失敗→BM25のみ + stderr警告）
    // 5. store.search_similar(query_embedding, limit * HYBRID_OVERSAMPLING_FACTOR)
    //    → セマンティック結果取得（失敗→BM25のみ + stderr警告）
    // 6. enrich_semantic_to_search_results() → SearchResult[]に変換
    //    （enrich失敗→BM25のみ + stderr警告。ただしローカルindex破損はfail-fast）
    // 7. apply_semantic_filters() → tag/path/file_typeフィルタ適用
    // 8. rrf_merge(bm25_results, filtered_semantic, limit) → 統合結果
}
```

**注意**: `try_hybrid_search()` は `Result` ではなく `Vec<SearchResult>` を返す。エラー時はBM25結果をそのまま返す（フォールバック）。

### 3.4 セマンティック結果のエンリッチ（src/cli/search.rs）

```rust
/// EmbeddingSimilarityResult を SearchResult に変換する。
/// 既存の enrich_with_metadata パターンを参考に、
/// tantivy search_by_exact_path でメタデータを取得してSearchResultを構築する。
fn enrich_semantic_to_search_results(
    semantic_results: &[EmbeddingSimilarityResult],
    reader: &IndexReaderWrapper,
) -> Vec<SearchResult> {
    // 各セマンティック結果に対して:
    // 1. reader.search_by_exact_path(file_path) でtantivyからメタデータ取得
    // 2. section_heading とマッチするドキュメントを探す
    // 3. SearchResult { path, heading, body, tags, heading_level, line_start, score: 0.0 } を構築
    //    (scoreは後でrrf_mergeで上書きされる)
    // 4. マッチしない場合はbody=空、heading_level=0 のフォールバック結果
}
```

### 3.5 RRFスコア統合アルゴリズム（src/search/hybrid.rs）

**純粋関数**: I/Oなし、副作用なし。テスタブル。

```rust
use crate::indexer::reader::SearchResult;

/// RRF定数（業界標準値）
/// Reference: Cormack et al., "Reciprocal Rank Fusion outperforms Condorcet and individual Rank Learning Methods"
const RRF_K: f32 = 60.0;

/// ハイブリッド検索用Oversampling倍率
/// 注: BM25側のOVERSAMPLING_FACTOR(=5, reader.rs)はpost-filter用で目的が異なる
pub const HYBRID_OVERSAMPLING_FACTOR: usize = 3;

/// RRFでBM25結果とセマンティック結果を統合する。
/// 両方の入力はSearchResult型（セマンティック側は事前にエンリッチ・フィルタ済み）。
/// score フィールドにRRFスコアを格納して返す。
///
/// ランクは1-based（1位が最高ランク）。
/// 片方のランキングにのみ存在するドキュメント:
///   標準RRFに従い、未出現側の寄与は0とする。
///   例: BM25のみヒット → score = 1/(k + bm25_rank) + 0
pub fn rrf_merge(
    bm25_results: &[SearchResult],
    semantic_results: &[SearchResult],
    limit: usize,
) -> Vec<SearchResult> {
    // 1. BM25結果からランクマップを構築: (path, heading) → (1-based rank, &SearchResult)
    // 2. セマンティック結果からランクマップを構築: (path, heading) → (1-based rank, &SearchResult)
    // 3. 全ドキュメントの和集合を取得
    // 4. 各ドキュメントのRRFスコアを計算
    //    両方に存在: score = 1/(k + bm25_rank) + 1/(k + semantic_rank)
    //    片方のみ: score = 1/(k + rank) + 0  ← 未出現側の寄与は0
    // 5. RRFスコア降順でソート（同点時は(path, heading)辞書順で安定ソート）
    // 6. 上位limit件を返す（scoreフィールドにRRFスコアを格納）
}
```

**設計ポイント**:
- 入力は両方 `&[SearchResult]` に統一（セマンティック側は呼び出し前にエンリッチ済み）
- `rrf_merge()` はI/Oなしの純粋関数 → 単体テストが容易
- `EmbeddingSimilarityResult` への依存なし → 型の不整合を回避

### 3.6 エラーハンドリング

**エラー分類**:

| カテゴリ | エラー例 | query引数時の挙動 |
|---------|---------|------------------|
| **外部依存の一時障害** | provider生成失敗、embed()失敗、search_similar()失敗、API/ネットワークエラー | BM25フォールバック + stderr警告 |
| **データ不在** | symbols.db不在、count_embeddings()==0、config.toml不在 | BM25フォールバック + stderr情報 |
| **ローカルindex/schema破損** | SchemaVersionMismatch、InvalidEmbedding | **fail-fast**（SearchErrorとして返す） |
| **BM25検索エラー** | Tantivy/Queryエラー | **fail-fast**（SearchErrorとして返す） |
| **enrich失敗** | search_by_exact_path()失敗 | BM25フォールバック + stderr警告 |

```
query引数時のエラー分岐（run()内部）:
├── use_hybrid == false → BM25のみ実行
├── use_hybrid == true
│   ├── try_hybrid_search()内でエラー
│   │   ├── symbols.db不在 → BM25のみ + stderr情報
│   │   ├── count_embeddings()==0 → BM25のみ + stderr情報
│   │   ├── EmbeddingConfig/config.toml不在 → BM25のみ + stderr情報
│   │   ├── provider生成失敗 → BM25のみ + stderr警告
│   │   ├── embed()失敗 → BM25のみ + stderr警告
│   │   ├── search_similar()失敗 → BM25のみ + stderr警告
│   │   ├── enrich_semantic失敗 → BM25のみ + stderr警告
│   │   ├── SchemaVersionMismatch → **fail-fast** SearchError
│   │   └── InvalidEmbedding → **fail-fast** SearchError
│   └── 正常 → RRF統合結果
└── BM25検索自体のエラー → SearchError として返す（従来通り）

--semantic明示時のエラー分岐（変更なし）:
├── 各種エラー → SearchError として返す
└── 正常 → セマンティック検索結果
```

**stderrメッセージ設計**:
- 情報メッセージ: `"Hint: No embeddings found. Run 'commandindex embed' to enable hybrid search."`
- 警告メッセージ: `"Warning: Semantic search unavailable ({reason}). Using BM25 only."`
- 出力ルール: フォールバック理由は**1回のみ**出力。BM25結果0件時の "No results found." より前に出力する。
- 既存E2Eテストは embeddings.db 不在環境で実行されるため、stderrメッセージはBM25検索後に出力し、既存の "No results found." メッセージと衝突しないようにする。

**ストレージ前提**: ハイブリッド検索は既存の `run_semantic_search()` と同じストレージを使用する。
- `SymbolStore::open(symbol_db_path)` で symbols.db を開く（`crate::indexer::symbol_db_path(Path::new("."))` で取得）
- `store.count_embeddings()` で埋め込み件数を確認
- `store.search_similar()` でコサイン類似度検索
- **注意**: プロジェクトには symbols.db（SymbolStore）と embeddings.db（EmbeddingStore）の2つのDBが存在する。検索系はすべてSymbolStore（symbols.db）を使用する。これは既存のrun_semantic_search()と同じパターン。

## 4. 設計判断とトレードオフ

### 判断1: SearchResult型再利用 vs 新HybridSearchResult型
- **決定**: SearchResult型を再利用
- **理由**: 既存の `format_results` フォーマッタをそのまま利用でき、出力モジュールの変更が不要
- **トレードオフ**: `score` フィールドの意味が検索モードにより異なる（BM25スコア or RRFスコア）
- **対策**: SearchResult.score のドキュメントコメントに「ハイブリッド検索時はRRFスコア、BM25検索時はtantivy BM25スコア」と明記
- **将来拡張**: 必要に応じて内部で `HybridSearchResult` を持ち、最終出力時に `SearchResult` に変換可能

### 判断2: query引数時のフォールバック vs エラー
- **決定**: query引数時はセマンティック側障害でBM25フォールバック
- **理由**: 後方互換性維持。従来のBM25のみの検索がネットワーク障害等で失敗するのは許容できない
- **トレードオフ**: セマンティック側の問題がサイレントに無視される可能性（stderr警告で対処）

### 判断3: RRF（ランクベース） vs スコア正規化方式
- **決定**: RRF (k=60)
- **理由**: BM25スコア(0〜任意値)とコサイン類似度(0〜1)のスケールが異なり、正規化が困難。RRFはランク順位のみ使用するため正規化不要
- **トレードオフ**: スコアの絶対値情報が失われる

### 判断4: query + --heading時のBM25のみ動作
- **決定**: --heading指定時はハイブリッド化しない
- **理由**: --headingはtantivy BM25固有のフィルタで、セマンティック側に対応する機能がない
- **トレードオフ**: --heading使用時はセマンティックの恩恵を受けられない

### 判断5: 均等重みRRF vs 重み付きRRF
- **決定**: 均等重みRRF（重みオプション非実装）
- **理由**: 標準RRFはランクベースで重みの概念がない。まず基本実装を行い、効果を検証してから重み調整を検討
- **トレードオフ**: BM25/Semantic の重要度をユーザーが調整できない

### 判断6: Oversampling倍率
- **決定**: `limit * 3`（定数 `HYBRID_OVERSAMPLING_FACTOR = 3`）
- **理由**: 既存のセマンティック検索は `limit * 5` を使用しているが、RRFでは両方の結果を統合するため、各側の候補数は少なめでもrecall十分
- **トレードオフ**: oversamplingが少なすぎるとrecallが低下
- **注意**: 既存の `OVERSAMPLING_FACTOR=5`（reader.rs）はpost-filter用で目的が異なる。二重管理ではない。

### 判断7: rrf_merge()を純粋関数として設計
- **決定**: rrf_merge()は&[SearchResult]のみを受け取り、I/O・副作用なし
- **理由**: テスタブル性、単一責任原則。メタデータ取得はオーケストレーション層（cli/search.rs）の責務
- **トレードオフ**: セマンティック結果のエンリッチがrrf_merge呼び出し前に必要

### 判断8: no_semanticをSearchOptionsに統合
- **決定**: SearchOptionsにno_semanticフィールドを追加し、run()シグネチャは変更しない
- **理由**: KISS原則。boolフラグを個別引数で渡すとシグネチャが肥大化する。has_heading_filterはoptions.heading.is_some()から導出可能

## 5. セキュリティ設計

| 脅威 | 対策 | 優先度 |
|------|------|--------|
| パストラバーサル | 既存のファイル操作パスはインデックス済みパスのみ参照 | 低 |
| unsafe使用 | 禁止（既存方針維持） | 中 |
| API Key漏洩 | EmbeddingConfig.resolve_api_key()の既存メカニズムを利用 | 低（変更なし） |
| **通常検索での外部送信** | auto-hybridにより通常`search <query>`がembedding providerへHTTPリクエストを送信する経路が新設 | **中** |

**auto-hybridによる新規リスク**: 従来はBM25のみでローカル完結していた通常検索が、embedding存在時に自動的にクエリテキストを外部API（Ollama/OpenAI）に送信する。
- **対策**: embedding provider設定はユーザーが明示的に`commandindex embed`を実行した時点で設定済み。つまりユーザーは外部送信を承知の上でembeddingを有効化している。
- **追加対策**: `--no-semantic`で外部送信を回避可能。フォールバック時は外部送信しない。
- **注意**: embedding provider未設定（config.toml不在）の場合はBM25のみで動作し、外部送信は発生しない。

## 6. 影響範囲

### 変更対象

| ファイル | 変更種別 | 変更内容 |
|---------|---------|---------|
| `src/main.rs` | 修正 | `--no-semantic` オプション追加、パターンマッチ更新 |
| `src/cli/search.rs` | 修正 | `run()` 内部にハイブリッド統合ロジック追加（シグネチャ変更なし） |
| `src/indexer/reader.rs` | 修正 | `SearchOptions` に `no_semantic` フィールド追加 |
| `src/search/hybrid.rs` | 新規 | RRFスコア統合アルゴリズム（純粋関数） |
| `src/search/mod.rs` | 修正 | `pub mod hybrid;` 追加 |
| `tests/cli_args.rs` | 修正 | `--no-semantic` CLIパーステスト追加 |

### 影響を受けないファイル

- `src/output/` - SearchResult型を再利用するためフォーマッタ変更なし
- `src/embedding/` - 変更なし
- `src/parser/` - 変更なし
- `src/search/related.rs` - 変更なし
- `src/cli/embed.rs`, `src/cli/index.rs` 等 - 他サブコマンドは変更なし

### 間接的影響

- `src/indexer/reader.rs` - コード変更はSearchOptions拡張のみ。search_by_exact_pathへの新規呼び出しパスが追加されるが、メソッド自体は変更なし
- `src/cli/context.rs` - SearchError型をインポートしているが、新バリアント追加なしのため影響なし

## 7. テスト戦略

### 単体テスト（src/search/hybrid.rs）

| テストケース | 内容 |
|-------------|------|
| `test_rrf_both_rankings` | BM25・Semantic両方にヒットするドキュメントのRRFスコア計算 |
| `test_rrf_bm25_only` | BM25のみにヒットするドキュメントのRRFスコア |
| `test_rrf_semantic_only` | Semanticのみにヒットするドキュメントのスコア |
| `test_rrf_empty_results` | 両方空の場合 |
| `test_rrf_stable_sort` | 同点時の(path, heading)辞書順ソート |
| `test_rrf_limit` | limit件に正しく絞り込まれること |

### CLIパーステスト（tests/cli_args.rs）

| テストケース | 内容 |
|-------------|------|
| `test_no_semantic_accepted` | `--no-semantic` が受理されること |
| `test_no_semantic_conflicts_with_semantic` | `--no-semantic` + `--semantic` が競合 |
| `test_no_semantic_conflicts_with_symbol` | `--no-semantic` + `--symbol` が競合 |
| `test_no_semantic_conflicts_with_related` | `--no-semantic` + `--related` が競合 |

### テスト影響対象ファイル

| ファイル | 影響 |
|---------|------|
| `tests/cli_args.rs` | SearchOptions構造体リテラル修正（no_semantic追加）、--no-semanticテスト追加 |
| `tests/indexer_tantivy.rs` | SearchOptions構造体リテラル修正（no_semantic追加） |
| `tests/common/mod.rs` | search helper関数の影響確認 |
| 既存E2Eテスト全般 | CLI経由のため直接的影響なし。embedding不在時にBM25フォールバックで結果同一を確認 |

### E2Eテスト影響確認

| テストケース | 内容 |
|-------------|------|
| 既存E2Eテスト回帰確認 | embedding不在環境でBM25フォールバックし、既存テストの結果が変わらないこと |
| stderrメッセージ確認 | フォールバック時のstderrメッセージが既存テストのアサーションと衝突しないこと |
| store不整合テスト | symbols.db不在、schema mismatch時の挙動確認 |

## 8. 品質基準

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |
