# 設計方針書: Issue #140 — `commandindex issue <number>` コマンドの実装

## 1. 概要

Issue番号を指定して、ナレッジグラフから関連する全ドキュメント（設計ポリシー、レビュー、作業計画、進捗レポート）を一括取得するCLIサブコマンドを追加する。

## 2. レイヤー構成と責務

本機能は既存の4層アーキテクチャに従い、各層に最小限の追加を行う。

| レイヤー | 変更対象 | 責務 |
|---------|---------|------|
| **CLI** | `src/main.rs`, `src/cli/issue.rs` | サブコマンド定義、ディスパッチ、エラー処理、出力フォーマット |
| **Indexer** | `src/indexer/symbol_store.rs` | SQLクエリ実行、DTOへの変換 |
| **Output** | （変更なし） | 出力フォーマッタは cli/issue.rs にインライン実装 |
| **Help** | `src/cli/help_llm.rs` | LLM向けヘルプ情報の更新 |

### データフロー

```
CLI (main.rs)
  │ Commands::Issue { number, format }
  │ resolve_commandindex_dir() で commandindex_dir を解決
  ↓
cli::issue::run(issue_number, format, &commandindex_dir)
  │ symbols.db 存在確認 → SymbolStore::open()
  │ symbol_store.find_documents_by_issue(issue_number)
  │ Rust側でソート（SQLはソート無し）
  │ grouped() で分類
  ↓
cli::issue 内の出力関数
  │ format_issue_documents(result, format, &mut stdout)
  ↓
stdout
```

## 3. 型設計

### 3.1 データ層（indexer/knowledge.rs）

既存の `KnowledgeRelation` / `DocSubtype` enum を再利用し、型安全性を維持する。

```rust
/// Issue関連ドキュメントの検索結果（metadataパース済みDTO）
#[derive(Debug, Clone, Serialize)]
pub struct IssueDocumentEntry {
    pub file_path: String,
    pub relation: KnowledgeRelation,
    pub doc_subtype: DocSubtype,
}
```

**設計判断**:
- `relation` と `doc_subtype` に既存の enum 型を使用し DRY 原則に準拠
- `Serialize` derive を付与（JSON出力で必要）
- 配置場所は `indexer/knowledge.rs`（既存の `KnowledgeRelatedResult` と同じモジュール）
- metadataのJSONパースはIndexer層で完結させ、Output層には構造化済みDTOのみを渡す

### 3.2 結果型（cli/issue.rs）

出力用の結果型は cli/issue.rs 内に定義する（SRP: output/mod.rs の凝集度維持）。

```rust
/// Issueドキュメント検索結果（出力用）
#[derive(Debug, Clone, Serialize)]
pub struct IssueDocumentsResult {
    pub issue_number: String,
    pub documents: Vec<IssueDocumentEntry>,
}
```

### 3.3 エラー型（cli/issue.rs）

```rust
#[derive(Debug)]
pub enum IssueCommandError {
    SymbolStore(SymbolStoreError),
    Output(OutputError),
    NotFound { issue_number: u64 },
    CorruptedMetadata { file_path: String, reason: String },
}

impl fmt::Display for IssueCommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SymbolStore(e) => write!(f, "{e}"),
            Self::Output(e) => write!(f, "{e}"),
            Self::NotFound { issue_number } => {
                write!(f, "No documents found for issue #{issue_number}")
            }
            Self::CorruptedMetadata { file_path, reason } => {
                write!(f, "Corrupted metadata for {file_path}: {reason}")
            }
        }
    }
}

impl std::error::Error for IssueCommandError {}

impl From<SymbolStoreError> for IssueCommandError {
    fn from(e: SymbolStoreError) -> Self { Self::SymbolStore(e) }
}
impl From<OutputError> for IssueCommandError {
    fn from(e: OutputError) -> Self { Self::Output(e) }
}
```

**設計判断**:
- `NotFound` バリアント: CLI契約として exit 1 を保証。パイプライン利用時に「成功したが結果なし」と「本当に成功」を区別する
- `CorruptedMetadata` バリアント: metadata破損を silent skip せずエラーとして報告。ナレッジグラフの整合性問題を検知可能にする
- `From` 実装: `?` 演算子で流せるようにし実装を簡潔にする

### 3.4 CLI定義（main.rs）

```rust
/// Show documents related to an Issue from knowledge graph
Issue {
    /// Issue number
    #[arg(value_parser = clap::value_parser!(u64).range(1..))]
    number: u64,
    /// Output format (human, json, path, llm)
    #[arg(long, value_enum, default_value_t = commandindex::output::OutputFormat::Human)]
    format: commandindex::output::OutputFormat,
},
```

## 4. SQLクエリ設計

ソートロジックはRust側で実行する（DRY: json_extract の結果を SQL と Rust の両方で扱うことを避ける）。

```sql
SELECT kn_doc.file_path, ke.relation, ke.metadata
FROM knowledge_nodes kn_issue
JOIN knowledge_edges ke ON ke.source_id = kn_issue.id
JOIN knowledge_nodes kn_doc ON ke.target_id = kn_doc.id AND kn_doc.type = 'document'
WHERE kn_issue.type = 'issue' AND kn_issue.identifier = ?1
LIMIT 100;
```

**設計判断**:
- `UNIQUE(type, identifier)` インデックスにより、WHERE句は効率的に実行される
- ソートはRust側で `IssueDocumentEntry` の `relation` と `doc_subtype` の enum 序数を使って実行
- `LIMIT 100` でDoS対策（通常1 Issueあたり10件以下）
- `json_extract` をSQL内で使わず、Rust側で `serde_json` でmetadataをパース

### Rust側ソートロジック

```rust
fn sort_order(entry: &IssueDocumentEntry) -> (u8, u8) {
    let relation_order = match entry.relation {
        KnowledgeRelation::HasDesign => 1,
        KnowledgeRelation::HasReview => 2,
        KnowledgeRelation::HasWorkplan => 3,
    };
    let subtype_order = match entry.doc_subtype {
        DocSubtype::DesignPolicy => 1,
        DocSubtype::IssueReview => 2,
        DocSubtype::DesignReview => 3,
        DocSubtype::WorkPlan => 4,
        DocSubtype::ProgressReport => 5,
    };
    (relation_order, subtype_order)
}
```

## 5. 出力カテゴリ分類ロジック

cli/issue.rs 内にインラインで実装する（suggest コマンドのパターン準拠）。

表示ラベルは cli::issue モジュール内のヘルパー関数として定義する（SRP: Indexer層のenumにUI責務を持たせない）。

```rust
// cli/issue.rs 内
fn display_label(subtype: &DocSubtype) -> &'static str {
    match subtype {
        DocSubtype::DesignPolicy => "設計",
        DocSubtype::IssueReview | DocSubtype::DesignReview => "レビュー",
        DocSubtype::WorkPlan => "作業計画",
        DocSubtype::ProgressReport => "進捗レポート",
    }
}
```

分類済みビューのヘルパー（DRY: 各フォーマッタで毎回分類しない）:

```rust
impl IssueDocumentsResult {
    /// カテゴリ別にグループ化した結果を返す
    pub fn grouped(&self) -> Vec<(&'static str, Vec<&IssueDocumentEntry>)> {
        let categories = ["設計", "レビュー", "作業計画", "進捗レポート"];
        categories.iter().filter_map(|&cat| {
            let docs: Vec<_> = self.documents.iter()
                .filter(|d| display_label(&d.doc_subtype) == cat)
                .collect();
            if docs.is_empty() { None } else { Some((cat, docs)) }
        }).collect()
    }
}
```

**設計判断**:
- 表示ラベルは cli/issue.rs 内のヘルパーに配置（SRP: Indexer層のenumにUI責務を持たせない）
- `grouped()` ヘルパーで分類ロジックを一箇所に集約（DRY）
- `progress_report` は `has_review` リレーションを持つが、出力時は「進捗レポート」として独立カテゴリに分類
- 出力フォーマッタは cli/issue.rs 内にインライン実装（suggest パターン準拠、4ファイル分散は過剰）
- human/llm フォーマッタでは `strip_control_chars()` を file_path に適用（出力インジェクション対策）

## 6. CLIディスパッチ設計

main.rs で `resolve_commandindex_dir()` 済みの `commandindex_dir` を渡す（resolve_commandindex_dir は main.rs の private 関数のため cli モジュールからは参照不可）。

```rust
Commands::Issue { number, format } => {
    let base_path = std::path::Path::new(".");
    let (commandindex_dir, _config) =
        match resolve_commandindex_dir(cli.index_path.as_deref(), base_path) {
            Ok(v) => v,
            Err(e) => { eprintln!("Error: {e}"); process::exit(1); }
        };
    match commandindex::cli::issue::run(number, format, &commandindex_dir) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("Error: {e}");
            1
        }
    }
}
```

**設計判断**:
- `resolve_commandindex_dir()` は main.rs の private 関数のため、main.rs 側で解決して渡す（impact/diff パターン準拠）
- Tantivyインデックスは不要で、SymbolStore（SQLite）のみ使用する
- `cli::issue::run()` 内で `symbols.db` の存在確認を明示的に行う（`SymbolStore::open()` は DB 未存在時に新規作成するため、open だけでは未インデックス状態を検出できない。impact/search パターンに倣い `symbol_db_path.exists()` で先行チェック）

## 7. エラー処理方針

| エラーケース | 処理方法 | 既存パターン準拠 |
|---|---|---|
| symbols.db不在 | `SymbolStoreError` でラップ → stderr出力、exit 1 | ✅ search/impact と同様 |
| Issue番号のドキュメントが0件 | `IssueCommandError::NotFound` → stderr に `No documents found for issue #N` を出力、exit 1 | ✅ CLI契約: 結果なし=失敗 |
| metadata破損 | `IssueCommandError::CorruptedMetadata` → stderr にエラー出力、exit 1 | ✅ silent skip 禁止 |
| 非数値・0入力 | clapの型バリデーション（`u64`, `range(1..)`) | ✅ |

## 8. セキュリティ設計

| 脅威 | 対策 |
|------|------|
| SQLインジェクション | パラメータバインド（`?1`）を使用。文字列連結なし |
| パストラバーサル | ファイルパスはDBから読み取るのみで、ファイルI/Oなし |
| 出力インジェクション | human/llm/path フォーマッタで `strip_control_chars()` を file_path に適用 |
| DoS対策 | SQLクエリに `LIMIT 100` を付与 |
| metadata破損 | Rust側 serde_json パースでエラー時は `CorruptedMetadata` エラーを返す（silent skip 禁止） |

## 9. 影響範囲

### 変更ファイル一覧

| ファイル | 変更種別 | 内容 |
|---------|---------|------|
| `src/main.rs` | 修正 | Commands enum に `Issue` バリアント追加 + match分岐追加 |
| `src/cli/mod.rs` | 修正 | `pub mod issue;` 追加 |
| `src/cli/issue.rs` | **新規** | issueコマンドロジック、IssueDocumentsResult型、出力フォーマッタ（インライン） |
| `src/indexer/symbol_store.rs` | 修正 | `find_documents_by_issue()` 追加 |
| `src/indexer/knowledge.rs` | 修正 | `IssueDocumentEntry` 型追加、`KnowledgeRelation`/`DocSubtype` に `Serialize` derive 追加 |
| `src/cli/help_llm.rs` | 修正 | build_commands, build_use_cases, build_workflows に issue コマンド情報追加 |
| `tests/cli_args.rs` | 修正 | help/help-llmテスト更新（expected配列に "issue" 追加、コマンド数更新） |
| `tests/e2e_issue.rs` | **新規** | E2Eテスト |

### 影響なしの確認

- 既存コマンド（search, index, update, diff, impact, suggest等）: 変更なし
- output/human.rs, json.rs, llm.rs, path.rs: **変更なし**（インライン実装のため）
- Tantivyインデックス: 使用しない
- Cargo.toml: 新規依存追加なし（既存 clap / rusqlite / serde_json / serde の範囲）

## 10. テスト戦略

| テスト種別 | ファイル | 内容 |
|-----------|---------|------|
| 引数パース | `tests/cli_args.rs` | `issue 140` のパース、helpテスト更新 |
| help出力回帰 | `tests/cli_args.rs` | help出力に "issue" 含まれること |
| help-llm回帰 | `tests/cli_args.rs` | help-llm JSON出力にissue含まれること、コマンド総数15に更新 |
| E2E human | `tests/e2e_issue.rs` | SymbolStore投入→CLI実行→human出力検証 |
| E2E json | `tests/e2e_issue.rs` | JSON出力形式の検証 |
| E2E llm | `tests/e2e_issue.rs` | Markdown出力形式の検証 |
| E2E path | `tests/e2e_issue.rs` | パス一覧出力の検証 |
| E2E not found | `tests/e2e_issue.rs` | 存在しないIssue番号のエラーメッセージ検証 |
| E2E category | `tests/e2e_issue.rs` | progress_reportが進捗レポートに分類されること |

テストデータは`tests/common`のcargo_bin + 一時ディレクトリパターンに従い、SymbolStoreにテストデータを直接投入する。

## 11. 設計判断とトレードオフ

| 判断 | 選択 | 理由 | 代替案 |
|------|------|------|--------|
| DTO型のフィールド型 | 既存enum（KnowledgeRelation/DocSubtype） | DRY・型安全性 | String型（型安全性が後退） |
| metadataパース位置 | Indexer層（SymbolStore） | 責務分離、Output層の単純化 | Output層でパース（責務が漏れる） |
| ソートロジック位置 | Rust側 | DRY（json_extractとRustの二重管理を回避）、メンテナンス性 | SQL側json_extract（二箇所で同じロジック） |
| 出力フォーマッタ配置 | cli/issue.rs 内インライン | KISS（suggestパターン準拠、4ファイル分散は過剰） | output/*.rs に分散（変更コスト高） |
| 結果型配置 | cli/issue.rs | SRP（output/mod.rsの凝集度維持） | output/mod.rs（14型+9関数に追加は過負荷） |
| 結果0件の扱い | NotFoundエラー（exit 1） | CLI契約の明確化、パイプライン利用時の区別 | 正常系（exit 0で曖昧） |
| 出力カテゴリ分類 | cli/issue.rs 内の display_label() ヘルパー | SRP（Indexer層にUI責務を持たせない）、テストと実装が同期 | DocSubtype::display_label()（層の責務が滲む） |
| metadata破損の扱い | CorruptedMetadataエラー | 障害の隠蔽防止、デバッグ性 | silent skip（破損を検知不可） |
| CLIディスパッチ | index_pathを渡してcli内で解決 | Suggestパターン準拠 | main.rsで解決（パターン不統一） |
| Issue番号の型 | u64（CLI）→ String（DB問い合わせ時） | clapバリデーション活用 + identifierがTEXT型 | String直接（0やマイナス値が通る） |
| Issueタイトル表示 | MVP: `Issue #N` のみ | knowledge_nodesにtitleが未格納 | GitHub API呼び出し（ネットワーク依存増） |
| 変更ファイル表示 | スコープ外 | ナレッジグラフにfile typeノードが未実装 | git log解析（複雑度増） |

## 12. 品質基準

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |
