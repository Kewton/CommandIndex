# Issue #169 仮説検証レポート

## 検証結果サマリー

| 仮説 | 判定 | 詳細 |
|---|---|---|
| 1. `issue` コマンド存在 | **Confirmed** | コマンド実装済み、Issue番号引数とformat対応 |
| 2. `knowledge_edges` に `issue_number` カラム | **Rejected** | knowledge_nodes を JOIN で取得する設計 |
| 3. `--format` オプション実装 | **Confirmed** | human/json/path/llm 形式、複数コマンドで使用 |
| 4. 設計書ファイルからラベル抽出 | **Confirmed** | パターンマッチ→DocSubtype→日本語ラベル |

## 仮説1: `issue` コマンドが既に存在するか — Confirmed

- `src/cli/issue.rs` (行120-159): `run(issue_number, format, commandindex_dir)` 実装
- `src/main.rs` (行295-302, 982-998): clapコマンド定義・ハンドラー

## 仮説2: `knowledge_edges` に `issue_number` カラム — Rejected

`knowledge_edges` は source_id/target_id/relation/metadata の構造。Issue番号は `knowledge_nodes` テーブル (type='issue', identifier=issue_number) に保存。`find_documents_by_issue()` で JOIN して取得。

## 仮説3: `--format` オプション — Confirmed

`OutputFormat` enum (Human/Json/Path/Llm) が `src/output/mod.rs` で定義済み。issue, why, suggest, before-change コマンドで使用。

## 仮説4: 設計書ファイルからラベル抽出 — Confirmed (メタデータ経由)

`src/indexer/knowledge.rs` (行369-415) でファイルパスパターンから DocSubtype を判定し、`display_label()` で日本語ラベル表示。

## Issue修正が必要な点

- 「knowledge_edgesテーブルからissue_numberのDISTINCTを取得」は不正確。正しくは knowledge_nodes テーブルで type='issue' のノードを DISTINCT 取得し、knowledge_edges で関連ドキュメントを参照する設計。
