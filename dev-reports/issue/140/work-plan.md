# 作業計画: Issue #140 — `commandindex issue <number>` コマンドの実装

## Issue概要

**Issue番号**: #140
**タイトル**: `commandindexdev issue <number>` コマンドの実装
**サイズ**: M（中規模）
**優先度**: Medium
**依存Issue**: ナレッジグラフ（#139、実装済み）

## タスク分解

### Phase 1: データ層（Indexer）

#### Task 1.1: KnowledgeRelation/DocSubtype に Serialize derive 追加
- **ファイル**: `src/indexer/knowledge.rs`
- **内容**:
  - `KnowledgeRelation` enum に `#[derive(Serialize)]` 追加
  - `DocSubtype` enum に `#[derive(Serialize)]` 追加
  - `use serde::Serialize;` 追加
- **依存**: なし
- **テスト**: 既存テストが壊れないことを確認

#### Task 1.2: IssueDocumentEntry 型追加
- **ファイル**: `src/indexer/knowledge.rs`
- **内容**:
  ```rust
  #[derive(Debug, Clone, Serialize)]
  pub struct IssueDocumentEntry {
      pub file_path: String,
      pub relation: KnowledgeRelation,
      pub doc_subtype: DocSubtype,
  }
  ```
- **依存**: Task 1.1

#### Task 1.3: find_documents_by_issue() 関数追加
- **ファイル**: `src/indexer/symbol_store.rs`
- **内容**:
  - SQLクエリ実行（LIMIT 100、ソートなし）
  - metadata JSON を serde_json でパース → IssueDocumentEntry に変換
  - metadata パース失敗時はエラーを返す（silent skip 禁止）
  ```rust
  pub fn find_documents_by_issue(
      &self,
      issue_number: &str,
  ) -> Result<Vec<IssueDocumentEntry>, SymbolStoreError>
  ```
- **依存**: Task 1.2
- **参考**: `find_knowledge_related()` の実装パターン

### Phase 2: CLI層

#### Task 2.1: IssueCommandError 型 + IssueDocumentsResult 型
- **ファイル**: `src/cli/issue.rs`（新規）
- **内容**:
  - `IssueCommandError` enum（SymbolStore, Output, NotFound, CorruptedMetadata）
  - `Display`, `Error`, `From` 実装
  - `IssueDocumentsResult` 構造体
  - `grouped()` ヘルパーメソッド
  - `display_label()` ヘルパー関数
- **依存**: Task 1.2

#### Task 2.2: run() 関数 + 出力フォーマッタ
- **ファイル**: `src/cli/issue.rs`
- **内容**:
  ```rust
  pub fn run(
      issue_number: u64,
      format: OutputFormat,
      commandindex_dir: &Path,
  ) -> Result<(), IssueCommandError>
  ```
  - symbols.db 存在確認（`symbol_db_path().exists()`）
  - SymbolStore::open() → find_documents_by_issue()
  - Rust側ソート（sort_order 関数）
  - 結果0件 → NotFound エラー
  - format_issue_documents_human/json/llm/path をインライン実装
  - human/llm/path で strip_control_chars() 適用
- **依存**: Task 2.1, Task 1.3
- **参考**: `src/cli/suggest.rs` の `run_suggest()` パターン

#### Task 2.3: Commands enum + ディスパッチ追加
- **ファイル**: `src/main.rs`, `src/cli/mod.rs`
- **内容**:
  - `Commands::Issue { number: u64, format: OutputFormat }` 追加
  - `match cli.command` に Issue 分岐追加（resolve_commandindex_dir → run）
  - `src/cli/mod.rs` に `pub mod issue;` 追加
- **依存**: Task 2.2

#### Task 2.4: help-llm 更新
- **ファイル**: `src/cli/help_llm.rs`
- **内容**:
  - `build_commands()` に CommandInfo 追加
  - `build_use_cases()` に UseCaseItem 追加
  - `build_workflows()` の Investigation ワークフローに issue コマンド追加
- **依存**: Task 2.3

### Phase 3: テスト

#### Task 3.1: cli_args.rs テスト更新
- **ファイル**: `tests/cli_args.rs`
- **内容**:
  - `help_flag_shows_usage` に `.stdout(predicate::str::contains("issue"))` 追加
  - `help_llm_contains_all_subcommands` の expected 配列に "issue" 追加、コマンド数更新
  - `issue <NUMBER>` 引数パーステスト追加
- **依存**: Task 2.3

#### Task 3.2: E2E テスト作成
- **ファイル**: `tests/e2e_issue.rs`（新規）
- **内容**:
  - テストデータセットアップ（一時ディレクトリ + SymbolStore にknowledge data 投入）
  - human フォーマット出力検証
  - json フォーマット出力検証（単一JSONオブジェクト）
  - llm フォーマット出力検証（Markdown形式）
  - path フォーマット出力検証（1行1パス）
  - Issue番号不在時のエラーメッセージ検証
  - カテゴリ分類検証（progress_report → 進捗レポート）
- **依存**: Task 2.3
- **参考**: `tests/common/mod.rs` の cargo_bin + 一時ディレクトリパターン

#### Task 3.3: symbol_store 単体テスト追加
- **ファイル**: `src/indexer/symbol_store.rs`（テストモジュール内）
- **内容**:
  - find_documents_by_issue() の正常系テスト
  - Issue番号不在時の空結果テスト
  - metadata パース検証テスト
- **依存**: Task 1.3

### Phase 4: 品質チェック

#### Task 4.1: 最終品質検証
- **コマンド**:
  ```bash
  cargo build
  cargo clippy --all-targets -- -D warnings
  cargo test --all
  cargo fmt --all -- --check
  ```
- **依存**: Task 3.1, Task 3.2, Task 3.3

## 実行順序

```
Phase 1 (データ層)
  Task 1.1 → Task 1.2 → Task 1.3
                            ↓
Phase 2 (CLI層)
  Task 2.1 → Task 2.2 → Task 2.3 → Task 2.4
                            ↓
Phase 3 (テスト)
  Task 3.1 (cli_args更新)
  Task 3.2 (E2Eテスト)
  Task 3.3 (単体テスト)  ← Task 1.3 完了後に並行可能
                            ↓
Phase 4 (品質チェック)
  Task 4.1
```

## TDD実装戦略

各タスクはRed→Green→Refactorサイクルで進める:

1. **Task 3.3（単体テスト）を先に書く** → Task 1.3 を実装
2. **Task 3.1（cli_argsテスト）を先に書く** → Task 2.3 を実装
3. **Task 3.2（E2Eテスト）を先に書く** → Task 2.2 を実装
4. 全テストパス後、Task 4.1 で最終検証

## Definition of Done

- [ ] すべてのタスク（Task 1.1 〜 4.1）が完了
- [ ] `cargo test --all` 全テストパス
- [ ] `cargo clippy --all-targets -- -D warnings` 警告ゼロ
- [ ] `cargo fmt --all -- --check` 差分なし
- [ ] `commandindex issue <N>` で human/json/llm/path 全フォーマット出力確認
- [ ] 存在しないIssue番号でエラーメッセージ出力確認

## 変更ファイル一覧（最終）

| ファイル | 変更種別 |
|---------|---------|
| `src/indexer/knowledge.rs` | 修正（Serialize + IssueDocumentEntry） |
| `src/indexer/symbol_store.rs` | 修正（find_documents_by_issue） |
| `src/cli/issue.rs` | **新規** |
| `src/cli/mod.rs` | 修正（pub mod issue;） |
| `src/main.rs` | 修正（Commands::Issue + ディスパッチ） |
| `src/cli/help_llm.rs` | 修正（commands/use_cases/workflows） |
| `tests/cli_args.rs` | 修正（help/help-llm テスト更新） |
| `tests/e2e_issue.rs` | **新規** |
