# 仮説検証レポート - Issue #108

## 検証日: 2026-03-23

## 仮説一覧

### H1: impact / search --related はスニペット情報を保持していない
- **判定**: ✅ Confirmed
- `RelatedSearchResult` は path, score, relation_types のみ
- `ImpactFileResult` は file_path, score, relation_types, impacted_by のみ
- heading, body 等のコンテンツ情報なし

### H2: 既存の --snippet-lines / --snippet-chars は search コマンド専用
- **判定**: ✅ Confirmed
- main.rs の search サブコマンドにのみ定義
- impact / search --related には存在しない

### H3: --format llm は未実装
- **判定**: ✅ Confirmed
- OutputFormat enum は Human, Json, Path の3値のみ
- Issue記載の `--format llm` は現時点で存在しない

### H4: Context Pack (context コマンド) はスニペット対応済み
- **判定**: ✅ Confirmed
- ContextEntry に snippet: Option<String> フィールドあり
- 参考実装として利用可能

### H5: ファイル内容の有無が LLM の成功/失敗を分ける
- **判定**: ✅ Confirmed (Issue記載の検証データに基づく)
- P3 (related のみ, パスだけ): 43秒で失敗
- P5 (search --format path): 117秒で失敗
- P2 (context=内容付き): 448秒で成功
- パス情報のみでは LLM がファイル読み取りループに入る

## 主要ファイルパス

| 目的 | ファイル |
|-----|---------|
| impact コマンド | src/cli/impact.rs |
| search コマンド | src/cli/search.rs |
| 関連検索エンジン | src/search/related.rs |
| 出力定義 | src/output/mod.rs |
| Context Pack | src/cli/context.rs, src/output/context_pack.rs |
| CLIオプション | src/main.rs |
