# 仮説検証レポート - Issue #87

## 検証日: 2026-03-23

| 仮説 | 判定 | 詳細 |
|-----|------|------|
| context コマンドに union + スコア最大値方式が存在 | **Confirmed** | `src/cli/context.rs:110-186` の `merge_related_results()` に実装済み |
| --related は現在1ファイルのみ対応 | **Confirmed** | `src/main.rs:34-36` で `Option<String>` 定義。num_args未設定 |
| --related 処理ロジックが1ファイルのみ | **Confirmed** | `run_related_search(&f)` が単一ファイルを受け取る設計 |

## 詳細

### 仮説1: context コマンドの複数ファイルマージ方式

- `src/cli/context.rs:110-186` に `merge_related_results()` が実装
- HashMap で union マージ、スコア最大値採用（行128-131）
- TagMatch の matched_tags も union（行132-160）
- Target file 除外処理あり（行164-167）
- テスト: `tests/e2e_context_pack.rs:129-141`

### 仮説2: --related の Clap 定義

- `src/main.rs:34-36`: `related: Option<String>` で単一値のみ
- `num_args` 未指定、`value_delimiter` 未設定

### 仮説3: --related の処理ロジック

- `src/cli/search.rs:269-310`: `run_related_search(file_path: &str, ...)` 単一ファイル
- `src/search/related.rs:86-129`: `find_related(target_path: &str, ...)` 単一ファイル
- context.rs の `collect_related_context()` は複数ファイル対応済み（再利用可能）

## 結論

Issue #87 の仮説は全て Confirmed。context コマンドの既存マージロジックを再利用することで、効率的に実装可能。
