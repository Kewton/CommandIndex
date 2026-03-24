# マルチステージ設計レビュー サマリーレポート - Issue #134

## レビュー実施結果

| Stage | 種別 | 実行者 | Must Fix | Should Fix | Nice to Have |
|-------|------|--------|----------|------------|--------------|
| 1 | 設計原則 (SOLID/KISS/YAGNI/DRY) | Claude (opus) | 2 | 3 | 3 |
| 2 | 整合性 | Claude (opus) | 2 | 4 | 3 |
| 3 | 影響分析 | Claude (opus) | 2 | 4 | 3 |
| 4 | セキュリティ | Claude (opus) | 0 | 3 | 3 |
| 5 | 設計原則（2回目） | Codex (gpt-5.4) | 2 | 3 | 1 |
| 6 | 指摘反映 | Claude (sonnet) | - | - | - |
| 7 | 整合性・影響分析（2回目） | Codex (gpt-5.4) | 3 | 3 | 1 |
| 8 | 指摘反映 | Claude (sonnet) | - | - | - |

## 主要な改善点

### Must Fix（全て反映済み）
1. **suggest.rsの影響漏れ** - search_similar()を使用しているが「変更なし」と誤記（3ステージから同一指摘）
2. **delete_staleの安全性** - 冪等性明記、空文字列時はInvalidEmbeddingエラー（Fail Fast原則）
3. **symbol_store.rsの除外明記** - 同名メソッドだが独立型のため変更対象外
4. **SRP違反** - warn_if_dimension_mismatch()のeprintln!をCLI層に移動→should_warn_dimension_mismatch()に変更
5. **テスト戦略の矛盾修正** - 空文字列テストの期待値をInvalidEmbeddingエラーに統一
6. **API名の不一致修正** - 設計書全体でshould_warn_dimension_mismatch()に統一
7. **既存エラー型の利用** - InvalidArgument新設ではなくInvalidEmbedding(既存variant)を使用

### Should Fix（主要なもの）
- 既存テスト修正範囲の具体化（store.rs 8件 + e2e 2箇所）
- API変更方針の明記（内部APIのためbreaking change許容）
- 警告表示ロジックのDRY化（SimilaritySearchOutputメソッド）

## 設計方針書の最終状態

設計方針書は8段階のレビューを経て以下が確立:
- **T1:** known_dimension 1行追加
- **T1.5:** has_current_embedding()にmodel引数追加 + delete_stale_model_embeddings()（Fail Fast + 冪等）
- **T2.5:** SimilaritySearchOutput構造体（should_warn_dimension_mismatch()副作用なしメソッド）
- **影響範囲:** suggest.rs含む全CLI経路をカバー
- **テスト戦略:** 新規テスト6件 + 既存修正10件を具体化
