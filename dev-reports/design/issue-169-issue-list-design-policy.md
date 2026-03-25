# 設計方針書: Issue #169 — issue listサブコマンドの追加

## 1. Issue概要

| 項目 | 内容 |
|------|------|
| Issue番号 | #169 |
| タイトル | issue listサブコマンドの追加 |
| 目的 | インデックス内の全Issue一覧を表示し、Issue番号を知らなくても俯瞰できるようにする |
| Breaking Change | `issue <number>` → `issue show <number>` |

## 2. システムアーキテクチャ概要

```
CLI Layer (main.rs / cli/)
  ├── issue list  ← 新規追加
  ├── issue show  ← issue <number> から移行
  ├── suggest     ← issue コマンド参照を更新
  └── help-llm   ← ガイダンス更新

Data Layer (indexer/)
  └── symbol_store.rs
      └── list_all_issues()  ← 新規追加

Output Layer (output/)
  └── OutputFormat (Human/Json/Path/Llm)  ← 既存活用
```

## 3. レイヤー構成と責務

| レイヤー | モジュール | 今回の変更 |
|---------|-----------|-----------|
| **CLI** | `src/main.rs` | Commands::Issue をサブコマンド構造に変更、IssueCommands enum 追加 |
| **CLI** | `src/cli/issue.rs` | `run_list()` 追加、`run()` → `run_show()` にリネーム（API対称性） |
| **Indexer** | `src/indexer/symbol_store.rs` | `list_all_issues()` メソッド追加 |
| **CLI** | `src/cli/help_llm.rs` | issue コマンドの説明・例を全箇所更新 |
| **CLI** | `src/cli/suggest.rs` | 生成コマンドを `issue show` に更新 |
| **Output** | `src/output/mod.rs` | 変更なし（既存 OutputFormat を再利用） |

## 4. 設計判断とトレードオフ

### 判断1: サブコマンド構造の採用

**選択**: ConfigCommands パターンに合わせた IssueCommands enum

```rust
#[derive(Subcommand)]
enum IssueCommands {
    /// List all issues in the knowledge graph
    List {
        #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
        format: OutputFormat,
    },
    /// Show documents related to an Issue
    Show {
        #[arg(value_parser = clap::value_parser!(u64).range(1..))]
        number: u64,
        #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
        format: OutputFormat,
    },
}
```

**理由**: clapのpositional argとsubcommandの共存は困難。ConfigCommandsで実績あるパターンを踏襲することで一貫性を保つ。

**トレードオフ**: `issue <number>` が breaking change になるが、`issue show <number>` の方がコマンド体系として明確。

### 判断2: 1クエリ集計

**選択**: LEFT JOIN + GROUP BY + 条件付きCOUNTで1クエリ

**理由**: N+1問題を回避し、パフォーマンスを確保。

**トレードオフ**: SQLが複雑になるが、Issue数×リレーション数の個別クエリよりも効率的。

### 判断3: label取得 — 設計書ファイル名のみ

**選択**: 設計書ファイル名から正規表現抽出、ない場合は空文字

**理由**: `knowledge_nodes.title` は現在のインデクシング経路（`scan_dev_reports()` → `insert_knowledge_entries()`）ではIssueノードに格納されない。存在しないデータに依存しない設計とする。

### 判断4: modifies リレーションの除外

**選択**: doc_count のカウント対象から modifies を除外

**理由**: modifies は file ノードを指し、ドキュメント（設計書・レビュー等）ではない。ドキュメント件数としてカウントすると意味が変わる。

## 5. データ構造設計

### IssueListEntry（新規）

### データ層 DTO（`src/indexer/symbol_store.rs`）

```rust
/// SymbolStore から返す最小限のrow DTO
pub struct IssueListRow {
    pub number: u64,
    pub doc_count: u32,
    pub design_file_path: Option<String>,  // 内部用、公開APIには含めない
    pub has_design: bool,
    pub has_review: bool,
    pub has_workplan: bool,
    pub has_progress: bool,
}
```

### CLI表示モデル（`src/cli/issue.rs`）

```rust
/// CLI出力用のIssue一覧エントリ
pub struct IssueListEntry {
    pub number: u64,
    pub doc_count: u32,
    pub label: String,         // design_file_path から抽出済み
    pub has_design: bool,
    pub has_review: bool,
    pub has_workplan: bool,
    pub has_progress: bool,
}
```

**責務分離**: `list_all_issues()` は `Vec<IssueListRow>` を返し、`cli/issue.rs` の `run_list()` 内で `IssueListRow` → `IssueListEntry` に変換（label抽出含む）。公開APIに実装詳細（design_file_path）を漏らさない。

**戻り値**: `Vec` を直接返す（YAGNI: ラッパー構造体は不要）。

## 6. SQLクエリ設計

```sql
SELECT
  kn_issue.identifier AS issue_number,
  COUNT(CASE WHEN ke.relation IN ('has_design','has_review','has_workplan','has_progress')
             AND kn_doc.type = 'document' THEN 1 END) AS doc_count,
  COUNT(CASE WHEN ke.relation = 'has_design' AND kn_doc.type = 'document' THEN 1 END) > 0 AS has_design,
  COUNT(CASE WHEN ke.relation = 'has_review' AND kn_doc.type = 'document' THEN 1 END) > 0 AS has_review,
  COUNT(CASE WHEN ke.relation = 'has_workplan' AND kn_doc.type = 'document' THEN 1 END) > 0 AS has_workplan,
  COUNT(CASE WHEN ke.relation = 'has_progress' AND kn_doc.type = 'document' THEN 1 END) > 0 AS has_progress,
  MAX(CASE WHEN ke.relation = 'has_design' AND kn_doc.type = 'document' THEN kn_doc.file_path END) AS design_file_path
FROM knowledge_nodes kn_issue
LEFT JOIN knowledge_edges ke ON ke.source_id = kn_issue.id
LEFT JOIN knowledge_nodes kn_doc ON ke.target_id = kn_doc.id AND kn_doc.type = 'document'
WHERE kn_issue.type = 'issue'
GROUP BY kn_issue.identifier
ORDER BY CAST(kn_issue.identifier AS INTEGER)
```

**インデックス利用**: `idx_kn_type` (knowledge_nodes.type) が WHERE 句で活用される。

## 7. label取得の設計

```rust
use regex::Regex;
use std::sync::LazyLock;

static DESIGN_LABEL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"issue-\d+-(.+)-design-policy\.md$").unwrap()
});

fn extract_label_from_design_path(path: &str) -> String {
    DESIGN_LABEL_RE
        .captures(path)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .unwrap_or_default()
}
```

label取得フロー:
1. `list_all_issues()` のSQLクエリ内で設計書パスも一括取得する（GROUP_CONCATまたはMAXで設計書パスを集約）
2. パスから正規表現でラベルを抽出
3. 設計書がない場合は空文字

**N+1回避**: 判断2の方針に沿い、Issue単位の追加クエリは発行しない。SQLクエリで設計書パスも一括取得する。

## 8. 出力フォーマット設計

### human format
```
Issue #47   (5 docs)  terminal-search
Issue #99   (5 docs)  markdown-editor-display-improvement
Issue #104  (7 docs)  ipad-fullscreen-bugfix
Total: 3 issues
```

### json format
```json
[
  {"number": 47, "doc_count": 5, "label": "terminal-search", "has_design": true, "has_review": true, "has_workplan": true, "has_progress": false},
  {"number": 99, "doc_count": 5, "label": "markdown-editor-display-improvement", "has_design": true, "has_review": false, "has_workplan": true, "has_progress": false}
]
```

### path format
```
47
99
104
```

### llm format
```markdown
| Issue | Docs | Label | Design | Review | Workplan | Progress |
|-------|------|-------|--------|--------|----------|----------|
| #47 | 5 | terminal-search | Yes | Yes | Yes | No |
| #99 | 5 | markdown-editor-display-improvement | Yes | No | Yes | No |

Total: 2 issues
```

### 0件時
- human: `No issues found.`
- json: `[]`
- path: （空出力）
- llm: `No issues found.`

### フォーマッタ命名規則
issue list のフォーマッタは `format_list_human()`, `format_list_json()` 等、`format_list_` プレフィックスを使用し、既存の issue show フォーマッタとの名前衝突を回避する。

## 9. DRY: 共通ヘルパー関数

DB存在チェックとSymbolStore::openのボイラープレートを共通化:

```rust
fn open_symbol_store(commandindex_dir: &Path) -> Result<SymbolStore, IssueCommandError> {
    let db_path = crate::indexer::symbol_db_path(commandindex_dir);
    if !db_path.exists() {
        return Err(IssueCommandError::SymbolStore(/* DB not found */));
    }
    SymbolStore::open(&db_path).map_err(IssueCommandError::SymbolStore)
}
```

`run_show()` と `run_list()` の両方からこのヘルパーを呼び出し、ボイラープレートの重複を排除する。

## 10. main.rs 変更設計

```rust
// Before
Commands::Issue { number, format } => { ... }

// After
/// Issue-related commands
Issue {
    #[command(subcommand)]
    command: IssueCommands,
},

// ディスパッチャー
Commands::Issue { command } => match command {
    IssueCommands::List { format } => {
        match commandindex::cli::issue::run_list(format, &commandindex_dir) {
            Ok(()) => 0,
            Err(e) => { eprintln!("Error: {e}"); 1 }
        }
    }
    IssueCommands::Show { number, format } => {
        match commandindex::cli::issue::run_show(number, format, &commandindex_dir) {
            Ok(()) => 0,
            Err(e) => { eprintln!("Error: {e}"); 1 }
        }
    }
}
```

## 10. 影響範囲

### 直接変更ファイル

| ファイル | 変更規模 | 変更内容 |
|---------|---------|---------|
| `src/cli/issue.rs` | 大 | `run_list()` 追加、4フォーマット関数（format_list_*） |
| `src/main.rs` | 中 | IssueCommands enum、ディスパッチャー変更 |
| `src/indexer/symbol_store.rs` | 中 | `list_all_issues()` メソッド + 単体テスト |
| `src/cli/help_llm.rs` | 中 | (1) build_use_cases() の `issue 140` → `issue show 140`, (2) build_workflows() の Investigation ワークフロー最終ステップ, (3) build_commands() の issue CommandInfo 全体（subcommands フィールド追加含む） |
| `src/cli/suggest.rs` | 小〜中 | `issue {num}` → `issue show {num}`（テスト3件も更新） |
| `tests/e2e_issue.rs` | 大 | 全6テスト `["issue", "N"]` → `["issue", "show", "N"]` 更新 + issue list テスト追加 |
| `tests/cli_args.rs` | 中 | 既存issue関連テスト一式をshow構文へ更新 + list パーステスト・旧構文エラーテスト追加 |

### 間接影響（確認のみ）
- `src/cli/before_change.rs`, `src/cli/why.rs`: 機能本体は影響なし、ヘルプ文脈の整合確認

## 11. セキュリティ設計

| 脅威 | 対策 | 優先度 |
|------|------|--------|
| 不正DBデータ | 不正identifier/壊れたknowledge graphデータの防御的処理、ログ出力時の制御文字除去 | 高 |
| パストラバーサル | label抽出は正規表現のみ、ファイルI/Oなし。`design_file_path` は表示専用 | 中 |
| unsafe使用 | 使用なし | 中 |
| CAST式の不正データ | Rust側で identifier の u64 パース検証、変換不可は警告ログ出力しスキップ（サイレントスキップ禁止） | 中 |
| エラーメッセージ漏洩 | IssueCommandError の Display でユーザー向けメッセージに変換 | 低 |
| 正規表現パニック | `unwrap()` を `expect("BUG: invalid regex pattern")` に変更 | 低 |

## 12. 品質基準

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |

## 13. テスト戦略

### 単体テスト（src/indexer/symbol_store.rs）
- `list_all_issues()` の基本動作
- modifies混在ケース（doc_countから除外確認）
- 設計書なしIssue（has_design=false）
- 0件ケース
- 非数値identifier混入ケース（警告ログ出力確認）

### 単体テスト（src/cli/issue.rs）
- `IssueListEntry` のフォーマッタ（human/json/path/llm）
- 0件時の出力
- label抽出の正規表現
- `IssueListRow` → `IssueListEntry` 変換

### E2Eテスト（tests/e2e_issue.rs）
- `issue list` 全フォーマット
- `issue show <number>` 既存動作確認
- `issue list` 0件ケース

### CLIテスト（tests/cli_args.rs）
- `issue --help` にlist/showが表示される
- `issue list --format json` パース確認
- 既存issue関連テスト（数値受理、0拒否、非数拒否等）をshow構文に更新
- 旧構文 `issue <number>` がエラーになることの確認

### 回帰テスト（help_llm / suggest）
- help_llm の出力に旧構文 `issue <number>` が含まれないこと
- suggest が `issue show <number>` を生成すること
