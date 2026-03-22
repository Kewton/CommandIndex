# 仮説検証レポート - Issue #76

## 検証結果サマリー

| 仮説 | 判定 | 詳細 |
|------|------|------|
| `config` モジュールを `src/config/mod.rs` に新規作成 | **Confirmed** | 現在configモジュールは存在しない。新規作成が必要 |
| `toml` crateで設定ファイルをパース | **Confirmed** | `toml = "0.8"` が既にCargo.tomlに含まれている |
| 既存の `embedding` モジュールの設定をconfig経由に移行 | **Partially Confirmed** | 既に `embedding/mod.rs` に `Config` 構造体が存在し `.commandindex/config.toml` を読んでいる。これを新configモジュールに移行する必要がある |

## 詳細検証

### 1. configモジュールの現状
- `src/config/mod.rs` は存在しない
- 現在のモジュール: `cli/`, `embedding/`, `indexer/`, `output/`, `parser/`, `rerank/`, `search/`
- 設定読み込みは `embedding/mod.rs` の `Config::load()` に集約されている

### 2. toml crateの状況
- `Cargo.toml` line 27: `toml = "0.8"` として既に依存に含まれている
- 現在は `embedding/mod.rs` で使用されている

### 3. 既存の設定基盤
- **Config構造体**: `embedding/mod.rs` に `Config` (= `EmbeddingConfig` + `RerankConfig`)
- **読み込み元**: `.commandindex/config.toml` のみ（ハードコード）
- **環境変数**: `COMMANDINDEX_OPENAI_API_KEY` のみ対応
- **優先順位**: 環境変数 > config.toml > デフォルト値（部分的に実装済み）

### 4. CLIサブコマンドの現状
- 現在のサブコマンド: `index`, `search`, `update`, `status`, `clean`, `context`, `embed`
- `config` サブコマンドは未実装

### 5. 追加発見事項
- `.cmindexignore` ファイルによるignoreパターンのサポートが既に存在（`src/parser/ignore.rs`）
- `RerankConfig` も `.commandindex/config.toml` から読み込まれている
- `.commandindex/` ディレクトリは tantivy index, SQLite DB, config, manifest, state を格納

## Issue記載内容への影響

### 修正推奨事項
1. **設定ファイル階層**: Issue提案の4層構造は妥当だが、既存の `.commandindex/config.toml` との後方互換性を明記すべき
2. **既存Config移行**: `embedding::Config` から新 `config::Config` への移行パスを明確にすべき
3. **RerankConfig**: Issue本文に `[rerank]` セクションが含まれているが、既存の `RerankConfig` フィールド（`top_candidates`, `timeout_secs`）との整合性を確認すべき
4. **ignoreパターン**: `.cmindexignore` との関係性を検討すべき（将来的にconfigに統合するか）
