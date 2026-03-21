# Issue #62 マルチステージ設計レビュー サマリーレポート

## 実施ステージ

| Stage | 種別 | 実施 | Must Fix | Should Fix | Nice to Have |
|-------|------|------|----------|------------|--------------|
| 1 | 設計原則 | Claude opus | 2 | 3 | 3 |
| 2 | 整合性 | Claude opus | 2 | 4 | 3 |
| 3 | 影響分析 | Claude opus | 2 | 4 | 3 |
| 4 | セキュリティ | Claude opus | 2 | 4 | 4 |
| 5-8 | 2回目 | Codex | スキップ（認証エラー） | - | - |

## Must Fix指摘と対応

### Stage 1: 設計原則
1. **SRP違反: SymbolStore肥大化** → SymbolStore拡張で対応（理由を設計書に明記）
2. **UPSERT戦略未定義** → INSERT OR REPLACE明記

### Stage 2: 整合性
1. **EmbeddingInfoにidフィールド欠落** → id: Option<i64>追加
2. **insert_embeddings UPSERT不明確** → INSERT OR REPLACE + docコメント

### Stage 3: 影響分析
1. **chrono依存の記述矛盾** → 技術選定にchrono使用を明記
2. **UPSERT戦略の曖昧さ** → INSERT OR REPLACE明記（Stage 2と同一）

### Stage 4: セキュリティ
1. **BLOB読み込みバリデーション欠如** → blob_to_embeddingをResult型に変更、dimension*4チェック
2. **入力バリデーション不足** → 空ベクトル・NaN/Infinityの検証追加

## 設計方針書への反映事項
- SRP考慮のコメント追加（分離せずSymbolStore拡張する理由を明記）
- EmbeddingInfoにid: Option<i64>追加
- blob_to_embeddingをResult型に変更（バリデーション付き）
- INSERT OR REPLACE戦略明記
- chrono依存を技術選定に追記
- 入力バリデーション（NaN/Infinity/空ベクトル）追加
- dimension不一致時のフィルタ+warnログ方式に変更
- InvalidEmbeddingエラーバリアント追加
- 影響範囲にcli_index.rs、e2e_phase3テストの分析追加
- テスト戦略に統合テスト3件追加

## 結論
設計方針書は4段階レビューを経て、入力バリデーション・BLOBバリデーション・UPSERT戦略・エラーハンドリングが強化された。
