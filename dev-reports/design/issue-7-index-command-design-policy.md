# 設計方針書: Issue #7 - index コマンド実装

## 1. 概要

`commandindex index` コマンドを実装する。リポジトリ内のMarkdownファイルを解析し、tantivy インデックスを構築する Phase 1 の主要コマンド。

## 2. システムアーキテクチャ概要

```
CLI Layer (main.rs + cli/)
    ↓ サブコマンドディスパッチ
Command Layer (cli/index.rs)
    ↓ オーケストレーション
Parser Layer (parser/)     Indexer Layer (indexer/)
  - ignore.rs               - writer.rs
  - markdown.rs              - schema.rs
  - frontmatter.rs           - state.rs
  - link.rs                  - manifest.rs
```

index コマンドは **Command Layer** に位置し、Parser と Indexer を統合するオーケストレーターとして機能する。

## 3. レイヤー構成と責務

| レイヤー | モジュール | 責務 |
|---------|-----------|------|
| **CLI** | `src/main.rs` | clap サブコマンド定義、ディスパッチ |
| **Command** | `src/cli/index.rs` | index コマンドのオーケストレーション |
| **Parser** | `src/parser/` | Markdown解析、.cmindexignore フィルタリング |
| **Indexer** | `src/indexer/` | tantivy書き込み、状態管理、マニフェスト管理 |

## 4. 新規モジュール設計

### 4.1 `src/cli/mod.rs`

```rust
pub mod index;
```

### 4.2 `src/cli/index.rs` - index コマンド本体

#### エラー型

```rust
#[derive(Debug)]
pub enum IndexError {
    Io(std::io::Error),
    Parse(ParseError),
    Writer(WriterError),
    State(StateError),
    Manifest(ManifestError),
    Ignore(IgnoreError),
}
```

各サブモジュールのエラーを統合する enum。`From` トレイトで自動変換。`Display` と `std::error::Error` トレイトを実装し、main.rs での `eprintln!("Error: {e}")` による出力に対応する。

#### 結果サマリー構造体

```rust
pub struct IndexSummary {
    pub scanned: u64,        // 走査したMarkdownファイル数
    pub indexed_sections: u64, // インデックスしたセクション数
    pub skipped: u64,        // パースエラーでスキップしたファイル数
    pub ignored: u64,        // .cmindexignore で除外したファイル数
    pub duration: Duration,  // 処理時間
}
```

#### メイン関数

```rust
pub fn run(path: &Path) -> Result<IndexSummary, IndexError>
```

**処理フロー:**

1. 対象ディレクトリの存在確認（`path.is_dir()`）
2. `.cmindexignore` 読み込み（`IgnoreFilter::from_file(&path.join(".cmindexignore"))`）
3. `walkdir` で `.md` ファイルを列挙
4. `IgnoreFilter::is_ignored()` でフィルタリング（ignored カウント）
5. 既存 `.commandindex/tantivy/` があれば `std::fs::remove_dir_all()` で削除
6. `.commandindex/tantivy/` に `IndexWriterWrapper::open()`
7. 各ファイルを `parse_file()` でパース
   - パースエラー時: `eprintln!` で警告出力、スキップ（skipped カウント）
8. `Section` → `SectionDoc` 変換（`section_to_doc()`）、`add_section()`
9. `manifest::compute_file_hash()` でSHA-256ハッシュ計算
10. `std::fs::metadata().modified()` からファイル更新日時を取得し `DateTime<Utc>` に変換
11. `Manifest` に `FileEntry { path, hash, last_modified, sections }` を追加
12. `commit()` でインデックス確定
13. `manifest.json` 保存
14. `IndexState::new()` で作成後、`state.total_files = scanned; state.total_sections = indexed_sections;` で値を設定し保存
15. `IndexSummary` を返却

### 4.3 CLIオプション変更（main.rs）

```rust
#[derive(Subcommand)]
enum Commands {
    /// Build search index from repository
    Index {
        /// Target directory to index
        #[arg(long, default_value = ".")]
        path: PathBuf,
    },
    // ... 他のサブコマンドは変更なし
}
```

### 4.4 lib.rs の変更

```rust
pub mod cli;       // 追加
pub mod indexer;
pub mod parser;
```

## 5. データフロー

```
.md files (on disk)
    ↓ walkdir + IgnoreFilter
filtered file list
    ↓ parse_file()
Vec<MarkdownDocument>
    ↓ Section → SectionDoc 変換
    ↓   tags = frontmatter.tags.join(", ")
    ↓   path = relative_path
    ↓   heading_level = section.level as u64
    ↓   line_start = section.line_start as u64
Vec<SectionDoc>
    ↓ IndexWriterWrapper::add_section()
tantivy index (.commandindex/tantivy/)
    ↓ commit()

Manifest (.commandindex/manifest.json)
    ← FileEntry { path, hash, last_modified, sections }

IndexState (.commandindex/state.json)
    ← { total_files, total_sections, ... }
```

### Section → SectionDoc 変換の詳細

```rust
fn section_to_doc(
    section: &Section,
    file_path: &str,
    frontmatter: Option<&Frontmatter>,
) -> SectionDoc {
    let tags = frontmatter
        .map(|fm| fm.tags.join(", "))
        .unwrap_or_default();

    SectionDoc {
        path: file_path.to_string(),
        heading: section.heading.clone(),
        body: section.body.clone(),
        tags,
        heading_level: section.level as u64,
        line_start: section.line_start as u64,
    }
}
```

## 6. ディレクトリ構成

### 生成されるディレクトリ構造

```
<target_dir>/
└── .commandindex/
    ├── tantivy/          # tantivy インデックスファイル群
    ├── manifest.json     # ファイルメタ情報
    └── state.json        # インデックス状態
```

### ソースコード変更

```
src/
├── main.rs              # Commands::Index に --path 追加、cli::index::run() 呼び出し
├── lib.rs               # pub mod cli 追加
├── cli/                 # 新規
│   ├── mod.rs           # pub mod index
│   └── index.rs         # run(), IndexError, IndexSummary
├── parser/              # 変更なし
└── indexer/             # 変更なし
```

## 7. エラーハンドリング設計

### エラーの分類と挙動

| エラー種別 | 発生箇所 | 挙動 | 終了コード |
|-----------|---------|------|-----------|
| ディレクトリ不在 | run() 冒頭 | エラーメッセージ表示、即座に終了 | 1 |
| .cmindexignore 読み込み失敗 | IgnoreFilter構築 | エラーメッセージ表示、即座に終了 | 1 |
| パースエラー | parse_file() | 警告表示、ファイルスキップ、処理続行 | 0（他ファイルの処理は成功） |
| tantivy書き込みエラー | add_section/commit | エラーメッセージ表示、即座に終了 | 1 |
| state/manifest保存エラー | save() | エラーメッセージ表示、即座に終了 | 1 |

### main.rs でのエラーハンドリング

```rust
Commands::Index { path } => {
    match cli::index::run(&path) {
        Ok(summary) => {
            // サマリー表示
            0
        }
        Err(e) => {
            eprintln!("Error: {e}");
            1
        }
    }
}
```

## 8. 出力フォーマット

```
Indexing /path/to/repo...
  Scanned: 42 files
  Indexed: 185 sections
  Skipped: 3 files (parse error)
  Ignored: 5 files (.cmindexignore)
  Duration: 1.2s
Index saved to .commandindex/
```

- 標準出力にサマリーを表示
- パースエラー時の警告は `eprintln!` で標準エラー出力

## 9. セキュリティ設計

| 脅威 | 対策 | 優先度 |
|------|------|--------|
| パストラバーサル | `--path` の正規化（`canonicalize()`）。ユーザー明示指定パスのためベースディレクトリチェックは不要 | 中 |
| シンボリックリンク追跡 | walkdir のデフォルト設定（シンボリックリンクを追跡しない）を維持 | 中 |
| 大量ファイルによるメモリ枯渇 | ファイル単位で逐次処理、tantivy WriterHeapSize 50MB制限 | 中 |
| unsafe 使用 | 禁止 | 高 |

## 10. 設計判断とトレードオフ

### 判断1: ファイル列挙を index コマンド側で行う

- **選択**: index コマンド側で `walkdir` + `IgnoreFilter` → `parse_file()` 個別呼び出し
- **代替案**: `parse_directory()` を拡張して `IgnoreFilter` を引数に追加
- **理由**: 既存API（`parse_directory()`）を壊さない。コマンド固有のフィルタリングロジックはコマンド層の責務。
- **将来課題**: update コマンドでも同じ walkdir + IgnoreFilter のロジックが必要になる可能性がある。その時点で共通ユーティリティへの抽出を検討する。

### 判断2: cli/ モジュールを新設

- **選択**: `src/cli/index.rs` にコマンドロジックを配置
- **代替案**: `main.rs` に直接実装
- **理由**: CLAUDE.md Phase 1 モジュール構成に準拠。main.rs の肥大化を防止。

### 判断3: 上書き再構築はtantivyディレクトリ削除方式

- **選択**: `.commandindex/tantivy/` を `std::fs::remove_dir_all()` で削除後、新規作成
- **代替案**: tantivy の `delete_all_documents()` → 再追加
- **理由**: クリーンな状態でのインデックス再構築を保証。delete_all_documents の断片化リスクを回避。

### 判断4: パスの相対パス化

- **選択**: manifest と tantivy index 内のパスは、対象ディレクトリからの相対パスで格納
- **理由**: リポジトリの移動・クローンに対応。ポータビリティの確保。

## 11. 影響範囲

### 変更ファイル

| ファイル | 変更内容 |
|---------|---------|
| `src/main.rs` | `Commands::Index` に `path` フィールド追加、`cli::index::run()` 呼び出し |
| `src/lib.rs` | `pub mod cli` 追加 |
| `src/cli/mod.rs` | **新規** - モジュール宣言 |
| `src/cli/index.rs` | **新規** - index コマンド実装 |

### 影響を受けるテスト

| テストファイル | 影響 |
|-------------|------|
| `tests/cli_args.rs` | `Commands::Index` の引数変更により更新が必要 |
| **新規テスト** | index コマンドの統合テスト |

### 影響を受けないモジュール

- `src/parser/*` - 既存API変更なし
- `src/indexer/*` - 既存API変更なし

## 12. テスト戦略

### 統合テスト（tests/cli_index.rs）

| テストケース | 内容 |
|------------|------|
| index_creates_commandindex_dir | .commandindex/ ディレクトリが作成される |
| index_creates_tantivy_index | .commandindex/tantivy/ にインデックスが作成される |
| index_creates_manifest | manifest.json が正しい内容で生成される |
| index_creates_state | state.json が正しい内容で生成される |
| index_applies_cmindexignore | .cmindexignore のルールが適用される |
| index_displays_summary | サマリーが標準出力に表示される |
| index_rebuilds_on_existing | 既存インデックスが削除・再構築される |
| index_skips_parse_errors | パースエラーファイルをスキップ |
| index_with_path_option | --path オプションが動作する |
| index_nonexistent_path | 不在ディレクトリでエラー終了 |
| index_empty_directory | .md ファイルなしでも正常終了 |

### 既存テスト更新（tests/cli_args.rs）

- `test_index_subcommand`: --path オプション対応に更新
- `test_index_not_implemented_yet`: 削除し、新規テスト `test_index_succeeds_with_valid_dir` に置き換え。一時ディレクトリで index コマンドが正常終了することを検証

## 13. 品質基準

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |
