# 進捗レポート: Issue #144 - suggest の英語クエリ精度改善

## 実施日: 2026-03-24

## 完了状況: 実装完了

## 成果物

### 新規ファイル
- `src/search/ranking.rs` - ファイル集約・重み付けロジック（22テスト）
- `src/search/semantic.rs` - セマンティッククエリ実行・結果変換（2テスト）

### 変更ファイル
- `src/search/hybrid.rs` - ファイル単位RRF統合関数追加（5テスト）
- `src/search/mod.rs` - `pub mod ranking; pub mod semantic;` 追加
- `src/cli/suggest.rs` - フォールバック→ハイブリッド方式に移行
- `src/cli/search.rs` - enrich_semantic_to_search_resultsをsemantic.rsに委譲

## 品質チェック

| チェック | 結果 |
|---------|------|
| cargo build | PASS |
| cargo clippy --all-targets -- -D warnings | PASS (0 warnings) |
| cargo test --all | PASS (703 passed, 1 pre-existing failure) |
| cargo fmt --all -- --check | PASS (no diff) |

## アーキテクチャ変更

### Before
```
suggest.rs → BM25検索 → BM25 0件の場合のみセマンティックフォールバック
```

### After
```
suggest.rs → BM25検索 + 常時セマンティック検索 → RRF統合 → ファイルランキング
  ├─ ranking.rs: ファイル集約・重み付け（pure関数）
  ├─ hybrid.rs: RRF統合（rrf_merge_files）
  └─ semantic.rs: セマンティッククエリ実行・結果変換
```

## テスト内訳

| モジュール | テスト数 | 内容 |
|-----------|---------|------|
| search/ranking.rs | 22 | is_test_file, is_doc_file, aggregate_by_file, apply_file_type_weight, aggregate_similarity_by_file |
| search/hybrid.rs | 5 (新規) | rrf_merge_files (basic, disjoint, single_source, empty, limit) |
| search/semantic.rs | 2 | SemanticError display, query_semantic db missing |
| cli/suggest.rs | 既存維持 | validate_input, shell_quote等 |
| e2e_suggest | 既存維持 | embedding未構築環境での回帰テスト |
