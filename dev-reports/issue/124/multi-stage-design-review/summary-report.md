# マルチステージ設計レビュー サマリーレポート

## Issue: #124 設計方針書レビュー

## レビュー結果

| Stage | 種別 | レビュアー | Must Fix | Should Fix | Nice to Have |
|-------|------|-----------|----------|------------|--------------|
| 1 | 設計原則 | Claude Opus | 3 | 3 | 3 |
| 2 | 整合性 | Claude Opus | 6 | 4 | 3 |
| 3 | 影響分析 | Claude Opus | 3 | 3 | 3 |
| 4 | セキュリティ | Claude Opus | 1 | 3 | 3 |
| 5 | 設計原則(2nd) | Codex | 3 | 3 | 2 |
| 7 | 整合性・影響(2nd) | Codex | 4 | 4 | 2 |

## 主要な改善点

1. **suggest.rsの漏れ発見**: `maybe_add_semantic_step()`もSymbolStoreを参照しており、変更対象に追加
2. **異常系契約の精緻化**: `embeddings.db`未存在はPath::exists()で事前判定、テーブル未作成はno such table変換
3. **SchemaVersionMismatch分離**: NoEmbeddingsとは分離して専用メッセージに
4. **BLOBバリデーション追加**: blob_to_embedding()検証関数とInvalidEmbeddingエラー
5. **NaN/Infフィルタリング**: search_similar()結果のフィルタリング追加
6. **#[deprecated]スコープ縮小**: 非推奨化は後続Issueに分離、バグ修正は最小変更に集中
7. **責務境界の明確化**: EmbeddingStore=検索、SymbolStore=メタデータ補完

## 全Must Fix対応状況
全てのMust Fix指摘が設計方針書に反映済み。
