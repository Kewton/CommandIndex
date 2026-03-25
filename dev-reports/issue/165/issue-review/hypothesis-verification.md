# 仮説検証レポート: Issue #165

## 仮説1: progress-report.mdがhas_reviewとして登録されている
- **判定**: Confirmed
- **根拠**: `src/indexer/knowledge.rs:400` の `build_pattern_rules()` で progress-report パターンの relation が `KnowledgeRelation::HasReview` に設定されている

## 仮説2: 影響ファイルは knowledge.rs と human.rs
- **判定**: Partially Confirmed
- **根拠**: Issue記載の2ファイルに加え、以下のファイルにも影響がある
  - `src/indexer/symbol_store.rs:880-888` — DB読み取り時の relation パース（`has_progress`追加必要）
  - `src/cli/before_change.rs:331-338` — relation_priority に `has_progress` 追加必要
  - `src/cli/issue.rs:98-103` — sort_order の match に `HasProgress` 追加必要
  - `src/output/human.rs:252-257` — relation_display_label に `has_progress` 追加必要

## 仮説3: KnowledgeRelation enumにHasProgressバリアント追加が必要
- **判定**: Confirmed
- **根拠**: 現在のenum（knowledge.rs:82-87）は HasDesign, HasReview, HasWorkplan, Modifies の4バリアント。HasProgress追加が必要

## 追加発見
- `src/indexer/symbol_store.rs:880-888` の DB relation パースは `parse()` メソッドではなく直接 match しているため、`has_progress` の追加が必要
- `src/cli/issue.rs:98-103` の `sort_order` は exhaustive match のため、新バリアント追加でコンパイルエラーになる（対応必須）
- テストファイル内にも `HasReview` を progress-report で使用しているケースが複数あり更新が必要
