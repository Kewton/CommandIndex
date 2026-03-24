# 作業計画: Issue #142 `before-change` コマンド

## Issue概要
- **Issue番号**: #142
- **タイトル**: `commandindexdev before-change <file>` コマンドの実装
- **サイズ**: L（新規サブコマンド + ナレッジグラフ拡張 + 出力フォーマット + E2Eテスト）
- **優先度**: Medium
- **依存Issue**: #134 (BGE-M3) 完了済み、#139 (ナレッジグラフ) 完了済み

---

## タスク分解

### Phase 1: データモデル・型定義

#### Task 1.1: KnowledgeDocResult型とfind_knowledge_by_issue()追加
- **成果物**: `src/indexer/symbol_store.rs`
- **依存**: なし
- **作業内容**:
  - `KnowledgeDocResult` 構造体追加（issue_number, relation: KnowledgeRelation, file_path, title）
  - `KnowledgeRelation::from_str()` 実装（未知値スキップ + warn）
  - `find_knowledge_by_issue(&self, issue_numbers: &[String])` メソッド追加
  - 空入力時は空Vec返却
  - SQL: knowledge_nodes JOIN knowledge_edges で Issue → document 1ホップ走査
- **テスト**: ユニットテスト（正常系、空入力、未知relation）

#### Task 1.2: BeforeChangeError型とBeforeChangeResult型
- **成果物**: `src/cli/before_change.rs`（新規）、`src/output/mod.rs`
- **依存**: Task 1.1
- **作業内容**:
  - `BeforeChangeError` enum（InvalidInput, IndexNotFound, SymbolDbNotFound, SymbolStore, Git, Output, ResolveIndexPath, Config, Io, NotGitRepository）
  - `fmt::Display`, `std::error::Error`, `From` impl
  - `BeforeChangeResult` 構造体（file_path, findings, total_issues, has_embeddings）
  - `BeforeChangeFinding` 構造体（issue_number, relation, doc_path, doc_title, similarity）

#### Task 1.3: cosine_similarity の pub(crate) 化
- **成果物**: `src/embedding/store.rs`
- **依存**: なし
- **作業内容**: `fn cosine_similarity` → `pub(crate) fn cosine_similarity`

### Phase 2: コアロジック実装

#### Task 2.1: git log走査ロジック（extract_issues_from_git_log）
- **成果物**: `src/cli/before_change.rs`
- **依存**: Task 1.2
- **作業内容**:
  - `extract_issues_from_git_log(file_path: &str, max_commits: usize)` 関数
  - `git log --max-count=N --format=%s%n%b -- file_path` 実行
  - 正規表現パターン: `#(\d+)`, `\(#(\d+)\)`, `(?i:fixes)\s+#(\d+)`, `(?i:refs)\s+#(\d+)`（case insensitive）
  - 出力行数上限（MAX_GIT_OUTPUT_LINES踏襲）
  - `--` セパレータ必須
  - NotGitRepository判定: stderr に "not a git repository" 含む場合
  - 重複除去して Vec<String> 返却
- **テスト**: ユニットテスト（各パターンマッチ、重複除去、case insensitive）

#### Task 2.2: セマンティックランキング（rank_by_max_similarity）
- **成果物**: `src/cli/before_change.rs`
- **依存**: Task 1.3, Task 1.2
- **作業内容**:
  - `rank_by_max_similarity(file_embs, docs, embedding_store)` 関数
  - max pooling: 各ファイルsection × 各文書section の最大cosine_similarity
  - embedding未取得の文書は similarity: None で末尾
  - Embedding系エラーは非致命（warning/info/silent の分類に従う）
- **テスト**: ユニットテスト（スコア計算、Noneフォールバック）

#### Task 2.3: 入力検証
- **成果物**: `src/cli/before_change.rs`
- **依存**: Task 1.2
- **作業内容**:
  - `validate_file_path()` 共通利用（stdin.rs）
  - 追加: 先頭 `-` チェック（git引数インジェクション防止）
- **テスト**: ユニットテスト（正常パス、空文字、先頭`-`、`..`含む）

#### Task 2.4: メインエントリポイント（run_before_change）
- **成果物**: `src/cli/before_change.rs`
- **依存**: Task 2.1, Task 2.2, Task 2.3, Task 1.1
- **作業内容**:
  - `pub fn run_before_change(file, format: OutputFormat, index_path, limit, max_commits)` 実装
  - フロー: 入力検証 → git log走査 → ナレッジグラフ文書取得 → セマンティックランキング → 出力
  - 設定読込 + resolve_index_path + symbol_db_path/embeddings_db_path
  - embedding非致命エラーハンドリング
  - BEFORE_CHANGE_AFTER_HELP 定数定義

### Phase 3: CLI統合

#### Task 3.1: Commands enumにBeforeChange追加
- **成果物**: `src/main.rs`
- **依存**: Task 2.4
- **作業内容**:
  - `Commands::BeforeChange` バリアント追加（file, format: OutputFormat, index_path, limit, max_commits）
  - max_commits: `value_parser = clap::value_parser!(usize).range(1..=10000)`
  - main()の分岐にBeforeChange処理追加（Impactパターン踏襲）

#### Task 3.2: cli/mod.rsにモジュール宣言追加
- **成果物**: `src/cli/mod.rs`
- **依存**: Task 2.4
- **作業内容**: `pub mod before_change;` 追加

### Phase 4: 出力フォーマット

#### Task 4.1: format_before_change_results ディスパッチャ
- **成果物**: `src/output/mod.rs`
- **依存**: Task 1.2
- **作業内容**:
  - `pub fn format_before_change_results(result: &BeforeChangeResult, format: OutputFormat, writer: &mut dyn Write) -> Result<(), OutputError>`
  - 4フォーマット分岐（Human, Json, Path, Llm）

#### Task 4.2: human/json/path/llm フォーマッタ
- **成果物**: `src/output/human.rs`, `src/output/json.rs`, `src/output/path.rs`, `src/output/llm.rs`
- **依存**: Task 4.1
- **作業内容**:
  - human: Issueの使用例に近い形式、strip_control_chars()適用
  - json: serde_json::to_string_pretty
  - path: ドキュメントパスのみ出力
  - llm: LLMコンテキスト向け構造化テキスト

### Phase 5: help-llm更新

#### Task 5.1: help_llm.rsにCommandInfo追加
- **成果物**: `src/cli/help_llm.rs`
- **依存**: Task 3.1
- **作業内容**: before-changeのCommandInfoエントリ追加

### Phase 6: テスト

#### Task 6.1: 既存テスト更新
- **成果物**: `tests/cli_args.rs`
- **依存**: Task 3.1, Task 5.1
- **作業内容**:
  - トップレベル --help 期待値にbefore-change追加
  - help-llm コマンド一覧・件数テスト更新
  - before-change --help テスト追加

#### Task 6.2: E2Eテスト
- **成果物**: `tests/e2e_before_change.rs`（新規）
- **依存**: Task 2.4, Task 4.2
- **作業内容**:
  - tempdir に git repo セットアップ（e2e_changed_since.rsパターン踏襲）
  - dev-reports/ にテスト用設計書配置
  - ナレッジグラフにテストデータ投入
  - テストケース:
    1. 正常系: git log + ナレッジグラフ → ドキュメント発見
    2. embedding未生成: ナレッジグラフのみで動作
    3. インデックス未作成: エラーメッセージ確認
    4. 関連Issue未発見: 適切なメッセージ
    5. 不正入力（空文字、先頭`-`）: エラー確認
    6. 非gitリポジトリ: エラー確認
    7. --format human/json/llm/path: 各出力確認
    8. --limit: 件数制限確認
    9. --max-commits: コミット数制限確認

#### Task 6.3: 出力フォーマットテスト
- **成果物**: `tests/` 内（既存パターンに統合）
- **依存**: Task 4.2
- **作業内容**: BeforeChangeResult の各フォーマット出力の単体テスト

---

## タスク依存関係

```
Task 1.1 ──┐
Task 1.2 ──┤
Task 1.3 ──┤
           ├→ Task 2.1 ──┐
           ├→ Task 2.2 ──┤
           ├→ Task 2.3 ──┤
           │              ├→ Task 2.4 ──┐
           │              │              ├→ Task 3.1 ──┐
           │              │              ├→ Task 3.2    ├→ Task 5.1 → Task 6.1
           │              │              │              │
           ├→ Task 4.1 ──┤              │              │
           │              ├→ Task 4.2 ──┘              │
           │                                           ├→ Task 6.2
           │                                           └→ Task 6.3
```

## 実装順序（TDD）

1. **Task 1.1**: KnowledgeDocResult + find_knowledge_by_issue() → テスト
2. **Task 1.3**: cosine_similarity pub(crate)化
3. **Task 1.2**: エラー型 + 結果型定義
4. **Task 2.3**: 入力検証 → テスト
5. **Task 2.1**: git log走査 → テスト
6. **Task 2.2**: セマンティックランキング → テスト
7. **Task 4.1 + 4.2**: 出力フォーマット → テスト
8. **Task 2.4**: run_before_change (統合)
9. **Task 3.1 + 3.2**: CLI統合
10. **Task 5.1**: help-llm更新
11. **Task 6.1**: 既存テスト更新
12. **Task 6.2 + 6.3**: E2Eテスト

---

## 品質チェック項目

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |

## Definition of Done

- [ ] すべてのタスク（Task 1.1〜6.3）完了
- [ ] cargo build エラー0件
- [ ] cargo clippy 警告0件
- [ ] cargo test 全パス
- [ ] cargo fmt 差分なし
- [ ] 受け入れ基準21項目すべてクリア
