# 設計方針書: Issue #141 `commandindexdev why <file>` コマンドの実装

## 1. 概要

ファイルパスを指定して、そのファイルの設計根拠となるドキュメント（設計ポリシー、レビュー指摘、作業計画）をナレッジグラフから走査して返すCLIサブコマンドを追加する。

## 2. レイヤー構成と責務

| レイヤー | モジュール | 責務 |
|---------|-----------|------|
| **CLI** | `src/main.rs` | `Commands::Why` バリアント定義、dispatch |
| **CLI** | `src/cli/why.rs` | whyコマンドのロジック（入力検証→DB接続→クエリ→出力） |
| **Indexer** | `src/indexer/symbol_store.rs` | `find_knowledge_related()` 拡張（title追加） |
| **Output** | `src/output/mod.rs` | `WhyResult` / `WhyIssueEntry` / `WhyDocumentEntry` 型定義 |
| **Output** | `src/output/{human,json,llm,path}.rs` | 4フォーマットの出力関数 |
| **Help** | `src/cli/help_llm.rs` | `why` コマンドのLLM向けヘルプエントリ |

## 3. 設計判断とトレードオフ

### 判断1: 既存 `find_knowledge_related` の拡張

**決定**: 既存の `find_knowledge_related()` を拡張し、`KnowledgeRelatedResult` に `title: Option<String>` フィールドを追加する。新規メソッドは作成しない。

**理由**:
- 4テーブル JOIN クエリの大部分は共通であり、差分は SELECT 句に title カラムを含むかどうかだけ
- DRY原則に従い、SQLクエリの重複を避ける
- 既存の呼び出し元（related.rs）は title フィールドを無視すればよく、破壊的変更にならない
- `find_knowledge_related()` のSQLに `kn_issue.title` を追加するのみ

### 判断2: 結果のグルーピング方式

**決定**: Issue単位でグルーピングし、各Issue配下にdocumentを配置する。

**理由**:
- ユーザーが「このファイルはどのIssueに関連するか」を直感的に把握できる
- Issueが設計の意思決定単位であり、自然なグルーピング
- JSON出力でのネスト構造も明確

### 判断3: 対象ドキュメント範囲

**決定**: ナレッジグラフ上の全sibling documentを返す（フィルタなし）。

**理由**:
- progress-reportも設計根拠の一部として有用
- フィルタリングは将来のオプション（`--relation`）で追加可能
- 初回実装はシンプルに保つ（YAGNI）

### 判断4: 入力ファイルの制約

**決定**: 単一ファイルパス入力。documentノードとしてナレッジグラフに登録されているファイルのみ対象。

**理由**:
- `why` コマンドの主要ユースケースは「このドキュメントの背景を知りたい」
- ソースコードファイルは現在ナレッジグラフに未登録（将来の拡張余地）
- 複数ファイル対応は将来のエンハンスメント

## 4. データフロー

```
CLI入力: commandindexdev why <file> [--format human]
    ↓
1. validate_file_paths([file], max_files=1)         # パス文字列検証
    ↓
2. resolve_index_path(index_path, config)            # インデックスパス解決（既存パターン）
    ↓
3. symbol_db_path(&index_dir) → SymbolStore::open()  # DB接続確認
    ↓
4. SymbolStore::find_knowledge_related(file_path)    # グラフ走査（title含む）
    ↓
5. WhyResult構築（Issue別グルーピング）
    ↓
6. format_why_results(result, format, writer)        # 出力
```

## 5. 型設計

### 5.1 結果型（src/output/mod.rs）

```rust
#[derive(Debug, Clone, Serialize)]
pub struct WhyResult {
    pub input_file: String,
    pub issues: Vec<WhyIssueEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WhyIssueEntry {
    pub identifier: String,
    pub title: Option<String>,
    pub documents: Vec<WhyDocumentEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WhyDocumentEntry {
    pub relation: String,
    pub file_path: String,
}
```

### 5.2 relation表示名変換

relation→日本語表示名の変換は `output/human.rs` のフォーマッタ内にプライベート関数として配置する（SRP準拠）。データ型には表示ロジックを持たせない。

```rust
// output/human.rs 内
fn relation_display_label(relation: &str) -> &str {
    match relation {
        "has_design" => "設計",
        "has_review" => "レビュー",
        "has_workplan" => "作業計画",
        other => other,
    }
}
```

### 5.3 エラー型（src/cli/why.rs）

ImpactError パターンに準拠。`Display` と `From` trait を実装。
SearchError は直接含めず、`InvalidArgument(String)` で吸収する。

```rust
#[derive(Debug)]
pub enum WhyError {
    IndexNotFound,
    SymbolDbNotFound,
    SymbolStore(SymbolStoreError),
    Output(OutputError),
    InvalidArgument(String),
}

impl From<SymbolStoreError> for WhyError { ... }
impl From<OutputError> for WhyError { ... }
```

validate_file_paths の呼び出し:
```rust
validate_file_paths(&files, 1)
    .map_err(|e| WhyError::InvalidArgument(e.to_string()))?;
```

## 6. SQLクエリ設計

### find_knowledge_related 拡張

```sql
SELECT
    kn_issue.identifier,
    kn_issue.title,
    ke2.relation,
    kn_sibling.file_path
FROM knowledge_nodes kn_doc
JOIN knowledge_edges ke1 ON ke1.target_id = kn_doc.id
JOIN knowledge_nodes kn_issue ON ke1.source_id = kn_issue.id AND kn_issue.type = 'issue'
JOIN knowledge_edges ke2 ON ke2.source_id = kn_issue.id
JOIN knowledge_nodes kn_sibling ON ke2.target_id = kn_sibling.id AND kn_sibling.type = 'document'
WHERE kn_doc.file_path = ?1
ORDER BY kn_issue.identifier, ke2.relation;
```

**インデックス活用**: 既存の `idx_kn_type`, `idx_ke_source`, `idx_ke_target` を活用。追加インデックスは不要。

## 7. CLIインターフェース

### コマンド定義（src/main.rs Commands enum）

Impact パターンに合わせて `files: Vec<String>` で受け取り、`validate_file_paths(&files, 1)` で単一ファイル制限を適用する。

```rust
/// Show design rationale for a file from the knowledge graph
#[command(after_help = why::WHY_AFTER_HELP)]
Why {
    /// Target file paths
    files: Vec<String>,

    /// Output format
    #[arg(short, long, default_value = "human")]
    format: OutputFormat,
},
```

**注**: `--index-path` はグローバルオプション `Cli::index_path` を使用する（既存CLIとの一貫性）。dispatch時は `cli.index_path.as_deref()` で参照。

### help-llm エントリ

```rust
CommandInfo {
    name: "why",
    description: "Show design rationale for a file (related issues and documents from knowledge graph)",
    examples: vec![
        "commandindexdev why dev-reports/design/issue-100-design-policy.md",
        "commandindexdev why dev-reports/issue/100/work-plan.md --format json",
    ],
    key_options: Some(vec!["--format <human|json|path|llm>"]),
}
```

## 8. 出力フォーマット設計

### Human形式
```
dev-reports/design/issue-299-design-policy.md の設計根拠:

Issue #299: iPad/スマホ レイアウト崩れ修正
  設計: dev-reports/design/issue-299-ipad-layout-fix-design-policy.md
  レビュー: dev-reports/issue/299/multi-stage-design-review/summary-report.md
  作業計画: dev-reports/issue/299/work-plan.md
```

### JSON形式
Issueで定義済みの単一JSONオブジェクト構造を使用（JSONL形式ではない）。

### Path形式
関連documentのファイルパスを1行1パスで出力（入力ファイル自身を含む。全フォーマット共通の出力契約）。

### LLM形式
```
file: <path>
issue: #<id> <title>
  <relation>: <path>
```

## 9. セキュリティ設計

| 脅威 | 対策 | 優先度 |
|------|------|--------|
| パストラバーサル | `validate_file_paths` による文字列検証（`..`, 絶対パス, バックスラッシュ拒否） | 高 |
| SQLインジェクション | rusqliteのパラメータバインディング（`?1`） | 高 |
| ターミナルインジェクション | DB由来の title/file_path に `strip_control_chars` 相当を適用（human/llm形式）。JSON/path形式は生データ出力 | 高 |
| unsafe使用 | 使用しない | 中 |

## 10. 影響範囲

### 変更ファイル

| ファイル | 変更種別 | 影響度 |
|---|---|---|
| `src/main.rs` | 既存変更（Commands enum + dispatch） | 低 |
| `src/cli/mod.rs` | 既存変更（`pub mod why;`追加） | 低 |
| `src/cli/why.rs` | **新規作成** | - |
| `src/indexer/symbol_store.rs` | 既存変更（`find_knowledge_related` にtitle追加） | 低 |
| `src/indexer/knowledge.rs` | 既存変更（`KnowledgeRelatedResult` にtitle追加） | 低 |
| `src/output/mod.rs` | 既存変更（WhyResult型 + format関数追加） | 低 |
| `src/output/human.rs` | 既存変更（`format_why_human`追加） | 低 |
| `src/output/json.rs` | 既存変更（`format_why_json`追加） | 低 |
| `src/output/llm.rs` | 既存変更（`format_why_llm`追加） | 低 |
| `src/output/path.rs` | 既存変更（`format_why_path`追加） | 低 |
| `src/cli/help_llm.rs` | 既存変更（whyエントリ追加） | 低 |
| `tests/cli_args.rs` | 既存変更（テスト追加 + 件数更新） | 低 |

### 既存機能への影響
- 純粋な追加変更。既存コマンドの挙動は一切変更しない
- `src/output/mod.rs` は共通ハブだが、型追加のみでシグネチャ変更なし
- 新規外部crateの追加なし（clap, serde_json, rusqlite 既存利用で完結）

## 11. テスト設計

| テスト種別 | 対象 | ファイル |
|---|---|---|
| 単体テスト | `find_knowledge_related` SQLクエリ（title含む、in-memory SQLite） | `src/indexer/symbol_store.rs` |
| 単体テスト | 空グラフ・未登録パスのケース | `src/indexer/symbol_store.rs` |
| 統合テスト | CLIパース（`why` + `--format`） | `tests/cli_args.rs` |
| 統合テスト | `--help` に `why` が表示される | `tests/cli_args.rs` |
| 統合テスト | `help-llm` 出力に `why` が含まれる（件数14→15更新） | `tests/cli_args.rs` |

## 12. 品質基準

| チェック項目 | コマンド | 基準 |
|---|---|---|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |
