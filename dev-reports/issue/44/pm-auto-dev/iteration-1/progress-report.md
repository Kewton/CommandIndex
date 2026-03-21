# 進捗レポート: Issue #44

## 概要
- **Issue**: #44 — searchコマンドのスニペット表示行数・文字数をCLIオプションで動的に調整可能にする
- **ブランチ**: feature/issue-44-snippet-options
- **ステータス**: 完了

## 実装サマリー

### 変更ファイル (5ファイル)

| ファイル | 変更内容 |
|---------|---------|
| `src/output/mod.rs` | `SnippetConfig` 構造体追加、`format_results` 内の Human 分岐で `SnippetConfig::default()` 使用 |
| `src/output/human.rs` | `format_human()` に `SnippetConfig` 引数追加、0=無制限ロジック実装 |
| `src/cli/search.rs` | `run()` に `SnippetConfig` 引数追加、Human 分岐で `format_human` 直接呼び出し |
| `src/main.rs` | `--snippet-lines` / `--snippet-chars` CLIオプション追加 |
| `tests/output_format.rs` | 新規テスト5件追加 |

### 設計判断

| 判断 | 採用理由 |
|------|---------|
| format_results シグネチャ不変 | ISP原則: 既存テスト11件に修正不要 |
| 0=無制限を format_human 側で制御 | SRP: truncate_body は純粋な切り詰め関数として維持 |
| truncate_body 不変 | context.rs への影響完全回避 |

## 品質チェック結果

| チェック項目 | 結果 |
|-------------|------|
| cargo build | PASS |
| cargo clippy --all-targets -- -D warnings | PASS (警告0件) |
| cargo test --all | PASS (全テストパス) |
| cargo fmt --all -- --check | PASS (差分なし) |

## Codexコードレビュー結果

| 種別 | 件数 | 内容 |
|------|------|------|
| Critical | 0件 | - |
| Warnings | 3件 | すべて既存コードの問題（本Issue scope外） |

warnings の詳細:
1. human.rs: result.path に strip_control_chars 未適用（既存問題）
2. human.rs: matched_tags に strip_control_chars 未適用（既存問題）
3. main.rs: --limit の下限チェック不足（既存問題）

## 受入テスト結果

| 受入条件 | 結果 |
|---------|------|
| --snippet-lines 10 で10行分表示 | PASS |
| --snippet-chars 200 で200文字表示 | PASS |
| --snippet-lines 0 で全文表示 | PASS |
| --snippet-chars 0 で全文表示（単一行） | PASS |
| デフォルト値で既存動作同一 | PASS |
| json/path フォーマットに影響なし | PASS |
| symbol/related モードに影響なし | PASS |
| 既存テストパス | PASS |
| 品質チェック全パス | PASS |

## リファクタリング結果
リファクタリング不要と判定。コードの可読性・命名の一貫性・テスト十分性すべて問題なし。
