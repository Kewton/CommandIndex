# 作業計画: Issue #169 — issue listサブコマンドの追加

## Issue概要
**Issue番号**: #169
**タイトル**: issue listサブコマンドの追加
**サイズ**: M
**優先度**: Medium
**依存Issue**: なし

## 作業フェーズ

### Phase 1: データ層の実装（src/indexer/symbol_store.rs）

#### Task 1.1: IssueListRow 構造体の定義
- **成果物**: `src/indexer/symbol_store.rs`
- **依存**: なし
- **内容**:
  ```rust
  pub struct IssueListRow {
      pub number: u64,
      pub doc_count: u32,
      pub design_file_path: Option<String>,
      pub has_design: bool,
      pub has_review: bool,
      pub has_workplan: bool,
      pub has_progress: bool,
  }
  ```

#### Task 1.2: list_all_issues() メソッドの実装
- **成果物**: `src/indexer/symbol_store.rs`
- **依存**: Task 1.1
- **内容**:
  - SQLクエリ（JOIN + GROUP BY + 条件付きCOUNT）
  - identifier の u64 パース検証（変換不可は警告ログ + スキップ）
  - `Vec<IssueListRow>` を返却
  - 設計書 Section 6 の SQL を実装

#### Task 1.3: list_all_issues() の単体テスト
- **成果物**: `src/indexer/symbol_store.rs` 内のテストモジュール
- **依存**: Task 1.2
- **テストケース**:
  - 基本動作（複数Issueの一覧取得）
  - modifies混在ケース（doc_countから除外）
  - 設計書なしIssue（has_design=false）
  - 0件ケース
  - Issue番号の昇順ソート確認

### Phase 2: CLI層の実装（src/cli/issue.rs, src/main.rs）

#### Task 2.1: IssueListEntry 構造体と変換ロジック
- **成果物**: `src/cli/issue.rs`
- **依存**: Task 1.1
- **内容**:
  ```rust
  pub struct IssueListEntry {
      pub number: u64,
      pub doc_count: u32,
      pub label: String,
      pub has_design: bool,
      pub has_review: bool,
      pub has_workplan: bool,
      pub has_progress: bool,
  }
  ```
  - `IssueListRow` → `IssueListEntry` 変換関数
  - `extract_label_from_design_path()` 正規表現（LazyLock、expect 使用）

#### Task 2.2: open_symbol_store() ヘルパー関数
- **成果物**: `src/cli/issue.rs`
- **依存**: なし
- **内容**:
  - 既存 `run()` のDB存在チェック + SymbolStore::open をヘルパーに抽出
  - `crate::indexer::symbol_db_path()` を使用

#### Task 2.3: run() → run_show() リネーム
- **成果物**: `src/cli/issue.rs`
- **依存**: Task 2.2
- **内容**:
  - `pub fn run()` を `pub fn run_show()` にリネーム
  - `open_symbol_store()` ヘルパーを使用するようリファクタリング

#### Task 2.4: run_list() 関数の実装
- **成果物**: `src/cli/issue.rs`
- **依存**: Task 1.2, Task 2.1, Task 2.2
- **内容**:
  - `open_symbol_store()` → `store.list_all_issues()` → `IssueListRow` → `IssueListEntry` 変換 → フォーマット出力
  - 0件時: `No issues found.`

#### Task 2.5: 4フォーマッタ関数の実装
- **成果物**: `src/cli/issue.rs`
- **依存**: Task 2.1
- **内容**:
  - `format_list_human()`: `Issue #N (M docs) label` + Total行
  - `format_list_json()`: JSON配列出力
  - `format_list_path()`: Issue番号を1行ずつ
  - `format_list_llm()`: Markdownテーブル形式

#### Task 2.6: main.rs サブコマンド構造変更
- **成果物**: `src/main.rs`
- **依存**: Task 2.3, Task 2.4
- **内容**:
  - `IssueCommands` enum 定義（List, Show）
  - `Commands::Issue` をサブコマンド構造に変更
  - ディスパッチャーで `run_show()` / `run_list()` を呼び分け

#### Task 2.7: フォーマッタの単体テスト
- **成果物**: `src/cli/issue.rs` 内のテストモジュール
- **依存**: Task 2.5
- **テストケース**:
  - human/json/path/llm 各フォーマットの出力確認
  - 0件時の出力
  - label抽出の正規表現テスト
  - `IssueListRow` → `IssueListEntry` 変換テスト

### Phase 3: 既存コードの更新

#### Task 3.1: suggest.rs の更新
- **成果物**: `src/cli/suggest.rs`
- **依存**: Task 2.6
- **内容**:
  - `prepend_knowledge_steps()` 内の `issue {issue_num}` → `issue show {issue_num}` に変更（行258）
  - 関連テスト3件のアサーション更新

#### Task 3.2: help_llm.rs の更新
- **成果物**: `src/cli/help_llm.rs`
- **依存**: Task 2.6
- **内容**:
  - `build_use_cases()`: `issue 140` → `issue show 140`
  - `build_workflows()`: Investigation ワークフローのissueステップ更新
  - `build_commands()`: issue CommandInfo 全面更新
    - name, description, key_options, examples
    - `subcommands` フィールド追加（list, show）

### Phase 4: テストの更新・追加

#### Task 4.1: e2e_issue.rs の既存テスト更新
- **成果物**: `tests/e2e_issue.rs`
- **依存**: Task 2.6
- **内容**:
  - 全6テストの `["issue", "N"]` → `["issue", "show", "N"]` 更新
  - テスト関数名は変更不要（動作自体は同じ）

#### Task 4.2: e2e_issue.rs の issue list テスト追加
- **成果物**: `tests/e2e_issue.rs`
- **依存**: Task 2.4
- **テストケース**:
  - `issue_list_human_format`: 全フォーマット出力確認
  - `issue_list_json_format`: JSON配列の構造確認
  - `issue_list_llm_format`: Markdownテーブル確認
  - `issue_list_path_format`: Issue番号のみ出力確認
  - `issue_list_empty`: 0件時の挙動確認

#### Task 4.3: cli_args.rs のテスト更新・追加
- **成果物**: `tests/cli_args.rs`
- **依存**: Task 2.6
- **内容**:
  - `help_flag_shows_usage`: "issue" 含有確認（既存、変更不要のはず）
  - `issue list --format json` パーステスト追加
  - `issue show` 関連テスト追加
  - 旧構文 `issue <number>` エラーテスト追加

#### Task 4.4: help_llm / suggest 回帰テスト
- **成果物**: `tests/e2e_issue.rs` または該当テストファイル
- **依存**: Task 3.1, Task 3.2
- **内容**:
  - help_llm 出力に旧構文が含まれないことの確認
  - suggest が `issue show <number>` を生成することの確認

### Phase 5: 品質チェック

#### Task 5.1: 品質チェック実行
- **依存**: 全タスク完了後
- **チェック項目**:
  ```bash
  cargo build
  cargo clippy --all-targets -- -D warnings
  cargo test --all
  cargo fmt --all -- --check
  ```

## タスク依存関係図

```
Task 1.1 (IssueListRow) ──→ Task 1.2 (list_all_issues) ──→ Task 1.3 (単体テスト)
     │                            │
     └──→ Task 2.1 (IssueListEntry) ──→ Task 2.5 (フォーマッタ) ──→ Task 2.7 (フォーマッタテスト)
                                   │
Task 2.2 (open_symbol_store) ──→ Task 2.3 (run_show) ──→ Task 2.6 (main.rs) ──→ Task 3.1 (suggest)
                              │                              │                 ──→ Task 3.2 (help_llm)
                              └──→ Task 2.4 (run_list) ──────┘                 ──→ Task 4.1 (e2e更新)
                                                                               ──→ Task 4.2 (e2e追加)
                                                                               ──→ Task 4.3 (cli_args)
                                                                               ──→ Task 4.4 (回帰テスト)
                                                                               ──→ Task 5.1 (品質チェック)
```

## 推奨実装順序

TDD方式での推奨順:

1. **Task 1.1** → **Task 1.3（テスト先行）** → **Task 1.2**（データ層）
2. **Task 2.1** → **Task 2.7（テスト先行）** → **Task 2.5**（フォーマッタ）
3. **Task 2.2**（ヘルパー）
4. **Task 2.3**（run_show リネーム）
5. **Task 2.4**（run_list）
6. **Task 2.6**（main.rs サブコマンド）
7. **Task 4.1** → **Task 4.2**（E2Eテスト）
8. **Task 3.1** → **Task 3.2**（既存コード更新）
9. **Task 4.3** → **Task 4.4**（テスト追加）
10. **Task 5.1**（品質チェック）

## Definition of Done

- [ ] すべてのタスクが完了
- [ ] `cargo test --all` 全テストパス
- [ ] `cargo clippy --all-targets -- -D warnings` 警告ゼロ
- [ ] `cargo fmt --all -- --check` 差分なし
- [ ] `issue list` が全4フォーマットで正しく動作
- [ ] `issue show <number>` が従来と同じ動作
- [ ] 旧構文 `issue <number>` がエラーを返す
- [ ] コードベース内に旧構文 `issue <number>` の参照が残っていない
