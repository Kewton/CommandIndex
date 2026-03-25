# 仮説検証レポート - Issue #157

## 対象Issue
suggestコマンドがナレッジグラフを参照していない

## 仮説と検証結果

### 仮説1: suggestはBM25全文検索の上位結果に基づいて戦略を生成している
**判定: Partially Confirmed**

suggestコマンドはBM25検索結果だけでなくセマンティック検索結果も使用し、RRFで統合している（`suggest.rs` 行245-283）。ただし、エンベディング未構築時はBM25結果のみに依存する。戦略生成はトップファイルに対して`context`, `search --related`, `impact`コマンドを機械的に提案するもの。

### 仮説2: クエリ中のIssue番号を認識してナレッジグラフを参照する仕組みがない
**判定: Confirmed**

suggestコマンドにはIssue番号パターン認識コードが存在せず、ナレッジグラフ（`SymbolStore`, `knowledge`モジュール）への参照も一切ない。`extract_issue_numbers()`関数は`indexer/knowledge.rs`に存在するが、suggestコマンドからは呼ばれていない。

### 仮説3: 汎用語がBM25で高スコアになり的外れなファイルが上位に来る
**判定: Confirmed**

根拠:
- ストップワード処理なし（`schema.rs`行55-66）
- クエリ前処理なし（`validate_input()`はトリムのみ）
- Issue番号`#NNN`は数字としてトークン化され無関連な数値とマッチし得る

## 関連ファイル

| ファイル | 役割 |
|---|---|
| `src/cli/suggest.rs` | suggestコマンド実装（戦略生成の全ロジック） |
| `src/cli/issue.rs` | issueコマンド実装（ナレッジグラフ参照の実例） |
| `src/indexer/knowledge.rs` | ナレッジグラフ型定義、`ISSUE_RE`, `extract_issue_numbers()` |
| `src/indexer/reader.rs` | BM25検索実装 |
| `src/indexer/schema.rs` | tantivy スキーマ・トークナイザー設定 |
| `src/search/hybrid.rs` | RRF統合 |
| `src/search/ranking.rs` | ファイル単位集約・ファイル種別重み付け |
| `src/indexer/symbol_store.rs` | SymbolStore（ナレッジグラフDB操作） |
