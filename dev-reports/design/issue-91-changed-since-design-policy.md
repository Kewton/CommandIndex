# 設計方針書: Issue #91 --changed-since オプション

## 1. Issue 概要

| 項目 | 内容 |
|------|------|
| Issue | #91 |
| タイトル | [Feature] --changed-since オプション（Git 履歴ベースの変更検索） |
| ラベル | enhancement |
| 依存 | #90 impact サブコマンド |
| 実装規模 | 中 |

## 2. 機能概要

指定期間内に変更されたファイルを Git 履歴から取得し、それらの関連情報（impact）を返す `search --changed-since` オプションを実装する。

```bash
commandindex search --changed-since "12 hours ago" --format json
commandindex search --changed-since "abc1234" --format json
```

## 3. レイヤー構成と責務

### 変更対象モジュール

| レイヤー | モジュール | 変更内容 | 変更量 |
|---------|-----------|---------|--------|
| **CLI** | `src/main.rs` | clap 定義に `--changed-since` 追加、if let 分岐追加 | 小 |
| **CLI** | `src/cli/search.rs` | `run_changed_since_search()` 新規関数追加、From 実装 | 中 |
| **CLI** | `src/cli/impact.rs` | `aggregate_impact()` を `pub(crate)` 化 | 微小 |
| **CLI** | `src/cli/git.rs` (新規) | Git 変更ファイル取得ロジック | 中 |
| **CLI** | `src/cli/mod.rs` | `pub mod git;` 追加 | 微小 |
| **Output** | 既存流用 | `format_impact_results()` をそのまま使用 | なし |
| **Test** | `tests/cli_args.rs` | 排他制御テスト追加 | 小 |
| **Test** | `tests/e2e_changed_since.rs` (新規) | E2E テスト | 中 |
| **Test** | `tests/common/mod.rs` | Git ヘルパー追加 | 小 |

### 変更不要モジュール

| モジュール | 理由 |
|-----------|------|
| `src/search/related.rs` | そのまま使用 |
| `src/output/` | `format_impact_results()` を再利用 |
| `src/indexer/` | 変更なし |
| `src/parser/` | 変更なし |

## 4. 詳細設計

### 4.1 CLI 引数定義（src/main.rs）

```rust
/// 指定期間内の変更ファイルの関連情報を検索
#[arg(
    long,
    conflicts_with_all = [
        "query", "symbol", "related", "related_stdin",
        "semantic", "workspace", "tag", "path",
        "file_type", "heading", "no_semantic", "rerank"
    ],
    help = "Show impact of files changed since (e.g. '12 hours ago', 'yesterday', or commit hash)"
)]
changed_since: Option<String>,
```

### 4.2 分岐ロジック（src/main.rs）

`related_stdin` と同様に、パターンマッチの前に `if let` で先行分岐。
**挿入位置**: `else if related_stdin { ... }` の直後、`match (query, symbol, ...)` の直前:

```rust
} else if let Some(ref since) = changed_since {
    let result = commandindex::cli::search::run_changed_since_search(
        since,
        effective_limit,
        format,
    );
    // エラーハンドリング（既存パターンに準拠）
```

### 4.3 Git 変更ファイル取得（src/cli/git.rs - 新規）

```rust
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use crate::cli::status::git_info::validate_commit_hash;

/// Git 操作に関するエラー
#[derive(Debug)]
pub enum GitError {
    GitNotFound,
    CommandFailed,  // stderr の内容は含めない（情報漏洩防止）
    InvalidInput(String),
}

/// git log 出力の最大読み取り行数
const MAX_GIT_OUTPUT_LINES: usize = 5000;

/// --changed-since の入力値をバリデーション
pub fn validate_changed_since_input(input: &str) -> Result<(), GitError> {
    if input.len() > 256 {
        return Err(GitError::InvalidInput("input too long (max 256 chars)".into()));
    }
    if input.bytes().any(|b| b < 0x20) {
        return Err(GitError::InvalidInput("control characters not allowed".into()));
    }
    if input.starts_with('-') {
        return Err(GitError::InvalidInput("input must not start with '-'".into()));
    }
    Ok(())
}

/// Git 履歴から変更ファイル一覧を取得
pub fn get_changed_files(
    repo_path: &Path,
    since: &str,
) -> Result<Vec<String>, GitError> {
    validate_changed_since_input(since)?;

    // validate_commit_hash() は crate::cli::status::git_info から再利用
    // 結果を1回だけ評価（DRY）
    let is_commit_hash = validate_commit_hash(since);
    let since_arg = if is_commit_hash {
        format!("{}..HEAD", since)
    } else {
        format!("--since={}", since)
    };

    let args: Vec<&str> = if is_commit_hash {
        vec!["log", "--name-only", "--format=", &since_arg]
    } else {
        vec!["log", &since_arg, "--name-only", "--format="]
    };

    let mut child = Command::new("git")
        .args(&args)
        .current_dir(repo_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())  // stderr は無視（情報漏洩防止）
        .spawn()
        .map_err(|_| GitError::GitNotFound)?;

    // BufReader で行単位読み取り、MAX_GIT_OUTPUT_LINES で制限
    let stdout = child.stdout.take().ok_or(GitError::CommandFailed)?;
    let reader = BufReader::new(stdout);
    let mut seen = std::collections::HashSet::new();
    let mut files = Vec::new();

    for line in reader.lines().take(MAX_GIT_OUTPUT_LINES) {
        let line = line.map_err(|_| GitError::CommandFailed)?;
        if !line.is_empty() && seen.insert(line.clone()) {
            files.push(line);
        }
    }

    let status = child.wait().map_err(|_| GitError::CommandFailed)?;
    if !status.success() {
        return Err(GitError::CommandFailed);
    }

    // ソートして決定的な順序を保証
    files.sort();

    Ok(files)
}
```

**設計ポイント**:
- `validate_commit_hash()` は `crate::cli::status::git_info` から再利用（DRY）
- `CommandFailed` に stderr 内容を含めない（情報漏洩防止、git_info.rs と同パターン）
- `BufReader` + `take(MAX_GIT_OUTPUT_LINES)` で大量出力時のメモリ保護
- `HashSet` + `sort()` で重複排除かつ決定的順序

### 4.4 検索メイン処理（src/cli/search.rs に追加）

```rust
use crate::cli::git::{self, GitError};
use crate::cli::impact::aggregate_impact;
use crate::cli::stdin::filter_existing_files;

pub fn run_changed_since_search(
    since: &str,
    limit: usize,
    format: OutputFormat,
) -> Result<(), SearchError> {
    // 1. Git 変更ファイル取得
    let repo_path = std::env::current_dir()
        .map_err(|e| SearchError::InvalidArgument(e.to_string()))?;
    let changed_files = git::get_changed_files(&repo_path, since)?;

    if changed_files.is_empty() {
        eprintln!("No files changed since: {}", since);
        return Ok(());
    }

    // 2. ファイル数上限チェック（impact.rs の MAX_INPUT_FILES 定数を共用）
    let files = if changed_files.len() > crate::cli::impact::MAX_INPUT_FILES {
        eprintln!(
            "Warning: {} files found, limiting to {}",
            changed_files.len(),
            crate::cli::impact::MAX_INPUT_FILES
        );
        changed_files[..crate::cli::impact::MAX_INPUT_FILES].to_vec()
    } else {
        changed_files
    };

    // 3. 存在チェック + フィルタ（impact.rs の validate_and_normalize パターン踏襲）
    let (valid_files, warnings) = filter_existing_files(&files);
    for warning in &warnings {
        eprintln!("{}", warning);
    }
    if valid_files.is_empty() {
        return Err(SearchError::InvalidArgument(
            "no valid file paths found".into()
        ));
    }

    // 4. インデックス・DB 確認（impact.rs の run_impact() と同パターン）
    //    index_dir(base_path: &Path) -> PathBuf, 戻り値は PathBuf（Result ではない）
    let index_path = crate::indexer::index_dir(Path::new("."));
    if !index_path.exists() {
        return Err(SearchError::IndexNotFound);
    }
    let reader = crate::indexer::reader::IndexReaderWrapper::open(&index_path)?;
    let symbol_db_path = index_path.join("symbols.db");
    if !symbol_db_path.exists() {
        return Err(SearchError::SymbolDbNotFound);
    }
    let store = crate::indexer::symbol_store::SymbolStore::open(&symbol_db_path)?;
    let engine = crate::search::related::RelatedSearchEngine::new(&reader, &store);

    // 5. aggregate_impact() で関連情報取得（impact.rs から共用）
    let result = aggregate_impact(&engine, &valid_files, limit)?;

    // 6. 出力
    crate::output::format_impact_results(&result, format, &mut std::io::stdout().lock())
        .map_err(SearchError::Output)
}
```

**設計ポイント**:
- `filter_existing_files()` の返り値 `(Vec<String>, Vec<String>)` タプルを正しく受ける
- インデックス・DB アクセスは `impact.rs` の `run_impact()` と同じ API パターン
  - `crate::indexer::index_dir()` / `IndexReaderWrapper::open()` / `SymbolStore::open()`
- エラーメッセージは英語で統一（既存パターンに準拠）

### 4.5 エラー型変換

```rust
// src/cli/search.rs に追加

impl From<ImpactError> for SearchError {
    fn from(e: ImpactError) -> Self {
        match e {
            // aggregate_impact() から実際に返りうるバリアント
            ImpactError::IndexNotFound => SearchError::IndexNotFound,
            ImpactError::SymbolDbNotFound => SearchError::SymbolDbNotFound,
            ImpactError::Reader(e) => SearchError::Reader(e),
            ImpactError::SymbolStore(e) => SearchError::SymbolStore(e),
            ImpactError::RelatedSearch(e) => SearchError::RelatedSearch(e),
            ImpactError::Output(e) => SearchError::Output(e),
            ImpactError::NoValidPaths => SearchError::InvalidArgument(
                "no valid file paths found".into()
            ),
            ImpactError::InvalidArgument(s) => SearchError::InvalidArgument(s),
            // 以下は aggregate_impact() からは到達しないが網羅性のため
            ImpactError::Stdin(e) => SearchError::Stdin(e),
        }
    }
}

impl From<GitError> for SearchError {
    fn from(e: GitError) -> Self {
        match e {
            GitError::GitNotFound => SearchError::InvalidArgument(
                "git command not found".into()
            ),
            GitError::CommandFailed => SearchError::InvalidArgument(
                "git command failed".into()
            ),
            GitError::InvalidInput(msg) => SearchError::InvalidArgument(msg),
        }
    }
}
```

### 4.6 impact.rs の変更

```rust
// fn aggregate_impact → pub(crate) fn aggregate_impact
pub(crate) fn aggregate_impact(
    engine: &RelatedSearchEngine,
    files: &[String],
    limit: usize,
) -> Result<ImpactResult, ImpactError>
```

## 5. セキュリティ設計

| 脅威 | 対策 | 優先度 |
|------|------|--------|
| コマンドインジェクション | `Command::new()` でシェル回避 | 高 |
| Git 引数インジェクション | 先頭 `-` 拒否、`--since=<value>` 単一引数形式 | 高 |
| 情報漏洩 | git stderr を無視、CommandFailed に詳細を含めない | 高 |
| パストラバーサル | `filter_existing_files()` で存在チェック + 正規化 | 高 |
| 制御文字注入 | 制御文字拒否バリデーション | 中 |
| 入力値DoS | 最大256文字制限 | 中 |
| git出力DoS | `BufReader` + `MAX_GIT_OUTPUT_LINES` (5000行) 制限 | 中 |

## 6. 設計判断とトレードオフ

### 判断1: search オプション vs 独立サブコマンド

**決定**: search サブコマンドのオプションとして追加

**理由**:
- Issue の仕様に準拠
- ユーザーにとって `search --changed-since` は直感的
- `--format` や `--limit` を search の既存オプションと共有可能

**トレードオフ**:
- search の conflicts_with_all が複雑化
- main.rs の分岐が増加

### 判断2: aggregate_impact() 共用 vs 独自集約ロジック

**決定**: aggregate_impact() を pub(crate) 化して共用

**理由**:
- 同一の処理フロー（ファイルリスト → related検索 → 集約 → 出力）
- ImpactResult 形式の出力フォーマッターを再利用可能
- コード重複回避（DRY）

**トレードオフ**:
- ImpactError → SearchError の変換が必要
- impact.rs の内部関数の可視性変更
- 将来的に context.rs の collect_related_context() との統合検討（別 Issue）

### 判断3: Git 操作ロジックの配置

**決定**: `src/cli/git.rs` を新規作成、`validate_commit_hash()` は既存を再利用

**理由**:
- git_info.rs は status サブコマンド専用（private 関数中心）
- --changed-since 固有のバリデーション・エラー型が必要
- `validate_commit_hash()` は pub なので再利用可能（DRY）

**トレードオフ**:
- git_info.rs の run_git() ヘルパーは private で再利用不可
- Command::new("git") パターンは十分シンプルで重複は許容範囲

## 7. 影響範囲

### 直接変更
| ファイル | 変更内容 |
|---------|---------|
| `src/main.rs` | clap 定義追加、if let 分岐追加、デストラクチャリングに `changed_since` 追加 |
| `src/cli/search.rs` | `run_changed_since_search()` 追加、`From<ImpactError>` / `From<GitError>` 実装追加 |
| `src/cli/impact.rs` | `aggregate_impact()` を `pub(crate)` 化 |
| `src/cli/git.rs` (新規) | Git 変更ファイル取得・バリデーション |
| `src/cli/mod.rs` | `pub mod git;` 追加 |

### テスト追加
| ファイル | 内容 |
|---------|------|
| `tests/cli_args.rs` | --changed-since 排他制御テスト（query/symbol/related/semantic/workspace） |
| `tests/e2e_changed_since.rs` (新規) | E2E テスト（git init + commit + 検索） |
| `tests/common/mod.rs` | `git_init_with_commit()` ヘルパー追加（CI 環境対応: `-c user.name/email`） |

### 影響なし
- `src/search/` - 変更なし（RelatedSearchEngine はそのまま使用）
- `src/output/` - 変更なし（format_impact_results はそのまま使用）
- `src/indexer/` - 変更なし
- `src/parser/` - 変更なし
- 既存テスト - 影響なし（新オプション追加のみ）

## 8. テスト設計方針

### CLI 引数テスト（cli_args.rs）
- `--changed-since` の受理テスト
- `--changed-since` と `--query` の排他テスト
- `--changed-since` と `--symbol` の排他テスト
- `--changed-since` と `--related` の排他テスト
- `--changed-since` と `--semantic` の排他テスト
- `--changed-since` と `--workspace` の排他テスト

### E2E テスト（e2e_changed_since.rs）

テスト環境: tempdir で git init → ファイル作成 → git add → git commit → index → search

```rust
fn git_init_with_commit(dir: &Path) -> String {
    // CI 環境対応: git -c user.name='test' -c user.email='test@test.com'
    // git init → ファイル作成 → git add → git commit
    // コミットハッシュを返す
}
```

テストケース:
1. 期間文字列（"1 hour ago"）での検索
2. コミットハッシュでの検索
3. 変更ファイルなしの場合のメッセージ
4. human/json/path 各出力形式
5. 先頭 `-` 拒否のエラーテスト
6. Git リポジトリ外でのエラーテスト
7. 入力バリデーションテスト（制御文字、256文字超）

## 9. 品質基準

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |
