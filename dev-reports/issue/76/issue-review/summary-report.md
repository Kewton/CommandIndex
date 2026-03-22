# マルチステージIssueレビュー サマリーレポート - Issue #76

## Issue
- **番号**: #76
- **タイトル**: [Feature] チーム共有設定ファイル（config.toml）
- **実施日**: 2026-03-22

## レビュー概要

| Stage | レビュー種別 | 実行エージェント | Must Fix | Should Fix | Nice to Have |
|-------|------------|----------------|----------|------------|--------------|
| 0.5 | 仮説検証 | Claude | - | - | - |
| 1 | 通常レビュー（1回目） | Claude (opus) | 3 | 4 | 4 |
| 2 | 指摘事項反映（1回目） | Claude (sonnet) | 反映完了 | 反映完了 | 反映完了 |
| 3 | 影響範囲レビュー（1回目） | Claude (opus) | 4 | 5 | 2 |
| 4 | 指摘事項反映（1回目） | Claude (sonnet) | 反映完了 | 反映完了 | - |
| 5 | 通常レビュー（2回目） | Codex (gpt-5.4) | 2 | 3 | 1 |
| 6 | 指摘事項反映（2回目） | Claude (sonnet) | 反映完了 | 反映完了 | 反映完了 |
| 7 | 影響範囲レビュー（2回目） | Codex (gpt-5.4) | 3 | 4 | 2 |
| 8 | 指摘事項反映（2回目） | Claude (sonnet) | 反映完了 | 反映完了 | - |

## 主要な改善点

### Stage 1-2 で追加された内容
1. **マージ仕様の明確化**: フィールドレベルマージの定義と具体例
2. **環境変数スコープの限定**: Phase 1 は COMMANDINDEX_OPENAI_API_KEY のみ
3. **既存 embedding::Config の移行計画**: 全6箇所の呼び出しサイト特定

### Stage 3-4 で追加された内容
4. **clean.rs のハードコード参照更新**: 新設定ファイル名への追従
5. **型設計**: EmbeddingConfig/RerankConfig は現モジュールに残す（循環依存回避）
6. **E2Eテスト更新計画**: e2e_embedding.rs, e2e_semantic_hybrid.rs
7. **CLI引数と設定ファイルの優先関係**: Option<usize> による実装方針
8. **Config::load() 呼び出し最適化**: 1回生成・引き回し設計

### Stage 5-6 で追加された内容（Codex レビュー）
9. **config show の秘匿値マスク**: api_key 等の平文表示防止
10. **旧設定ファイルの deprecated fallback**: breaking change 回避
11. **ベースパス検出のコマンド別明記**: --path 有無による差異
12. **config path での旧ファイル [deprecated] 注記表示**
13. **--format オプションの Phase 1 スコープ外明記**

### Stage 7-8 で追加された内容（Codex レビュー）
14. **clean --keep-embeddings の保持対象更新**: LOCAL_CONFIG_FILE, LEGACY_CONFIG_FILE
15. **deprecated 警告の stderr 統一**: 使用時のみ1回出力
16. **Serialize 対応と view model**: config show 用マスク済み構造体
17. **E2Eテスト3系統化**: 新設定のみ、ローカル上書き、レガシーfallback
18. **CLI引数優先順位テスト**: 未指定/設定あり/CLI明示の3パターン

## 仮説検証結果

| 仮説 | 判定 |
|------|------|
| config モジュールを新規作成 | Confirmed（未存在） |
| toml crate でパース | Confirmed（既に依存に含まれている） |
| embedding モジュール設定をconfig経由に移行 | Partially Confirmed（既存Config構造体の移行が必要） |

## 最終 Issue 品質評価

- **整合性**: 既存コードベースとの整合性が確保されている
- **網羅性**: 受け入れ基準が20項目に拡充され、テスト要件も明確
- **実装方針**: 10ステップの具体的な実装手順が定義されている
- **リスク対策**: deprecated fallback、秘匿値マスク、warning 出力統一が対策済み
