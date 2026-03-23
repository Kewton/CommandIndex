# マルチステージ設計レビュー サマリーレポート - Issue #116

## 実施結果

| Stage | 種別 | 実行者 | Must Fix | Should Fix | Nice to Have |
|-------|------|--------|----------|------------|-------------|
| 1 | 設計原則 | Claude (opus) | 1 | 3 | 2 |
| 2 | 整合性 | Claude (opus) | 3 | 3 | 3 |
| 3 | 影響分析 | Claude (opus) | 3 | 4 | 3 |
| 4 | セキュリティ | Claude (opus) | 0 | 2 | 3 |
| 5 | 設計原則(2回目) | Codex (gpt-5.4) | 3 | 4 | 2 |
| 6 | 指摘反映 | Claude (sonnet) | 反映済 | - | - |
| 7 | 整合性・影響(2回目) | Codex (gpt-5.4) | 3 | 3 | 2 |
| 8 | 指摘反映 | Claude (sonnet) | 反映済 | - | - |

## 主要な設計変更

1. **ISP準拠のAPI設計**: llm_optionsをformat_results/format_impact_resultsの2関数のみに限定（当初は全8関数に追加する案）
2. **--snippet-lines=0の統一**: LLMでも0は「無制限」として統一（当初は「空出力」の案）
3. **cli/impact.rsの追加**: 変更対象ファイルに漏れていたcli/impact.rsを追加
4. **YAGNI対応**: show_token_estimateフィールドを削除
5. **エラーハンドリング章の追加**: 新規エラー型不要、OutputError範囲内を明記
6. **セキュリティ明確化**: 既存より悪化させない、フェンス閉じ保証を明記

## 最終設計品質
- 設計原則: SOLID/KISS/YAGNI/DRY準拠
- 整合性: 既存コードベースと整合
- 影響範囲: 明確に文書化
- セキュリティ: リスク低、対策明記
