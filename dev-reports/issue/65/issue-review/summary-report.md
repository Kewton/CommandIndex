# Issue #65 マルチステージレビュー サマリーレポート

## 実施日: 2026-03-22

## レビュー概要

| Stage | 種別 | 実行エージェント | Must Fix | Should Fix | Nice to Have |
|-------|------|-----------------|----------|------------|--------------|
| 0.5 | 仮説検証 | Claude | - | - | - |
| 1 | 通常レビュー（1回目） | Claude (opus) | 3 | 4 | 4 |
| 2 | 指摘反映（1回目） | Claude (sonnet) | 7件反映 | - | - |
| 3 | 影響範囲レビュー（1回目） | Claude (opus) | 5 | 6 | 4 |
| 4 | 指摘反映（1回目） | Claude (sonnet) | 8件反映 | - | - |
| 5 | 通常レビュー（2回目） | Codex (gpt-5.4) | 4 | 5 | 3 |
| 6 | 指摘反映（2回目） | Claude (sonnet) | 8件反映 | - | - |
| 7 | 影響範囲レビュー（2回目） | Codex (gpt-5.4) | 4 | 4 | 3 |
| 8 | 指摘反映（2回目） | Claude (sonnet) | 7件反映 | - | - |

## 主要な改善ポイント

### 技術的実現性（Stage 1 M1 → Stage 5 S2）
- Ollama `/api/rerank` エンドポイントが存在しない問題を特定
- `/api/generate` ベースのプロンプト方式をプライマリに決定
- 汎用生成モデル（llama3等）ベースに変更

### 実装箇所の整合性（Stage 5 M1）
- 検索フロー変更箇所を `src/search/hybrid.rs` → `src/cli/search.rs` に修正
- run() と run_semantic_search() の両経路を考慮

### スコープ限定（Stage 5 M2, Stage 7 M4）
- `--semantic` 経路を初期実装スコープ外に限定
- Cohere は トレイト/enum 定義のみ、実装は将来Issue

### パフォーマンス設計（Stage 5 M3, Stage 7 M1, M3）
- `top_candidates` デフォルト 50 → 20 に縮小
- 全体タイムアウト 30秒追加
- 候補取得数 = `max(limit, rerank_top)` ルール追加
- パフォーマンス要件を「運用目安」に位置づけ

### 候補本文の定義（Stage 5 M4, Stage 7 S4）
- `document_text = heading + "\n" + body`（最大4096文字でtruncate）

### CLI設計改善（Stage 5 S3）
- `--rerank-top` に `requires = "rerank"` 追加

### 型安全性（Stage 5 S5）
- `RerankConfig.provider` を `String` → `RerankProviderType` enum に変更

### Graceful Degradation（Stage 5 S4, Stage 7 M3）
- 全件失敗時は元順序を返す
- 部分失敗のtie-breakは元順序維持（安定ソート）
- タイムアウト時はスコア取得済み候補のみで再ソート

## 最終Issue状態

Issue #65 は全8ステージのレビューを経て、以下が明確化されました:
- 実装箇所と変更範囲
- APIエンドポイントとリクエスト/レスポンス形式
- 初期実装のスコープ（Ollamaのみ、SearchResult経路のみ）
- パフォーマンス要件と障害時動作
- テスト方針と既存テストへの影響
- Config構造体の拡張設計
