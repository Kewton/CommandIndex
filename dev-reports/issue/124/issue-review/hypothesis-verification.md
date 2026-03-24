# 仮説検証レポート: Issue #124

## Issue概要
`search --semantic` が embed済みでも「No embeddings found」エラーを返す

## 仮説一覧と検証結果

### 仮説1: embeddingのDBパスとインデックスパスの不一致
- **判定: Partially Confirmed**
- `embed`コマンドは `embeddings_db_path()` → `embeddings.db` を使用
- `search`コマンドは `symbol_db_path()` → `symbols.db` を使用
- パスの不一致というよりも、完全に異なるDBを参照している

### 仮説2: `embed`コマンドと`search`コマンドでembeddingストアの参照先が異なる
- **判定: Confirmed (Root Cause)**
- `embed`コマンド (`src/cli/embed.rs:136`): `EmbeddingStore` (`embeddings.db`) に書き込み
- `search --semantic` (`src/cli/search.rs:512-531`): `SymbolStore` (`symbols.db`) から読み取り
- `SymbolStore`のembeddingsテーブルは存在するが、データが投入されることがない
- `status`コマンド (`src/cli/status/mod.rs:238`): 正しく`embeddings.db`を参照

### 仮説3: embedding DBのスキーマバージョン不一致
- **判定: Rejected**
- EmbeddingStore: schema version 1
- SymbolStore: schema version 3
- 別々のDBなのでバージョン不一致は問題の原因ではない

## 根本原因の詳細

### データフロー不整合

| コンポーネント | ファイル | 使用DB | 動作 |
|---|---|---|---|
| embed コマンド | src/cli/embed.rs:136 | embeddings.db | EmbeddingStoreに書き込み |
| search --semantic | src/cli/search.rs:512,531 | symbols.db | 空のSymbolStoreから読み取り |
| status コマンド | src/cli/status/mod.rs:238 | embeddings.db | 正しくEmbeddingStoreを参照 |
| hybrid search | src/cli/search.rs:643 | symbols.db | 同様に誤ったDBを参照 |

### 問題の本質
`search`コマンドは`SymbolStore`の`count_embeddings()`と`search_similar()`を使用するが、
`embed`コマンドが書き込むのは`EmbeddingStore`であり、`SymbolStore`にはembeddingが投入されない。
結果として、`SymbolStore`のembeddingsテーブルは常に空であり、検索時に必ず`NoEmbeddings`エラーとなる。

## 修正方針（概要）
`search`コマンドのセマンティック検索を`EmbeddingStore`から読み取るように修正する、
または`embed`コマンドで`SymbolStore`にも書き込むようにする。
