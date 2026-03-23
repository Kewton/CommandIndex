# 進捗レポート: Issue #109 - suggest サブコマンド

## ステータス: 完了

## 実装サマリー

### 新規ファイル
| ファイル | 行数 | 内容 |
|---------|------|------|
| `src/cli/suggest.rs` | ~470行 | suggestコマンドのコアロジック + 単体テスト18件 |
| `tests/e2e_suggest.rs` | ~90行 | E2Eテスト6件 |

### 変更ファイル
| ファイル | 変更内容 |
|---------|---------|
| `src/output/mod.rs` | SuggestStep, SuggestResult 構造体 + format_suggest_results フォーマッタ |
| `src/cli/mod.rs` | `pub mod suggest;` 追加 |
| `src/main.rs` | Commands::Suggest バリアント + match アーム |
| `src/cli/help_llm.rs` | build_commands() に suggest の CommandInfo 追加 |
| `tests/cli_args.rs` | ヘルプテスト + help-llm契約テスト更新(13→14) |

### 品質チェック結果
| チェック | 結果 |
|---------|------|
| `cargo build` | PASS |
| `cargo clippy --all-targets -- -D warnings` | PASS (0警告) |
| `cargo test --all` | PASS (全テストパス) |
| `cargo fmt --all -- --check` | PASS (差分なし) |

### Codexコードレビュー結果
- **Critical 2件**: 修正済み
  - C1: semantic検索ステップにオリジナルのtask descriptionを使用するよう修正
  - C2: sanitize_for_command_arg() → shell_quote() に変更（シングルクオート保護 + `--` でオプション解釈停止）
- **Warnings 3件**: リファクタリングで対応済み
  - W1: shell_quote()適用確認済み
  - W2: SymbolStoreをオプショナルに変更（DB不在でもBM25ベース戦略を返す）
  - W3: path形式もshell_quote()適用済み

### 受入テスト結果
全10項目PASS（AC-1〜AC-10）

### 主要な設計判断
1. **ルールベースアプローチ**: BM25 → related → impact パイプライン（外部API依存なし）
2. **shell_quote()**: シングルクオートによるコマンド引数保護（セキュリティ）
3. **SymbolStoreオプショナル**: DB不在でもBM25ベースの最小戦略を返す（可用性向上）
4. **SearchContext::new()直接使用**: 独自のresolve_context不要
5. **impact.rs変更なし**: RelatedSearchEngineを直接利用（回帰リスクゼロ）
