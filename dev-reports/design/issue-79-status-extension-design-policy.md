# 設計方針書: Issue #79 チーム向けstatusコマンド拡張

## 1. Issue情報

| 項目 | 内容 |
|------|------|
| Issue番号 | #79 |
| タイトル | [Feature] チーム向けstatusコマンド拡張（インデックスカバレッジ・統計） |
| ラベル | enhancement |
| 作成日 | 2026-03-22 |

## 2. 設計概要

既存の `commandindex status` コマンドを拡張し、`--detail` / `--coverage` オプションで詳細な統計情報（ファイルカバレッジ、Embeddingカバレッジ、Staleness、Storage内訳）を表示する。既存の出力は完全互換を維持する。

## 3. システムアーキテクチャ上の位置づけ

```
┌─────────────────────────────────────────────────────┐
│ CLI Layer (src/main.rs)                             │
│   Commands::Status { path, format, detail, coverage }│
│   → StatusOptions 構築 → status::run() 呼び出し    │
└──────────────┬──────────────────────────────────────┘
               │
┌──────────────▼──────────────────────────────────────┐
│ Status Module (src/cli/status.rs)                   │
│   run(path, options, writer) → StatusInfo           │
│   ├── BasicInfo (既存)                              │
│   ├── CoverageInfo (新規: --detail/--coverage)      │
│   ├── StalenessInfo (新規: --detail)                │
│   └── StorageBreakdown (新規: --detail)             │
└──────────┬────────┬─────────┬──────────┬────────────┘
           │        │         │          │
    ┌──────▼──┐ ┌───▼────┐ ┌─▼────────┐ ┌▼───────────┐
    │IndexState│ │Manifest│ │Embedding │ │ Git Info   │
    │state.rs  │ │manifest│ │Store     │ │ git_info.rs│
    └─────────┘ └────────┘ │store.rs  │ └────────────┘
                           └──────────┘
```

## 4. レイヤー構成と責務

### 変更対象モジュール

| レイヤー | ファイル | 変更内容 | 責務 |
|---------|---------|---------|------|
| **CLI定義** | `src/main.rs` (L88-95, L255-262) | `--detail`, `--coverage` フラグ追加、`conflicts_with` 設定、dispatch で `StatusOptions` 構築 → `run()` 呼び出し | CLIオプションのパース・dispatch |
| **Status実行** | `src/cli/status.rs` (全209行) | `StatusOptions` 導入、`run()` シグネチャ変更、詳細表示ロジック追加 | status表示の全ロジック |
| **Git情報取得** | `src/cli/status/git_info.rs` (新規) | Git staleness 情報取得ロジックを独立モジュールとして切り出し（SRP） | Git コマンド実行・結果パース |
| **状態管理** | `src/indexer/state.rs` (L53-62) | `last_commit_hash: Option<String>` 追加 | インデックス状態の永続化 |
| **インデックス** | `src/indexer/mod.rs` (全35行) | パスヘルパーの公開利用 | パス定義の一元管理 |
| **Embedding** | `src/embedding/store.rs` (全485行、テスト含む。production code L1-262) | `count_distinct_files()` メソッド追加 | Embeddingデータアクセス |
| **インデックス構築** | `src/cli/index.rs` | `run()` および `run_incremental()` で `git rev-parse HEAD` を実行し `state.last_commit_hash` を設定するフローを追加 | インデックス構築時の Git commit hash 記録 |

### 変更なしモジュール

| レイヤー | ファイル | 理由 |
|---------|---------|------|
| Parser | `src/parser/` | 解析ロジックに変更なし |
| Search | `src/search/` | 検索ロジックに変更なし |
| Output | `src/output/mod.rs` | StatusInfo は status.rs 内で定義済み。OutputFormat は status 固有の StatusFormat を使用 |

## 5. 技術選定

| 技術要素 | 選定 | 理由 | 代替案と却下理由 |
|---------|------|------|-----------------|
| Git情報取得 | `std::process::Command` | 依存追加不要、クロスコンパイル問題回避 | `git2` crate: ビルド時間増、libgit2のクロスコンパイル問題 |
| ファイル走査 | `walkdir` (既存依存) | Cargo.toml に既に含まれる | 新規crate追加不要 |
| オプション集約 | `StatusOptions` 構造体 | 将来の拡張に対応、既存テスト互換 | 引数追加: テスト破壊リスク大 |
| ストレージ計算 | `fs::metadata` + `walkdir` | 既存の `compute_dir_size()` パターン踏襲 | - |
| Git情報モジュール分離 | `src/cli/status/git_info.rs` | SRP（Single Responsibility Principle）遵守。status.rs は表示ロジックに集中 | status.rs 内にプライベート関数: SRP違反、テスト困難 |

## 6. 新規追加する型の設計

### 6.1 StatusOptions

```rust
/// status コマンドのオプションを集約する構造体
/// path と writer は独立引数として run() に渡す
pub struct StatusOptions {
    pub detail: bool,
    pub coverage: bool,
    pub format: StatusFormat,
}

impl Default for StatusOptions {
    fn default() -> Self {
        Self {
            detail: false,
            coverage: false,
            format: StatusFormat::Human,
        }
    }
}
```

**設計判断**: `run()` のシグネチャを `run(path, options, writer)` に変更。`path` と `writer` は `StatusOptions` に含めず独立引数として維持する（path はファイルシステムパス、writer は I/O 先であり、オプション集約体とは性質が異なるため）。既存テストは `StatusOptions::default()` で互換維持。将来のオプション追加は `StatusOptions` にフィールド追加 + `Default` 更新のみで対応可能。

### 6.2 StorageBreakdown

```rust
#[derive(Debug, Serialize)]
pub struct StorageBreakdown {
    pub tantivy_bytes: u64,
    pub symbols_db_bytes: u64,
    pub embeddings_db_bytes: u64,
    pub other_bytes: u64,
    pub total_bytes: u64,
}
```

**設計判断**: 個別ファイル/ディレクトリのサイズを構造化。`indexer::index_dir()`, `indexer::symbol_db_path()`, `indexer::embeddings_db_path()` を活用してパスのハードコーディングを回避。

### 6.3 CoverageInfo

```rust
#[derive(Debug, Serialize)]
pub struct CoverageInfo {
    /// プロジェクト内の発見可能なファイル数（除外ルール適用後）
    pub discoverable_files: u64,
    pub indexed_files: u64,
    pub skipped_files: u64,
    pub embedding_file_count: u64,
    pub embedding_model: Option<String>,
}
```

**設計判断**:
- `total_files` を `discoverable_files` にリネーム — "total" はインデックス済みファイル数と紛らわしいため、走査で発見されたファイル数であることを名前で明示。
- `file_type_counts` を **CoverageInfo から除外** — トップレベルの `StatusInfo.file_type_counts`（既存フィールド）と重複するため。ファイルタイプ別カウントはトップレベルのみで管理し、CoverageInfo は集約カバレッジ情報に集中する。

### 6.4 StalenessInfo

```rust
#[derive(Debug, Serialize)]
pub struct StalenessInfo {
    pub last_commit_hash: Option<String>,
    pub commits_since_index: Option<u64>,
    pub files_changed_since_index: Option<u64>,
    pub recommendation: Option<String>,
}
```

**設計判断**: 全フィールドが `Option` — git 未インストール時は全て `None` となり、表示時に `(Git info unavailable)` とする。

### 6.5 StatusInfo 拡張

```rust
#[derive(Debug, Serialize)]
pub struct StatusInfo {
    // 既存フィールド（互換維持）
    #[serde(flatten)]
    pub state: IndexState,
    pub index_size_bytes: u64,
    pub file_type_counts: FileTypeCounts,
    pub symbol_count: u64,

    // 新規フィールド（--detail/--coverage 時のみ）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coverage: Option<CoverageInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub staleness: Option<StalenessInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage: Option<StorageBreakdown>,
}
```

**設計判断**: 新フィールドは全て `Option` + `skip_serializing_if` により、デフォルト（オプションなし）の JSON 出力は既存と完全互換。新規出力フィールドには `strip_control_chars()` を適用してサニタイズする。

## 7. IndexState スキーマ変更

### 変更内容

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IndexState {
    pub version: String,
    pub schema_version: u32,         // CURRENT_SCHEMA_VERSION = 1（変更なし）
    pub created_at: DateTime<Utc>,
    pub last_updated_at: DateTime<Utc>,
    pub total_files: u64,
    pub total_sections: u64,
    pub index_root: PathBuf,
    #[serde(default)]                // ← 新規追加
    pub last_commit_hash: Option<String>,  // ← 新規追加
}
```

**注**: IndexState には既に `PartialEq` が derive されている（現行コード L53 で `#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]`）。新規フィールド `last_commit_hash` は `Option<String>` であり `PartialEq` を自動導出可能なため、derive 一覧の変更は不要。

### 後方互換性戦略

| 項目 | 判断 | 理由 |
|------|------|------|
| schema_version バンプ | **不要** | `Option<String>` + `#[serde(default)]` により、古い state.json の読み込みでも `None` として問題なくデシリアライズされる |
| マイグレーション | **不要** | 次回の `index` / `update` 実行時に自動的に `Some(commit_hash)` が書き込まれる |
| check_schema_version() | **変更なし** | CURRENT_SCHEMA_VERSION = 1 のまま |

### clean コマンドとの関係

`clean` コマンドは `.commandindex/` ディレクトリ全体を削除するため、`state.json` 内の `last_commit_hash` も削除される。次回 `index` 実行時に新しい `last_commit_hash` が記録されるため、特別な対応は不要。

### トレードオフ

- **採用**: `serde(default)` による暗黙的マイグレーション — シンプルで破壊リスクゼロ
- **却下**: schema_version を 2 にバンプ — `check_schema_version()` でエラーとなり、ユーザーに `clean` + 再インデックスを強制する。フィールド追加のみでその負担は不釣り合い

## 8. EmbeddingStore 拡張

### 新規メソッド

```rust
impl EmbeddingStore {
    /// インデックスされたユニークファイル数を返す
    pub fn count_distinct_files(&self) -> Result<u64, EmbeddingStoreError> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(DISTINCT section_path) FROM embeddings",
            [],
            |row| row.get(0),
        )?;
        Ok(count as u64)
    }
}
```

### count_distinct_files() のユニットテスト

```rust
#[cfg(test)]
mod tests {
    // ... 既存テスト ...

    #[test]
    fn test_count_distinct_files_empty() {
        let store = EmbeddingStore::open(":memory:").unwrap();
        assert_eq!(store.count_distinct_files().unwrap(), 0);
    }

    #[test]
    fn test_count_distinct_files_with_data() {
        let store = EmbeddingStore::open(":memory:").unwrap();
        // 同一ファイルに2セクション、別ファイルに1セクション挿入
        store.upsert("file_a.md#sec1", &[0.1, 0.2], "model-1").unwrap();
        store.upsert("file_a.md#sec2", &[0.3, 0.4], "model-1").unwrap();
        store.upsert("file_b.md#sec1", &[0.5, 0.6], "model-1").unwrap();
        // section_path のプレフィックス（ファイル部分）でカウントされるため、
        // 正確なカウントはクエリのDISTINCT対象に依存
        let count = store.count_distinct_files().unwrap();
        assert!(count >= 2); // 少なくとも2つのDISTINCTなパスがある
    }
}
```

### status.rs 側のヘルパー

```rust
/// embeddings.db が存在しない場合は 0 を返す（get_symbol_count パターン踏襲）
fn get_embedding_file_count(base_path: &Path) -> u64 {
    let db_path = indexer::embeddings_db_path(base_path);
    if !db_path.exists() {
        return 0;
    }
    match EmbeddingStore::open(&db_path) {
        Ok(store) => store.count_distinct_files().unwrap_or(0),
        Err(_) => 0,
    }
}
```

**設計判断**: `get_symbol_count()` (status.rs L126-141) の既存パターンを踏襲。DB不在 / スキーマ不整合時はエラーではなく 0 を返す。

## 9. Git 情報取得の設計

### モジュール配置

Git 操作ロジックを `src/cli/status.rs` から分離し、**`src/cli/status/git_info.rs`** として独立モジュールに切り出す（SRP: Single Responsibility Principle）。

`status.rs` をディレクトリモジュール化する:
```
src/cli/status/
├── mod.rs          # 既存の status.rs のロジック（表示・集約）
└── git_info.rs     # Git 操作ロジック（コマンド実行・結果パース）
```

### git_info.rs の公開 API

```rust
// src/cli/status/git_info.rs

use std::path::Path;

/// last_commit_hash のバリデーション（コマンドインジェクション防止）
/// 有効な Git commit hash パターン: 4〜40文字の16進数
fn validate_commit_hash(hash: &str) -> bool {
    let re = regex::Regex::new(r"^[0-9a-f]{4,40}$").unwrap();
    re.is_match(hash)
}

/// Git の staleness 情報を best-effort で取得
pub fn get_staleness_info(base_path: &Path, last_commit_hash: Option<&str>) -> Option<StalenessInfo> {
    // 1. git が利用可能か確認
    // 2. last_commit_hash が Some の場合、validate_commit_hash() でバリデーション
    //    - バリデーション失敗時は last_commit_hash を None として扱う
    // 3. last_commit_hash が None の場合は staleness 算出不可
    // 4. git log --oneline <last_commit>..HEAD でコミット数取得
    // 5. git diff --name-only <last_commit>..HEAD でファイル変更数取得
    // 6. 全て Option で返す（失敗時は None）
}

/// index/update 時に現在の HEAD commit hash を取得
pub fn get_current_commit_hash(repo_path: &Path) -> Option<String> {
    // git rev-parse HEAD を実行
    // 成功時: Some(hash) — validate_commit_hash() で検証済みの値を返す
    // 失敗時: None
}
```

### index.rs での last_commit_hash 設定フロー

`src/cli/index.rs` の `run()` および `run_incremental()` に以下のフローを追加:

```rust
// src/cli/index.rs の run() 内（state 保存前）
use crate::cli::status::git_info;

// ... 既存のインデックス構築ロジック ...

// Git commit hash を取得して state に設定
let commit_hash = git_info::get_current_commit_hash(path);
state.last_commit_hash = commit_hash;

// state を保存
state.save(&commandindex_dir)?;
```

同様に `run_incremental()` でも state 保存前に `last_commit_hash` を設定する。

### last_commit_hash バリデーション

Git コマンドに `last_commit_hash` を渡す前に、以下のバリデーションを実施:

```rust
/// commit hash が有効な Git hash 形式であることを検証
/// パターン: ^[0-9a-f]{4,40}$
/// バリデーション失敗時は None として扱い、staleness 計算をスキップ
fn validate_commit_hash(hash: &str) -> bool {
    hash.len() >= 4
        && hash.len() <= 40
        && hash.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
```

**設計判断**: 正規表現 crate を避け、バイト単位のチェックで実装可能。`git rev-parse HEAD` の出力は信頼できるが、`state.json` が手動編集される可能性があるため、コマンド実行前の検証は必須。バリデーション失敗時はエラーとせず `None` として扱い、staleness セクションで `(unknown)` と表示する。

### エラーハンドリング方針

| ケース | 挙動 | 表示 |
|--------|------|------|
| git 未インストール | `None` を返す | `(Git info unavailable: git not found)` |
| .git 不在 | `None` を返す | `(Git info unavailable: not a git repository)` |
| last_commit_hash が None | 部分的な情報のみ | `Last commit at index: (unknown)` |
| last_commit_hash バリデーション失敗 | `None` として扱う | `Last commit at index: (unknown)` |
| git コマンドエラー | `None` を返す | `(Git info unavailable)` |
| CI shallow clone 環境 | git log/diff が不完全な結果を返す可能性 | `None` を返す。`(Git info may be incomplete: shallow clone detected)` |
| git stderr 出力 | debug レベルでログ出力 | ユーザー向けには `(Git info unavailable)` の汎用メッセージ |

**全てのケースで status コマンド自体は exit code 0 で正常終了。**

git stderr の扱い:
```rust
let output = Command::new("git")
    .args(["log", "--oneline", &format!("{commit_hash}..HEAD")])
    .current_dir(base_path)
    .output();

match output {
    Ok(o) if o.status.success() => { /* stdout をパース */ },
    Ok(o) => {
        // stderr は debug レベルでログ（ユーザーには見せない）
        let stderr = String::from_utf8_lossy(&o.stderr);
        eprintln!("[debug] git stderr: {stderr}");  // TODO: proper logging
        None
    },
    Err(_) => None,
}
```

## 10. ファイル走査（Coverage計算）の設計

### 走査ロジック

```rust
fn count_discoverable_files(base_path: &Path) -> u64 {
    // walkdir でプロジェクトルートを走査
    // デフォルト除外: .git/, node_modules/, target/, .commandindex/
    // .cmindexignore ルールを適用
    // 対象拡張子（FileType::all_extensions()）のファイルをカウント
}
```

### パフォーマンス考慮

| 対策 | 説明 |
|------|------|
| デフォルト除外 | `.git/`, `node_modules/`, `target/`, `.commandindex/` を walkdir のフィルタで早期除外 |
| 拡張子フィルタ | `FileType::all_extensions()` に含まれる拡張子のみカウント |
| `.cmindexignore` | 既存の `parser::ignore` モジュールのロジックを再利用 |

**設計判断**: `--detail` / `--coverage` 指定時のみ走査を実行。デフォルトの `status` では走査しない（パフォーマンス影響ゼロ）。

## 11. CLI オプション設計

### main.rs の変更（CLIオプション定義）

```rust
Commands::Status {
    /// Target directory
    #[arg(long, default_value = ".")]
    path: PathBuf,
    /// Output format (human, json)
    #[arg(long, value_enum, default_value_t = StatusFormat::Human)]
    format: StatusFormat,
    /// Show detailed status (coverage, staleness, storage)
    #[arg(long, conflicts_with = "coverage")]
    detail: bool,
    /// Show coverage information only
    #[arg(long, conflicts_with = "detail")]
    coverage: bool,
},
```

### main.rs の変更（dispatch部分）

```rust
Commands::Status { path, format, detail, coverage } => {
    let options = commandindex::cli::status::StatusOptions {
        detail,
        coverage,
        format,
    };
    match commandindex::cli::status::run(&path, &options, &mut std::io::stdout()) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("{e}");
            1
        }
    }
}
```

### オプション組み合わせマトリクス

| オプション | 表示内容 |
|-----------|---------|
| (なし) | 既存の基本情報のみ（完全互換） |
| `--detail` | 全セクション（Index Status + Coverage + Staleness + Storage） |
| `--coverage` | Coverage セクションのみ |
| `--detail --coverage` | **エラー**（clap が排他エラーを返す） |
| `--format json` | JSON 形式で出力（各モードと併用可能） |
| `--detail --format json` | 全情報を JSON で出力 |
| `--coverage --format json` | Coverage 情報を JSON で出力 |

## 12. 出力フォーマット設計

### Human フォーマット（--detail 時）

```
CommandIndex Status
  Index root:    .
  Version:       0.0.5
  Created:       2026-03-22 14:30:00 UTC
  Last updated:  2026-03-22 14:30:00 UTC
  Total files:   150
  Total sections: 420
  Files by type: Markdown=80, TypeScript=45, Python=25
  Symbols:       312
  Index size:    45.0 MB
  Last commit:   abc1234

Coverage:
  Discoverable files: 1500
  Indexed files:      1420
  Skipped files:      80

  Embedding coverage:
    Files:         1200 / 1420 (85%)
    Model:         nomic-embed-text

Staleness:
  Commits since last index:  12
  Files changed:             23
  Recommendation:            Run `commandindex update`

Storage:
  tantivy/:       45.0 MB
  symbols.db:     12.0 MB
  embeddings.db:   8.0 MB
  Other:           0.1 MB
  Total:          65.1 MB
```

### Human フォーマット（--coverage 時）

```
Coverage:
  Discoverable files: 1500
  Indexed files:      1420
  Skipped files:      80

  Embedding coverage:
    Files:         1200 / 1420 (85%)
    Model:         nomic-embed-text
```

### Human フォーマット（オプションなし -- 既存互換）

```
CommandIndex Status
  Index root:    .
  Version:       0.0.5
  Created:       2026-03-22 14:30:00 UTC
  Last updated:  2026-03-22 14:30:00 UTC
  Total files:   150
  Total sections: 420
  Files by type: Markdown=80, TypeScript=45, Python=25
  Symbols:       312
  Index size:    45.0 MB
```

**注**: 既存フォーマットのヘッダーは実際のコード（status.rs L172）に合わせ `CommandIndex Status` とする。既存の出力フィールド名・順序は完全に維持する。

## 13. セキュリティ設計

| 脅威 | 対策 | 優先度 |
|------|------|--------|
| パストラバーサル | `base_path` からの相対パスのみ使用、`..` を含むパスは正規化 | 高 |
| Git コマンドインジェクション | `std::process::Command` の引数は配列で渡す（シェル経由しない） | 高 |
| last_commit_hash インジェクション | `^[0-9a-f]{4,40}$` でバリデーション。失敗時は `None` として扱う | 高 |
| 大量ファイル走査によるDoS | デフォルト除外ディレクトリ + `walkdir` の max_depth 制限検討 | 中 |
| 制御文字インジェクション | 新規出力フィールド（commit hash, recommendation 等）に `strip_control_chars()` を適用 | 中 |
| git stderr 情報漏洩 | stderr は debug レベルでログ出力のみ。ユーザー向けは汎用メッセージ `(Git info unavailable)` | 中 |
| unsafe 使用 | 禁止 | - |

### last_commit_hash バリデーション詳細

```rust
// git_info.rs 内
fn validate_commit_hash(hash: &str) -> bool {
    hash.len() >= 4
        && hash.len() <= 40
        && hash.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

// 使用箇所: get_staleness_info() 内
let validated_hash = last_commit_hash.filter(|h| validate_commit_hash(h));
match validated_hash {
    Some(hash) => { /* git log/diff コマンドに hash を使用 */ },
    None => { /* staleness 算出不可、(unknown) と表示 */ },
}
```

## 14. 影響範囲

### 変更対象ファイル

| ファイル | 変更種別 | 影響度 |
|---------|---------|--------|
| `src/cli/status.rs` → `src/cli/status/mod.rs` | 大幅拡張 + ディレクトリモジュール化 | 高 |
| `src/cli/status/git_info.rs` | 新規作成（Git操作ロジック） | 中 |
| `src/main.rs` (L88-95, L255-262) | CLIオプション追加 + dispatch 変更 | 中 |
| `src/indexer/state.rs` (L53-62) | フィールド追加 | 低 |
| `src/embedding/store.rs` (production code L1-262) | メソッド追加 | 低 |
| `src/cli/index.rs` | `run()` / `run_incremental()` に git rev-parse HEAD → state.last_commit_hash 設定追加 | 中 |

### 影響を受けるテストファイル

| テストファイル | 影響内容 | 対応 |
|-------------|---------|------|
| `tests/cli_status.rs` (L97-132) | `run()` シグネチャ変更 | `StatusOptions::default()` に移行 |
| `tests/cli_args.rs` | 新オプションのテスト追加 | `--detail`, `--coverage`, 排他テスト追加 |
| `tests/indexer_state.rs` (L20-32) | `last_commit_hash` フィールド追加 | serde 後方互換テスト追加 |
| `tests/cli_status.rs` (L136-197) | E2E テスト | `--detail` 付き E2E テスト追加 |
| `tests/incremental_update.rs` | `IndexState` フィールド追加の影響 | `last_commit_hash` フィールド追加に伴うテストデータ更新 |

### 新規テスト

| テスト | 検証内容 |
|--------|---------|
| `test_status_detail_human` | `--detail` の Human 出力に全セクション（Coverage, Staleness, Storage）が含まれる |
| `test_status_detail_json` | `--detail --format json` の出力に拡張フィールドが含まれ、既存フィールドが維持される |
| `test_status_coverage_only` | `--coverage` で Coverage セクションのみ出力 |
| `test_status_default_compatible` | オプションなしで既存出力と同一 |
| `test_detail_coverage_conflict` | `--detail --coverage` 同時指定でエラー |
| `test_staleness_no_git` | git 未インストール環境での graceful degradation |
| `test_embedding_count_no_db` | embeddings.db 不在時に 0 返却 |
| `test_count_distinct_files_empty` | count_distinct_files() が空DBで 0 を返す |
| `test_count_distinct_files_with_data` | count_distinct_files() が正しいユニーク数を返す |
| `test_state_backward_compat` | 古い state.json（last_commit_hash なし）の読み込み |
| `test_storage_breakdown` | StorageBreakdown の各項目が正しいサイズを返す |
| `test_validate_commit_hash` | 有効/無効な commit hash のバリデーション |
| `test_staleness_shallow_clone` | shallow clone 環境での graceful degradation |

### JSON format テストの具体的コード例

```rust
#[test]
fn test_status_detail_json() {
    // setup: テスト用インデックスを作成
    let dir = tempdir().unwrap();
    // ... インデックス構築 ...

    let options = StatusOptions {
        detail: true,
        coverage: false,
        format: StatusFormat::Json,
    };
    let mut buf = Vec::new();
    run(dir.path(), &options, &mut buf).unwrap();
    let output = String::from_utf8(buf).unwrap();
    let json: serde_json::Value = serde_json::from_str(&output).unwrap();

    // 既存フィールドが維持されていること
    assert!(json.get("version").is_some());
    assert!(json.get("total_files").is_some());
    assert!(json.get("file_type_counts").is_some());
    assert!(json.get("symbol_count").is_some());

    // 拡張フィールドが含まれること
    assert!(json.get("coverage").is_some());
    assert!(json.get("staleness").is_some());
    assert!(json.get("storage").is_some());

    // CoverageInfo に file_type_counts が含まれないこと
    let coverage = json.get("coverage").unwrap();
    assert!(coverage.get("file_type_counts").is_none());
    assert!(coverage.get("discoverable_files").is_some());
}

#[test]
fn test_status_default_json_no_extra_fields() {
    // setup: テスト用インデックスを作成
    let dir = tempdir().unwrap();
    // ... インデックス構築 ...

    let options = StatusOptions::default(); // format: Human → Json に変更してテスト
    let options = StatusOptions { format: StatusFormat::Json, ..StatusOptions::default() };
    let mut buf = Vec::new();
    run(dir.path(), &options, &mut buf).unwrap();
    let output = String::from_utf8(buf).unwrap();
    let json: serde_json::Value = serde_json::from_str(&output).unwrap();

    // デフォルト時は拡張フィールドが含まれないこと（既存互換）
    assert!(json.get("coverage").is_none());
    assert!(json.get("staleness").is_none());
    assert!(json.get("storage").is_none());
}
```

## 15. 品質基準

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |

## 16. 設計判断とトレードオフまとめ

| 判断項目 | 採用 | 却下 | 理由 |
|---------|------|------|------|
| Git 情報取得 | `std::process::Command` | `git2` crate | ビルド依存最小化、クロスコンパイル問題回避 |
| Git ロジック配置 | `src/cli/status/git_info.rs` 独立モジュール | status.rs 内にプライベート関数 | SRP遵守。Git操作と表示ロジックの責務を分離。テスト容易性向上 |
| schema_version | バンプなし (v1維持) | v2 にバンプ | `Option` + `serde(default)` で互換維持可能。バンプするとユーザーに再インデックス強制 |
| run() シグネチャ | `run(path, options, writer)` — path/writer は独立引数 | path を StatusOptions に含める | path はファイルシステムパス、writer は I/O 先であり、オプション集約体とは性質が異なる |
| JSON 互換 | `skip_serializing_if` | 常に全フィールド出力 | 既存スクリプトの破壊防止 |
| ファイル走査タイミング | `--detail`/`--coverage` 時のみ | 常に走査 | デフォルト動作のパフォーマンス維持 |
| Storage 計算 | 既存パスヘルパー活用 | パスハードコーディング | 一元管理、変更追従 |
| CoverageInfo の file_type_counts | トップレベル StatusInfo のみ | CoverageInfo にも含める | 重複排除。ファイルタイプ情報は既存の StatusInfo.file_type_counts で一元管理 |
| CoverageInfo.total_files | `discoverable_files` にリネーム | `total_files` のまま | IndexState.total_files との混同を避け、走査で発見されたファイル数であることを名前で明示 |
| last_commit_hash バリデーション | `^[0-9a-f]{4,40}$` チェック | バリデーションなし | state.json 手動編集によるインジェクション防止 |
