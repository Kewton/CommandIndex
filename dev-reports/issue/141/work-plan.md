# 作業計画: Issue #141 `commandindexdev why <file>` コマンドの実装

## Issue概要
**Issue番号**: #141
**タイトル**: `commandindexdev why <file>` コマンドの実装
**サイズ**: M（中規模）
**優先度**: Medium
**依存Issue**: ナレッジグラフ実装（#139、実装済み）

## Phase 1: データ層（Indexer）

### Task 1.1: KnowledgeRelatedResult に title フィールド追加
- **ファイル**: `src/indexer/knowledge.rs`
- **内容**:
  - `KnowledgeRelatedResult` に `pub title: Option<String>` フィールド追加
- **依存**: なし
- **テスト**: 既存テストのコンパイル確認

### Task 1.2: find_knowledge_related() の SQL 拡張
- **ファイル**: `src/indexer/symbol_store.rs`
- **内容**:
  - SELECT句に `kn_issue.title` を追加
  - 戻り値マッピングに title を追加
- **依存**: Task 1.1
- **テスト**: 既存の find_knowledge_related テストが引き続きパスすること

### Task 1.3: find_knowledge_related 単体テスト追加
- **ファイル**: `src/indexer/symbol_store.rs`（テストモジュール内）
- **内容**:
  - title を含む結果の検証テスト
  - 空グラフでの挙動テスト
  - 未登録パスでの空結果テスト
- **依存**: Task 1.2

## Phase 2: 出力層（Output）

### Task 2.1: WhyResult 型定義
- **ファイル**: `src/output/mod.rs`
- **内容**:
  - `WhyResult`, `WhyIssueEntry`, `WhyDocumentEntry` 構造体定義
  - `#[derive(Debug, Clone, Serialize)]`
  - `format_why_results()` ディスパッチ関数
- **依存**: なし

### Task 2.2: Human フォーマッタ
- **ファイル**: `src/output/human.rs`
- **内容**:
  - `format_why_human()` 関数
  - `relation_display_label()` プライベート関数
  - `strip_control_chars` 相当のサニタイズ適用
- **依存**: Task 2.1

### Task 2.3: JSON フォーマッタ
- **ファイル**: `src/output/json.rs`
- **内容**:
  - `format_why_json()` 関数（単一JSONオブジェクト出力）
- **依存**: Task 2.1

### Task 2.4: LLM フォーマッタ
- **ファイル**: `src/output/llm.rs`
- **内容**:
  - `format_why_llm()` 関数
  - `strip_control_chars` 相当のサニタイズ適用
- **依存**: Task 2.1

### Task 2.5: Path フォーマッタ
- **ファイル**: `src/output/path.rs`
- **内容**:
  - `format_why_path()` 関数（1行1パス、入力ファイル含む）
- **依存**: Task 2.1

## Phase 3: CLI層

### Task 3.1: WhyError 定義 + run_why() 関数
- **ファイル**: `src/cli/why.rs`（新規作成）
- **内容**:
  - `WhyError` enum（IndexNotFound, SymbolDbNotFound, SymbolStore, Output, InvalidArgument）
  - `Display`, `From` trait 実装
  - `WHY_AFTER_HELP` 定数
  - `run_why()` メインロジック:
    1. validate_file_paths(&files, 1)
    2. resolve_index_path
    3. symbol_db_path → SymbolStore::open
    4. find_knowledge_related(file_path)
    5. KnowledgeRelatedResult → WhyResult 変換（Issue別グルーピング）
    6. format_why_results
- **依存**: Task 1.2, Task 2.1〜2.5

### Task 3.2: Commands::Why 追加 + dispatch
- **ファイル**: `src/main.rs`, `src/cli/mod.rs`
- **内容**:
  - `Commands::Why { files, format }` バリアント追加
  - `#[command(after_help = why::WHY_AFTER_HELP)]`
  - main() の match に Why アーム追加（cli.index_path.as_deref() 使用）
  - `cli/mod.rs` に `pub mod why;` 追加
- **依存**: Task 3.1

### Task 3.3: help-llm エントリ追加
- **ファイル**: `src/cli/help_llm.rs`
- **内容**:
  - `build_commands()` に why コマンドの `CommandInfo` 追加
- **依存**: Task 3.2

## Phase 4: テスト

### Task 4.1: CLI パーステスト
- **ファイル**: `tests/cli_args.rs`
- **内容**:
  - `help_flag_shows_usage` に `why` 検証追加
  - `help_llm_contains_all_subcommands` の expected 配列に `"why"` 追加、件数 14→15 更新
  - why サブコマンドの基本パーステスト
- **依存**: Task 3.2

### Task 4.2: 品質チェック
- **内容**:
  - `cargo build` — エラー0件
  - `cargo clippy --all-targets -- -D warnings` — 警告0件
  - `cargo test --all` — 全テストパス
  - `cargo fmt --all -- --check` — 差分なし
- **依存**: Task 4.1

## タスク依存関係

```
Task 1.1 → Task 1.2 → Task 1.3
                  ↓
Task 2.1 → Task 2.2〜2.5 → Task 3.1 → Task 3.2 → Task 3.3 → Task 4.1 → Task 4.2
```

Phase 1（データ層）と Phase 2（出力型定義 Task 2.1）は並列実行可能。

## Definition of Done

- [ ] すべてのタスク（Task 1.1〜4.2）が完了
- [ ] `cargo test --all` 全テストパス
- [ ] `cargo clippy --all-targets -- -D warnings` 警告0件
- [ ] `cargo fmt --all -- --check` 差分なし
- [ ] `commandindexdev why --help` が正しく表示される
- [ ] `commandindexdev help-llm` に why が含まれる
