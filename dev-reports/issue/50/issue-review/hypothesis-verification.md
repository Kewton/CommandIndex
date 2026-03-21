# 仮説検証レポート: Issue #50

## 検証日: 2026-03-21

## Issue概要
`commandindex search --related <file>` オプション実装（関連ドキュメント・コード検索）

---

## 仮説1: Markdownリンク（`[[]]` / `[]()`）による参照関係が検出可能

**判定: Confirmed ✓**

- `src/parser/link.rs`: WikiLink / MarkdownLink 解析が実装済み
- `src/indexer/symbol_store.rs`: `file_links` テーブルでリンク情報を永続化
- `find_file_links_by_source()` メソッドでソースファイルからのリンク検索が可能
- Issue #51 (a0a9a08) でリンクインデックス構築が実装・マージ済み

## 仮説2: frontmatterタグ一致による関連検出

**判定: Confirmed ✓**

- `src/parser/frontmatter.rs`: YAML frontmatter のタグ解析が実装済み
- `Frontmatter.tags: Vec<String>` でタグ情報を保持
- tantivy スキーマに `tags` フィールドが存在（linderaトークナイザ付き）
- `--tag` フィルタオプションも実装済み

## 仮説3: import/require依存関係（symbols.db）による関連検出

**判定: Confirmed ✓**

- `src/indexer/symbol_store.rs`: `dependencies` テーブルが実装済み
- `insert_dependencies()` / `find_imports_by_target()` メソッドが利用可能
- tree-sitter による TypeScript/Python のインポート解析が実装済み
- Issue #41 で import 依存関係の symbols.db 格納が完了

## 仮説4: searchコマンドの拡張が可能

**判定: Confirmed ✓**

- `src/cli/search.rs` (171行): 検索ロジック基盤が安定
- `src/main.rs`: clap サブコマンドに `--related` を追加可能な構造
- `SearchOptions` / `SearchFilters` が拡張可能
- 出力フォーマット（human/json/path）の基盤が存在

## 仮説5: スコアリング統合が可能

**判定: Partially Confirmed ⚠**

- tantivy の BM25 スコアリングは自動算出される
- `SearchResult.score: f32` でスコア保持済み
- **ただし**: 現在は全文検索のBM25スコアのみ。リンク関連度・import依存度の独自スコアリングは新規実装が必要
- マルチファクタスコアリングの統合方法は Issue に明確な仕様記載なし

---

## 検証サマリー

| 仮説 | 判定 | 備考 |
|------|------|------|
| Markdownリンク参照関係 | Confirmed | Issue #51 で完成 |
| frontmatterタグ一致 | Confirmed | 解析・インデックス・フィルタ全て実装済み |
| import/require依存関係 | Confirmed | symbols.db + tree-sitter で完成 |
| searchコマンド拡張 | Confirmed | clap + 検索基盤が拡張可能な設計 |
| スコアリング統合 | Partially Confirmed | BM25は既存、独自スコアリングは新規実装必要 |

## 依存Issue実装状態

全ての依存Issue（#9, #36, #37）およびリンク関連のIssue #51 がマージ済み。
実装に必要な基盤は全て揃っている状態。

## 要注意ポイント

1. **スコアリングの具体的な重み付け**: Issue には「高スコア/中スコア/低スコア」の記述はあるが、具体的な数値・アルゴリズムの定義がない
2. **逆方向リンク検索**: `find_file_links_by_source()` は存在するが、`find_file_links_by_target()`（指定ファイルを参照しているファイルの検索）は未実装 → 追加が必要
3. **パス近接性のロジック**: ディレクトリ構造に基づくスコアリングは完全に新規実装
