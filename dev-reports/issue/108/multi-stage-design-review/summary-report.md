# マルチステージ設計レビュー サマリーレポート

## Issue #108: impact/related にコードスニペット付きモード (--with-snippet)

## レビュー概要

| Stage | 種別 | 実行者 | Must Fix | Should Fix | Nice to Have |
|-------|------|--------|----------|------------|-------------|
| 1 | 設計原則レビュー | Claude opus | 2 | 4 | 3 |
| 2 | 整合性レビュー | Claude opus | 4 | 5 | 2 |
| 3 | 影響分析レビュー | Claude opus | 3 | 5 | 4 |
| 4 | セキュリティレビュー | Claude opus | 0 | 3 | 3 |
| 5 | 設計原則（2回目） | Codex | 3 | 4 | 2 |
| 6 | 指摘反映 | Claude sonnet | 全反映 | - | - |
| 7 | 整合性・影響分析（2回目） | Codex | 4 (3既修正) | 4 (2既修正) | 2 (1既修正) |
| 8 | 指摘反映 | Claude sonnet | 残1反映 | 残2反映 | - |

## 主要な設計改善

### Stage 1-4 で改善
- OutputFormat に PartialEq 追加（→ Stage 5 で matches! マクロに変更）
- changed_since.rs への波及を影響範囲に追加
- 構造体構築箇所（aggregate_impact, merge_related_results）の snippet: None 追加を明記
- serde アトリビュートの矛盾解消
- CLIバリデーション（snippet_lines/chars の範囲制限）
- 機密情報リスクのセキュリティ設計追加

### Stage 5-6 で改善（Codex レビュー後）
- **snippet 取得モジュールを output 層から cli 層に移動**（SRP 準拠: データ取得 ≠ 出力整形）
- `SnippetOptions` 構造体導入（enabled + config をまとめ、API 意味の明確化）
- `enrich_impact_with_snippets()` / `enrich_related_with_snippets()` 共通関数（DRY）
- `requires = "related"` 削除、実行時バリデーションに変更（--related-stdin 対応）
- `0=無制限` 特殊扱い削除（KISS）
- serde アトリビュートを「将来の備え」として残すのをやめ削除（YAGNI）
- `matches!` マクロで OutputFormat 比較（PartialEq derive 不要に）

### Stage 7-8 で改善
- context コマンドを変更しないことの明確化
- セキュリティ: 情報露出増加リスクの明記
- changed_since の回帰確認詳細化

## 設計品質評価
- **初版**: 基本的な設計は正しいが、レイヤー分離・API設計・既存コードとの整合性に課題
- **最終版**: SRP/KISS/YAGNI/DRY の原則に準拠し、影響範囲・セキュリティ・テスト戦略が明確な設計方針書
