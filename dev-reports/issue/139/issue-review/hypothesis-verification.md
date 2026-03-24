# 仮説検証レポート: Issue #139

## 検証日: 2026-03-24

---

## 仮説1: `search --related` はimportグラフとマークダウンリンクベースのみで関連ファイルを検出している

**判定: Partially Confirmed**

`find_related()` (`src/search/related.rs` 237-281行) は5種類のスコアリングを使用:

| # | メカニズム | 重み | メソッド |
|---|---|---|---|
| 1 | マークダウンリンク (双方向) | 1.0 | `score_markdown_links()` (283-345行) |
| 2 | import依存関係 (双方向) | 0.9 | `score_import_deps()` (347-388行) |
| 3 | タグマッチ | 0.5 | `score_tag_match()` (390-449行) |
| 4 | パス類似度 | 0.4 | `score_path_proximity()` (451-525行) |
| 5 | ディレクトリ近接 | 0.2/0.1 | `score_path_proximity()` (451-525行) |

ただしタグマッチとパス近接は**既にスコアが付いたファイルへの補助的加点のみ**。主要な発見メカニズムはimportグラフとマークダウンリンクで正しい。

## 仮説2: 同一Issueのドキュメント間の関連性を検出できない

**判定: Confirmed**

`src/search/related.rs` に「issue」「dev-reports」への参照は一切なし。Issue番号やプロジェクト構造に基づく概念的な関連性の自動検出機能は存在しない。

## 仮説3: symbols.dbに knowledge_nodes / knowledge_edges テーブルは存在しない

**判定: Confirmed**

`src/indexer/symbol_store.rs` 276-334行の `create_tables()` で定義されるテーブルは5つのみ:
- `schema_meta`, `symbols`, `dependencies`, `file_links`, `embeddings`

ソースコード全体で `knowledge_nodes` / `knowledge_edges` / `knowledge_graph` の検索結果はゼロ。

## 補足: 既存テーブル構造

**`dependencies`**: source_file, target_module, imported_names, file_hash
**`file_links`**: source_file, target_file, link_type, file_hash

---

## まとめ

| 仮説 | 判定 |
|---|---|
| 仮説1 | **Partially Confirmed** |
| 仮説2 | **Confirmed** |
| 仮説3 | **Confirmed** |

Issue #139の問題認識は正確であり、ナレッジグラフの追加は既存の関連性検出の限界を補完する有効なアプローチ。
