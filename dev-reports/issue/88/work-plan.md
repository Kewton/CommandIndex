# 作業計画書: Issue #88 --index-path オプション

## Issue: [Feature] --index-path オプション（インデックスパス指定）
**Issue番号**: #88
**サイズ**: M（中）
**優先度**: High
**依存Issue**: なし
**ブランチ**: `feature/issue-88-index-path`（作成済み）

---

## Phase 1: 基盤変更（定数統一 + パス解決関数 + ヘルパー関数）

### Task 1.1: INDEX_DIR_NAME 定数の統一
- **成果物**: `src/indexer/mod.rs`
- **作業内容**: `COMMANDINDEX_DIR` 定数を削除し、`crate::INDEX_DIR_NAME` に統一
- **影響**: indexer/mod.rs 内のヘルパー関数
- **依存**: なし
- **見積り**: 小

### Task 1.2: ResolveIndexPathError + resolve_index_path 新設
- **成果物**: `src/indexer/mod.rs`
- **作業内容**:
  - `ResolveIndexPathError` エラー型（CurrentDirUnavailable, CanonicalizeFailed, PathTraversal, SymlinkDetected）
  - `resolve_index_path(cli_index_path, config_index_path, base_path) -> Result<PathBuf>`
  - `reject_symlink(path) -> Result<()>`（destructive/write コマンド用）
  - パストラバーサル検出（`..` コンポーネントを含む未作成パスはエラー）
  - 存在するパスは `canonicalize` で正規化
- **依存**: Task 1.1
- **見積り**: 中

### Task 1.3: ヘルパー関数のシグネチャ変更
- **成果物**: `src/indexer/mod.rs`
- **作業内容**:
  - `index_dir(commandindex_dir)`, `symbol_db_path(commandindex_dir)`, `embeddings_db_path(commandindex_dir)` に変更
  - `commandindex_dir()` 関数を削除
- **影響**: 28箇所以上の呼び出し元がコンパイルエラー（Phase 3 で修正）
- **依存**: Task 1.1
- **見積り**: 小

### Task 1.4: ヘルパー関数のユニットテスト
- **成果物**: `src/indexer/mod.rs` 内テスト
- **作業内容**: resolve_index_path の優先順位テスト、パストラバーサル検出テスト、相対パス/絶対パス解決テスト
- **依存**: Task 1.2, 1.3
- **見積り**: 中

---

## Phase 2: Config 拡張

### Task 2.1: RawIndexConfig / IndexConfig に path フィールド追加
- **成果物**: `src/config/mod.rs`
- **作業内容**:
  - `RawIndexConfig` に `path: Option<String>` 追加
  - `IndexConfig` に `path: Option<String>` 追加（raw 値として保持）
  - `merge_index` に `path: h.path.or(b.path)` 追加
  - `resolve_config` で `path` を変換
- **依存**: なし
- **見積り**: 小

### Task 2.2: Config テスト
- **成果物**: `src/config/mod.rs` 内テスト
- **作業内容**: `[index].path` の読み込み・マージテスト
- **依存**: Task 2.1
- **見積り**: 小

---

## Phase 3: CLI + サブコマンド修正

### Task 3.1: Cli グローバルオプション追加
- **成果物**: `src/main.rs`
- **作業内容**:
  - `Cli` 構造体に `#[arg(long, global = true)] index_path: Option<PathBuf>` 追加
  - main 関数で `load_config` → `resolve_index_path` の2段階解決を実装
- **依存**: Task 1.2, 2.1
- **見積り**: 中

### Task 3.2: index サブコマンド修正
- **成果物**: `src/cli/index.rs`, `src/main.rs`
- **作業内容**:
  - run / run_incremental に commandindex_dir パラメータ追加
  - 内部のヘルパー関数呼び出しを commandindex_dir ベースに修正（10箇所以上）
  - `reject_symlink` チェック追加（write 系）
  - 出力メッセージ「Index saved to {path}」を動的化
- **依存**: Task 1.3, 3.1
- **見積り**: 中

### Task 3.3: search サブコマンド修正
- **成果物**: `src/cli/search.rs`, `src/main.rs`
- **作業内容**:
  - SearchContext に `commandindex_dir` フィールド追加
  - `new(base_path, index_path)` コンストラクタ新設
  - `from_current_dir` / `from_path` を移行
  - `run_symbol_search`, `run_related_search`, `run_semantic_search` に `ctx: &SearchContext` パラメータ追加
  - Path::new(".") 9箇所修正
- **依存**: Task 1.3, 3.1
- **見積り**: 大

### Task 3.4: status サブコマンド修正
- **成果物**: `src/cli/status/mod.rs`, `src/main.rs`
- **作業内容**:
  - run に commandindex_dir パラメータ追加
  - get_symbol_count, get_embedding_file_count, compute_storage_breakdown, run_verify の内部関数修正
  - `get_embedding_model` の既存 cwd バグ修正（`Path::new(".")` → `base_path`）
- **依存**: Task 1.3, 3.1
- **見積り**: 中

### Task 3.5: context サブコマンド修正
- **成果物**: `src/cli/context.rs`, `src/main.rs`
- **作業内容**: run_context に commandindex_dir パラメータ追加、Path::new(".") 2箇所修正
- **依存**: Task 1.3, 3.1
- **見積り**: 小

### Task 3.6: clean サブコマンド修正
- **成果物**: `src/cli/clean.rs`, `src/main.rs`
- **作業内容**:
  - run に commandindex_dir パラメータ追加（2引数→3引数）
  - `validate_index_directory` 関数新設（インデックスマーカー検証）
  - `NotAnIndexDirectory` エラー追加
  - `reject_symlink` チェック追加
  - 既存挙動ベースの削除（通常時: 全削除、keep_embeddings時: 部分削除）
  - 出力メッセージ動的化
- **依存**: Task 1.2, 1.3, 3.1
- **見積り**: 中

### Task 3.7: export / import サブコマンド修正
- **成果物**: `src/cli/export.rs`, `src/cli/import_index.rs`, `src/main.rs`
- **作業内容**:
  - run 関数が commandindex_dir を外部から受け取る形に変更
  - main.rs の Path::new(".") ハードコード修正
  - import に `reject_symlink` チェック追加
- **依存**: Task 1.3, 3.1
- **見積り**: 小

### Task 3.8: embed サブコマンド修正
- **成果物**: `src/cli/embed.rs`, `src/main.rs`
- **作業内容**: run に commandindex_dir パラメータ追加、内部ヘルパー呼び出し修正
- **依存**: Task 1.3, 3.1
- **見積り**: 小

### Task 3.9: config サブコマンド修正
- **成果物**: `src/cli/config.rs`, `src/main.rs`
- **作業内容**:
  - run_show / run_path に base_path パラメータ追加
  - Path::new(".") 2箇所修正
  - config show で effective index path を追加表示
  - config path で effective index dir を追加表示
- **依存**: Task 1.2, 3.1
- **見積り**: 中

---

## Phase 4: workspace + IgnoreFilter

### Task 4.1: workspace 対応
- **成果物**: `src/cli/workspace.rs`
- **作業内容**:
  - SearchContext::new 経由で各リポジトリの config [index].path を尊重
  - .commandindex ハードコード（L159, L203）修正
  - workspace 横断の search / status / update が per-repo index_path を正しく参照
- **依存**: Task 3.3, 3.4
- **見積り**: 中

### Task 4.2: IgnoreFilter 拡張
- **成果物**: `src/parser/ignore.rs`
- **作業内容**:
  - IgnoreFilter に `patterns: Vec<String>` フィールド追加
  - `build_glob_set` ヘルパー関数新設
  - `with_custom_index_path` メソッド追加
  - リポジトリ内カスタムパスのみ除外、リポジトリ外は無視
- **依存**: Task 1.2
- **見積り**: 小

---

## Phase 5: テスト

### Task 5.1: 既存テスト期待値更新
- **成果物**: `tests/cli_clean.rs`, `tests/cli_index.rs`, `tests/e2e_embedding.rs`, `tests/e2e_phase3_integration.rs`
- **作業内容**:
  - 出力メッセージの部分一致アサーションへの緩和
  - symbol_db_path 呼び出しの修正
- **依存**: Phase 3 全タスク
- **見積り**: 中

### Task 5.2: --index-path E2E テスト
- **成果物**: `tests/cli_index_path.rs`（新規）
- **作業内容**:
  - カスタムパスに index/search/update/clean
  - search 全モード（fulltext/symbol/related/semantic）
  - パス不存在時の挙動（書き込み系: 自動作成、読み取り系: エラー）
  - 優先順位テスト（CLI > config > default）
- **依存**: Phase 3 全タスク
- **見積り**: 大

### Task 5.3: config [index].path テスト
- **成果物**: `tests/cli_index_path.rs` に追加
- **作業内容**:
  - commandindex.toml の [index].path でのインデックスパス指定
  - 相対パスがリポジトリルート基準で解決されること
- **依存**: Task 2.1, Phase 3
- **見積り**: 小

### Task 5.4: clean 安全ガードテスト
- **成果物**: `tests/cli_index_path.rs` に追加
- **作業内容**: symlink 拒否、非インデックスディレクトリ拒否、keep_embeddings
- **依存**: Task 3.6
- **見積り**: 小

### Task 5.5: IgnoreFilter テスト
- **成果物**: `tests/ignore_filter.rs` に追加
- **作業内容**: リポジトリ内カスタムパス除外、リポジトリ外パス、自己インデックス防止
- **依存**: Task 4.2
- **見積り**: 小

---

## Phase 6: 品質チェック + 仕上げ

### Task 6.1: 品質チェック
- **作業内容**:
  ```bash
  cargo build
  cargo clippy --all-targets -- -D warnings
  cargo test --all
  cargo fmt --all -- --check
  ```
- **依存**: Phase 5 全タスク
- **見積り**: 小

### Task 6.2: ドキュメント更新
- **成果物**: 必要に応じて README.md
- **作業内容**: 共有インデックスの同時書き込み制約をドキュメントに記載
- **依存**: Task 6.1
- **見積り**: 小

---

## 品質チェック項目

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |

## Definition of Done

- [ ] すべてのタスク（Task 1.1 〜 6.2）が完了
- [ ] `--index-path` グローバルオプションが全サブコマンドで動作
- [ ] `commandindex.toml` の `[index].path` が正しく読み込まれる
- [ ] 後方互換性が維持されている（既存テスト全パス）
- [ ] 新規テスト 15 項目が全パス
- [ ] cargo test / clippy / fmt 全パス
- [ ] セキュリティ対策（パストラバーサル検出、symlink チェック、機密情報保護）が実装済み

## タスク依存関係

```
Phase 1 (基盤)
  Task 1.1 ─→ Task 1.2 ─→ Task 1.4
  Task 1.1 ─→ Task 1.3 ─┘

Phase 2 (Config)
  Task 2.1 ─→ Task 2.2

Phase 3 (CLI + サブコマンド) ← Phase 1 + Phase 2
  Task 3.1 ─→ Task 3.2 〜 3.9（並列可能）

Phase 4 (workspace + Ignore) ← Phase 3
  Task 4.1, 4.2（並列可能）

Phase 5 (テスト) ← Phase 3 + Phase 4
  Task 5.1 〜 5.5（並列可能）

Phase 6 (品質) ← Phase 5
  Task 6.1 ─→ Task 6.2
```

## 実装順序サマリー

1. Phase 1: 基盤変更（定数統一 → resolve_index_path → ヘルパー関数 → テスト）
2. Phase 2: Config 拡張（path フィールド追加 → テスト）
3. Phase 3: CLI + サブコマンド修正（9タスク、search が最大）
4. Phase 4: workspace + IgnoreFilter
5. Phase 5: テスト（既存更新 + 新規 15 項目）
6. Phase 6: 品質チェック + ドキュメント
