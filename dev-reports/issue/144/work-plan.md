# 作業計画書: Issue #144 - suggest の英語クエリ精度改善

## Issue概要

| 項目 | 内容 |
|------|------|
| **Issue番号** | #144 |
| **タイトル** | suggest の英語クエリ精度改善 |
| **サイズ** | M |
| **優先度** | High |
| **依存Issue** | #134 (BGE-M3, 完了済み) |
| **設計方針書** | `dev-reports/design/issue-144-design-policy.md` |

## 作業タスク

### Phase 1: Search層の共通モジュール作成（テスト駆動）

#### Task 1.1: `src/search/ranking.rs` 新規作成
- **成果物**: `src/search/ranking.rs`, `src/search/mod.rs`更新
- **依存**: なし
- **内容**:
  1. `src/search/mod.rs` に `pub mod ranking;` 追加
  2. `suggest.rs` から以下の関数を移動:
     - `is_test_file` → `ranking::is_test_file`
     - `is_doc_file` → `ranking::is_doc_file`
     - `file_type_weight_factor` → `ranking::file_type_weight_factor`
     - `apply_file_type_weight` → `ranking::apply_file_type_weight`
     - `deduplicate_by_file` → `ranking::aggregate_by_file`（リネーム）
  3. `aggregate_similarity_by_file` 新規作成（`deduplicate_by_file_pairs`の置き換え）
  4. 定数 `TEST_FILE_WEIGHT`, `DOC_FILE_WEIGHT` も移動
  5. `suggest.rs` の既存テストも `ranking.rs` の `#[cfg(test)]` に移動
  6. `suggest.rs` から移動元の関数を削除し、`use crate::search::ranking::*` で参照
- **テスト**:
  - 既存テスト（`test_is_test_file_*`, `test_is_doc_file_*`, `test_deduplicate_by_file_*`, `test_apply_file_type_weight_*` 等）を移動して全パス確認
  - `test_aggregate_similarity_by_file_basic` 新規追加
  - `test_aggregate_similarity_by_file_empty` 新規追加

#### Task 1.2: `src/search/hybrid.rs` にファイル単位RRF追加
- **成果物**: `src/search/hybrid.rs` 更新
- **依存**: なし（ranking.rsと並行可能）
- **内容**:
  1. 非公開共通ヘルパー `compute_rrf_scores` 追加
  2. `pub fn rrf_merge_files` 追加
  3. 既存 `rrf_merge` / `rrf_merge_multiple` は変更なし
- **テスト**:
  - `test_rrf_merge_files_basic`: 2リストの統合が正しいランキングを返す
  - `test_rrf_merge_files_disjoint`: 共通ファイルなしの場合
  - `test_rrf_merge_files_single_source`: 片方空の場合
  - `test_rrf_merge_files_empty`: 両方空の場合

#### Task 1.3: `src/search/semantic.rs` 新規作成
- **成果物**: `src/search/semantic.rs`, `src/search/mod.rs`更新
- **依存**: Task 1.1（ranking.rsのaggregate_similarity_by_file使用）
- **内容**:
  1. `src/search/mod.rs` に `pub mod semantic;` 追加
  2. `SemanticError` エラー型定義
  3. `query_semantic` 関数: セマンティッククエリ実行パイプライン
     - Result<Option<Vec<EmbeddingSimilarityResult>>, SemanticError> を返す
     - embeddings.db不在時はOk(None)（ログなし）
     - エラー時はErr(SemanticError)
  4. `enrich_semantic_to_search_results` 関数: `search.rs`から移動
     - 戻り値を `Result<..., ReaderError>` に変更
  5. `search.rs` の `enrich_semantic_to_search_results` を削除し、`use crate::search::semantic::*` で参照
  6. `search.rs` に `impl From<ReaderError> for SearchError` が必要なら追加
- **テスト**:
  - `query_semantic` はI/O依存のため統合テストで検証
  - `enrich_semantic_to_search_results` の既存テスト（あれば）を移動

### Phase 2: suggest.rs のハイブリッド化

#### Task 2.1: run_suggest のフロー変更
- **成果物**: `src/cli/suggest.rs` 更新
- **依存**: Task 1.1, 1.2, 1.3
- **内容**:
  1. `try_semantic_fallback` を削除
  2. `deduplicate_by_file_pairs` を削除
  3. `search_entry_files` の内部ロジックを直接呼び出す新フローを実装:
     - `reader.search(query, BM25_SEARCH_LIMIT)` → BM25検索
     - `ranking::aggregate_by_file(bm25_results)` → ファイル集約
     - `ranking::apply_file_type_weight(bm25_files, DEDUP_FILE_LIMIT * 3)` → 重み付け
  4. セマンティック検索の常時試行:
     ```rust
     let semantic_results = match semantic::query_semantic(&db_path, &config, &query, SEMANTIC_FALLBACK_LIMIT) {
         Ok(Some(results)) => Some(results),
         Ok(None) => None,
         Err(e) => { eprintln!("[suggest] semantic search failed: {e}"); None }
     };
     ```
  5. セマンティック結果があれば:
     - `ranking::aggregate_similarity_by_file(results)` → ファイル集約
     - `ranking::apply_file_type_weight(semantic_files, DEDUP_FILE_LIMIT * 3)` → 重み付け
  6. 結果統合:
     - 両方あり → `hybrid::rrf_merge_files(bm25, semantic, DEDUP_FILE_LIMIT)`
     - BM25のみ → `bm25_files.truncate(DEDUP_FILE_LIMIT)`
     - セマンティックのみ → `semantic_files.truncate(DEDUP_FILE_LIMIT)`
     - 両方なし → `build_fallback_strategy`
  7. `build_strategy` / `maybe_add_semantic_step` は変更なし
- **テスト**:
  - `test_file_type_weight_with_rrf`: weight適用→再ソート→RRFの一連フロー
  - `test_double_weight_penalty`: テストファイルがBM25+semantic両方に出現するケース
  - 既存テスト全パス確認

### Phase 3: 統合テスト

#### Task 3.1: e2e_suggest テスト拡張
- **成果物**: `tests/e2e_suggest.rs` 更新
- **依存**: Task 2.1
- **内容**:
  1. 既存テスト6件の回帰確認（embedding未構築環境）
  2. `test_suggest_bm25_only_graceful_degradation`: embedding DB不在時にBM25のみで動作
  3. `test_suggest_provider_failure_fallback`: 存在しないOllamaホスト設定でBM25フォールバック

### Phase 4: 品質チェック

#### Task 4.1: 最終検証
- **依存**: Task 3.1
- **内容**:
  1. `cargo build` → エラー0件
  2. `cargo clippy --all-targets -- -D warnings` → 警告0件
  3. `cargo test --all` → 全テストパス
  4. `cargo fmt --all -- --check` → 差分なし

## タスク依存関係

```
Task 1.1 (ranking.rs) ─────┐
                            ├─→ Task 1.3 (semantic.rs) ─→ Task 2.1 (suggest.rs) ─→ Task 3.1 (e2e) ─→ Task 4.1 (品質)
Task 1.2 (hybrid.rs) ──────┘
```

Task 1.1 と Task 1.2 は並列実行可能。

## Definition of Done

- [ ] すべてのタスクが完了
- [ ] `cargo test --all` 全テストパス
- [ ] `cargo clippy --all-targets -- -D warnings` 警告0件
- [ ] `cargo fmt --all -- --check` 差分なし
- [ ] embedding未構築環境で既存テスト6件がパス（回帰なし）
- [ ] 新規単体テスト全パス

## 品質チェック

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |
