# 仮説検証レポート: Issue #51

## 検証日: 2026-03-21

---

## 仮説1: symbols.dbが既に存在するか（#36で導入済みか）

**判定: Confirmed**

- `src/indexer/mod.rs` で `SYMBOLS_DB_FILE = "symbols.db"` が定義済み
- `src/indexer/symbol_store.rs` に完全な `SymbolStore` 実装（366行）
- スキーマバージョン管理（`CURRENT_SYMBOL_SCHEMA_VERSION: u32 = 1`）実装済み
- 既存テーブル: `schema_meta`、`symbols`、`dependencies`
- `FOREIGN_KEYS = ON`、カスケード削除、複数インデックスも整備済み

## 仮説2: Markdownリンク抽出パーサーが必要

**判定: Partially Confirmed（パーサーは実装済み、DB保存が未実装）**

- `src/parser/link.rs` に `extract_links()` が実装済み（`[[wiki-link]]`と`[text](path)`両対応）
- `src/parser/markdown.rs` の `MarkdownDocument` に `links: Vec<Link>` フィールドあり
- `parse_content()` 内で `link::extract_links(body)` を呼び出し済み
- **未実装**: 抽出したリンクをsymbols.dbに保存するフロー

## 仮説3: index/updateでリンク保存が必要

**判定: Confirmed**

- `src/cli/index.rs` の `index_markdown_file()` で `doc.links` は完全に無視されている
- コードファイルの `index_code_file()` ではsymbols.dbへの書き込みが行われている
- Markdownファイル処理にはDB保存処理が欠落

## 仮説4: file_linksテーブル設計の妥当性

**判定: Confirmed**

- 既存の `dependencies` テーブルと同じパターンで実装可能
- `rusqlite` は `Cargo.toml` で bundled フィーチャー付きで導入済み
- CRUD操作パターン（トランザクション一括挿入、ファイル単位削除）が確立済み

## 仮説5: parser/モジュール構成

**判定: Confirmed（CLAUDE.mdの想定より充実）**

実際の構成:
```
src/parser/
├── mod.rs          — モジュール宣言 + parse_directory()
├── markdown.rs     — MarkdownDocument, Section, parse_file(), parse_content()
├── frontmatter.rs  — Frontmatter, extract_frontmatter(), parse_frontmatter()
├── link.rs         — Link, LinkType, extract_links()
├── code.rs         — SymbolInfo, CodeParseResult, parse_code_file()
├── typescript.rs   — tree-sitter TypeScript/TSXパーサー
├── python.rs       — tree-sitter Pythonパーサー
└── ignore.rs       — .cmindexignoreフィルター
```

---

## 総合まとめ

| 検証項目 | 状態 | 判定 |
|---|---|---|
| symbols.db / SymbolStore | 実装済み | Confirmed |
| file_linksテーブル | 未実装 | Confirmed（実装が必要） |
| Markdownリンク抽出パーサー | 実装済み | Partially Confirmed |
| index/update連携 | 部分実装 | Confirmed |
| rusqlite依存 | 実装済み | Confirmed |

**実装スコープの明確化**: Issueの提案する解決策は妥当だが、リンク抽出パーサーは既に実装済みのため、実装作業はDB保存フローの追加が中心となる。
