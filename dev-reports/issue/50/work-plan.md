# 作業計画: Issue #50 --related 検索オプション実装

## Issue: [Feature] --related 検索オプション実装（関連ドキュメント・コード検索）
**Issue番号**: #50
**サイズ**: L
**優先度**: High
**依存Issue**: #9, #36, #37, #51（全てマージ済み）
**ブランチ**: `feature/issue-50-related-search`（作成済み）

---

## 詳細タスク分解

### Phase 1: データ層（Indexer追加メソッド）

- [ ] **Task 1.1**: SymbolStore に `find_imports_by_source()` 追加
  - 成果物: `src/indexer/symbol_store.rs`
  - 内容: dependencies テーブルの source_file で検索するメソッド追加
  - テスト: `#[cfg(test)]` でユニットテスト追加
  - 依存: なし

- [ ] **Task 1.2**: SymbolStore に `find_file_links_by_target()` 追加
  - 成果物: `src/indexer/symbol_store.rs`
  - 内容: file_links テーブルの target_file で検索するメソッド追加
  - テスト: `#[cfg(test)]` でユニットテスト追加
  - 依存: なし

- [ ] **Task 1.3**: IndexReaderWrapper に `search_by_exact_path()` 追加
  - 成果物: `src/indexer/reader.rs`
  - 内容: tantivy TermQuery で path 完全一致検索
  - テスト: `from_index()` を使ったユニットテスト
  - 依存: なし

### Phase 2: 出力型定義

- [ ] **Task 2.1**: `RelatedSearchResult` / `RelationType` 型定義
  - 成果物: `src/output/mod.rs`
  - 内容: 関連検索結果の構造体と関連タイプenum定義
  - 依存: なし

- [ ] **Task 2.2**: `format_related_results()` 実装
  - 成果物: `src/output/mod.rs`, `src/output/human.rs`, `src/output/json.rs`, `src/output/path.rs`
  - 内容: human/json/path 各フォーマットの出力関数
  - テスト: ユニットテスト
  - 依存: Task 2.1

### Phase 3: コアロジック（スコアリングエンジン）

- [ ] **Task 3.1**: `src/search/` モジュール作成
  - 成果物: `src/search/mod.rs`, `src/search/related.rs`, `src/lib.rs` 変更
  - 内容: モジュール宣言、`RelatedSearchEngine` 構造体、`RelatedSearchError` enum
  - 依存: なし

- [ ] **Task 3.2**: パス正規化 `normalize_path()` 実装
  - 成果物: `src/search/related.rs`
  - 内容: `./` 除去、`\\` → `/` 統一、`..` 排除、長さ・空文字チェック
  - テスト: ユニットテスト
  - 依存: Task 3.1

- [ ] **Task 3.3**: `score_markdown_links()` 実装
  - 成果物: `src/search/related.rs`
  - 内容: file_links テーブルから双方向リンク検索 → スコア 1.0
  - テスト: ユニットテスト
  - 依存: Task 1.1, 1.2, 3.1

- [ ] **Task 3.4**: `score_import_deps()` 実装
  - 成果物: `src/search/related.rs`
  - 内容: dependencies テーブルから双方向import検索 → スコア 0.9
  - テスト: ユニットテスト
  - 依存: Task 1.1, 1.2, 3.1

- [ ] **Task 3.5**: `score_tag_match()` 実装
  - 成果物: `src/search/related.rs`
  - 内容: tantivy TermQuery でタグ取得 → 全ファイルのタグと照合 → スコア 0.5 × 一致数
  - テスト: ユニットテスト
  - 依存: Task 1.3, 3.1

- [ ] **Task 3.6**: `score_path_proximity()` 実装
  - 成果物: `src/search/related.rs`
  - 内容: 共通パスプレフィックス（0.2/0.1）+ パスセグメント部分一致（0.4）
  - テスト: ユニットテスト
  - 依存: Task 3.1

- [ ] **Task 3.7**: `find_related()` 統合実装
  - 成果物: `src/search/related.rs`
  - 内容: 全score_*を呼び出し、スコア加算、ソート、上位N件返却
  - テスト: ユニットテスト
  - 依存: Task 3.3, 3.4, 3.5, 3.6

### Phase 4: CLI統合

- [ ] **Task 4.1**: main.rs に `--related` オプション追加
  - 成果物: `src/main.rs`
  - 内容: clap `conflicts_with_all` で query/symbol/tag/path/type/heading と排他
  - テスト: `tests/cli_args.rs` にパーステスト追加
  - 依存: なし

- [ ] **Task 4.2**: `run_related_search()` オーケストレーション実装
  - 成果物: `src/cli/search.rs`
  - 内容: SearchError 拡張、RelatedSearchEngine 生成・呼び出し・出力
  - 依存: Task 3.7, 2.2, 4.1

- [ ] **Task 4.3**: main.rs 分岐ロジック追加
  - 成果物: `src/main.rs`
  - 内容: `(None, None, Some(f))` → `run_related_search()` 呼び出し
  - 依存: Task 4.2

### Phase 5: E2Eテスト

- [ ] **Task 5.1**: E2E関連検索テスト
  - 成果物: `tests/e2e_related_search.rs`
  - 内容:
    - Markdownリンクによる関連検出
    - タグ一致による関連検出
    - パス近接性による関連検出
    - import依存関係による関連検出
    - スコアソート検証
    - human/json/path 出力検証
    - 排他エラー検証
    - ファイル未存在エラー検証
    - 結果0件メッセージ検証
  - 依存: Task 4.3

### Phase 6: 品質チェック

- [ ] **Task 6.1**: 全品質チェック実行
  - コマンド: `cargo build && cargo clippy --all-targets -- -D warnings && cargo test --all && cargo fmt --all -- --check`
  - 基準: 全てパス、警告0件

---

## TDD実装順序

設計方針書の実装順序に従い、各タスクはTDD（Red→Green→Refactor）で進める:

```
Phase 1 (並列可)
  ├── Task 1.1: find_imports_by_source() + テスト
  ├── Task 1.2: find_file_links_by_target() + テスト
  └── Task 1.3: search_by_exact_path() + テスト

Phase 2 (Phase 1完了後)
  ├── Task 2.1: 型定義
  └── Task 2.2: フォーマッタ + テスト

Phase 3 (Phase 1完了後)
  ├── Task 3.1: モジュール作成
  ├── Task 3.2: normalize_path() + テスト
  ├── Task 3.3-3.6: 各スコアリング + テスト (並列可)
  └── Task 3.7: find_related() 統合 + テスト

Phase 4 (Phase 2,3完了後)
  ├── Task 4.1: CLI引数 + テスト
  ├── Task 4.2: オーケストレーション
  └── Task 4.3: 分岐ロジック

Phase 5 (Phase 4完了後)
  └── Task 5.1: E2Eテスト

Phase 6 (Phase 5完了後)
  └── Task 6.1: 品質チェック
```

---

## 見積もり

| Phase | タスク数 | 複雑度 |
|-------|---------|--------|
| Phase 1 | 3 | 低（既存パターンの踏襲） |
| Phase 2 | 2 | 低（型定義＋出力関数） |
| Phase 3 | 7 | 高（コアロジック） |
| Phase 4 | 3 | 中（CLI統合） |
| Phase 5 | 1 | 中（E2Eテスト） |
| Phase 6 | 1 | 低（品質チェック） |
| **合計** | **17** | - |

---

## Definition of Done

- [ ] すべてのタスクが完了
- [ ] `cargo test --all` 全テストパス
- [ ] `cargo clippy --all-targets -- -D warnings` 警告ゼロ
- [ ] `cargo fmt --all -- --check` 差分なし
- [ ] 受け入れ基準12項目全て充足
- [ ] 設計方針書の設計パターンに準拠
