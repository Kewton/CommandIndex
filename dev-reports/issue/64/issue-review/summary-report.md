# Issue #64 マルチステージIssueレビュー サマリーレポート

## 対象Issue
- **Issue**: #64 [Feature] Hybrid Retrieval（BM25 + Semantic統合検索）
- **レビュー日**: 2026-03-22

## レビュー結果概要

| Stage | 種別 | 実行者 | Must Fix | Should Fix | Nice to Have |
|-------|------|--------|----------|------------|--------------|
| 0.5 | 仮説検証 | Claude | - | - | - |
| 1 | 通常レビュー(1回目) | Claude opus | 2 | 3 | 3 |
| 2 | 指摘反映(1回目) | Claude sonnet | 反映完了 | | |
| 3 | 影響範囲レビュー(1回目) | Claude opus | 5 | 5 | 4 |
| 4 | 指摘反映(1回目) | Claude sonnet | 反映完了 | | |
| 5 | 通常レビュー(2回目) | Codex | 2 | 4 | 2 |
| 6 | 指摘反映(2回目) | Claude sonnet | 反映完了 | | |
| 7 | 影響範囲レビュー(2回目) | Codex | 2 | 4 | 2 |
| 8 | 指摘反映(2回目) | Claude sonnet | 反映完了 | | |

## 主要な改善点

### 1回目レビューで追加
- CLIオプション設計の明確化（--no-semantic、既存--semanticとの関係）
- ドキュメント識別キー(path, heading)の定義
- 片方のみヒット時の扱い（limit+1方式）
- 出力結果型（SearchResult再利用）
- 重みオプション非実装の決定

### 2回目レビューで追加
- query+--heading時のBM25のみ動作方針
- (path, heading)ユニーク性の制約と根拠
- フォールバック条件の詳細化（query引数時/--semantic時の分離）
- 候補取得深さ（oversampling=limit*3）
- RRFスコア同点時の安定ソート
- **後方互換性**: provider/API障害時のBM25フォールバック（最重要改善）
- テスト観点の網羅的追加

## 未解決事項
なし（全Must Fix指摘が対応済み）

## Issue品質評価
レビュー前後でIssueの品質が大幅に向上。特に以下の点が改善された:
1. 既存CLI構造との整合性が明確化
2. エッジケースの処理方針が網羅的に定義
3. 後方互換性に配慮したフォールバック設計
4. テスト観点が明確化
