# 設計方針書: Issue #89 stdin パイプ入力対応

## 1. Issue概要

| 項目 | 内容 |
|------|------|
| Issue番号 | #89 |
| タイトル | [Feature] stdin パイプ入力対応 |
| ラベル | enhancement |
| 実装規模 | 中 |
| 目的 | stdin からファイルパスリストを受け取り、変更影響範囲分析や関連ファイル検索の入力として使用 |

## 2. システムアーキテクチャ概要

```
stdin / args
    │
    ▼
┌─────────────────────────────────────────────────┐
│  CLI Layer (src/main.rs + src/cli/)             │
│  ┌──────────┐ ┌──────────┐ ┌──────────────────┐│
│  │ impact   │ │ search   │ │ stdin (共通)     ││
│  │ (新規)   │ │ (拡張)   │ │ (新規)           ││
│  └────┬─────┘ └────┬─────┘ └────────┬─────────┘│
│       │            │                │           │
│       └────────────┼────────────────┘           │
└────────────────────┼────────────────────────────┘
                     ▼
┌─────────────────────────────────────────────────┐
│  Search Layer (src/search/)                     │
│  ┌──────────────────────────────┐               │
│  │ RelatedSearchEngine          │               │
│  │ ::find_related() (既存再利用)│               │
│  └──────────────────────────────┘               │
└─────────────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────┐
│  Output Layer (src/output/)                     │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐        │
│  │ human.rs │ │ json.rs  │ │ path.rs  │        │
│  │ (拡張)   │ │ (拡張)   │ │ (拡張)   │        │
│  └──────────┘ └──────────┘ └──────────┘        │
└─────────────────────────────────────────────────┘
```

## 3. レイヤー構成と責務

| レイヤー | モジュール | 責務 | 変更種別 |
|---------|-----------|------|---------|
| **CLI** | `src/main.rs` | Commands enum に Impact 追加、Search に --related-stdin 追加 | 変更 |
| **CLI** | `src/cli/impact.rs` | impact サブコマンドのロジック | **新規** |
| **CLI** | `src/cli/stdin.rs` | stdin 読み取り・バリデーション共通ユーティリティ | **新規** |
| **CLI** | `src/cli/search.rs` | SearchError に stdin 関連バリアント追加、related-stdin ハンドラ | 変更 |
| **Search** | `src/search/related.rs` | RelatedSearchEngine（変更なし、再利用のみ） | 変更なし |
| **Output** | `src/output/mod.rs` | ImpactResult, ImpactFileResult 型追加 | 変更 |
| **Output** | `src/output/json.rs` | format_impact_json() 追加 | 変更 |
| **Output** | `src/output/human.rs` | format_impact_human() 追加 | 変更 |
| **Output** | `src/output/path.rs` | format_impact_path() 追加 | 変更 |

## 4. 技術選定

| カテゴリ | 選定技術 | 選定理由 |
|---------|---------|---------|
| TTY検出 | `std::io::IsTerminal` | Rust 1.70+ 標準ライブラリ。外部 crate 不要 |
| stdin読み取り | `std::io::BufRead` | 1行ずつ効率的に読み取り |
| パス正規化 | 自前実装 (cli/stdin.rs) | `./` 除去等の軽量な正規化。context.rs と同じ制約 |
| テスト | `assert_cmd::Command::write_stdin()` | E2E でパイプ入力をシミュレート |

## 5. 設計パターン

### 5.1 stdin 共通ユーティリティ（cli/stdin.rs）

**注**: パスバリデーション関数 `validate_file_path` は context.rs からも呼び出す共通関数とする（DRY原則）。context.rs の既存バリデーションロジックはこの関数に統一する。

```rust
use std::io::{self, BufRead, IsTerminal, Read};

/// stdin 入力全体のバイト数上限（巨大入力によるメモリ枯渇対策）
const MAX_STDIN_BYTES: u64 = 512 * 1024; // 512KB

/// stdin 入力ファイルパス数のデフォルト上限
pub const DEFAULT_MAX_STDIN_PATHS: usize = 500;

/// stdin エラー型
#[derive(Debug)]
pub enum StdinError {
    NotPiped,
    ReadError(io::Error),
    EmptyInput,
    NoValidPaths,
    TooManyPaths { count: usize, max: usize },
    InvalidPath { path: String, reason: String },
}

impl std::fmt::Display for StdinError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotPiped => write!(f, "stdin is a terminal. Pipe input required. Example: git diff --name-only | commandindex impact"),
            Self::ReadError(e) => write!(f, "Failed to read from stdin: {e}"),
            Self::EmptyInput => write!(f, "no input received from stdin"),
            Self::NoValidPaths => write!(f, "no valid file paths found in input"),
            Self::TooManyPaths { count, max } => write!(f, "too many paths ({count}), maximum is {max}"),
            Self::InvalidPath { path, reason } => write!(f, "invalid path '{}': {reason}", &path[..path.len().min(100)]),
        }
    }
}

impl std::error::Error for StdinError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ReadError(e) => Some(e),
            _ => None,
        }
    }
}

/// stdin からファイルパスリストを読み取る
pub fn read_file_paths_from_stdin(max_paths: usize) -> Result<Vec<String>, StdinError> {
    let stdin = io::stdin();
    if stdin.is_terminal() {
        return Err(StdinError::NotPiped);
    }

    let mut paths = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // バイト数上限でラップし、巨大入力によるメモリ枯渇を防止
    let reader = stdin.lock().take(MAX_STDIN_BYTES);
    for line in io::BufReader::new(reader).lines() {
        let line = line.map_err(StdinError::ReadError)?;
        let trimmed = line.trim().to_string();

        // 空行スキップ
        if trimmed.is_empty() { continue; }

        // パスバリデーション（warning出力してスキップ）
        if let Err(e) = validate_file_path(&trimmed) {
            eprintln!("Warning: {e}");
            continue;
        }

        // 正規化（./ 除去）- strip_prefix を使用
        let normalized = normalize_path_prefix(&trimmed);

        // 重複排除（正規化後で比較）
        if !seen.insert(normalized.clone()) { continue; }

        paths.push(normalized);

        if paths.len() > max_paths {
            return Err(StdinError::TooManyPaths { count: paths.len(), max: max_paths });
        }
    }

    if paths.is_empty() {
        return Err(StdinError::EmptyInput);
    }

    Ok(paths)
}

/// パスバリデーション（context.rs と共通。pub(crate) で context からも呼び出し可能）
pub(crate) fn validate_file_path(path: &str) -> Result<(), StdinError> {
    if path.is_empty() { return Err(StdinError::InvalidPath { path: path.to_string(), reason: "empty path".to_string() }); }
    if path.len() > 1024 { return Err(StdinError::InvalidPath { path: path.to_string(), reason: "path too long (max 1024)".to_string() }); }
    if path.contains('\0') { return Err(StdinError::InvalidPath { path: path.to_string(), reason: "path contains null byte".to_string() }); }
    if path.contains("..") { return Err(StdinError::InvalidPath { path: path.to_string(), reason: "path must not contain '..'".to_string() }); }
    if path.starts_with('/') || path.starts_with('\\') { return Err(StdinError::InvalidPath { path: path.to_string(), reason: "absolute path not allowed".to_string() }); }
    if path.contains('\\') { return Err(StdinError::InvalidPath { path: path.to_string(), reason: "backslash not allowed".to_string() }); }
    Ok(())
}

/// パス正規化（./ 除去）- strip_prefix を使用（バイトスライス回避）
fn normalize_path_prefix(path: &str) -> String {
    path.strip_prefix("./").unwrap_or(path).to_string()
}

/// 存在チェック + warning（impact / search --related-stdin 共通）
pub(crate) fn filter_existing_files(paths: &[String]) -> (Vec<String>, Vec<String>) {
    let mut valid = Vec::new();
    let mut warnings = Vec::new();
    for p in paths {
        if std::path::Path::new(p).exists() {
            valid.push(p.clone());
        } else {
            warnings.push(format!("file not found, skipping: {p}"));
        }
    }
    (valid, warnings)
}
```

### 5.2 impact サブコマンド（cli/impact.rs）

```rust
use crate::cli::stdin::{read_file_paths_from_stdin, StdinError};
use crate::output::{ImpactResult, ImpactFileResult, OutputFormat};
use crate::search::related::RelatedSearchEngine;

const MAX_INPUT_FILES: usize = 500;

/// impact エラー型
/// 注: Context サブコマンドは SearchError を再利用しているが、Impact は
/// impacted_by 等の独自ロジックを持つため専用エラー型を使用する。
/// Display + Error trait を実装し、main.rs で表示可能にする。
#[derive(Debug)]
pub enum ImpactError {
    Stdin(StdinError),
    IndexNotFound,
    SymbolDbNotFound,
    Reader(ReaderError),
    SymbolStore(SymbolStoreError),
    RelatedSearch(RelatedSearchError),
    Output(OutputError),
    NoValidPaths,
}

// impl Display for ImpactError { ... }  // 各バリアントのユーザー向けメッセージ
// impl std::error::Error for ImpactError { ... }

/// impact サブコマンド実行
pub fn run_impact(
    files: &[String],  // CLI引数からのファイルリスト
    format: OutputFormat,
    limit: Option<usize>,
) -> Result<(), ImpactError> {
    // 1. ファイルリスト取得（引数優先、なければstdin）
    let input_files = if files.is_empty() {
        read_file_paths_from_stdin(MAX_INPUT_FILES)?
    } else {
        // 引数のバリデーション
        validate_and_normalize(files)?
    };

    // 2. 存在チェック（warning出力してスキップ）
    let (valid_files, warnings) = filter_existing_files(&input_files);
    for w in &warnings {
        eprintln!("Warning: {}", w);
    }
    if valid_files.is_empty() {
        return Err(ImpactError::NoValidPaths);
    }

    // 3. インデックス・DB確認
    let tantivy_dir = crate::indexer::index_dir(Path::new("."));
    let db_path = crate::indexer::symbol_db_path(Path::new("."));
    // ...

    // 4. 各ファイルの関連ファイル検索 & 集約
    let engine = RelatedSearchEngine::new(&reader, &store);
    let effective_limit = limit.unwrap_or(20);
    let result = aggregate_impact(&engine, &valid_files, effective_limit)?;

    // 5. 出力
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    output::format_impact_results(&result, format, &mut handle)?;
    Ok(())
}

/// 複数ファイルの関連結果を集約
fn aggregate_impact(
    engine: &RelatedSearchEngine,
    files: &[String],
    limit: usize,
) -> Result<ImpactResult, ImpactError> {
    let mut scores: HashMap<String, (f32, HashSet<String>, Vec<String>)> = HashMap::new();
    // key: file_path, value: (max_score, relation_types, impacted_by)

    for file in files {
        match engine.find_related(file, 1000) {
            Ok(results) => {
                for r in results {
                    let entry = scores.entry(r.file_path.clone())
                        .or_insert_with(|| (0.0, HashSet::new(), Vec::new()));
                    // 最大スコア採用
                    if r.score > entry.0 { entry.0 = r.score; }
                    // relation_types union
                    for rt in &r.relation_types {
                        entry.1.insert(relation_type_to_string(rt));
                    }
                    // impacted_by 追加
                    if !entry.2.contains(file) {
                        entry.2.push(file.clone());
                    }
                }
            }
            Err(RelatedSearchError::FileNotFound(_))
            | Err(RelatedSearchError::FileNotIndexed(_)) => {
                // スキップ（warningは上で出力済み）
            }
            Err(e) => return Err(ImpactError::RelatedSearch(e)),
        }
    }

    // 入力ファイル自体を除外
    for file in files {
        scores.remove(file);
    }

    // スコア降順ソート & トリム
    let mut impacted: Vec<ImpactFileResult> = scores.into_iter()
        .map(|(path, (score, types, by))| ImpactFileResult {
            file_path: path,
            score,
            relation_types: types.into_iter().collect(),
            impacted_by: by,
        })
        .collect();
    impacted.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    impacted.truncate(limit);

    Ok(ImpactResult {
        input_files: files.to_vec(),
        impacted_files: impacted,
        total_input_files: files.len(),
        total_impacted_files: impacted.len(),
    })
}
```

### 5.3 search --related-stdin ハンドラ（cli/search.rs 拡張）

```rust
/// stdin からの複数ファイル関連検索
pub fn run_related_search_from_stdin(
    limit: usize,
    format: OutputFormat,
) -> Result<(), SearchError> {
    let files = read_file_paths_from_stdin(500)
        .map_err(|e| SearchError::Stdin(e))?;

    // 存在チェック + warning
    let (valid_files, warnings) = filter_existing_files(&files);
    for w in &warnings { eprintln!("Warning: {}", w); }
    if valid_files.is_empty() {
        return Err(SearchError::Stdin(StdinError::NoValidPaths));
    }

    // インデックス確認
    let tantivy_dir = crate::indexer::index_dir(Path::new("."));
    let db_path = crate::indexer::symbol_db_path(Path::new("."));
    // ...

    // 集約（context.rs の merge_related_results と同じロジック）
    let engine = RelatedSearchEngine::new(&reader, &store);
    let results = collect_and_merge_related(&engine, &valid_files, limit)?;

    // 既存の format_related_results で出力（impacted_by なし）
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    output::format_related_results(&results, format, &mut handle)?;
    Ok(())
}
```

### 5.4 出力型定義（output/mod.rs 拡張）

```rust
/// impact サブコマンドの結果
#[derive(Debug, Clone, serde::Serialize)]
pub struct ImpactResult {
    pub input_files: Vec<String>,
    pub impacted_files: Vec<ImpactFileResult>,
    pub total_input_files: usize,
    pub total_impacted_files: usize,
}

/// impact の個別ファイル結果
#[derive(Debug, Clone, serde::Serialize)]
pub struct ImpactFileResult {
    pub file_path: String,
    pub score: f32,
    pub relation_types: Vec<String>,  // snake_case: "import_dependency" 等
    pub impacted_by: Vec<String>,
}
```

### 5.5 main.rs の clap 定義拡張

```rust
/// Impact サブコマンド定義
#[command(about = "Analyze impact of file changes")]
Impact {
    /// Input files (if not provided, reads from stdin)
    #[arg()]
    files: Vec<String>,

    /// Output format（既存パターンに合わせ value_enum + default_value_t を使用）
    #[arg(long, value_enum, default_value_t = commandindex::output::OutputFormat::Human)]
    format: OutputFormat,

    /// Maximum number of impacted files to show（既存 Search の limit と同じパターン）
    #[arg(long)]
    limit: Option<usize>,
},

// Search に --related-stdin 追加
Search {
    // ... 既存フィールド ...

    /// Read related file paths from stdin (one per line)
    /// 注: clap の conflicts_with は双方向に効くため、--related 側への追加は不要
    #[arg(long, conflicts_with_all = ["query", "symbol", "related", "semantic", "tag", "path", "file_type", "heading", "workspace", "no_semantic", "rerank"])]
    related_stdin: bool,
}
```

### 5.6 main.rs の match 分岐拡張

```rust
Commands::Search { query, symbol, related, related_stdin, semantic, format, ... } => {
    // related_stdin を先にチェック
    if related_stdin {
        commandindex::cli::search::run_related_search_from_stdin(effective_limit, format)?;
    } else {
        match (query, symbol, related, semantic) {
            // ... 既存の4分岐 ...
            // (None, None, None, None) のエラーメッセージに --related-stdin を追記:
            // "Either <QUERY>, --symbol <NAME>, --related <FILE>, --related-stdin, or --semantic <QUERY> is required"
        }
    }
}

Commands::Impact { files, format, limit } => {
    commandindex::cli::impact::run_impact(&files, format, limit)?;
}
```

## 6. セキュリティ設計

| 脅威 | 対策 | 優先度 |
|------|------|--------|
| パストラバーサル | `..` を含むパスを禁止、絶対パスを禁止 | 高 |
| 大量入力によるDoS | 入力ファイル数上限500件 + stdin バイト数上限 512KB | 高 |
| 巨大行入力によるメモリ枯渇 | `stdin.lock().take(MAX_STDIN_BYTES)` でバイト数上限 | 高 |
| バックスラッシュ注入 | バックスラッシュを含むパスを禁止 | 中 |
| パス長攻撃 | 1024文字上限 | 中 |
| null バイト注入 | `\0` を含むパスを禁止 | 中 |
| unsafe使用 | 原則禁止 | 中 |

## 7. 設計判断とトレードオフ

### 判断1: impact を「逆依存分析」ではなく「関連ファイル集約」として実装

**選択**: RelatedSearchEngine の双方向集計をそのまま利用
**理由**: 既存エンジンの再利用でコスト最小。純粋な逆依存のみが必要な場合は将来 `--reverse-only` オプションで拡張
**トレードオフ**: impact が context と出力形式以外で差別化しにくい

### 判断2: --related-stdin を独立フラグとして追加

**選択**: `--related-stdin` (bool フラグ) を `--related` と相互排他で追加
**代替案**: `--related -` (ハイフンで stdin を示す Unix 慣例), `--related-from stdin`
**理由**: clap の既存排他モデル（conflicts_with_all）を最小限の変更で拡張可能。bool フラグは値パースが不要でシンプル
**トレードオフ**: オプション名がやや冗長

### 判断3: stdin ユーティリティを cli/stdin.rs に独立配置

**選択**: impact と search --related-stdin の両方から呼び出せる共通モジュール
**代替案**: 各 CLI モジュール内で個別実装
**理由**: DRY 原則。TTY 検出、パスバリデーション、正規化ロジックの一元管理
**トレードオフ**: モジュール1つ増加

### 判断4: search --related-stdin は既存 related 出力互換を維持

**選択**: impacted_by フィールドを含まず、既存の RelatedSearchResult で出力
**理由**: 既存パイプラインとの互換性。`impact` コマンドで詳細情報が必要な場合に使い分け
**トレードオフ**: search --related-stdin ではどのファイルから関連付けられたか不明

### 判断5: 集約ロジックの配置

**選択**: impact の集約ロジックは cli/impact.rs 内に配置。search --related-stdin は context.rs の merge_related_results を pub(crate) にして再利用
**理由**: impact は impacted_by を持つ独自の集約が必要。search --related-stdin は context と同じ union ロジックで十分
**トレードオフ**: 集約ロジックが2箇所に存在するが、用途が異なるため許容

## 8. 影響範囲

### 変更ファイル一覧

| ファイル | 変更種別 | 変更内容 |
|---------|---------|---------|
| `src/main.rs` | 変更 | Impact バリアント追加、Search に related_stdin 追加、match 分岐拡張 |
| `src/cli/mod.rs` | 変更 | `pub mod impact;` `pub mod stdin;` 追加 |
| `src/cli/impact.rs` | **新規** | impact サブコマンドロジック（run_impact, aggregate_impact） |
| `src/cli/stdin.rs` | **新規** | stdin 共通ユーティリティ（read_file_paths_from_stdin, validate_file_path） |
| `src/cli/search.rs` | 変更 | SearchError に Stdin バリアント追加、run_related_search_from_stdin 追加 |
| `src/output/mod.rs` | 変更 | ImpactResult, ImpactFileResult 型追加、format_impact_results 追加 |
| `src/output/json.rs` | 変更 | format_impact_json() 追加 |
| `src/output/human.rs` | 変更 | format_impact_human() 追加 |
| `src/output/path.rs` | 変更 | format_impact_path() 追加 |
| `tests/cli_args.rs` | 変更 | help テストに impact 追加、排他テスト追加 |
| `tests/e2e_impact.rs` | **新規** | impact E2E テスト（.write_stdin() 使用） |
| `tests/e2e_related_search.rs` | 変更 | --related-stdin テスト追加 |

| `src/cli/context.rs` | 変更 | merge_related_results を pub(crate) に変更、validate_file_path を stdin.rs の共通関数に置き換え |

### 変更なしのファイル

| ファイル | 理由 |
|---------|------|
| `src/search/related.rs` | 既存 API をそのまま再利用 |
| `Cargo.toml` | 外部依存追加不要 |

## 9. 品質基準

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |

### テスト計画

| テスト種別 | ファイル | カバー範囲 |
|-----------|---------|-----------|
| CLI排他テスト | `tests/cli_args.rs` | help_flag_shows_usage に impact 追加、--related と --related-stdin の排他、--related-stdin と --no-semantic/--rerank の排他 |
| impact E2E | `tests/e2e_impact.rs` | stdin 入力、引数入力、JSON/human/path 出力、TTYエラー、空stdin、有効パス0件、関連結果0件 |
| related-stdin E2E | `tests/e2e_related_search.rs` | stdin 入力、集約ルール（union + 最大スコア）、排他確認（--related-stdin と --tag 等） |
| stdin ユニットテスト | `src/cli/stdin.rs` (#[cfg(test)]) | バリデーション（null バイト, .., 絶対パス, バックスラッシュ）、正規化（strip_prefix）、エッジケース |
| 出力フォーマット | `tests/output_format.rs` | ImpactResult の JSON/human/path フォーマット検証（既存テストパターン踏襲） |
