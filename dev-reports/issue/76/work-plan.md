# 作業計画書 - Issue #76: チーム共有設定ファイル（config.toml）

## Issue概要

**Issue番号**: #76
**タイトル**: [Feature] チーム共有設定ファイル（config.toml）
**サイズ**: L
**優先度**: High
**依存Issue**: なし
**設計方針書**: dev-reports/design/issue-76-team-config-design-policy.md

---

## Phase 1: コアモジュール実装

### Task 1.1: config モジュールの型定義とエラー型

**成果物**: `src/config/mod.rs`（定数・エラー型・RawConfig・AppConfig・ConfigSource）
**依存**: なし

- [ ] `src/config/mod.rs` を新規作成
- [ ] 定数定義: `TEAM_CONFIG_FILE`, `LOCAL_CONFIG_FILE`, `LEGACY_CONFIG_FILE`
- [ ] `ConfigError` enum（手動 Display + Error 実装）
  - `ReadError`, `ParseError`, `SerializeError`, `SecretInTeamConfig`
- [ ] `RawConfig` / `RawSearchConfig` / `RawIndexConfig` / `RawEmbeddingConfig` / `RawRerankConfig`（マージ用中間構造体）
- [ ] `AppConfig` / `IndexConfig` / `SearchConfig` / `ConfigSource` / `ConfigSourceKind`（最終設定構造体）
- [ ] `AppConfigView` / `EmbeddingConfigView` / `RerankConfigView`（config show 用 view model）
- [ ] `src/lib.rs` に `pub mod config;` 追加

**テスト（TDD: テスト先行）**:
- RawConfig のデフォルト値テスト
- AppConfig の to_masked_view() で api_key がマスクされることを検証

### Task 1.2: マージロジックとローダー関数

**成果物**: `src/config/mod.rs`（load_config, merge_raw, read_toml, validate_no_secrets, resolve_config）
**依存**: Task 1.1

- [ ] `read_toml()`: TOML ファイル読み込み
- [ ] `validate_no_secrets()`: チーム設定の api_key 拒否バリデーション
- [ ] `merge_raw()`: フィールドレベルマージ（higher が優先）
- [ ] `resolve_config()`: RawConfig → AppConfig 変換（デフォルト値適用）
- [ ] `load_config()`: 公開ローダー関数（優先順位に従ったファイル発見・読込・マージ・警告出力）

**テスト（TDD: テスト先行）**:
- merge_raw: フィールドレベルマージの正確性（base に higher が上書き）
- load_config: ファイルなし → デフォルト値
- load_config: commandindex.toml のみ → 読み込み成功
- load_config: config.local.toml の上書き
- load_config: legacy config.toml の deprecated fallback + 警告
- load_config: 新旧両方存在時の優先順位
- validate_no_secrets: チーム設定に api_key があればエラー
- RawConfig ↔ AppConfig のフィールド同期ラウンドトリップ検証

### Task 1.3: 既存型への Serialize / Debug 追加

**成果物**: `src/embedding/mod.rs`, `src/embedding/openai.rs`, `src/rerank/mod.rs`
**依存**: Task 1.1

- [ ] `EmbeddingConfig`: 既存の Custom Debug（api_key マスク）を確認
- [ ] `ProviderType`: `Serialize` derive 追加
- [ ] `RerankConfig`: Custom Debug 実装（api_key マスク、既存の derive(Debug) を置換）
- [ ] `OpenAiProvider`: Custom Debug 実装（api_key マスク）

**テスト**:
- RerankConfig の Debug 出力で api_key が "***" になること
- OpenAiProvider の Debug 出力で api_key が "***" になること

## Phase 2: 既存コード移行

### Task 2.1: embedding::Config の削除と呼び出し箇所の移行

**成果物**: `src/embedding/mod.rs`, `src/cli/search.rs`, `src/cli/embed.rs`, `src/cli/index.rs`
**依存**: Task 1.2

- [ ] `embedding::Config` 構造体と `Config::load()` メソッドを削除
- [ ] `cli/search.rs`: run() で `load_config()` を1回呼出し、`&AppConfig` を内部関数に引き回し
  - L130: rerank_top_resolved → `config.rerank.top_candidates`
  - L291: run_semantic_search → `&config` 引数追加
  - L424: try_hybrid_search → `&config` 引数追加
  - L650: try_rerank → `&config` 引数追加
- [ ] `cli/embed.rs` L110: `Config::load()` → `load_config()`
- [ ] `cli/index.rs` L795: `Config::load()` → `load_config()`
- [ ] 各ファイルの `use crate::embedding::Config` インポートを削除・更新
- [ ] エラー型に `From<ConfigError>` を追加（SearchError, EmbedError, IndexError 等）

**テスト**:
- 既存テスト（cargo test）が全パスすることを確認

### Task 2.2: clean.rs の保持対象更新

**成果物**: `src/cli/clean.rs`
**依存**: Task 1.1

- [ ] L67, L94 の `config.toml` ハードコード参照を定数（LEGACY_CONFIG_FILE）に更新
- [ ] 保持対象に `LOCAL_CONFIG_FILE` (`config.local.toml`) を追加

**テスト**:
- clean --keep-embeddings で config.toml と config.local.toml が保持されることを確認

## Phase 3: CLI サブコマンド追加

### Task 3.1: config show / config path サブコマンド

**成果物**: `src/cli/config.rs`, `src/cli/mod.rs`, `src/main.rs`
**依存**: Task 1.2

- [ ] `src/cli/config.rs` を新規作成
  - `run_show()`: load_config → to_masked_view → toml::to_string_pretty → stdout
  - `run_path()`: load_config → loaded_sources を優先順位順に表示（legacy は [deprecated] 注記）
- [ ] `src/cli/mod.rs` に `pub mod config;` 追加
- [ ] `src/main.rs` の Commands enum に `Config { command: ConfigCommands }` 追加
- [ ] `ConfigCommands` enum: `Show`, `Path`
- [ ] main.rs の match 文に Config ハンドラ追加

**テスト（TDD: テスト先行）**:
- config show: TOML 形式で出力、api_key がマスクされていること
- config path: 存在するファイルのパスが表示されること
- config path: legacy ファイルに [deprecated] 注記があること

### Task 3.2: 検索引数の Option 化

**成果物**: `src/main.rs`, `src/cli/search.rs`
**依存**: Task 2.1

- [ ] `--limit` を `Option<usize>` に変更（help テキストにデフォルト値明示）
- [ ] `--snippet-lines` を `Option<usize>` に変更
- [ ] `--snippet-chars` を `Option<usize>` に変更
- [ ] search.rs の run() で `cli_limit.unwrap_or(config.search.default_limit)` パターンを適用

**テスト**:
- CLI引数未指定時: ハードコードデフォルト値（20, 2, 120）が使われること
- 設定ファイルあり時: 設定値が使われること
- CLI引数明示時: CLI値が優先されること

## Phase 4: テスト更新・追加

### Task 4.1: E2E テスト更新

**成果物**: `tests/e2e_embedding.rs`, `tests/e2e_semantic_hybrid.rs`
**依存**: Task 2.1

- [ ] e2e_embedding.rs: `.commandindex/config.toml` → `commandindex.toml` に変更
- [ ] e2e_semantic_hybrid.rs: `create_test_config()` を `commandindex.toml` に変更
- [ ] legacy fallback テスト追加: 旧 `.commandindex/config.toml` のみ存在するケース

### Task 4.2: CLI args テスト更新

**成果物**: `tests/cli_args.rs`
**依存**: Task 3.1

- [ ] help 出力に "config" サブコマンドが含まれることを検証
- [ ] config show / config path の基本動作テスト

### Task 4.3: config モジュール E2E テスト

**成果物**: `tests/` 内（新規または既存ファイルに追加）
**依存**: Task 3.1, 3.2

- [ ] E2E 3系統: commandindex.toml のみ / config.local.toml 上書き / legacy fallback
- [ ] 各コマンドのベースパス別設定検出テスト（--path 有無）

## Phase 5: 品質チェックと最終調整

### Task 5.1: 品質チェック

**依存**: 全タスク

- [ ] `cargo build` エラー0件
- [ ] `cargo clippy --all-targets -- -D warnings` 警告0件
- [ ] `cargo test --all` 全テストパス
- [ ] `cargo fmt --all -- --check` 差分なし

### Task 5.2: deprecated 警告の動作確認

- [ ] 旧 config.toml のみ存在: stderr に移行案内が出ること
- [ ] 新旧両方存在: stderr に「旧設定は無視」が出ること
- [ ] 新設定のみ存在: 警告なし

---

## タスク依存関係

```
Task 1.1 ──→ Task 1.2 ──→ Task 2.1 ──→ Task 3.2 ──→ Task 4.3
   │              │            │
   │              │            └──→ Task 4.1
   │              │
   │              └──→ Task 3.1 ──→ Task 4.2
   │
   └──→ Task 1.3
   └──→ Task 2.2
```

## Definition of Done

- [ ] すべてのタスクが完了
- [ ] cargo test --all が全パス
- [ ] cargo clippy --all-targets -- -D warnings で警告ゼロ
- [ ] cargo fmt --all -- --check で差分なし
- [ ] 受け入れ基準（Issue #76 の20項目）を全て満たしている
