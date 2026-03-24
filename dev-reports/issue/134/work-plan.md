# 作業計画 - Issue #134: 多言語embeddingモデル対応 (BGE-M3)

## Issue概要

| 項目 | 内容 |
|------|------|
| Issue番号 | #134 |
| タイトル | 多言語embeddingモデル対応 (BGE-M3) |
| サイズ | M |
| 優先度 | High |
| 依存Issue | なし |
| ブランチ | feature/issue-134-bge-m3 |

## タスク分解

### Phase 1: コア変更（store.rs）

#### Task 1.1: SimilaritySearchOutput構造体の追加とsearch_similar()返り値変更
- **ファイル:** `src/embedding/store.rs`
- **内容:**
  - `SimilaritySearchOutput` 構造体を定義（results, total_records, skipped_dimension_mismatch）
  - `should_warn_dimension_mismatch()` メソッド実装（副作用なし）
  - `search_similar()` の返り値を `SimilaritySearchOutput` に変更
  - 既存のsearch_similar内ループでtotal_recordsとskipped_dimension_mismatchをカウント
- **テスト（TDD: テスト先行）:**
  - 既存5テスト（test_search_similar_basic, test_search_similar_top_k, test_search_similar_dimension_mismatch, test_search_similar_empty_db, cosine_similarity関連）を `SimilaritySearchOutput` 対応に修正
  - 新規: `test_search_similar_output_skipped_count` - 次元不一致時のskipped数検証
  - 新規: `test_should_warn_dimension_mismatch` - 閾値判定の検証
- **依存:** なし

#### Task 1.2: has_current_embedding()のmodel引数追加
- **ファイル:** `src/embedding/store.rs`
- **内容:**
  - `has_current_embedding(path, file_hash, model)` にシグネチャ変更
  - SQLに `AND model = ?3` 条件追加
- **テスト（TDD: テスト先行）:**
  - 既存3テスト（test_has_current_embedding_true, _false_different_hash, _false_no_record）にmodel引数追加
  - 新規: `test_has_current_embedding_false_different_model` - model不一致時にfalse
- **依存:** なし

#### Task 1.3: delete_stale_model_embeddings()の追加
- **ファイル:** `src/embedding/store.rs`
- **内容:**
  - 新メソッド `delete_stale_model_embeddings(current_model)` 追加
  - 空文字列は `InvalidEmbedding` エラー（Fail Fast）
  - `DELETE FROM embeddings WHERE model != ?1` 実行
- **テスト（TDD: テスト先行）:**
  - 新規: `test_delete_stale_model_embeddings` - 旧モデルのみ削除、現モデルは保持
  - 新規: `test_delete_stale_model_embeddings_empty_model` - 空文字列でInvalidEmbeddingエラー
  - 新規: `test_delete_stale_model_embeddings_idempotent` - 二重呼び出しで0件削除
- **依存:** なし

### Phase 2: known_dimension追加（ollama.rs）

#### Task 2.1: BGE-M3のknown_dimension追加
- **ファイル:** `src/embedding/ollama.rs`
- **内容:**
  - `known_dimension()` に `"qllama/bge-m3:q8_0" => Some(1024)` を追加
- **テスト（TDD: テスト先行）:**
  - 既存 `test_dimension_known_models` にBGE-M3アサーション追加
- **依存:** なし

### Phase 3: CLI層の更新

#### Task 3.1: embed.rsの更新
- **ファイル:** `src/cli/embed.rs`
- **内容:**
  - `has_current_embedding()` 呼び出しにmodel引数追加
  - embedding生成開始前に `delete_stale_model_embeddings()` 呼び出し
  - 削除件数 > 0 の場合に情報メッセージ表示
- **依存:** Task 1.2, Task 1.3

#### Task 3.2: index.rsの更新
- **ファイル:** `src/cli/index.rs`
- **内容:**
  - `generate_embeddings_for_manifest()` 内の `has_current_embedding()` 呼び出しにmodel引数追加
  - embedding生成開始前に `delete_stale_model_embeddings()` 呼び出し
  - 削除件数 > 0 の場合に情報メッセージ表示
- **依存:** Task 1.2, Task 1.3

#### Task 3.3: search.rsの更新
- **ファイル:** `src/cli/search.rs`
- **内容:**
  - `run_semantic_search()` の `search_similar()` 返り値を `SimilaritySearchOutput` に対応
  - `should_warn_dimension_mismatch()` の結果に応じて `eprintln!` で警告表示
  - `try_hybrid_search()` も同様に対応
- **依存:** Task 1.1

#### Task 3.4: suggest.rsの更新
- **ファイル:** `src/cli/suggest.rs`
- **内容:**
  - `try_semantic_fallback()` (L281) の `search_similar()` 返り値を `SimilaritySearchOutput` に対応
  - `output.results` でアクセスするよう変更
  - `should_warn_dimension_mismatch()` の結果に応じて警告表示
- **依存:** Task 1.1

### Phase 4: テスト修正

#### Task 4.1: e2e_semantic_hybrid.rsの修正
- **ファイル:** `tests/e2e_semantic_hybrid.rs`
- **内容:**
  - `search_similar()` の返り値を `output.results` でアクセスするよう修正（2箇所: L203, L255）
- **依存:** Task 1.1

### Phase 5: ドキュメント

#### Task 5.1: README.mdにEmbeddingセクション新設
- **ファイル:** `README.md`
- **内容:**
  - 対応モデル一覧（nomic-embed-text, bge-m3）
  - 前提条件: Ollamaモデルの事前pull手順
  - モデル変更手順: clean → index → embed（またはembed再実行のみ）
  - 注意事項: モデル変更時は再生成に時間がかかる旨
- **依存:** なし

## 実装順序

```
Phase 1 (並列可能)
├── Task 1.1: SimilaritySearchOutput + search_similar変更
├── Task 1.2: has_current_embedding model引数追加
└── Task 1.3: delete_stale_model_embeddings追加

Phase 2 (Phase 1と並列可能)
└── Task 2.1: known_dimension BGE-M3追加

Phase 3 (Phase 1完了後)
├── Task 3.1: embed.rs更新
├── Task 3.2: index.rs更新
├── Task 3.3: search.rs更新
└── Task 3.4: suggest.rs更新

Phase 4 (Phase 1完了後)
└── Task 4.1: e2eテスト修正

Phase 5 (独立)
└── Task 5.1: README更新
```

## TDD実装順序（推奨）

1. **Task 1.1** のテストを先に書く → SimilaritySearchOutput実装 → search_similar変更
2. **Task 1.2** のテストを先に書く → has_current_embedding変更
3. **Task 1.3** のテストを先に書く → delete_stale_model_embeddings実装
4. **Task 2.1** のテストを先に書く → known_dimension追加
5. `cargo test` で全テストパス確認
6. **Task 3.1-3.4** CLI層更新（コンパイルエラー解消）
7. **Task 4.1** e2eテスト修正
8. `cargo test --all` で全テストパス確認
9. **Task 5.1** README更新
10. 最終品質チェック

## 品質チェック

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |

## Definition of Done

- [ ] T1: known_dimensionにBGE-M3追加、テストパス
- [ ] T1.5: has_current_embedding model引数追加、delete_stale実装、テストパス
- [ ] T2.5: SimilaritySearchOutput導入、全CLI経路で警告表示、テストパス
- [ ] T4: README Embeddingセクション新設
- [ ] cargo build / clippy / test / fmt 全パス
