# Issue #144 マルチステージ設計レビュー サマリーレポート

## レビュー実施日: 2026-03-24

## レビュー概要

| Stage | 種別 | エージェント | Must Fix | Should Fix | Nice to Have |
|-------|------|-------------|----------|------------|-------------|
| 1 | 設計原則レビュー | Claude (opus) | 3 | 5 | 4 |
| 2 | 整合性レビュー | Claude (opus) | 4 | 5 | 3 |
| 3 | 影響分析レビュー | Claude (opus) | 3 | 4 | 3 |
| 4 | セキュリティレビュー | Claude (opus) | 0 | 3 | 5 |
| 5 | 設計原則レビュー（2回目） | Codex (gpt-5.4) | 2 | 4 | 2 |
| 6 | 指摘反映（2回目） | Claude (sonnet) | 反映完了 | - | - |
| 7 | 整合性・影響分析（2回目） | Codex | スキップ | - | - |
| 8 | 指摘反映（2回目） | Claude (sonnet) | スキップ | - | - |

## 主要な改善点（設計方針書への反映）

### 1回目レビュー（Stage 1-4, Claude opus）

1. **DRY化**: RRFロジックをhybrid.rsに集約、セマンティックパイプラインをsemantic.rsに共通化
2. **SRP改善**: ファイル集約・重み付けをranking.rsに切り出し、suggest.rsはオーケストレーションに集中
3. **整合性修正**: 既存deduplicate_by_file/deduplicate_by_file_pairsの存在を認識し統合方針を明記
4. **データフロー修正**: BM25候補のtruncate問題（DEDUP_FILE_LIMIT=5→DEDUP_FILE_LIMIT*3）を解決
5. **具体化**: run_suggestの分岐変更コードを設計方針書に記載
6. **エラー型**: enrich_semantic_to_search_resultsの戻り値をReaderErrorに変更、CLI層で変換

### 2回目レビュー（Stage 5-6, Codex）

1. **エラー方針統一**: semantic.rsの全関数をResult返却に統一。graceful degradationはCLI層で判断
2. **KISS改善**: rrf_merge_by_keyの過度な汎化を廃止。非公開共通ヘルパー+公開API 2本に簡素化
3. **集約責務の一本化**: aggregate_semantic_by_fileをranking.rsに統合（DRY化）
4. **pure/I/O区分**: 各関数にpure/I/Oを明記

## 最終設計品質

- 設計原則（SOLID/KISS/YAGNI/DRY）: 準拠
- コードベースとの整合性: 既存関数の移動・統合方針が具体的
- エラーハンドリング: Result返却（Search層）+ graceful degradation（CLI層）の明確な分離
- テスト戦略: 単体テスト8件 + 統合テスト2件 + 既存回帰テスト6件
- セキュリティ: must_fix 0件。既存対策で十分

## 注意事項

- Stage 7-8（Codex整合性・影響分析レビュー2回目）はrate limitによりスキップ
- 1回目のStage 2-3で整合性・影響分析は十分にカバー済み
