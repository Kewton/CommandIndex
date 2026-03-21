# 作業計画: Issue #51

## Issue: [Feature] Markdownリンク解析・リンクインデックス構築
**Issue番号**: #51
**サイズ**: M
**優先度**: High
**依存Issue**: #36 SQLite symbols.db（完了済み）
**ブランチ**: `feature/issue-51-markdown-link-analysis`（既存）

---

## タスク分解

### Phase 1: データモデル・SymbolStore拡張

#### Task 1.1: FileLinkInfo構造体とfile_linksテーブル定義
- **ファイル**: `src/indexer/symbol_store.rs`
- **作業内容**:
  - `FileLinkInfo` 構造体を追加（id, source_file, target_file, link_type: String, file_hash）
  - `CURRENT_SYMBOL_SCHEMA_VERSION` を `1` → `2` に変更
  - `create_tables()` に `file_links` テーブルのCREATE TABLE文とインデックスを追加
- **依存**: なし
- **テスト**: `test_schema_version_2`（スキーマバージョン2でテーブルが正しく作成される）

#### Task 1.2: SymbolStore CRUDメソッド追加
- **ファイル**: `src/indexer/symbol_store.rs`
- **作業内容**:
  - `insert_file_links(&self, links: &[FileLinkInfo])` — バルクインサート（unchecked_transaction パターン）
  - `find_file_links_by_source(&self, source: &str)` — テスト検証用
  - `delete_by_file()` に `DELETE FROM file_links WHERE source_file = ?1` を追加（同一トランザクション内）
- **依存**: Task 1.1
- **テスト**: `test_insert_and_find_file_links`, `test_delete_by_file_removes_file_links`

### Phase 2: エラーハンドリング統一

#### Task 2.1: SchemaVersionMismatch正規化（IndexError）
- **ファイル**: `src/cli/index.rs`
- **作業内容**:
  - `From<SymbolStoreError> for IndexError` の実装を修正: `SchemaVersionMismatch` バリアントのみ `IndexError::SchemaVersionMismatch` にマッピング
- **依存**: Task 1.1
- **テスト**: 既存テスト `test_index_error_from_symbol_store_error` の更新

#### Task 2.2: SchemaVersionMismatch正規化（SearchError）
- **ファイル**: `src/cli/search.rs`
- **作業内容**:
  - `SearchError::SchemaVersionMismatch` バリアント追加
  - `From<SymbolStoreError>` 実装でマッチして正規化
  - `Display` 実装で `clean → index` 案内メッセージ
- **依存**: Task 1.1
- **テスト**: SearchErrorのSchemaVersionMismatch変換テスト

#### Task 2.3: SchemaVersionMismatch正規化（StatusError）
- **ファイル**: `src/cli/status.rs`
- **作業内容**:
  - `get_symbol_count()` で `SchemaVersionMismatch` 時に警告メッセージ表示 + `symbol_count=0` で継続
- **依存**: Task 1.1
- **テスト**: status実行時のSchemaVersionMismatch挙動テスト

### Phase 3: インデックス処理統合

#### Task 3.1: is_indexable_link()フィルタ関数
- **ファイル**: `src/cli/index.rs`
- **作業内容**:
  - `is_indexable_link(link: &Link) -> bool` 関数を追加
  - フィルタ条件: 外部URL, 外部スキーム, mailto:, フラグメントのみ, target長さ > 1024
- **依存**: なし
- **テスト**: `test_is_indexable_link`（各フィルタ条件の網羅テスト）

#### Task 3.2: index_markdown_file()のリンク格納統合
- **ファイル**: `src/cli/index.rs`
- **作業内容**:
  - `index_markdown_file()` シグネチャに `symbol_store: Option<&SymbolStore>` 追加
  - `index_file_and_upsert()` のMarkdown分岐で `symbol_store` を転送
  - リンク処理フロー実装:
    1. `symbol_store.delete_by_file()` （古いリンク削除）
    2. Tantivy書き込み（既存のsectionループ）
    3. `is_indexable_link()` でフィルタリング
    4. リンク数上限チェック（10,000件）
    5. `LinkType.to_string()` で文字列変換
    6. `symbol_store.insert_file_links()` でDB格納
  - Tantivy書き込み失敗時: エラー返却（delete済みのためクリーン）
  - insert_file_links失敗時: エラーを上位へ返す
- **依存**: Task 1.2, Task 3.1
- **テスト**: `test_index_markdown_with_wiki_links`, `test_index_markdown_with_markdown_links`, `test_index_markdown_excludes_external_urls`, `test_index_markdown_excludes_fragments`

#### Task 3.3: run_incremental()の削除処理拡張
- **ファイル**: `src/cli/index.rs`
- **作業内容**:
  - 削除処理の `if entry.file_type.is_code()` 条件を削除し、全ファイルタイプで `symbol_store.delete_by_file()` を呼ぶ
- **依存**: Task 1.2
- **テスト**: `test_update_delete_markdown_removes_links`, `test_update_markdown_file_links_rebuild`

### Phase 4: テスト・品質チェック

#### Task 4.1: 単体テスト（symbol_store.rs）
- **ファイル**: `src/indexer/symbol_store.rs`（テストモジュール内）
- **テストケース**:
  - `test_insert_and_find_file_links` — 挿入と取得の基本動作
  - `test_delete_by_file_removes_file_links` — ファイル削除時のリンク削除
  - `test_schema_version_2` — スキーマバージョン2のテーブル作成
  - `test_schema_version_mismatch_v1_to_v2` — v1→v2不一致エラー
- **依存**: Task 1.2

#### Task 4.2: 統合テスト
- **ファイル**: `tests/` 以下に新規テストファイルまたは既存に追加
- **テストケース**:
  - WikiLink/MarkdownLinkのindex後file_links格納
  - 外部URL/フラグメント除外
  - update時のリンク再構築
  - Markdown削除時のリンク削除
  - link_typeカラムの正しい値
- **依存**: Task 3.2, Task 3.3

#### Task 4.3: 既存テスト更新
- **作業内容**:
  - `src/indexer/symbol_store.rs`: `test_index_error_from_symbol_store_error` の正規化対応
  - 注意: `tests/cli_index.rs` の `schema_version` 期待値は据え置き（state.jsonのCURRENT_SCHEMA_VERSIONは不変）
- **依存**: Task 2.1

#### Task 4.4: 品質チェック
- `cargo build` — エラー0件
- `cargo clippy --all-targets -- -D warnings` — 警告0件
- `cargo test --all` — 全テストパス
- `cargo fmt --all -- --check` — 差分なし

---

## 実行順序（依存関係）

```
Task 1.1 (FileLinkInfo + テーブル定義)
  ├── Task 1.2 (CRUD メソッド)
  │     ├── Task 3.2 (index_markdown_file統合)
  │     │     └── Task 4.2 (統合テスト)
  │     └── Task 3.3 (run_incremental拡張)
  ├── Task 2.1 (IndexError正規化)
  │     └── Task 4.3 (既存テスト更新)
  ├── Task 2.2 (SearchError正規化)
  └── Task 2.3 (StatusError正規化)

Task 3.1 (is_indexable_link) ← 独立、並列実行可能
Task 4.1 (単体テスト) ← Task 1.2 完了後
Task 4.4 (品質チェック) ← 全タスク完了後
```

---

## TDD実装順序（推奨）

1. **Task 1.1**: FileLinkInfo構造体 + テーブル定義 + `test_schema_version_2`
2. **Task 1.2**: CRUD メソッド + `test_insert_and_find_file_links` + `test_delete_by_file_removes_file_links`
3. **Task 3.1**: `is_indexable_link()` + テスト
4. **Task 2.1**: IndexError SchemaVersionMismatch正規化 + 既存テスト更新
5. **Task 2.2**: SearchError SchemaVersionMismatch正規化
6. **Task 2.3**: StatusError SchemaVersionMismatch正規化
7. **Task 3.2**: index_markdown_file()統合 + 統合テスト
8. **Task 3.3**: run_incremental()削除処理拡張 + テスト
9. **Task 4.4**: 最終品質チェック

---

## Definition of Done

- [ ] すべてのタスクが完了
- [ ] `cargo build` エラー0件
- [ ] `cargo clippy --all-targets -- -D warnings` 警告0件
- [ ] `cargo test --all` 全テストパス
- [ ] `cargo fmt --all -- --check` 差分なし
- [ ] 受け入れ基準12項目すべてチェック済み
