# 進捗レポート: Issue #108 - --with-snippet

## ステータス: 完了

## 成果物

### 新規ファイル (1件)
| ファイル | 行数 | 内容 |
|---------|------|------|
| `src/cli/snippet_helper.rs` | 73行 | SnippetOptions, fetch_snippet(), enrich_*_with_snippets() |

### 変更ファイル (12件)
| ファイル | 変更行 | 内容 |
|---------|--------|------|
| `src/output/mod.rs` | +2 | RelatedSearchResult, ImpactFileResult に snippet フィールド追加 |
| `src/output/json.rs` | +23 | JSON 出力に snippet フィールド条件付き追加 |
| `src/output/human.rs` | +14 | Human 出力にスニペット表示追加 |
| `src/cli/impact.rs` | +16 | run_impact() にスニペット取得処理、AFTER_HELP 更新 |
| `src/cli/search.rs` | +24 | run_related_search/from_stdin にスニペット取得処理、AFTER_HELP 更新 |
| `src/cli/changed_since.rs` | +9 | run_impact() シグネチャ変更への追従 |
| `src/cli/context.rs` | +1 | merge_related_results() に snippet: None 追加 |
| `src/cli/mod.rs` | +1 | pub mod snippet_helper 追加 |
| `src/cli/help_llm.rs` | +4 | key_options に --with-snippet 等追加 |
| `src/main.rs` | +66 | CLI オプション追加、SnippetOptions 構築 |
| `src/search/related.rs` | +1 | find_related() に snippet: None 追加 |
| `tests/output_format.rs` | +144 | snippet 出力テスト 8件追加 |

### 合計: +282行追加, -23行削除 (13ファイル)

## 品質チェック結果

| チェック | 結果 |
|---------|------|
| cargo build | PASS |
| cargo clippy --all-targets -- -D warnings | PASS (警告0件) |
| cargo test --all | PASS (全テストパス) |
| cargo fmt --all -- --check | PASS (差分なし) |

## 受入テスト結果: 全14項目 PASS

## Codex コードレビュー結果
- Critical: 0件
- Warnings: 6件 → W-1, W-3 を修正済み、W-2 は設計判断、W-4〜W-6 はスコープ外
