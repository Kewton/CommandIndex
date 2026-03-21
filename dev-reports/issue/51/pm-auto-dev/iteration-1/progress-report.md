# 進捗レポート: Issue #51 Markdownリンク解析・リンクインデックス構築

## 実施日: 2026-03-21

---

## 実装結果サマリー

| 項目 | 結果 |
|------|------|
| ステータス | 実装完了 |
| テスト | 283テスト全合格（0失敗） |
| Clippy | 警告0件 |
| フォーマット | 差分なし |
| 変更ファイル | 4ファイル |
| 追加行数 | +351行 |
| 削除行数 | -16行 |

---

## 変更ファイル

### 1. src/indexer/symbol_store.rs (+155行)
- `FileLinkInfo` 構造体追加
- `CURRENT_SYMBOL_SCHEMA_VERSION` を 1 → 2 に変更
- `create_tables()` に `file_links` テーブル + インデックス追加
- `insert_file_links()` バルクインサートメソッド追加
- `find_file_links_by_source()` テスト検証用メソッド追加
- `delete_by_file()` に `file_links` DELETE追加
- 5件の新規単体テスト

### 2. src/cli/index.rs (+195行)
- `is_indexable_link()` フィルタ関数（URIスキームallowlist方式）
- `index_markdown_file()` にsymbol_store引数追加 + リンク格納統合
- `index_file_and_upsert()` のMarkdown分岐でsymbol_store転送
- `run_incremental()` の削除処理を全ファイルタイプに拡張
- `From<SymbolStoreError> for IndexError` でSchemaVersionMismatch正規化
- Tantivy失敗時のロールバック実装
- 11件の新規テスト

### 3. src/cli/search.rs (+11行)
- `SearchError::SchemaVersionMismatch` バリアント追加
- `From<SymbolStoreError>` でマッチして正規化
- Display実装（clean → index案内メッセージ）

### 4. src/cli/status.rs (+6行)
- `get_symbol_count()` でSchemaVersionMismatch検出 → 警告表示 + 0返却

---

## 受け入れ基準チェック

| # | 基準 | 状態 |
|---|------|------|
| 1 | `[[wiki-link]]` 形式のリンクが解析される | 完了 |
| 2 | `[text](path)` 形式のリンクが解析される | 完了 |
| 3 | 外部URL（http/https）リンクは `file_links` に格納されない | 完了 |
| 4 | その他の外部スキーム・フラグメントのみリンクも除外される | 完了 |
| 5 | リンク情報がsymbols.dbの `file_links` テーブルに格納される | 完了 |
| 6 | `index` でリンクが正しく格納される | 完了 |
| 7 | `update` 時にファイル変更で `file_links` が正しく削除・再挿入される | 完了 |
| 8 | `update` 時にMarkdownファイル削除で `file_links` が削除される | 完了 |
| 9 | `CURRENT_SYMBOL_SCHEMA_VERSION` がインクリメントされている | 完了 |
| 10 | スキーマバージョン不一致時に統一メッセージが表示される | 完了 |
| 11 | Tantivy書き込み失敗時の整合性が保たれる | 完了 |
| 12 | cargo test / clippy / fmt 全パス | 完了 |

---

## Codexコードレビュー結果

| 種別 | 件数 | 対応 |
|------|------|------|
| Critical | 2件 | C1（Tantivy失敗時ロールバック）修正済み。C2（差分更新整合性）は既存設計の制約であり本Issue固有ではない |
| Warning | 4件 | W1（URIスキーム判定改善）修正済み。W2-W4は既存コードの制約であり本Issue固有ではない |

---

## 品質チェック結果

| チェック | コマンド | 結果 |
|----------|---------|------|
| ビルド | `cargo build` | 成功 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 283テスト全合格 |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |
