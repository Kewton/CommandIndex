# 仮説検証レポート: Issue #90

## 検証日時
2026-03-23

## Issue概要
`impact` サブコマンド（git diff ベースの影響分析）

## 検証結果

### 仮説1: 既存の `--related` 検索を内部利用できる
- **判定**: ✅ Confirmed
- **根拠**: `src/cli/search.rs` の `run_related_search` は `&[String]` を受け取り、`context.rs` の `collect_related_context` で複数ファイルのマージを行う。`impact.rs` では `RelatedSearchEngine` を直接利用して同等の処理を実装済み。

### 仮説2: stdin からのパイプ入力が可能
- **判定**: ✅ Confirmed
- **根拠**: `src/cli/stdin.rs` に `read_file_paths_from_stdin()` が実装済み。パイプ判定、バリデーション、重複排除を含む。

### 仮説3: overlap（共通影響ファイル）の検出が可能
- **判定**: ✅ Confirmed
- **根拠**: `src/cli/impact.rs` の `aggregate_impact()` が HashMap で各結果の `impacted_by` を追跡し、2つ以上の入力ファイルから影響を受けるファイルを overlap として検出する設計。

### 仮説4: human / json / path 出力形式に対応
- **判定**: ✅ Confirmed
- **根拠**: `src/output/mod.rs` に `OutputFormat` enum（Human, Json, Path）が存在し、`format_impact_results()` で全形式に対応。

## 特記事項
- 本ブランチ (`feature/issue-90-impact`) には既に impact サブコマンドの実装が存在する
- `src/cli/impact.rs` (248行) に完全な実装あり
- `src/main.rs` に `Impact` サブコマンド定義あり
- レビューは既存実装の品質・整合性を中心に行う
