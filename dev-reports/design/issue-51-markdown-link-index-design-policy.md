# 設計方針書: Issue #51 Markdownリンク解析・リンクインデックス構築

## 1. Issue概要

| 項目 | 内容 |
|------|------|
| Issue番号 | #51 |
| タイトル | [Feature] Markdownリンク解析・リンクインデックス構築 |
| ラベル | enhancement |
| 依存Issue | #36 SQLite symbols.db（完了済み） |
| スコープ | リンクの抽出・SQLite格納のみ（パス解決・検索活用は後続Issue） |

## 2. システムアーキテクチャ概要

### レイヤー構成と本Issue変更対象

```
┌─────────────────────────────────────────────────┐
│  CLI Layer (src/cli/)                           │
│  ┌─────────┐  ┌──────────┐  ┌────────┐         │
│  │ index.rs │  │search.rs │  │status.rs│         │
│  │ ★変更   │  │ ★変更   │  │ ★変更  │         │
│  └────┬────┘  └────┬─────┘  └───┬────┘         │
├───────┼────────────┼────────────┼────────────────┤
│  Indexer Layer (src/indexer/)  │                │
│  ┌──────────────────┐  ┌──────┴──────┐         │
│  │ symbol_store.rs  │  │   mod.rs    │         │
│  │ ★変更（テーブル追加）│  │ ★変更(エラー)│         │
│  └──────────────────┘  └─────────────┘         │
├─────────────────────────────────────────────────┤
│  Parser Layer (src/parser/)                     │
│  ┌──────────┐  ┌────────┐                       │
│  │markdown.rs│  │link.rs │                       │
│  │（変更なし）│  │（変更なし）│                       │
│  └──────────┘  └────────┘                       │
├─────────────────────────────────────────────────┤
│  Storage                                         │
│  ┌──────────┐  ┌──────────┐                     │
│  │  tantivy  │  │symbols.db│                     │
│  │（変更なし）│  │ ★変更   │                     │
│  └──────────┘  └──────────┘                     │
└─────────────────────────────────────────────────┘
```

## 3. 設計判断とトレードオフ

### 3.1 パーサー層 vs インデックス層でのフィルタリング

**判断**: フィルタリングはインデックス層（`src/indexer/` 配下）で行う

| 選択肢 | メリット | デメリット |
|--------|---------|----------|
| A: パーサー層でフィルタ | パーサーが完全な結果を返す | パーサーの責務範囲が広がる、テストが複雑化 |
| **B: インデックス層でフィルタ** ✅ | SRP準拠、パーサーは純粋な抽出に専念 | インデックス層にフィルタロジックが必要 |

**理由**: 既存の `extract_links()` は全リンクを抽出する設計。除外ルール（外部URL、スキーム、フラグメント）はビジネスロジックであり、インデックス層の責務。

**配置**: `is_indexable_link()` は `src/cli/index.rs` に配置する（SymbolStoreはCRUDに専念し、フィルタリングはCLI層の責務として分離）。

### 3.2 テーブル設計: link_text vs link_type

**判断**: `link_text` カラムを廃止し `link_type` カラムを採用

| 選択肢 | メリット | デメリット |
|--------|---------|----------|
| A: link_text（表示テキスト保存） | 将来の表示に使える | Link構造体にフィールドがなく実装不可 |
| **B: link_type（種別保存）** ✅ | 既存LinkType enumと対応、YAGNI | 表示テキストは後で追加が必要 |

### 3.3 ターゲット格納: raw vs 正規化

**判断**: raw target文字列をそのまま格納

| 選択肢 | メリット | デメリット |
|--------|---------|----------|
| **A: raw target格納** ✅ | 実装がシンプル、スコープ限定 | 検索時にパス解決が必要 |
| B: 正規化パス格納 | 即座に検索可能 | 実装複雑化、スコープ肥大 |

**理由**: 本Issueのスコープは「抽出・格納」のみ。パス解決は `--related` 検索の後続Issueで対応。

### 3.4 整合性パターン: Tantivy書き込み順序

**判断**: Tantivy書き込み成功後にDB書き込みを行う

```
1. symbol_store.delete_by_file()     // 古いリンク・データ削除
2. for section: writer.add_section() // Tantivy書き込み（複数section）
3. filter links (is_indexable_link)  // フィルタリング
4. symbol_store.insert_file_links()  // 新しいリンク挿入
```

**失敗時の挙動**:
- ステップ2（Tantivy書き込み）が途中で失敗した場合 → ステップ1で既にクリーン。ステップ3-4は実行されない。エラーを返す
- ステップ4（insert_file_links）が失敗した場合 → そのファイルのindexing失敗としてエラーを上位へ返す（Tantivy側は既に書き込み済みだが、次回のindex/updateで整合性が回復する）

**既存 `index_code_file()` との差異**:
- 既存: delete → insert_symbols → insert_dependencies → tantivy → 失敗時delete_by_fileロールバック
- Markdown: delete → tantivy → insert_file_links → 失敗時はエラー返却（次回indexで回復）
- Markdown特有: コードファイルと異なりシンボル・依存関係がないため、DB操作がinsert_file_linksのみでシンプル

### 3.5 スキーマバージョン不一致のエラー正規化

**判断**: 全コマンドで `SchemaVersionMismatch` を統一メッセージに正規化

**実装方式**: `From<SymbolStoreError> for IndexError` の実装内で `match` し、`SchemaVersionMismatch` バリアントのみ `IndexError::SchemaVersionMismatch` にマッピングする。

```rust
impl From<SymbolStoreError> for IndexError {
    fn from(e: SymbolStoreError) -> Self {
        match e {
            SymbolStoreError::SchemaVersionMismatch { .. } => IndexError::SchemaVersionMismatch,
            other => IndexError::SymbolStore(other),
        }
    }
}
```

`SearchError` と `StatusError` にも同様に `SchemaVersionMismatch` バリアントを追加し、`From<SymbolStoreError>` 実装でマッピングする。

### 3.6 run_incremental() の削除処理条件

**判断**: 全ファイルタイプで `symbol_store.delete_by_file()` を呼び出す

**理由**: `is_code()` 条件を `is_code() || is_markdown()` に拡張する方法は、将来 FileType が追加された際に漏れるリスクがある。`delete_by_file()` は対象テーブルにレコードがなければ空操作（0行削除）で無害なため、常に呼び出す方がシンプルかつ安全。

### 3.7 リンクの重複排除

**判断**: 重複は許容する（排除しない）

**理由**: 同一ファイル内で同じリンクが複数回出現することは正常なケース。重複排除は将来の検索・集計ロジックで必要に応じて行う。

### 3.8 FileLinkInfo の行番号

**判断**: 行番号は含めない

**理由**: 現在の `Link` 構造体には行番号フィールドがなく、`MarkdownDocument.links` はドキュメント全体から抽出される。行番号を含めるにはパーサー変更が必要になり、本Issueのスコープ（「パーサー層は変更なし」）に反する。

## 4. データモデル

### 4.1 file_links テーブル

```sql
CREATE TABLE IF NOT EXISTS file_links (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source_file TEXT NOT NULL,    -- リンク元ファイルの相対パス
    target_file TEXT NOT NULL,    -- リンク先（raw target文字列）
    link_type TEXT NOT NULL,      -- "WikiLink" / "MarkdownLink"
    file_hash TEXT NOT NULL       -- ファイルハッシュ（変更検知用）
);
CREATE INDEX IF NOT EXISTS idx_file_links_source ON file_links(source_file);
CREATE INDEX IF NOT EXISTS idx_file_links_target ON file_links(target_file);
```

**外部キー**: `file_links` は `symbols` テーブルへの外部キー参照を持たない。`source_file` TEXT列で独立に識別する。これにより `delete_by_file()` 内の DELETE 順序に依存しない。

### 4.2 FileLinkInfo 構造体（新規）

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct FileLinkInfo {
    pub id: Option<i64>,
    pub source_file: String,
    pub target_file: String,
    pub link_type: String,       // "WikiLink" / "MarkdownLink"（文字列としてstore層に閉じる）
    pub file_hash: String,
}
```

**設計**: `ImportInfo` と同パターン。`id` は挿入時 `None`、取得時 `Some(i64)`。

**レイヤー分離**: `FileLinkInfo.link_type` は `String` 型でstore層に閉じる。`parser::link::LinkType` → `String` への変換は `src/cli/index.rs`（CLI層）で行い、indexer層がparser層に依存しないようにする。

**不正値ハンドリング**: DB取得時に `link_type` が未知の文字列だった場合は、`SymbolStoreError` としてエラーを返す（データ破損として扱う）。

### 4.3 リンクフィルタリングルール

`src/cli/index.rs` に配置（SymbolStoreはCRUDに専念）:

```rust
use crate::parser::link::Link;

fn is_indexable_link(link: &Link) -> bool {
    let target = &link.target;
    // 長さ制限（DoS対策）
    if target.len() > 1024 {
        return false;
    }
    // 外部URL除外
    if target.starts_with("http://") || target.starts_with("https://") {
        return false;
    }
    // その他の外部スキーム除外
    if target.contains("://") || target.starts_with("mailto:") {
        return false;
    }
    // フラグメントのみ除外
    if target.starts_with('#') {
        return false;
    }
    true
}
```

**リンク数上限**: `index_markdown_file()` でフィルタリング後のリンク数が 10,000件を超えた場合は警告を出力し、超過分を切り捨てる。

## 5. モジュール変更計画

### 5.1 src/indexer/symbol_store.rs

| 変更 | 内容 |
|------|------|
| 定数変更 | `CURRENT_SYMBOL_SCHEMA_VERSION: u32 = 1` → `2` |
| テーブル追加 | `create_tables()` に `file_links` DDLを追加 |
| メソッド追加 | `insert_file_links(&self, links: &[FileLinkInfo]) -> Result<(), SymbolStoreError>` |
| メソッド追加 | `find_file_links_by_source(&self, source: &str) -> Result<Vec<FileLinkInfo>, SymbolStoreError>`（テスト検証用。`#[cfg(test)]` または後続Issueで公開API化を検討） |
| メソッド変更 | `delete_by_file()` に `file_links` DELETEを追加（同一トランザクション内） |
| 構造体追加 | `FileLinkInfo` |

**YAGNI**: `find_file_links_by_target()` は本Issueでは実装しない。テスト検証用に `find_file_links_by_source()` のみ実装。後続Issueで必要に応じて追加。

### 5.2 src/cli/index.rs

| 変更 | 内容 |
|------|------|
| 関数変更 | `index_markdown_file()` に `symbol_store: Option<&SymbolStore>` 引数追加 |
| 呼び出し変更 | `index_file_and_upsert()` のMarkdown分岐（L481）で `symbol_store` を転送 |
| ロジック追加 | リンクフィルタリング + `insert_file_links()` 呼び出し |
| ロジック追加 | delete-before-insert パターン + Tantivy失敗時ロールバック |
| ロジック変更 | `run_incremental()` の削除処理: `is_code()` 条件を削除し、全ファイルタイプで `delete_by_file()` を呼ぶ |
| エラー変更 | `From<SymbolStoreError> for IndexError` で `SchemaVersionMismatch` を正規化 |

### 5.3 src/cli/search.rs

| 変更 | 内容 |
|------|------|
| エラー追加 | `SearchError::SchemaVersionMismatch` バリアント追加 |
| From実装変更 | `From<SymbolStoreError>` で `SchemaVersionMismatch` をマッチして正規化 |
| Display実装 | `clean → index` の統一案内メッセージ |

### 5.4 src/cli/status.rs

| 変更 | 内容 |
|------|------|
| エラー追加 | `StatusError::SchemaVersionMismatch` バリアント追加 |
| ロジック変更 | `get_symbol_count()` で `SymbolStoreError::SchemaVersionMismatch` を特別扱いしてエラー伝播 |
| Display実装 | `clean → index` の統一案内メッセージ |

### 5.5 変更なしのモジュール

- `src/parser/link.rs` — 抽出ロジックは変更不要
- `src/parser/markdown.rs` — MarkdownDocument.linksは既に実装済み
- `src/cli/clean.rs` — ディレクトリ全削除のため影響なし

## 6. 処理フロー

### 6.1 index コマンド（フル再構築）

```
run()
  ├── SymbolStore::open() → create_tables()  // file_linksテーブル含む
  └── for each file:
      └── index_file_and_upsert(symbol_store)
          ├── [Code] → index_code_file(symbol_store)  // 既存
          └── [Markdown] → index_markdown_file(symbol_store)  // ★変更
              ├── parse_file()  → MarkdownDocument { links, sections, ... }
              ├── symbol_store.delete_by_file()  // ★追加
              ├── filter links (is_indexable_link)  // ★追加
              ├── symbol_store.insert_file_links()  // ★追加（失敗時はeprintln+skip）
              ├── for section: writer.add_section()  // 既存
              └── [add_section失敗時] symbol_store.delete_by_file()  // ロールバック
```

### 6.2 update コマンド（差分更新）

```
run_incremental()
  ├── SymbolStore::open() → create_tables()
  │   └── [SchemaVersionMismatch] → IndexError::SchemaVersionMismatch → clean→index案内
  ├── for deleted files:
  │   ├── writer.delete_by_path()
  │   └── symbol_store.delete_by_file()  // ★全ファイルタイプで呼び出し
  ├── for modified files:
  │   └── index_file_and_upsert()  // delete-before-insert含む
  └── for added files:
      └── index_file_and_upsert()
```

## 7. セキュリティ設計

| 脅威 | 対策 | 優先度 |
|------|------|--------|
| パストラバーサル | リンクtargetはraw格納のみ（ファイルアクセスなし） | 中 |
| SQLインジェクション | パラメータバインド（rusqlite params!マクロ） | 高 |
| 悪意あるリンク格納 | 格納のみで実行しない。検索時に表示するだけ | 低 |
| DoS（target長大化） | `is_indexable_link()` で target 長さ上限 1024 文字（DoS緩和のための暫定値） | 中 |
| DoS（リンク数爆発） | 1ファイルあたりリンク数上限 10,000件（DoS緩和のための暫定値。超過分は警告+切り捨て） | 中 |

## 8. 影響範囲

| 対象 | 影響 | 対応 |
|------|------|------|
| `index` コマンド | `index_markdown_file()` シグネチャ変更 | symbol_store引数追加、Markdown分岐でsymbol_store転送 |
| `update` コマンド | 削除処理を全ファイルタイプに拡張 | `is_code()` 条件を削除 |
| `search --symbol` | `SymbolStore::open()` のschema bump影響 | `SearchError::SchemaVersionMismatch` 追加（fail-fast: エラーを返しclean→index案内） |
| `status` コマンド | `SymbolStore::open()` のschema bump影響 | SchemaVersionMismatch時は警告メッセージ表示＋`symbol_count=0`で継続（fail-fastにはしない。statusは情報表示コマンドのため） |
| `clean` コマンド | 実装変更不要 | 対応不要 |
| `SymbolStore` | テーブル追加、CRUD追加、フィルタ関数追加 | file_links対応 |
| 既存テスト | schema_version変更(1→2)、delete_by_file拡張 | テスト更新（後述） |
| パフォーマンス | Markdownごとのlink抽出+SQLite write追加 | バルクinsertで軽減 |

## 9. テスト戦略

### 9.1 単体テスト（symbol_store.rs）

| テストケース | 検証内容 |
|-------------|---------|
| `test_insert_and_find_file_links` | FileLinkInfoの挿入と取得 |
| `test_delete_by_file_removes_file_links` | ファイル削除時にfile_linksも削除される |
| `test_schema_version_2` | スキーマバージョン2で正しくテーブルが作成される |
| `test_schema_version_mismatch_v1_to_v2` | v1 DBをv2で開くとSchemaVersionMismatch |
| `test_is_indexable_link` | フィルタリングルールの網羅テスト |

### 9.2 統合テスト

| テストケース | 検証内容 |
|-------------|---------|
| `test_index_markdown_with_wiki_links` | WikiLinkがfile_linksに格納される |
| `test_index_markdown_with_markdown_links` | MarkdownLinkがfile_linksに格納される |
| `test_index_markdown_excludes_external_urls` | 外部URLが除外される |
| `test_index_markdown_excludes_fragments` | フラグメントのみリンクが除外される |
| `test_update_markdown_file_links_rebuild` | update時にリンクが再構築される |
| `test_update_delete_markdown_removes_links` | Markdown削除時にリンクが削除される |
| `test_link_type_values` | "WikiLink"/"MarkdownLink"が正しく格納される |

### 9.3 既存テスト更新

| テストファイル | 更新内容 |
|---------------|---------|
| `tests/cli_index.rs` | `schema_version` 期待値は据え置き（`state.json` の `CURRENT_SCHEMA_VERSION` は変更なし。変更するのは `symbols.db` の `CURRENT_SYMBOL_SCHEMA_VERSION` のみ） |
| `src/indexer/symbol_store.rs` テスト | `test_index_error_from_symbol_store_error` のSchemaVersionMismatch正規化に対応 |

## 10. 品質基準

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |
