# Issue #64 マルチステージ設計レビュー サマリーレポート

## レビュー結果概要

| Stage | 種別 | 実行者 | Must Fix | Should Fix | Nice to Have |
|-------|------|--------|----------|------------|--------------|
| 1 | 設計原則(SOLID/KISS/YAGNI/DRY) | Claude opus | 2 | 4 | 3 |
| 2 | 整合性 | Claude opus | 4 | 4 | 3 |
| 3 | 影響分析 | Claude opus | 1 | 3 | 3 |
| 4 | セキュリティ | Claude opus | 2(既存) | 3 | 4 |
| 5 | 設計原則(2回目) | Codex | 4 | 3 | 1 |
| 6 | 指摘反映 | Claude sonnet | 反映完了 | | |
| 7 | 整合性・影響分析(2回目) | Codex | 2 | 3 | 2 |
| 8 | 指摘反映 | Claude sonnet | 反映完了 | | |

## 主要な設計改善

### 1回目レビューで改善
- run()シグネチャ変更なし（no_semanticをSearchOptionsに統合）
- rrf_merge()を純粋関数化（入力を&[SearchResult]に統一）
- score意味の明確化（ドキュメントコメント）
- E2Eテスト影響分析の追加
- HYBRID_OVERSAMPLING_FACTOR定数の導入

### 2回目レビューで改善
- RRF片側ヒット仕様を標準RRF準拠に修正（未出現側の寄与=0）
- ストレージ前提の正確な記述（SymbolStore使用）
- フィルタ適用位置の明確化（semantic側もフィルタ後にRRF）
- エラー分類の体系化（外部依存一時障害→フォールバック、ローカル破損→fail-fast）
- セキュリティリスクの明示（auto-hybridによる外部送信）
- テスト影響対象の拡充

## 品質確認
設計方針書の変更なので、ビルド/テストへの影響なし。
