# 設計方針書: Issue #142 `before-change` コマンド

## 1. 概要

### Issue情報
- **Issue番号**: #142
- **タイトル**: `commandindexdev before-change <file>` コマンドの実装
- **目的**: ファイル変更前に知るべき設計制約・過去のレビュー指摘を集約して返す

### スコープ
git log走査 → Issue特定 → ナレッジグラフ文書取得 → セマンティックランキングの多段フローで、関連するナレッジドキュメントを発見・提示する新規CLIサブコマンドを追加する。

---

## 2. システムアーキテクチャ概要

### レイヤー構成と責務

| レイヤー | モジュール | 責務 | before-changeでの役割 |
|---------|-----------|------|---------------------|
| **CLI** | `src/main.rs`, `src/cli/` | サブコマンド定義・入力検証 | BeforeChangeバリアント追加、before_change.rs新規 |
| **Indexer** | `src/indexer/` | SQLiteインデックス操作 | find_knowledge_by_issue()追加（読み取りのみ） |
| **Embedding** | `src/embedding/` | Embedding生成・検索 | find_by_path()利用（読み取りのみ） |
| **Output** | `src/output/` | 出力フォーマット | BeforeChangeResult用フォーマッタ追加 |

### データフロー
```
[CLI入力] file_path
    │
    ▼
[git log走査] extract_issues_from_git_log(file_path, max_commits)
    │
    ▼  Vec<String> (issue番号群)
[ナレッジグラフ] symbol_store.find_knowledge_by_issue(&issues)
    │
    ▼  Vec<KnowledgeDoc> (関連ドキュメント)
[セマンティックランキング] rank_by_max_similarity(file_embs, docs, emb_store)  ← オプショナル
    │
    ▼  Vec<Finding> (スコア付きドキュメント)
[出力フォーマット] format_before_change_results(findings, format, writer)
    │
    ▼
[stdout]
```

---

## 3. 詳細設計

### 3.1 CLI定義（src/main.rs）

既存パターン（Impact, Suggest）に倣い、Commands enumにバリアントを追加:

```rust
/// ファイル変更前の設計制約・レビュー指摘を表示
#[command(name = "before-change", after_help = BEFORE_CHANGE_AFTER_HELP)]
BeforeChange {
    /// 対象ファイルパス
    file: String,

    /// 出力フォーマット
    #[arg(long, value_enum, default_value_t = commandindex::output::OutputFormat::Human)]
    format: commandindex::output::OutputFormat,

    /// インデックスパス（別worktree参照用）
    #[arg(long)]
    index_path: Option<PathBuf>,

    /// 結果件数上限（全体上位N件）
    #[arg(long, default_value = "10")]
    limit: usize,

    /// git log走査の最大コミット数（上限: 10000）
    #[arg(long, default_value = "200", value_parser = clap::value_parser!(usize).range(1..=10000))]
    max_commits: usize,
},
```

### 3.2 コマンド本体（src/cli/before_change.rs）

#### エラー型

既存パターン（ImpactError, SuggestError）に倣い構造化enum:

```rust
pub enum BeforeChangeError {
    InvalidInput(String),
    IndexNotFound,
    SymbolDbNotFound,
    SymbolStore(SymbolStoreError),
    Git(GitError),
    Output(OutputError),
    ResolveIndexPath(ResolveIndexPathError),
    Config(String),
    Io(std::io::Error),
    NotGitRepository,
}
// NotGitRepository判定: git log実行時のstderrに "not a git repository" を含む場合にマップ
// Config: 設定ファイル読込失敗時（SearchContextパターン踏襲）
// 注意: EmbeddingStoreError はこの enum に含めない（embedding は非致命エラー扱い）
```

- `fmt::Display`, `std::error::Error` トレイト実装
- サブエラーは `From<SubError>` で自動変換

#### 入力検証

既存の `src/cli/stdin.rs::validate_file_path()` を共通利用し、git固有の追加条件のみ重ねる:
- `validate_file_path()` で共通検証（空文字、NUL、`..`、絶対パス、バックスラッシュ等）
- 追加: 先頭 `-` 禁止（git引数インジェクション防止）

#### git log走査

```rust
fn extract_issues_from_git_log(
    file_path: &str,
    max_commits: usize,
) -> Result<Vec<String>, BeforeChangeError> {
    // Command::new("git").args(["log", "--max-count", &max_commits.to_string(), "--format=%s%n%b", "--", file_path])
    // 注意: "--" セパレータを必ず使用（file_pathがオプションとして解釈されることを防止）
    // 出力行数上限: MAX_GIT_OUTPUT_LINES (既存パターン踏襲) で制限
    // 正規表現: #(\d+), \(#(\d+)\), fixes #(\d+), refs #(\d+)
    // 重複除去して返す
}
```

#### メインエントリポイント

```rust
pub fn run_before_change(
    file: &str,
    format: OutputFormat,
    index_path: Option<&Path>,
    limit: usize,
    max_commits: usize,
) -> Result<(), BeforeChangeError>
```

### 3.3 ナレッジグラフ拡張（src/indexer/symbol_store.rs）

#### 新メソッド: find_knowledge_by_issue

```rust
pub fn find_knowledge_by_issue(
    &self,
    issue_numbers: &[String],
) -> Result<Vec<KnowledgeDocResult>, SymbolStoreError> {
    // SQL:
    // SELECT kn_issue.identifier AS issue_number,
    //        ke.relation,
    //        kn_doc.file_path,
    //        kn_doc.title
    // FROM knowledge_nodes kn_issue
    // JOIN knowledge_edges ke ON ke.source_id = kn_issue.id
    // JOIN knowledge_nodes kn_doc ON ke.target_id = kn_doc.id AND kn_doc.type = 'document'
    // WHERE kn_issue.type = 'issue'
    //   AND kn_issue.identifier IN (?, ?, ...)
    // ORDER BY kn_issue.identifier, ke.relation
}
```

#### 新しい結果型

```rust
pub struct KnowledgeDocResult {
    pub issue_number: String,
    pub relation: KnowledgeRelation,  // 既存enumを再利用（型安全）
    pub file_path: String,
    pub title: Option<String>,
}
// DB復元: KnowledgeRelation::from_str() を実装。未知のrelation文字列はスキップ（Warn出力）
// 出力層で文字列化: relation.to_string() で "has_design" 等に変換
// 空入力時: issue_numbers が空の場合は DB クエリせず空 Vec を返す
```

**設計判断**: 既存の `find_knowledge_related()` は変更しない。起点が異なる（file_path起点 vs issue番号起点）ため、別メソッドとして追加する。

### 3.4 セマンティックランキング

#### max pooling方式

```rust
fn rank_by_max_similarity(
    file_embs: &[EmbeddingRecord],
    docs: &[KnowledgeDocResult],
    embedding_store: &EmbeddingStore,
) -> Result<Vec<Finding>, BeforeChangeError> {
    // 各ドキュメントについて:
    //   doc_embs = embedding_store.find_by_path(&doc.file_path)?
    //   score = max(cosine_similarity(f, d) for f in file_embs, d in doc_embs)
    //   → Finding { doc, similarity: Some(score) }
    // embedding未取得のドキュメントは similarity: None で末尾に
}
```

**設計判断**:
- max pooling（各ペアの最大コサイン類似度）を採用。理由: section単位で最も関連の強い部分を捉えるため。avgだと無関連sectionがスコアを希釈する。
- cosine_similarity は既存の `embedding/store.rs:140` の実装を `pub(crate)` に変更して利用する。クレート外への公開は避けつつ、before_change.rs から利用可能にする。
- embedding未使用時（フォールバック）のソート順: Issue番号昇順 → relation種別順（has_design > has_review > has_workplan）

#### Embedding系エラーの致命/非致命分類

| ケース | 分類 | 動作 |
|--------|------|------|
| embeddings.db 不存在 | Warning | ナレッジグラフのみで動作（ランキングなし） |
| embeddings.db schema mismatch | Warning | フォールバック（ランキングなし） |
| embeddings.db SQLiteエラー | Warning | フォールバック（ランキングなし） |
| 対象ファイルのembedding未取得 | Info | 全ドキュメント similarity: None |
| 個別文書のembedding未取得 | Silent | その文書のみ similarity: None、末尾表示 |

### 3.5 出力構造体（src/output/mod.rs）

```rust
pub struct BeforeChangeResult {
    pub file_path: String,
    pub findings: Vec<BeforeChangeFinding>,
    pub total_issues: usize,
    pub has_embeddings: bool,
}

pub struct BeforeChangeFinding {
    pub issue_number: String,
    pub relation: String,
    pub doc_path: String,
    pub doc_title: Option<String>,
    pub similarity: Option<f32>,
}
```

統一API: `format_before_change_results(result: &BeforeChangeResult, format: OutputFormat, writer: &mut dyn Write) -> Result<(), OutputError>`

各フォーマッタに追加:
- `output/human.rs`: `format_before_change_human(result: &BeforeChangeResult, writer: &mut dyn Write)`
- `output/json.rs`: `format_before_change_json(result: &BeforeChangeResult, writer: &mut dyn Write)`
- `output/path.rs`: `format_before_change_path(result: &BeforeChangeResult, writer: &mut dyn Write)`
- `output/llm.rs`: `format_before_change_llm(result: &BeforeChangeResult, writer: &mut dyn Write)`

### 3.6 help-llm更新（src/cli/help_llm.rs）

CommandInfoリストにbefore-changeエントリを追加:

```rust
CommandInfo {
    name: "before-change",
    description: "Show design constraints and review findings for a file before making changes",
    when_to_use: "Before modifying a file, to understand related design decisions and review history",
    prerequisites: Some("commandindexdev index".to_string()),
    modes: None,
    conflicts: None,
    key_options: Some(vec!["--format", "--index-path", "--limit", "--max-commits"]),
    output_formats: Some(vec!["human", "json", "llm", "path"]),
    pipe_support: None,
}
```

---

## 4. インデックスパス解決

### 設計判断

既存の `resolve_index_path()` を使用:

```rust
let resolved = resolve_index_path(
    index_path.as_deref(),    // CLI引数
    config_index_path,         // 設定ファイル
    &std::env::current_dir()?, // ベースパス
)?;
let sym_db = symbol_db_path(&resolved);
let emb_db = embeddings_db_path(&resolved);
```

優先順位: CLI引数 > 設定ファイル > デフォルト（.commandindex/）

---

## 5. セキュリティ設計

| 脅威 | 対策 | 優先度 |
|------|------|--------|
| git引数インジェクション | file_pathの先頭`-`チェック、`--`区切り使用 | 高 |
| パストラバーサル | resolve_index_path()のCanonicalize + パストラバーサル検出 | 高 |
| シンボリックリンク | resolve_index_path()の経路検証で対応（read-onlyコマンドのためreject_symlink()は不要） | 低 |
| 制御文字インジェクション | 入力: validate_file_path()で検証。出力: 既存のstrip_control_chars()でpath/titleをサニタイズ | 中 |
| SQLインジェクション | rusqliteのパラメータバインディング使用 | 高 |
| unsafe使用 | 原則禁止 | 中 |

---

## 6. 設計判断とトレードオフ

### 判断1: git log走査 vs ナレッジグラフ拡張

**選択**: git logランタイム走査
**理由**: ナレッジグラフにソースファイル→Issueのエッジを追加するとindex/updateの処理が複雑化する。git logは常に最新の情報を返し、インデックス更新のタイミングに依存しない。
**トレードオフ**: ランタイムのgitプロセス起動コスト。--max-commitsで制御。

### 判断2: cosine_similarity の max pooling vs avg pooling

**選択**: max pooling
**理由**: セクション単位で最も関連の強い部分を捉えるため。avgは無関連セクションがスコアを希釈する。
**トレードオフ**: 1つのセクションだけが高スコアでも全体として関連性が高いと判定される。

### 判断3: embedding未生成時のフォールバック

**選択**: ナレッジグラフのみで動作（セマンティックランキングなし）
**理由**: before-changeコマンドの主目的は「関連ドキュメントの発見」であり、ランキングは付加価値。embedding未生成でも使える方がユーザーにとって有用。
**トレードオフ**: embedding未使用時は関連度順ではなくIssue番号順等の固定順序。

### 判断4: Issue番号抽出の正規表現

**選択**: `#(\d+)`, `\(#(\d+)\)`, `fixes #(\d+)`, `refs #(\d+)` の4パターン
**理由**: GitHubの標準的なIssue参照パターンをカバー。PR番号も#NNNで参照されるが、PRとIssueの区別はGitHub上でも曖昧なため、全て含める。
**トレードオフ**: PR番号も拾うためノイズが増える可能性。ただしナレッジグラフにエッジがないIssue番号は自然にフィルタされる。

---

## 7. 影響範囲

### 変更対象

| ファイル | 変更内容 | リスク |
|---------|---------|--------|
| `src/cli/before_change.rs` | 新規作成 | 低（新規） |
| `src/cli/mod.rs` | モジュール宣言追加 | 低 |
| `src/main.rs` | Commandsバリアント追加、分岐追加 | 低 |
| `src/indexer/symbol_store.rs` | find_knowledge_by_issue()追加 | 低（追加のみ） |
| `src/output/mod.rs` | BeforeChangeResult型追加 | 低 |
| `src/output/human.rs` | format_before_change_human()追加 | 低 |
| `src/output/json.rs` | format_before_change_json()追加 | 低 |
| `src/output/path.rs` | format_before_change_path()追加 | 低 |
| `src/output/llm.rs` | format_before_change_llm()追加 | 低 |
| `src/cli/help_llm.rs` | CommandInfoエントリ追加 | 低 |
| `tests/cli_args.rs` | help期待値更新 | 中（既存テスト変更） |
| `tests/e2e_before_change.rs` | E2Eテスト新規作成 | 低（新規） |
| `tests/` | output層の単体テスト追加（BeforeChangeResult各フォーマット） | 低（新規） |

### 既存機能への影響
- 検索・indexingの既存アルゴリズム: 影響なし
- CLI公開面: --help / help-llm の出力とそれに依存するテスト・連携に影響
- output層: BeforeChangeResult型とformat関数の追加（既存format関数には影響なし）
- symbol_store公開API: find_knowledge_by_issue()メソッド追加

---

## 8. パフォーマンス考慮

| 処理 | コスト | 制御方法 |
|------|--------|---------|
| git log走査 | O(max_commits) + プロセス起動 | --max-commits (デフォルト200) |
| ナレッジグラフクエリ | O(issues × docs) | SQLiteインデックス |
| embedding比較 | O(file_sections × doc_sections × docs) | --limit (デフォルト10) |
| 全体 | git log > embedding比較 > DB検索 | 上記の組み合わせ |

---

## 9. テスト戦略

### ユニットテスト
- `extract_issues_from_git_log()`: 各パターン（#NNN, (#NNN), fixes #NNN, refs #NNN）のマッチ
- `rank_by_max_similarity()`: max poolingの正確性
- 入力検証: 不正パス拒否

### E2Eテスト（tests/e2e_before_change.rs）
- 正常系: git repo + ナレッジグラフ + embedding ありの場合
- embedding未生成: ナレッジグラフのみで動作
- インデックス未作成: エラーメッセージ確認
- 関連Issue未発見: 適切なメッセージ確認
- 不正入力: エラー確認
- 非gitリポジトリ: エラー確認
- 各出力フォーマット: human, json, llm, path

### 既存テスト更新
- `tests/cli_args.rs`: トップレベルhelp、help-llm件数

---

## 10. 品質基準

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |
