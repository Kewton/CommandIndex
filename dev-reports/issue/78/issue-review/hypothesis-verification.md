# 仮説検証レポート: Issue #78「マルチリポジトリ横断検索」

## 検証日: 2026-03-22

## 検証結果サマリー

| 仮説 | 判定 | 実装状況 | 根拠ファイル |
|------|------|--------|------------|
| 1. インデックス独立性 | Confirmed | 70% | `indexer/mod.rs:11-34` |
| 2. 結果マージ | Unverifiable | 0% | `cli/search.rs:121-188` |
| 3. CLI オプション追加 | Confirmed | 0% | `main.rs:15-78` |
| 4. チーム設定(#76)依存 | Confirmed | 100% | `config/mod.rs:1-450` |
| 5. 並列検索(rayon) | Rejected | 0% | `Cargo.toml` |

## 詳細

### 仮説1: 「各リポジトリのインデックスは独立（既存の `.commandindex/` を利用）」
**判定: Confirmed**

- `src/lib.rs:11` で `.commandindex` が定数定義
- `src/indexer/mod.rs:11-34` に4つのパス管理関数が存在（`index_dir`, `commandindex_dir`, `symbol_db_path`, `embeddings_db_path`）
- `src/cli/index.rs:249-344` の `run()` 関数が `path` パラメータを受け取り、各リポジトリごとに独立インデックスを生成可能な設計
- 現在は単一リポジトリ検索のみ対応

### 仮説2: 「ワークスペース検索は各リポジトリの検索結果をマージ」
**判定: Unverifiable（実装なし）**

- マージ処理は config の `merge_raw` のみ存在（設定ファイルの優先度制御用）
- `src/cli/search.rs:121-188` は単一インデックスに対して1回の検索を実行し直接出力
- `src/indexer/reader.rs:109-183` に複数リポ対応コードなし
- 結果マージロジックは新規実装が必要

### 仮説3: CLIインターフェースに `--workspace` や `--repo` オプションを追加可能
**判定: Confirmed（テクニカルに可能）**

- `src/main.rs:15-78` の `Commands::Search` に既存オプション（`path`, `tag`, `file_type`, `limit`）が存在
- 全コマンド（Index/Search/Status/Clean/Embed）が `--path` 引数をサポート
- clap の構造に `--workspace` と `--repo` オプションを追加するのは容易

### 仮説4: チーム共有設定ファイル(#76)が依存として存在
**判定: Confirmed（完全実装済み）**

- `src/config/mod.rs:14-18` に定数定義（`TEAM_CONFIG_FILE`, `LOCAL_CONFIG_FILE`, `LEGACY_CONFIG_FILE`）
- 設定階層: 環境変数 > `.commandindex/config.local.toml` > `commandindex.toml` > レガシー > デフォルト
- `validate_no_secrets()` でチーム設定にAPI key禁止のセキュリティ機構実装済み
- `merge_raw()` で複数ファイルからの設定統合済み

### 仮説5: 「並列検索（rayon等）でパフォーマンスを確保」
**判定: Rejected（未実装、依存なし）**

- `Cargo.toml` に rayon, tokio, crossbeam 等の並列化ライブラリなし
- `src/cli/index.rs:296-317` のファイル走査は順序処理
- `src/indexer/reader.rs:109-183` は単一リーダーで順序検索
- マルチリポ対応には rayon 依存追加が必要

## 注意点・修正提案

1. **インデックス再利用**: 各リポの `.commandindex/` を独立保持する場合、tantivy クエリに `repository_id` フィールド追加を推奨
2. **設定優先度**: マルチリポ環境での設定階層（global/workspace/repo）の明確化が必要
3. **パフォーマンス**: 10以上のリポを順序検索すると総レイテンシが数秒に及ぶため、rayon による並列化が必須
4. **セキュリティ**: ワークスペース設定ファイルのパストラバーサル対策が必要
