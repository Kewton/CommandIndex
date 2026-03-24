# 仮説検証レポート — Issue #140

## 検証日: 2026-03-24

## 仮説一覧と検証結果

### 仮説1: ナレッジグラフ（knowledge_nodes / knowledge_edges）が実装済み
- **判定**: ✅ Confirmed
- **根拠**: `src/indexer/symbol_store.rs` にテーブル定義（knowledge_nodes, knowledge_edges）とCRUD操作が完全実装済み

### 仮説2: SQLクエリが現在のスキーマで実行可能
- **判定**: ⚠️ Partially Confirmed
- **根拠**: Issue記載のSQLクエリは概ね正しいが、実装のマッピングに差異あり
  - ノードtype: `issue`, `document`（Issueでは `design_policy`, `review` 等を想定）
  - リレーション: `has_design`, `has_review`, `has_workplan`
  - doc_subtypeはmetadataのJSON内に格納

### 仮説3: 出力フォーマット（human/json/llm）が利用可能
- **判定**: ✅ Confirmed
- **根拠**: `src/output/mod.rs` に `OutputFormat` enum（Human, Json, Path, Llm）が定義済み

### 仮説4: CLIサブコマンド構造が拡張可能
- **判定**: ✅ Confirmed
- **根拠**: `main.rs` に `Commands` enum、`src/cli/` に各コマンドモジュールが整理されており、新規サブコマンド追加が容易

## 実装ギャップ

| 必要な実装 | 状態 |
|---|---|
| `issue` サブコマンド定義（main.rs） | ❌ 未実装 |
| `src/cli/issue.rs` モジュール | ❌ 未実装 |
| Issue番号による検索クエリ | ❌ 未実装 |
| 出力フォーマット処理（issue用） | ❌ 未実装 |

## 結論

前提条件であるナレッジグラフは完全に実装済み。CLIフレームワークと出力フォーマットも整備済み。
Issue記載のSQLクエリはtype/relationのマッピングを実装に合わせて調整が必要。
実装準備度: **高**（CLI層の追加のみ）
