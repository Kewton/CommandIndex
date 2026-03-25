# 仮説検証レポート - Issue #179

## Issue: セマンティック検索結果にスニペット（本文抜粋）が含まれない

## 仮説一覧と検証結果

### 仮説1: BM25検索の`--format llm`では既にスニペットが付いている
**判定: Confirmed**

BM25検索では以下の経路でスニペットが機能:
- `src/main.rs:487-490` で `SnippetOptions` を生成
- `src/cli/snippet_helper.rs:61-74` でtantivyからスニペット取得・切り詰め
- `src/output/human.rs:13-55` で `snippet_config` を使った出力
- `src/output/llm.rs:120-194` で `llm_options.max_body_lines` を参照した出力

### 仮説2: embeddingはセクション単位で生成されている
**判定: Confirmed**

`src/embedding/store.rs` の構造体で確認:
- `EmbeddingSimilarityResult` に `section_heading` フィールド
- `EmbeddingRecord` に `section_path` + `section_heading`
- SQLiteスキーマで `UNIQUE(section_path, section_heading, model)`

### 仮説3: 同じ仕組みをセマンティック検索結果にも適用できる
**判定: Confirmed**

`enrich_with_metadata()` (search.rs:749-802) でtantivyから全文bodyを取得済み。
スニペット生成の仕組みは存在するが、セマンティック検索パスでは接続されていない。

### 仮説4: `--snippet-lines`/`--snippet-chars`をセマンティック検索でも有効にする
**判定: Confirmed（要実装）**

CLIオプションは定義済み（main.rs:76-80）だが、`run_semantic_search()` に渡されていない。

## 根本原因分析

| 項目 | BM25検索 | セマンティック検索 |
|------|----------|-------------------|
| snippet_options渡し | ✅ あり | ❌ なし |
| snippetフィールド | SearchResult.body | SemanticSearchResult.bodyのみ（snippet無し）|
| フォーマッタのconfig参照 | ✅ snippet_config使用 | ❌ ハードコード(2行,120文字) |
| セクション本文取得 | N/A | ✅ enrich_with_metadataで取得済み |

## 修正方針

1. `run_semantic_search()` に `SnippetOptions` パラメータを追加
2. `SemanticSearchResult` に `snippet: Option<String>` フィールド追加（またはbodyの切り詰め）
3. `main.rs` からセマンティック検索呼び出し時に `snippet_options` を渡す
4. フォーマッタ（human/llm/json）でsnippet設定を参照するよう更新
5. ハードコードされた切り詰め（human.rs:283 の `truncate_body(&..., 2, 120)`）を設定ベースに変更
