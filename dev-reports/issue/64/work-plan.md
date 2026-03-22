# 作業計画書: Issue #64 Hybrid Retrieval（BM25 + Semantic統合検索）

## Issue概要
- **Issue番号**: #64
- **タイトル**: [Feature] Hybrid Retrieval（BM25 + Semantic統合検索）
- **サイズ**: M
- **優先度**: High
- **依存Issue**: #63 Semantic Search（実装済み）
- **ブランチ**: `feature/issue-64-hybrid-retrieval`（既存）

## 詳細タスク分解

### Phase 1: コアアルゴリズム実装（TDD）

#### Task 1.1: SearchOptions拡張
- **成果物**: `src/indexer/reader.rs`
- **依存**: なし
- **変更内容**:
  - `SearchOptions` に `no_semantic: bool` フィールド追加（L14-19）
  - `#[derive(Debug, Clone)]` は維持
- **テスト影響**:
  - `tests/indexer_tantivy.rs`: 6箇所のSearchOptionsリテラルに `no_semantic: false` 追加（L297, 316, 334, 352, 370, 392）
  - `src/main.rs` L155-159: SearchOptions構築時に `no_semantic` 追加
  - `src/indexer/reader.rs` L97-104: `search()` wrapper内のSearchOptions構築に `no_semantic: false` 追加

#### Task 1.2: RRFスコア統合アルゴリズム（src/search/hybrid.rs 新規作成）
- **成果物**: `src/search/hybrid.rs`, `src/search/mod.rs`
- **依存**: なし
- **実装内容**:
  - `RRF_K: f32 = 60.0` 定数
  - `HYBRID_OVERSAMPLING_FACTOR: usize = 3` 定数
  - `rrf_merge(bm25: &[SearchResult], semantic: &[SearchResult], limit: usize) -> Vec<SearchResult>`
    - 純粋関数（I/Oなし、副作用なし）
    - ランクは1-based
    - 片側のみヒット: 未出現側の寄与=0（標準RRF準拠）
    - 同点時は(path, heading)辞書順で安定ソート
    - scoreフィールドにRRFスコアを格納
  - `src/search/mod.rs` に `pub mod hybrid;` 追加
- **テスト（TDD: テスト先行）**:
  - `test_rrf_both_rankings`: 両方ヒットのRRFスコア計算
  - `test_rrf_bm25_only`: BM25のみヒット（score = 1/(k+rank) + 0）
  - `test_rrf_semantic_only`: Semanticのみヒット
  - `test_rrf_empty_results`: 両方空
  - `test_rrf_stable_sort`: 同点時の安定ソート
  - `test_rrf_limit`: limit件に正しく絞り込み
  - `test_rrf_score_calculation`: 具体的なスコア値の検証

### Phase 2: CLIオプション追加

#### Task 2.1: --no-semantic オプション追加（src/main.rs）
- **成果物**: `src/main.rs`
- **依存**: Task 1.1
- **変更内容**:
  - Search enum に `#[arg(long, conflicts_with_all = ["semantic", "symbol", "related"])] no_semantic: bool` 追加
  - パターンマッチ (Some(q), None, None, None) 内で `no_semantic` を `SearchOptions` に渡す
- **テスト（TDD）**:
  - `tests/cli_args.rs` に追加:
    - `test_no_semantic_accepted`: `--no-semantic` が受理される
    - `test_no_semantic_conflicts_with_semantic`: 競合
    - `test_no_semantic_conflicts_with_symbol`: 競合
    - `test_no_semantic_conflicts_with_related`: 競合

### Phase 3: ハイブリッド統合オーケストレーション

#### Task 3.1: セマンティック結果エンリッチ関数（src/cli/search.rs）
- **成果物**: `src/cli/search.rs`
- **依存**: Task 1.1
- **実装内容**:
  - `enrich_semantic_to_search_results(semantic_results: &[EmbeddingSimilarityResult], reader: &IndexReaderWrapper) -> Vec<SearchResult>`
  - 既存の `enrich_with_metadata` パターンを参考
  - `search_by_exact_path` + `section_heading` マッチでSearchResult構築
  - マッチ失敗時はbody=空、heading_level=0のフォールバック

#### Task 3.2: try_hybrid_search関数（src/cli/search.rs）
- **成果物**: `src/cli/search.rs`
- **依存**: Task 1.2, Task 3.1
- **実装内容**:
  - `try_hybrid_search(bm25_results: Vec<SearchResult>, options: &SearchOptions, filters: &SearchFilters) -> Vec<SearchResult>`
  - 手順:
    1. SymbolStore::open(symbol_db_path)
    2. store.count_embeddings() チェック
    3. EmbeddingConfig読み込み → provider生成
    4. provider.embed(query) → クエリ埋め込み
    5. store.search_similar(query_embedding, limit * HYBRID_OVERSAMPLING_FACTOR)
    6. enrich_semantic_to_search_results()
    7. apply_semantic_filters() でtag/path/file_typeフィルタ適用
    8. rrf_merge(bm25, filtered_semantic, limit)
  - エラーハンドリング:
    - 外部依存一時障害 → BM25結果そのまま返す + stderr警告
    - ローカルindex/schema破損(SchemaVersionMismatch等) → panic/SearchError
  - stderrメッセージ: フォールバック理由1回のみ出力

#### Task 3.3: run()関数のハイブリッド統合（src/cli/search.rs）
- **成果物**: `src/cli/search.rs`
- **依存**: Task 3.2
- **変更内容**:
  - run() 内部でハイブリッド判定: `!options.no_semantic && options.heading.is_none()`
  - ハイブリッド判定true → BM25検索後に `try_hybrid_search()` 呼び出し
  - ハイブリッド判定false → 従来のBM25のみ（変更なし）

### Phase 4: テスト・品質確認

#### Task 4.1: 既存テスト修正
- **成果物**: `tests/indexer_tantivy.rs`
- **依存**: Task 1.1
- **変更内容**: 6箇所のSearchOptionsリテラルに `no_semantic: false` 追加

#### Task 4.2: 品質チェック
- **依存**: 全タスク完了後
- `cargo build` → エラー0件
- `cargo clippy --all-targets -- -D warnings` → 警告0件
- `cargo test --all` → 全テストパス
- `cargo fmt --all -- --check` → 差分なし

## 実装順序

```
Task 1.1 (SearchOptions拡張) ──┐
Task 1.2 (RRF hybrid.rs)  ────┤
                               ├──→ Task 2.1 (--no-semantic CLI)
Task 3.1 (enrich関数)     ────┤
                               ├──→ Task 3.2 (try_hybrid_search)
                               │              │
                               │              ▼
                               ├──→ Task 3.3 (run()統合)
                               │              │
Task 4.1 (既存テスト修正)  ────┘              ▼
                                    Task 4.2 (品質チェック)
```

## Definition of Done

- [ ] すべてのタスクが完了
- [ ] `cargo test --all` 全パス
- [ ] `cargo clippy --all-targets -- -D warnings` 警告ゼロ
- [ ] `cargo fmt --all -- --check` 差分なし
- [ ] 受け入れ基準（Issue #64）の全項目を満たす
