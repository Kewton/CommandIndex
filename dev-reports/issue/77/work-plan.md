# 作業計画書 - Issue #77: インデックス共有モード

## Issue概要

**Issue番号**: #77
**タイトル**: [Feature] インデックス共有モード（--shared / CI連携）
**サイズ**: L（新規サブコマンド2つ + status拡張 + セキュリティ実装）
**優先度**: Medium
**依存Issue**: #76 チーム共有設定ファイル（完了済み）

## タスク分解

### Phase 1: 基盤（データ構造・型定義）

#### Task 1.1: ExportMeta 構造体とスナップショットモジュール
- **成果物**: `src/indexer/snapshot.rs`
- **依存**: なし
- **内容**:
  - `ExportMeta` 構造体（export_format_version, commandindex_version, git_commit_hash, exported_at）
  - `#[serde(deny_unknown_fields)]` 付与
  - `EXPORT_FORMAT_VERSION` 定数（初期値: 1）
  - `EXPORT_META_FILE` 定数（"export_meta.json"）
  - `ExportMeta::save()` / `ExportMeta::load()` メソッド
  - `src/indexer/mod.rs` に `pub mod snapshot;` 追加
- **テスト**: ExportMeta のシリアライズ/デシリアライズ、deny_unknown_fields の検証

#### Task 1.2: エラー型定義
- **成果物**: `src/cli/export.rs`（エラー型部分）, `src/cli/import_index.rs`（エラー型部分）
- **依存**: Task 1.1
- **内容**:
  - `ExportError` enum + `fmt::Display` + `std::error::Error` + `From<T>` 実装
  - `ImportError` enum（SymlinkDetected, DecompressionBomb 含む）+ 同様の実装
  - `ExportResult` / `ImportResult` 構造体
  - `ExportOptions` / `ImportOptions` 構造体
- **テスト**: エラー型の Display 出力確認

#### Task 1.3: Cargo.toml 依存追加
- **成果物**: `Cargo.toml`
- **依存**: なし
- **内容**:
  - `tar = "0.4"` 追加
  - `flate2 = "1"` 追加（default features = miniz_oxide）
- **テスト**: `cargo build` 成功確認

### Phase 2: コア実装（エクスポート）

#### Task 2.1: エクスポートロジック実装
- **成果物**: `src/cli/export.rs`
- **依存**: Task 1.1, 1.2, 1.3
- **内容**:
  - `pub fn run(path: &Path, output: &Path, options: &ExportOptions) -> Result<ExportResult, ExportError>`
  - `fn current_git_hash(repo_path: &Path) -> Option<String>` ユーティリティ関数
  - .commandindex/ 存在確認
  - IndexState::load() でインデックス状態読み込み
  - state.json の index_root サニタイズ（placeholder 置換してパック）
  - tar::Builder + flate2::GzEncoder ストリーミング圧縮
  - config.local.toml 除外ロジック
  - embeddings.db の --with-embeddings 制御
  - 出力ファイルサイズ計算
- **テスト**: TDDで実装（テスト先行）

#### Task 2.2: エクスポートテスト
- **成果物**: `tests/cli_export.rs`
- **依存**: Task 2.1
- **内容**:
  - export 基本動作テスト（インデックス作成 → export → アーカイブ内容検証）
  - NotInitialized エラーテスト
  - config.local.toml 除外検証
  - embeddings.db デフォルト除外検証
  - --with-embeddings 時の embeddings.db 含む検証

### Phase 3: コア実装（インポート）

#### Task 3.1: パストラバーサル検証ロジック
- **成果物**: `src/cli/import_index.rs`（検証関数部分）
- **依存**: Task 1.2
- **内容**:
  - `fn validate_entry_path(entry_path: &Path, target_dir: &Path) -> Result<PathBuf, ImportError>`
  - `fn validate_entry_type(entry: &tar::Entry) -> Result<(), ImportError>`
  - 絶対パス拒否、`..` コンポーネント拒否
  - Symlink/Link エントリ拒否
  - 累積サイズ/エントリ数チェックロジック
- **テスト**: TDDで実装（パストラバーサル、シンボリックリンク、圧縮爆弾の各テストケース）

#### Task 3.2: インポートロジック実装
- **成果物**: `src/cli/import_index.rs`
- **依存**: Task 1.1, 1.2, 1.3, 3.1
- **内容**:
  - `pub fn run(path: &Path, archive: &Path, options: &ImportOptions) -> Result<ImportResult, ImportError>`
  - アーカイブファイル存在確認
  - 既存 .commandindex/ チェック + --force 制御
  - ストリーミング展開（各エントリにセキュリティチェック適用）
  - export_meta.json 読み込み + バージョン互換性チェック
  - state.json の index_root 書き換え
  - git hash 比較 + 不一致警告
  - tantivy インデックスオープン確認
  - パーミッション固定（0o644/0o755）
- **テスト**: TDDで実装

#### Task 3.3: インポートテスト
- **成果物**: `tests/cli_import.rs`
- **依存**: Task 3.2
- **内容**:
  - import 基本動作テスト
  - 既存インデックスありで --force なしのエラー
  - --force での上書きインポート
  - パストラバーサル検出テスト（`../` パス）
  - シンボリックリンクエントリ拒否テスト
  - ハードリンクエントリ拒否テスト
  - 圧縮爆弾検出テスト
  - export_format_version 不一致エラー
  - コミットハッシュ不一致警告

### Phase 4: Status --verify 拡張

#### Task 4.1: verify ロジック実装
- **成果物**: `src/cli/status.rs`（変更）
- **依存**: なし（他 Phase と並行可能）
- **内容**:
  - `run()` シグネチャに `verify: bool` 引数追加
  - `VerifyResult` / `VerifyIssue` / `VerifySeverity` 構造体追加
  - verify ロジック: state確認 → tantivy確認 → manifest確認 → symbols確認
  - Human/Json 両フォーマットでの verify 結果出力
- **テスト**: TDDで実装

#### Task 4.2: verify テスト
- **成果物**: `tests/e2e_verify.rs`
- **依存**: Task 4.1
- **内容**:
  - 正常インデックスの verify パス
  - 破損インデックスの verify エラー検出

### Phase 5: CLI統合

#### Task 5.1: main.rs + cli/mod.rs 統合
- **成果物**: `src/main.rs`, `src/cli/mod.rs`
- **依存**: Task 2.1, 3.2, 4.1
- **内容**:
  - Commands enum に `Export` / `Import` バリアント追加（英語 doc comment）
  - Status バリアントに `verify: bool` 追加
  - main.rs の match 分岐に Export / Import 処理追加
  - Status 分岐の run() 呼び出しに verify 引数追加
  - cli/mod.rs に `pub mod export;` `pub mod import_index;` 追加

#### Task 5.2: 既存テスト修正
- **成果物**: `tests/cli_args.rs`, `tests/cli_status.rs`
- **依存**: Task 5.1
- **内容**:
  - cli_args.rs: help_flag_shows_usage に export/import 検証追加
  - cli_status.rs: run() 呼び出し3箇所に verify: false 追加

### Phase 6: E2Eテスト

#### Task 6.1: export → import → search E2Eテスト
- **成果物**: `tests/e2e_export_import.rs`
- **依存**: Task 5.1
- **内容**:
  - インデックス作成 → export → import → search の完全フロー
  - import 後に tantivy インデックスがオープンできることの確認
  - import 後に update が正常動作することの確認

### Phase 7: 品質チェック

#### Task 7.1: 最終品質チェック
- **依存**: 全タスク完了
- **内容**:
  - `cargo build` エラー0件
  - `cargo clippy --all-targets -- -D warnings` 警告0件
  - `cargo test --all` 全テストパス
  - `cargo fmt --all -- --check` 差分なし

## タスク依存関係図

```
Task 1.1 (ExportMeta) ──┐
Task 1.2 (エラー型)   ──┤
Task 1.3 (Cargo.toml) ──┤
                         ├──→ Task 2.1 (export) ──→ Task 2.2 (export テスト)
                         ├──→ Task 3.1 (パス検証) ──→ Task 3.2 (import) ──→ Task 3.3 (import テスト)
                         │
Task 4.1 (verify) ───────┤──→ Task 4.2 (verify テスト)
                         │
                         └──→ Task 5.1 (main.rs 統合) ──→ Task 5.2 (既存テスト修正)
                                                        ──→ Task 6.1 (E2E テスト)
                                                        ──→ Task 7.1 (品質チェック)
```

## TDD実装順序

1. **Task 1.1** → テスト: ExportMeta シリアライズ
2. **Task 1.2 + 1.3** → テスト: エラー型, ビルド確認
3. **Task 3.1** → テスト: パストラバーサル検証（セキュリティテスト先行）
4. **Task 2.1 + 2.2** → テスト: export 基本動作
5. **Task 3.2 + 3.3** → テスト: import 基本動作 + セキュリティ
6. **Task 4.1 + 4.2** → テスト: verify
7. **Task 5.1 + 5.2** → テスト: CLI統合 + 既存テスト修正
8. **Task 6.1** → テスト: E2E
9. **Task 7.1** → 品質チェック

## Definition of Done

- [ ] すべてのタスクが完了
- [ ] `cargo test --all` 全テストパス
- [ ] `cargo clippy --all-targets -- -D warnings` 警告0件
- [ ] `cargo fmt --all -- --check` 差分なし
- [ ] セキュリティテスト（パストラバーサル、シンボリックリンク、圧縮爆弾）全パス
- [ ] E2E テスト（export → import → search）パス
