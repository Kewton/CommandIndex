# 作業計画: Issue #102 LLM向けヘルプ改善

## Issue概要

| 項目 | 内容 |
|------|------|
| Issue | #102 [Feature] LLM向けヘルプ改善（自律的なCLI理解・活用を可能にする） |
| サイズ | M（中） |
| 優先度 | High |
| 依存Issue | なし |
| ブランチ | `feature/issue-102-llm-help`（作成済み） |

## タスク分解

### Phase 1: 型定義・コアロジック（TDD: テスト→実装）

#### Task 1.1: HelpLlmError型とhelp_llmモジュール骨格
- **成果物**: `src/cli/help_llm.rs`, `src/cli/mod.rs`
- **依存**: なし
- **内容**:
  - `src/cli/mod.rs` に `pub mod help_llm;` 追加（アルファベット順: gitの後、impactの前）
  - `HelpLlmError` enum 定義（`Serialize(String)` バリアント）
  - `Display` trait 実装
  - `run_help_llm() -> Result<(), HelpLlmError>` 関数の骨格
- **テスト先行**: なし（型定義のみ）

#### Task 1.2: HelpLlmOutput構造体群の定義
- **成果物**: `src/cli/help_llm.rs`
- **依存**: Task 1.1
- **内容**:
  - `HelpLlmOutput` 構造体（schema_version, tool, version, description, global_options, use_cases, workflows, commands）
  - `GlobalOption` 構造体（name, flag, description）
  - `UseCaseItem` 構造体（name, command）
  - `Workflow` 構造体（name, description, steps）
  - `CommandInfo` 構造体（13フィールド、全Option型に`skip_serializing_if`付き）
  - `SearchMode` 構造体
  - `PipeSupport` 構造体
  - 全構造体に `#[derive(Serialize)]`
- **テスト先行**: `test_build_output_serializes_to_json`

#### Task 1.3: build_help_llm_output() 実装
- **成果物**: `src/cli/help_llm.rs`
- **依存**: Task 1.2
- **内容**:
  - 全13サブコマンドの `CommandInfo` データ構築
  - `use_cases` リスト構築（13ユースケース）
  - `workflows` リスト構築（3ワークフロー）
  - `global_options` 構築
  - `version: env!("CARGO_PKG_VERSION").to_string()`
- **テスト先行**:
  - `test_build_output_all_commands_have_examples`（全CommandInfoのexamplesが1つ以上）
  - `test_build_output_all_commands_have_name`（全nameが空でない）
  - `test_build_output_schema_version`（"1.0"である）

#### Task 1.4: run_help_llm() 完成
- **成果物**: `src/cli/help_llm.rs`
- **依存**: Task 1.3
- **内容**:
  - `serde_json::to_string_pretty` でJSON出力
  - `Result<(), HelpLlmError>` を返却
- **テスト先行**: ユニットテスト（出力がJSONとしてパース可能）

### Phase 2: CLI統合

#### Task 2.1: Commands enumにHelpLlm追加
- **成果物**: `src/main.rs`
- **依存**: Task 1.4
- **内容**:
  - `Commands` enumに `#[command(name = "help-llm")] HelpLlm` バリアント追加
  - `/// Show structured JSON help for LLM integration` docコメント
  - main()のmatch分岐にHelpLlmアーム追加（resolve_commandindex_dir未呼び出し、整数値返却）

#### Task 2.2: 各サブコマンドのabout拡充
- **成果物**: `src/main.rs`
- **依存**: なし（他タスクと並行可能）
- **内容**: 13サブコマンド全ての `///` コメントを用途ベースに変更
  - `/// Search the index` → `/// Search the index (full-text, --related, --semantic, --changed-since, --symbol)`
  - `/// Analyze impact of file changes` → `/// Analyze impact of changed files (stdin pipe supported, JSON output)`
  - 他11サブコマンドも同様

#### Task 2.3: after_help定数追加（全サブコマンド）
- **成果物**: 各 `src/cli/*.rs` + `src/main.rs`
- **依存**: Task 2.2
- **内容**:
  - 各cliモジュールに `<COMMAND_NAME>_AFTER_HELP` 定数追加（12ファイル、13定数）
    - search.rs: `SEARCH_AFTER_HELP`
    - impact.rs: `IMPACT_AFTER_HELP`
    - diff.rs: `DIFF_AFTER_HELP`
    - context.rs: `CONTEXT_AFTER_HELP`
    - index.rs: `INDEX_AFTER_HELP`, `UPDATE_AFTER_HELP`
    - status/mod.rs: `STATUS_AFTER_HELP`
    - embed.rs: `EMBED_AFTER_HELP`
    - config.rs: `CONFIG_AFTER_HELP`
    - export.rs: `EXPORT_AFTER_HELP`
    - import_index.rs: `IMPORT_AFTER_HELP`
    - watch.rs: `WATCH_AFTER_HELP`
    - clean.rs: `CLEAN_AFTER_HELP`
  - main.rsの各バリアントに `#[command(after_help = commandindex::cli::xxx::XXX_AFTER_HELP)]` 追加
  - 各定数は「When to use:」「Examples:」セクションを含む

### Phase 3: テスト

#### Task 3.1: 既存テスト更新
- **成果物**: `tests/cli_args.rs`, `tests/e2e_embedding.rs`
- **依存**: Task 2.2
- **内容**:
  - `help_flag_shows_usage`: help-llmのcontains検証追加
  - `embed_help_shows_usage`: about変更に合わせてアサーション更新（`"Generate embeddings"` 部分文字列維持確認）
  - `impact_help_shows_usage`, `config_help_shows_subcommands`, `watch_help_shows_options`: about変更の影響確認・更新

#### Task 3.2: help-llm E2Eテスト追加
- **成果物**: `tests/cli_args.rs`
- **依存**: Task 2.1
- **内容**:
  - `help_llm_outputs_valid_json`: JSON出力のパース検証
  - `help_llm_contains_all_subcommands`: 全13サブコマンドが含まれること
  - `help_llm_has_schema_version`: schema_versionフィールド存在
  - `help_llm_version_matches_cargo`: バージョン一致

### Phase 4: 品質チェック

#### Task 4.1: 品質チェック実行
- **成果物**: なし（検証のみ）
- **依存**: Task 3.1, 3.2
- **内容**:
  - `cargo build` — エラー0件
  - `cargo clippy --all-targets -- -D warnings` — 警告0件
  - `cargo test --all` — 全テストパス
  - `cargo fmt --all -- --check` — 差分なし

## 実行順序（依存関係グラフ）

```
Task 1.1 → Task 1.2 → Task 1.3 → Task 1.4 → Task 2.1
                                                  ↓
Task 2.2（並行可能）→ Task 2.3 → Task 3.1
                                    ↓
                        Task 3.2 → Task 4.1
```

## 推定タスク数

| Phase | タスク数 | 変更ファイル数 |
|-------|---------|------------|
| Phase 1: 型定義・コアロジック | 4 | 2 |
| Phase 2: CLI統合 | 3 | 14 |
| Phase 3: テスト | 2 | 2 |
| Phase 4: 品質チェック | 1 | 0 |
| **合計** | **10** | **16** |

## Definition of Done

- [ ] `commandindexdev help-llm` が構造化JSONを出力する
- [ ] 全13サブコマンドの --help に Examples と When to use が含まれる
- [ ] トップレベル --help の各コマンド説明が用途ベースに拡充されている
- [ ] cargo build / clippy / test / fmt 全パス
- [ ] 既存テストが全て通過する
- [ ] help-llm用の新規テストが全て通過する
