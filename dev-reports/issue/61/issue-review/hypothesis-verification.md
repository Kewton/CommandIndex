# 仮説検証レポート - Issue #61

## 検証日: 2026-03-22

## 検証結果サマリー

| # | 仮説 | 判定 | 詳細 |
|---|---|---|---|
| 1 | tantivy内のセクション単位がEmbedding対象 | **Confirmed** | 既存SectionDoc構造で実装済み |
| 2 | `index`/`update`に`--with-embedding`オプション | **Partially Confirmed** | コマンド存在するがオプション未実装 |
| 3 | `commandindex embed`新コマンド追加可能 | **Unverifiable** | 設計上は可能だが未実装 |
| 4 | `clean`に`--keep-embeddings`オプション | **Partially Confirmed** | コマンド存在するがオプション未実装 |
| 5 | `.commandindex/config.toml`設定管理 | **Rejected** | 現在は非存在。TOMLクレートも未追加 |
| 6 | Phase 4完了が前提 | **Confirmed** | v0.0.4リリース済み |

## 詳細

### 仮説1: tantivy内のセクション単位がEmbedding対象 - Confirmed

- `src/indexer/schema.rs`: IndexSchemaで `heading`, `body`, `tags` のテキストフィールド定義済み
- `src/cli/index.rs`: `section_to_doc()` でMarkdownセクション（見出し単位）をSectionDocに変換
- `src/parser/markdown.rs`: MarkdownDocumentが複数のSection（見出し単位）を保持

### 仮説2: `index`/`update`に`--with-embedding`追加可能 - Partially Confirmed

- `src/main.rs`: `Index`, `Update` サブコマンド定義済み
- 現在は `path` パラメータのみ。`--with-embedding` フラグの追加が必要

### 仮説3: `commandindex embed`新コマンド追加可能 - Unverifiable

- 現在のcliモジュール: `clean`, `context`, `index`, `search`, `status`
- clap Subcommandパターンが確立しており、`embed.rs` 追加は容易な設計

### 仮説4: `clean`に`--keep-embeddings`追加可能 - Partially Confirmed

- `src/cli/clean.rs`: `.commandindex/` ディレクトリ全体削除の実装あり
- `--keep-embeddings` オプション追加と削除ロジックの条件分岐化が必要

### 仮説5: `.commandindex/config.toml`設定管理 - Rejected

- config.toml の仕組みは現在存在しない
- 現在の設定: `state.json`（メタ情報）、`manifest.json`（ファイル一覧）、`.cmindexignore`（除外ルール）
- `toml` クレートの依存追加が必要

### 仮説6: Phase 4完了が前提 - Confirmed

- v0.0.4リリース済み（`c2b78b6 chore: merge release v0.0.4 to develop`）
- `src/cli/context.rs` 実装済み、`--related` オプション実装済み

## Issue修正への提案

1. **設定管理**: config.toml は新規構築が必要であることをIssueに明記すべき
2. **依存追加**: HTTPクライアント（reqwest等）、toml クレートの追加が必要
3. **Embedding格納**: tantivy スキーマ拡張 or 別ファイル（SQLite等）でのベクトル格納方法を明確化すべき
4. **非同期処理**: Ollama/OpenAI API呼び出しは非同期が望ましい（tokio等の検討）
