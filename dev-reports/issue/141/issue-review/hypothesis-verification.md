# Issue #141 仮説検証レポート

## 検証対象
SQLクエリ仮説を用いて「`commandindexdev why <file>`コマンド」の技術的実現可能性を検証。

## 最終判定

| 検証項目 | 判定 | 備考 |
|---------|------|------|
| スキーマ存在確認 | **Confirmed** | 完全実装済み |
| カラム検証 | **Confirmed** | すべて存在 |
| ノードタイプ使用確認 | **Confirmed** | issue/document実装済み |
| エッジ方向性 | **Partially Confirmed** | 実装と仮説で差異あり |
| CLI構造整備 | **Confirmed** | 拡張可能な構造 |
| 出力フォーマット | **Confirmed** | 4形式利用可能 |
| ナレッジ操作API | **Confirmed** | メソッド完備 |
| **`why`コマンド実装可能性** | **Confirmed** | 基盤完全、設計調整のみ必要 |

## 主な知見

### エッジ方向性の差異
- **仮説**: `file → (modifies) → issue → (has_design/has_review) → document`
- **実装**: `document ← issue → document`（sibling検索パターン）
- 現在の実装では `file` ノードタイプは使われておらず、`issue` と `document` のみ
- 既存メソッド `find_knowledge_related` は document → issue → document の経路を走査

### ナレッジグラフスキーマ
- knowledge_nodes: id, type, identifier, title, file_path, created_at, updated_at
- knowledge_edges: id, source_id, target_id, relation, metadata
- relation値: has_design, has_review, has_workplan

### CLI構造
- 既存サブコマンド: index, search, update, status, clean, diff, context, embed, config, export, impact, import, suggest, help-llm, watch
- `why` サブコマンドは未実装 → 新規追加が必要

### 出力フォーマット
- Human / Json / Path / Llm の4形式が既存実装済み
