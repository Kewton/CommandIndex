# 仮説検証レポート: Issue #62

## 検証結果サマリー

| 項目 | 仮説 | 検証結果 |
|---|---|---|
| `CURRENT_SYMBOL_SCHEMA_VERSION` | 存在する | Confirmed (v2) |
| `delete_by_file()` | 存在する | Confirmed (symbols, dependencies, file_links を削除) |
| `byteorder` crate | 直接依存にある | Partially Confirmed (間接依存のみ、明示追加が必要) |
| SQLiteスキーマ管理 | schema_metaテーブルで管理 | Confirmed |
| delete-before-insert パターン | 既に利用中 | Confirmed (cli/index.rs) |
| スキーマバージョン不一致チェック | 実装済み | Confirmed |

## 詳細

### 1. CURRENT_SYMBOL_SCHEMA_VERSION
- `src/indexer/symbol_store.rs:6` で `const CURRENT_SYMBOL_SCHEMA_VERSION: u32 = 2;`
- Issue #62でv3にインクリメントが必要

### 2. delete_by_file()
- `src/indexer/symbol_store.rs:334-350`
- symbols, dependencies, file_links の3テーブルをトランザクション内で削除
- embeddingsテーブルのDELETEを追加する必要あり

### 3. byteorder crate
- Cargo.tomlに直接依存なし
- Cargo.lockでは間接依存として存在
- `byteorder = "1"` を明示追加が必要

### 4. SQLiteスキーマ管理
- `schema_meta` テーブルで `version` キーを管理
- `create_tables()` がidempotent
- バージョン不一致時のチェック機構あり

### 5. delete-before-insert パターン
- `src/cli/index.rs` でMarkdown/Codeファイルの両方で利用中
- embeddingsも同パターンで自然に対応可能

## 結論
すべての仮説がConfirmed/Partially Confirmed。実装基盤は整っている。
