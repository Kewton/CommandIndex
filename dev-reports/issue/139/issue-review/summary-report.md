# マルチステージIssueレビュー サマリーレポート

## Issue: #139 SQLiteベースの簡易ナレッジグラフの実装

### レビュー概要

| Stage | レビュー種別 | レビュアー | Must Fix | Should Fix | Nice to Have |
|-------|------------|-----------|----------|-----------|-------------|
| 0.5 | 仮説検証 | Claude | - | - | - |
| 1 | 通常レビュー（1回目） | Claude Opus | 4 | 5 | 5 |
| 2 | 指摘反映（1回目） | Claude Sonnet | 反映済み | - | - |
| 3 | 影響範囲レビュー（1回目） | Claude Opus | 3 | 5 | 4 |
| 4 | 指摘反映（1回目） | Claude Sonnet | 反映済み | - | - |
| 5 | 通常レビュー（2回目） | Codex (gpt-5.4) | 3 | 3 | 2 |
| 6 | 指摘反映（2回目） | Claude Sonnet | 反映済み | - | - |
| 7 | 影響範囲レビュー（2回目） | Codex (gpt-5.4) | 3 | 1 | 0 |
| 8 | 指摘反映（2回目） | Claude Sonnet | 反映済み | - | - |

### 仮説検証結果

| 仮説 | 判定 |
|------|------|
| search --related はimport/リンクベースのみ | Partially Confirmed |
| 同一Issueドキュメント間の関連性を検出できない | Confirmed |
| knowledge_nodes/edges テーブルは存在しない | Confirmed |

### 主要な改善点（レビューを通じて追加・修正された内容）

1. **ノードタイプの整理**: 5種→2種（issue, document）に簡素化。file は後続Issue
2. **スキーマバージョン管理**: CURRENT_SYMBOL_SCHEMA_VERSION 3→4 を明記。IndexState は変更なし
3. **delete_by_file() 対応**: ON DELETE CASCADE による連鎖削除を設計
4. **独立走査パス**: 既存インデクサとは別にdev-reports/を走査する方式に変更
5. **index/update の処理フロー**: フル構築（全削除→再構築）と差分更新（git diff ベース）を明確化
6. **対象ドキュメントの限定**: Markdownサマリーファイルのみ対象、JSON除外
7. **RelationType 影響範囲**: 全出力フォーマッタ + impact/context のmatch文更新を明記
8. **テスト影響一覧**: schema_version, e2e_related_search, e2e_impact, output_format を明記
9. **スコア合成ルール**: add_relation() と同様の加算方式を明記
10. **impact/context 出力変化**: 許容される副作用として明文化

### 最終Issue品質評価

- **整合性**: 既存コードベースとの整合性が確保された（独立走査パス、スキーマバージョン管理）
- **正確性**: ディレクトリ構造に基づく正確なパースパターン定義
- **網羅性**: 完了条件が11項目に拡充され、テスト要件も明確
- **実装可能性**: スコープが適切に絞られ、後続Issueへの分割も明確

### レビュー完了日: 2026-03-24
