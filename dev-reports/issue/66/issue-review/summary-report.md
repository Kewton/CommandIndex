# マルチステージIssueレビュー サマリーレポート - Issue #66

## 概要
- **Issue**: #66 [Feature] Phase 5 E2E統合テスト（Semantic Search・Hybrid・Rerank検証）
- **全8ステージ完了**

## ステージ別結果

| Stage | 種別 | 実行者 | Must Fix | Should Fix | Nice to Have |
|-------|------|--------|----------|------------|--------------|
| 0.5 | 仮説検証 | - | スキップ（仮説なし） | - | - |
| 1 | 通常レビュー(1回目) | Claude opus | 3 | 5 | 3 |
| 2 | 指摘反映(1回目) | Claude sonnet | 全8件反映 | - | - |
| 3 | 影響範囲レビュー(1回目) | Claude opus | 0 | 4 | 4 |
| 4 | 指摘反映(1回目) | Claude sonnet | 全4件反映 | - | - |
| 5 | 通常レビュー(2回目) | Codex | 4 | 4 | 3 |
| 6 | 指摘反映(2回目) | Claude sonnet | 全8件反映 | - | - |
| 7 | 影響範囲レビュー(2回目) | Codex | 2 | 4 | 3 |
| 8 | 指摘反映(2回目) | Claude sonnet | 全6件反映 | - | - |

## 主要な改善点

### Stage 1-2（1回目通常レビュー）
- シナリオ9「Context Pack + Semantic」→ 未実装機能のためスコープ変更
- Ollamaなし環境でのテスト方式（SymbolStoreへの固定embedding直接挿入）を明記
- 排他制御テストがclapパースエラーであることを明確化
- embedコマンド単体テスト追加
- BM25フォールバックテスト追加

### Stage 3-4（1回目影響範囲レビュー）
- Ollama依存テストに#[ignore]付与方針
- テストファイル名をe2e_semantic_hybrid.rsに決定
- 共通ヘルパー関数の方針追加

### Stage 5-6（2回目通常レビュー - Codex）
- **重大発見**: CLIのsemantic/hybrid検索は検索時にもクエリembedding生成でOllamaが必要
- テスト方式を「ライブラリAPI層」と「CLI層」の2層に分離
- EmbeddingStore(embeddings.db)とSymbolStore(symbols.db)の保存先分裂を明記
- embedの終了コード（非ゼロ）を修正
- --semanticのBM25フォールバック誤認を修正

### Stage 7-8（2回目影響範囲レビュー - Codex）
- ライブラリAPI層テストを公開APIで検証可能な範囲に限定
- 本体crateの新規公開API追加は別Issue対応と明記
- embedテスト（Ollama停止時エラー）をCI通常実行可能に再分類
- シナリオ13をCLI層テストに再分類

## 最終Issue構成
- 13テストシナリオ（2層構成）
- ライブラリAPI層: Ollama不要でCI通常実行
- CLI層: Ollama依存で#[ignore]
- 排他制御テスト: 既存cli_args.rsで担保済みのため除外
