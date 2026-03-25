# 進捗レポート: Issue #169 — issue listサブコマンドの追加

## ステータス: 完了

## 実装サマリー

| 項目 | 値 |
|------|-----|
| 変更ファイル数 | 7 |
| 追加行数 | ~790 |
| 新規テスト数 | 25 |
| 全テストパス | Yes（Ollama依存テスト1件除く） |
| Clippy警告 | 0件 |
| フォーマット差分 | なし |
| コミット数 | 3 |

## 実装内容

### データ層 (src/indexer/symbol_store.rs)
- `IssueListRow` 構造体（データ層DTO）
- `list_all_issues()` メソッド（JOIN + GROUP BY + 条件付きCOUNT、1クエリ集計）
- 単体テスト5件

### CLI層 (src/cli/issue.rs)
- `IssueListEntry` 構造体（CLI表示モデル）
- `extract_label_from_design_path()` — LazyLock正規表現でlabel抽出
- `open_symbol_store()` — DB存在チェック共通ヘルパー (DRY)
- `convert_row_to_entry()` — IssueListRow → IssueListEntry 変換
- `sanitize_label()` — 改行含む制御文字除去（Codexレビュー指摘対応）
- `run_show()` — 既存run()からリネーム (API対称性)
- `run_list()` — 新規一覧表示関数
- 4フォーマッタ: `format_list_human/json/path/llm`
- 単体テスト12件

### main.rs
- `IssueCommands` enum (List, Show)
- サブコマンドディスパッチャー

### 既存コード更新
- `suggest.rs`: `issue {num}` → `issue show {num}`
- `help_llm.rs`: use_cases/workflows/CommandInfo全箇所更新 + subcommands追加

### テスト更新
- `e2e_issue.rs`: 既存6テスト show構文に更新 + 5テスト新規追加
- `cli_args.rs`: 既存テスト更新 + 3テスト新規追加

## Codexコードレビュー結果

| 種別 | 件数 | 対応 |
|------|------|------|
| Critical | 0 | - |
| Warning (medium) | 1 | 非数値identifierスキップは設計方針通り（テスト有） |
| Warning (low) | 1 | sanitize_label()で改行除去を追加 |

## 受入テスト結果

全16項目 **PASS**
