# マルチステージ設計レビュー サマリーレポート - Issue #66

## 概要
設計方針書の8段階レビュー（4段階×2回）が完了。

## ステージ別結果

| Stage | 種別 | 実行者 | Must Fix | Should Fix |
|-------|------|--------|----------|------------|
| 1 | 設計原則 | Claude opus | 2 | 4 |
| 2 | 整合性 | Claude opus | 2 | 2 |
| 3 | 影響分析 | Claude opus | 0 | 2 |
| 4 | セキュリティ | Claude opus | 0 | 2 |
| 5 | 設計原則(2回目) | Codex | 2 | 4 |
| 6 | 反映 | Claude sonnet | 全件反映 | - |
| 7 | 整合性(2回目) | Codex | 3 | 4 |
| 8 | 反映 | Claude sonnet | 全件反映 | - |

## 主要な改善点

### 1回目レビュー（Stage 1-4）
- try_rerankが非公開関数 → CLI経由テストに方針変更
- SymbolStore::open_in_memory()が統合テスト不可 → ファイルベース初期化+create_tables()
- ヘルパーをcommon/mod.rsからテストファイル内ローカルに移動
- tempfile::tempdir()による安全なランダムディレクトリ使用を明記

### 2回目レビュー（Stage 5-8 Codex）
- SymbolStoreの書込元記述を修正（本番経路では直接書込なし、テストfixture専用）
- ヘルパーのResult返却方針を統一（Box<dyn Error>返却）
- 公開APIパスを正確に修正（commandindex::indexer::symbol_store::*）
- BM25フォールバックの分類を「環境依存テスト」に修正
- config.tomlでendpoint=localhost:11434を明示
- フィクスチャ図にembeddings.db、config.tomlを追加

## 最終設計方針書の状態
- 全Must Fix指摘（9件）が反映済み
- 全Should Fix指摘の主要項目が反映済み
- 設計方針書は実装可能な状態
