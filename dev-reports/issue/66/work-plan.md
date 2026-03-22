# 作業計画書 - Issue #66

## Issue: [Feature] Phase 5 E2E統合テスト（Semantic Search・Hybrid・Rerank検証）
**Issue番号**: #66
**サイズ**: M
**優先度**: Medium
**依存Issue**: #63 (Semantic Search), #64 (Hybrid Retrieval), #65 (Reranking) - 全て完了済み
**ブランチ**: `feature/issue-66-e2e-tests`（作成済み）

## 詳細タスク分解

### Phase 1: テスト基盤（ヘルパー関数）

- [ ] **Task 1.1**: テストファイル作成 + ローカルヘルパー実装
  - 成果物: `tests/e2e_semantic_hybrid.rs`
  - 内容:
    - `setup_semantic_test_dir()` - tempdir作成、Markdown 2ファイル作成、index実行
    - `insert_test_embeddings()` - SymbolStore::open() + create_tables() + insert_embeddings()
    - `create_test_config()` - config.toml作成（provider=ollama, endpoint=localhost:11434）
    - 固定ベクトル定数（QUERY_VEC, SIMILAR_VEC, DIFFERENT_VEC, TEST_MODEL, TEST_HASH）
  - 依存: なし
  - TDD: ヘルパー自体のテストはTask 1.2で兼ねる

### Phase 2: ライブラリAPI層テスト（Ollama不要）

- [ ] **Task 2.1**: test_embedding_insert_and_count（シナリオ1）
  - 検証: insert_embeddings() → count_embeddings() で件数確認
  - 対象API: `commandindex::indexer::symbol_store::{SymbolStore, EmbeddingInfo}`

- [ ] **Task 2.2**: test_semantic_search_basic（シナリオ3）
  - 検証: search_similar() でコサイン類似度順の結果を返すこと
  - SIMILAR_VECが上位、DIFFERENT_VECが下位

- [ ] **Task 2.3**: test_semantic_search_filter（シナリオ4）
  - 検証: search_similar() の結果取得確認（フィルタ適用は非公開APIのため結果確認まで）

- [ ] **Task 2.4**: test_rrf_merge_integration（RRF統合）
  - 検証: rrf_merge() でBM25+Semantic結果が統合され、スコア順に返ること
  - 対象API: `commandindex::search::hybrid::rrf_merge()`

### Phase 3: CLI層テスト（Ollama不要）

- [ ] **Task 3.1**: test_embed_without_ollama_fails（シナリオ2）
  - 検証: `commandindex embed` → 非ゼロ終了 + stderrエラーメッセージ
  - common::cmd() 使用

- [ ] **Task 3.2**: test_hybrid_no_semantic（シナリオ7）
  - 検証: `search --no-semantic` → BM25のみで結果が返ること
  - embedding挿入済み環境で実行

- [ ] **Task 3.3**: test_hybrid_no_embeddings（シナリオ8）
  - 検証: embedding未生成環境で `search` → エラーなくBM25結果が返ること

- [ ] **Task 3.4**: test_rerank_fallback_via_cli（シナリオ10）
  - 検証: `search --rerank` → Ollamaなし環境でフォールバック（結果が返ること）

- [ ] **Task 3.5**: test_rerank_top_accepted_via_cli（シナリオ11）
  - 検証: `search --rerank --rerank-top 5` → 引数が受理されること

- [ ] **Task 3.6**: test_context_with_embeddings（シナリオ13）
  - 検証: embedding存在下で `context` → Context Packが正常生成

### Phase 4: 環境依存テスト（#[ignore]）

- [ ] **Task 4.1**: test_hybrid_auto_switch（シナリオ6）
  - `#[ignore]` 付与
  - 検証: embedding存在 + Ollama起動時に `search` → hybridモード動作

- [ ] **Task 4.2**: test_hybrid_bm25_fallback（シナリオ9）
  - `#[ignore]` 付与
  - 検証: embedding存在 + Ollama停止時に `search` → BM25フォールバック

### Phase 5: 品質チェック

- [ ] **Task 5.1**: cargo build 確認
- [ ] **Task 5.2**: cargo clippy --all-targets -- -D warnings 確認
- [ ] **Task 5.3**: cargo test --all 確認（全テストパス）
- [ ] **Task 5.4**: cargo fmt --all -- --check 確認

## 実装順序

```
Task 1.1（ヘルパー）
  ├── Task 2.1（insert_and_count）← ヘルパーの動作確認を兼ねる
  ├── Task 2.2（semantic_search_basic）
  ├── Task 2.3（semantic_search_filter）
  └── Task 2.4（rrf_merge）
  ├── Task 3.1（embed_fails）
  ├── Task 3.2（no_semantic）
  ├── Task 3.3（no_embeddings）
  ├── Task 3.4（rerank_fallback）
  ├── Task 3.5（rerank_top）
  └── Task 3.6（context）
  ├── Task 4.1（hybrid_auto_switch）
  └── Task 4.2（bm25_fallback）
  └── Task 5.1-5.4（品質チェック）
```

## TDD方針

各タスクでRed→Green→Refactorサイクルを回す:
1. **Red**: テストを書く（コンパイルエラーを除き、期待値を先に定義）
2. **Green**: テストが通るようにヘルパー/テストデータを調整
3. **Refactor**: 重複コードの共通化、命名の改善

## Definition of Done

- [ ] 12テスト全てがパス（cargo test --all）
- [ ] #[ignore]テスト2件が実装されている
- [ ] clippy警告ゼロ
- [ ] cargo fmt差分なし
- [ ] プロダクションコード(src/)への変更なし
