# 仮説検証レポート - Issue #102

## 検証日: 2026-03-23

## 検証結果サマリー

| # | 仮説 | 結果 | 判定 |
|---|---|---|---|
| 1 | 現状は「サブコマンド名＋1行説明」のみ | after_help/before_help/long_about 0件 | **Confirmed** |
| 2 | `--format json` が使える | search, impact, diff, status のみ対応 | **Partially Confirmed** |
| 3 | stdin パイプ対応 | impact, search --related-stdin のみ | **Partially Confirmed** |
| 4 | 提案サブコマンド全て存在 | 13個すべて実装済み | **Confirmed** |
| 5 | clap の after_help / before_help 未使用 | 全ファイルで0件 | **Confirmed** |
| 6 | search の高度オプション存在 | --related, --semantic, --changed-since, --symbol 全て実装済み | **Confirmed** |

## 詳細

### 仮説1: 「サブコマンド名＋1行説明」のみ

**判定: Confirmed**

各サブコマンドは `Commands` enum の `///` コメントで1行説明のみ定義。
`after_help`, `before_help`, `long_about` は全Rustファイルで0件。

### 仮説2: `--format json` 対応状況

**判定: Partially Confirmed**

Issueでは全サブコマンドで使えるかのような記述があるが、実際は一部のみ:
- 対応: search, impact, diff, status
- 非対応: index, update, clean, embed, context, config, export, import, watch

`OutputFormat` enum は `src/output/mod.rs` で `Human`, `Json`, `Path` を定義。

### 仮説3: stdin パイプ対応

**判定: Partially Confirmed**

- impact: ✓ ファイルリストなし時にstdinから読み込み（最大500ファイル）
- search: ✓ `--related-stdin` フラグで対応
- その他: ✗ 非対応

`src/cli/stdin.rs` で一元管理。

### 仮説4: サブコマンド一覧

**判定: Confirmed**

13個全て実装済み: search, impact, diff, context, index, update, status, embed, config, export, import, watch, clean

### 仮説5: clap拡張属性未使用

**判定: Confirmed**

実装方針で「clapのafter_help/before_helpで追加」と記載あり。現時点で未使用のため、新規追加の障害なし。

### 仮説6: searchの高度オプション

**判定: Confirmed**

`src/cli/search.rs` に全て実装済み:
- `--symbol`: L35-37
- `--related`: L39-40
- `--related-stdin`: L42-43
- `--semantic`: L45-46
- `--changed-since`: L92-93
- `--rerank` / `--rerank-top`: L80-84

## Issueへの修正提案

1. **format対応状況の明記**: `--help-llm` のJSON出力で、各コマンドのformat対応状況を正確に反映すべき
2. **stdin対応の正確な記述**: `stdin_support` に `search --related-stdin` も含めるべき
3. **search --related-stdin の明記**: Issue内のuse_casesに `--related-stdin` の例も追加すべき
