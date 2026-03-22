# Issue #90 マルチステージ設計レビュー サマリーレポート

## レビュー日時
2026-03-23

## レビュー対象
dev-reports/design/issue-90-impact-design-policy.md

## レビュー完了ステージ

| Stage | 種別 | エージェント | Must Fix | Should Fix | Nice to Have |
|-------|------|-------------|----------|------------|-------------|
| 1 | 設計原則 | Claude opus | 4 | 3 | 3 |
| 2 | 整合性 | Claude opus | 3 | 4 | 3 |
| 3 | 影響分析 | Claude opus | 4 | 5 | 3 |
| 4 | セキュリティ | Claude opus | 1 | 4 | 4 |
| 5 | 設計原則（2回目） | Codex | 2 | 4 | 2 |
| 7 | 整合性・影響（2回目） | Codex | 2 | 4 | 2 |

## 主要な改善点

1. **データモデル**: フィールド名マッピングテーブル追加、全新型に derive 明記
2. **aggregate_impact**: 擬似コードで全面書き換え仕様を明確化
3. **overlap**: 検出アルゴリズムとテストデータ仕様を追加
4. **--limit**: per-file 適用 + 破壊的変更として明記
5. **エラーハンドリング**: FileNotFound/FileNotIndexed は warning 継続、その他は fail-fast
6. **セキュリティ**: UTF-8 truncation 修正、引数ファイル数制限、JSON は serde escape に委ねる
7. **影響範囲**: tests/cli_args.rs 追加、制限事項（1000件上限）明記
8. **path 出力**: max スコア代表値を明記

## 結論
設計方針書は実装可能な品質に達した。主要な設計判断（ネスト構造、overlap検出、limit適用タイミング）が明確に文書化されている。
