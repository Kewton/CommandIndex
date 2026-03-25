# マルチステージ設計レビュー サマリーレポート

## Issue #167: suggestコマンドのナレッジグラフ展開制限

### レビュー概要

| Stage | 種別 | 実行エージェント | Must Fix | Should Fix | Nice to Have |
|---|---|---|---|---|---|
| 1 | 設計原則 | Claude Opus | 2 | 3 | 3 |
| 2 | 整合性 | Claude Opus | 2 | 3 | 2 |
| 3 | 影響分析 | Claude Opus | 1 | 3 | 3 |
| 4 | セキュリティ | Claude Opus | 0 | 2 | 3 |
| 5 | 設計原則(2回目) | Codex gpt-5.4 | 3 | 3 | 2 |
| 6 | 指摘反映 | Claude Sonnet | - | - | - |
| 7 | 整合性(2回目) | Claude Opus (Codex接続エラー代替) | 1 | 1 | 1 |
| 8 | 指摘反映 | Claude Sonnet | - | - | - |

### 主要な修正事項

#### 1回目レビュー（Stage 1-4）
- `KnowledgeRelation::priority()` メソッド追加によるDRY改善
- `ProgressReport` DocSubtypeのフィルタ条件の明記
- `SuggestKgDoc → KnowledgeDocResult` 変換の設計明確化
- 既存テスト影響の明記
- エラーハンドリング方針の追記
- セキュリティリスク認識の記載

#### 2回目レビュー（Stage 5-8）
- `prepend_knowledge_steps()` を `&[SuggestKgDoc]` 受け取りに変更（KISS改善）
- `before_change.rs` の `relation_priority()` を互換ラッパーとして残す設計（未知値フォールバック維持）
- `ctx.symbol_store_db_path()` → `ctx.symbol_db_path()` のAPI名修正
- `strategy` 引数型の `Vec<String>` → `Vec<SuggestStep>` 修正
- `filter_and_limit_kg_docs()` にissue_numbers引数追加（Issue順序維持）
- 部分失敗時の方針明文化

### 最終設計方針書の状態

設計方針書は全8ステージのレビューを経て成熟。以下が確定:
- 変更対象: `suggest.rs`, `knowledge.rs`, `before_change.rs`
- 新規要素: `SuggestKgDoc` 構造体, `filter_and_limit_kg_docs()` 関数, `KnowledgeRelation::priority()` メソッド
- テスト: 新規ユニットテスト7件 + 既存テスト修正3件 + E2Eテスト2件
- セキュリティ: 問題なし
