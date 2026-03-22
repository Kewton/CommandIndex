# 設計方針書 - Issue #77: インデックス共有モード

## 1. 概要

CI/CDパイプラインやチーム共有サーバーでインデックスを事前生成し、`export`/`import` サブコマンドでチームメンバーが再利用できる仕組みを提供する。また `status --verify` でインデックスの整合性チェックを行う。

## 2. レイヤー構成と責務

### 新規モジュール

| レイヤー | モジュール | 責務 |
|---------|-----------|------|
| **CLI** | `src/cli/export.rs` | エクスポートサブコマンドの実行ロジック（tar.gz圧縮含む） |
| **CLI** | `src/cli/import_index.rs` | インポートサブコマンドの実行ロジック（tar.gz展開・パス検証含む） |
| **Indexer** | `src/indexer/snapshot.rs` | ExportMeta 構造体定義 + export_meta.json の読み書きのみ（SRP） |

> **設計判断**: snapshot.rs の責務を ExportMeta の型定義とシリアライズ/デシリアライズに限定する。tar.gz 操作は export.rs / import_index.rs の責務とし、パストラバーサル検証は import_index.rs 内の private 関数として実装する。

### 変更対象モジュール

| モジュール | 変更内容 |
|-----------|---------|
| `src/main.rs` | `Commands` enum に `Export` / `Import` バリアント追加 |
| `src/cli/mod.rs` | `pub mod export;` `pub mod import_index;` 追加 |
| `src/cli/status.rs` | `--verify` フラグ対応、`run()` に `verify: bool` 引数追加 |
| `src/indexer/mod.rs` | `pub mod snapshot;` 追加 |
| `Cargo.toml` | `tar`, `flate2` 依存追加 |

### 変更不要モジュール

既存の `index`, `search`, `update`, `clean`, `context`, `embed`, `config` コマンドには変更なし。

### 定数参照の方針

- CLI層 (`src/cli/`): `crate::indexer::commandindex_dir(path)` ヘルパー関数を使用
- Indexer層 (`src/indexer/`): 同モジュール内の既存定数 `COMMANDINDEX_DIR`, `TANTIVY_DIR` を使用

## 3. 技術選定

| カテゴリ | 選定技術 | 選定理由 |
|---------|---------|---------|
| tar操作 | `tar` (0.4) | Rust標準的なtarクレート、既にトランジティブ依存として存在 |
| gzip圧縮 | `flate2` (1, default features = miniz_oxide) | 純Rustバックエンド、クロスコンパイル影響なし |
| Git情報取得 | `fn current_git_hash(repo_path: &Path) -> Option<String>` | export.rs 内のユーティリティ関数として分離。テスト時はモック可能 |

## 4. データ構造設計

### 4.1 ExportMeta（エクスポートメタデータ）

```rust
// src/indexer/snapshot.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportMeta {
    pub export_format_version: u32,       // 初期値: 1、前方互換ポリシー
    pub commandindex_version: String,      // env!("CARGO_PKG_VERSION")
    pub git_commit_hash: Option<String>,   // エクスポート時のHEADコミットハッシュ
    pub exported_at: DateTime<Utc>,        // エクスポート日時
}
```

> **設計変更**: `index_root` を ExportMeta から削除。エクスポート元の絶対パス漏洩を防止するため。

> **バージョン互換性ポリシー**: `export_format_version` は整数インクリメント。import 側は「自身が対応する最大バージョン以下であれば受け入れる」前方互換ポリシー。新フィールド追加時に旧バージョンの import を壊さない。

### 4.2 ExportOptions / ImportOptions / ExportResult / ImportResult

```rust
// src/cli/export.rs

pub struct ExportOptions {
    pub with_embeddings: bool,
}

pub struct ExportResult {
    pub output_path: PathBuf,
    pub archive_size: u64,
    pub git_commit_hash: Option<String>,
}

// src/cli/import_index.rs

pub struct ImportOptions {
    pub force: bool,
}

pub struct ImportResult {
    pub imported_files: u64,
    pub git_hash_match: bool,
    pub warnings: Vec<String>,
}
```

> **設計判断**: 既存の IndexSummary/CleanResult パターンに合わせ、ExportResult/ImportResult 構造体で結果を返す。

### 4.3 status.rs の verify 対応

```rust
// src/cli/status.rs — run() シグネチャ

pub fn run(
    path: &Path,
    format: StatusFormat,  // format は別引数のまま（CleanOptions パターン準拠）
    verify: bool,          // 新規追加
    writer: &mut dyn Write,
) -> Result<(), StatusError>
```

> **設計判断**: `StatusOptions` 構造体は導入しない。`format` は別引数のまま維持し、`verify: bool` のみ追加する。既存の CleanOptions が `keep_embeddings` のみ（format は含まない）であるパターンに合わせ、オプション構造体の導入は実際の要求が発生してからとする（YAGNI）。既存テストは `verify: false` を追加するだけの最小修正。

### 4.4 VerifyResult（整合性チェック結果）

```rust
// src/cli/status.rs

#[derive(Debug, Serialize)]
pub struct VerifyResult {
    pub state_valid: bool,
    pub tantivy_valid: bool,
    pub manifest_valid: bool,
    pub symbols_valid: bool,
    pub issues: Vec<VerifyIssue>,
}

#[derive(Debug, Serialize)]
pub struct VerifyIssue {
    pub component: String,
    pub severity: VerifySeverity,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub enum VerifySeverity {
    Error,
    Warning,
}
```

## 5. エラー型設計

### 5.1 ExportError

```rust
// src/cli/export.rs

#[derive(Debug)]
pub enum ExportError {
    NotInitialized,
    Io(std::io::Error),
    State(StateError),
    Manifest(ManifestError),
    Serialize(serde_json::Error),
    GitError(String),
}

// 既存パターン準拠: impl fmt::Display, impl std::error::Error (source付き), impl From<T>
impl fmt::Display for ExportError { ... }
impl std::error::Error for ExportError { fn source(&self) -> Option<&(dyn std::error::Error + 'static)> { ... } }
impl From<std::io::Error> for ExportError { ... }
impl From<StateError> for ExportError { ... }
impl From<ManifestError> for ExportError { ... }
impl From<serde_json::Error> for ExportError { ... }
```

### 5.2 ImportError

```rust
// src/cli/import_index.rs

#[derive(Debug)]
pub enum ImportError {
    Io(std::io::Error),
    ExistingIndex(PathBuf),
    PathTraversal(String),
    SymlinkDetected(PathBuf),              // シンボリックリンク/ハードリンク検出
    InvalidArchive(String),
    IncompatibleVersion { expected: u32, found: u32 },
    DecompressionBomb { limit: u64 },      // 展開サイズ上限超過
    State(StateError),
    Deserialize(serde_json::Error),
}

// 同様に impl fmt::Display, impl std::error::Error, impl From<T>
```

## 6. 処理フロー

### 6.1 エクスポートフロー

```
commandindex export <OUTPUT_PATH> [--with-embeddings]
  1. .commandindex/ の存在確認（NotInitialized エラー）
  2. IndexState::load() でインデックス状態読み込み
  3. current_git_hash(path) でコミットハッシュ取得（失敗時は None）
  4. ExportMeta を生成（index_root は含めない）
  5. export 前バリデーション:
     - tantivy ドキュメント内の path が相対パスであることを確認
     - symbols.db 内の file_path が相対パスであることを確認
  6. tar::Builder + flate2::GzEncoder でストリーミング圧縮
     - export_meta.json を最初に追加
     - state.json を追加（index_root を placeholder に置換してからパック）
     - manifest.json, symbols.db を追加
     - tantivy/ ディレクトリを再帰的に追加
     - --with-embeddings 時のみ embeddings.db を追加
     - config.local.toml は常に除外
  7. ExportResult を返す（出力パス、サイズ、git hash）
```

### 6.2 インポートフロー

```
commandindex import <INPUT_PATH> [--force]
  1. アーカイブファイルの存在確認
  2. .commandindex/ の既存チェック
     - 存在する場合: --force なしならエラー、ありなら警告後に削除
  3. flate2::GzDecoder + tar::Archive でストリーミング展開
     各エントリに対して:
     a. パストラバーサルチェック（絶対パス拒否、.. 拒否）
     b. シンボリックリンク/ハードリンク拒否（entry_type チェック）
     c. 累積展開サイズチェック（上限: 1GB、エントリ数上限: 10000）
     d. ファイルパーミッション固定（0o644/0o755）
     - 不正検出時は即座にエラー、展開済みファイルをクリーンアップ
  4. export_meta.json を読み込み
     - export_format_version の互換性チェック（前方互換ポリシー）
     - commandindex_version の長さバリデーション
  5. state.json の index_root をインポート先の絶対パスに書き換え
  6. import 後バリデーション:
     - manifest.json 内の各 FileEntry.path が相対パスであることを確認
     - symbols.db 内のパスが相対パスであることを確認
  7. git rev-parse HEAD で現在のコミットハッシュ取得
     - export_meta の git_commit_hash と比較
     - 不一致時に警告メッセージ表示
  8. tantivy インデックスのオープン確認
  9. ImportResult を返す
```

### 6.3 整合性チェックフロー

```
commandindex status --verify
  1. .commandindex/ の存在確認
  2. state.json の読み込みと schema_version チェック
  3. tantivy/ ディレクトリの存在確認
  4. tantivy::Index::open_in_dir() でオープン可能性チェック
  5. manifest.json の読み込みと各ファイルの存在確認
  6. symbols.db のオープンとスキーマバージョン確認
  7. VerifyResult を構築して出力
```

## 7. セキュリティ設計

| 脅威 | 対策 | 実装方針 |
|------|------|---------|
| パストラバーサル（ファイルパス） | 文字列レベルの検証 | 絶対パス拒否 + `..` コンポーネント拒否 + join 後の components() 再チェック。**canonicalize() は使わない**（展開前にファイルが存在しないため） |
| パストラバーサル（シンボリックリンク） | entry_type チェック | `Symlink` / `Link` エントリは即座に拒否。展開済み親ディレクトリの `symlink_metadata()` 確認。clean.rs の SymlinkDetected パターン踏襲 |
| パストラバーサル（ハードリンク） | entry_type チェック | `Link`（ハードリンク）エントリも拒否 |
| 機密情報漏洩（ファイル） | config.local.toml 除外 | エクスポート時に明示的スキップ |
| 機密情報漏洩（パス） | index_root サニタイズ | state.json の index_root を placeholder に置換してパック。export_meta.json には index_root を含めない |
| 圧縮爆弾 | サイズ/エントリ数上限 | 累積展開バイト数上限 (1GB)、個別エントリサイズ上限、エントリ数上限 (10000) |
| 悪意あるメタデータ | デシリアライズ検証 | `#[serde(deny_unknown_fields)]`、文字列長上限チェック |
| 権限昇格 | パーミッション固定 | 展開ファイルのパーミッションを 0o644/0o755 に固定、tarエントリの権限情報は無視 |

### パストラバーサル検証の実装方針

```rust
fn validate_entry_path(entry_path: &Path, target_dir: &Path) -> Result<PathBuf, ImportError> {
    // 1. 絶対パスの拒否
    if entry_path.is_absolute() {
        return Err(ImportError::PathTraversal(format!("absolute path: {:?}", entry_path)));
    }
    // 2. ".." コンポーネントの拒否
    for component in entry_path.components() {
        if matches!(component, std::path::Component::ParentDir) {
            return Err(ImportError::PathTraversal(format!("parent dir: {:?}", entry_path)));
        }
    }
    // 3. 展開先パスの構築
    let full_path = target_dir.join(entry_path);
    // 4. 正規化後の prefix チェック（canonicalize は使わない）
    // components() で再構築して target_dir で始まることを確認
    Ok(full_path)
}

fn validate_entry_type(entry: &tar::Entry<impl Read>) -> Result<(), ImportError> {
    match entry.header().entry_type() {
        tar::EntryType::Symlink | tar::EntryType::Link => {
            Err(ImportError::SymlinkDetected(entry.path()?.to_path_buf()))
        }
        _ => Ok(()),
    }
}
```

## 8. CLIインターフェース設計

### main.rs への追加

```rust
// Commands enum に追加（doc comment は英語で統一）
/// Export index as portable tar.gz archive
Export {
    /// Output file path (.tar.gz)
    output: PathBuf,
    /// Include embedding database
    #[arg(long)]
    with_embeddings: bool,
},
/// Import index from tar.gz archive
Import {
    /// Input archive file path (.tar.gz)
    input: PathBuf,
    /// Overwrite existing index
    #[arg(long)]
    force: bool,
},

// Status バリアントに verify 追加
Status {
    #[arg(long, default_value = ".")]
    path: PathBuf,
    #[arg(long, value_enum, default_value_t = StatusFormat::Human)]
    format: StatusFormat,
    /// Verify index integrity
    #[arg(long)]
    verify: bool,
},
```

### run() 関数シグネチャ（既存パターン準拠）

```rust
// export.rs — 第1引数は base path（既存パターン）
pub fn run(path: &Path, output: &Path, options: &ExportOptions) -> Result<ExportResult, ExportError>

// import_index.rs — 第1引数は base path、第2引数は archive path
pub fn run(path: &Path, archive: &Path, options: &ImportOptions) -> Result<ImportResult, ImportError>

// status.rs — format は別引数のまま、verify を追加
pub fn run(path: &Path, format: StatusFormat, verify: bool, writer: &mut dyn Write) -> Result<(), StatusError>
```

## 9. アーカイブ内部構造

```
index-snapshot.tar.gz
├── export_meta.json          # エクスポートメタデータ（最初のエントリ）
├── state.json                # インデックス状態（index_root はサニタイズ済み）
├── manifest.json             # ファイルマニフェスト（相対パス）
├── symbols.db                # シンボルデータベース（相対パス）
├── tantivy/                  # tantivy インデックスディレクトリ
│   ├── meta.json
│   ├── .managed.json
│   └── *.{del,fast,fieldnorm,idx,pos,store,term}
└── embeddings.db             # (--with-embeddings 時のみ)
```

**注意**: アーカイブ内のパスは `.commandindex/` プレフィックスなしのフラットな構造。インポート時に `.commandindex/` ディレクトリ内に展開される。commandindex.toml（リポジトリルート）はエクスポート対象外。

## 10. 設計判断とトレードオフ

| 判断 | 選択 | 代替案 | 理由 |
|------|------|--------|------|
| CLI設計 | 独立サブコマンド (`export`/`import`) | `index --export`/`index --import` | 単一責任パターンとの整合性、clap排他制御の複雑化回避 |
| メタデータ | 別ファイル (`export_meta.json`) | `state.json` にフィールド追加 | state.json の後方互換性維持、エクスポート固有情報の分離 |
| パス検証 | 手動エントリ展開 | `Archive::unpack()` | セキュリティ：パストラバーサル防止を確実にするため |
| パス検証方式 | 文字列レベル + components() | `canonicalize()` | 展開前にファイルが存在しないため canonicalize() は使えない |
| embeddings.db | デフォルト除外 | デフォルト含む | モデル依存データであり、異なる環境では意味をなさない可能性 |
| アーカイブパス | フラットパス（プレフィックスなし） | `.commandindex/` プレフィックス付き | インポート時の柔軟性、展開先ディレクトリの明示的制御 |
| status拡張 | `verify: bool` 引数追加 | StatusOptions構造体 | YAGNI — CleanOptionsパターン準拠、構造体は実際に複数オプションが必要になってから |
| import モジュール名 | `import_index.rs` | `import.rs` | 既存モジュールは単一単語だが、import はキーワード衝突の可能性回避 |
| snapshot.rs の責務 | ExportMeta の型+読み書きのみ | tar操作も含む | SRP — tar操作は export.rs/import_index.rs の責務 |
| index_root 漏洩防止 | ExportMeta に含めない + state.json サニタイズ | そのまま含める | 絶対パスによるインフラ情報漏洩を防止 |
| 圧縮爆弾対策 | サイズ/エントリ数上限 | ストリーミングのみ | ストリーミングはメモリ節約のみ、ディスク枯渇は防げない |
| git hash 取得 | 分離関数 | snapshot.rs 内に含める | DIP — テスタビリティ向上、外部コマンド依存の分離 |

## 11. 影響範囲

### 直接影響

| ファイル | 変更種別 | 影響度 |
|---------|---------|--------|
| `src/main.rs` | enum バリアント追加 + match 分岐追加 | 低 |
| `src/cli/mod.rs` | モジュール宣言追加 | 低 |
| `src/cli/status.rs` | run() に verify 引数追加、verify ロジック実装 | 中 |
| `src/indexer/mod.rs` | モジュール宣言追加 | 低 |
| `Cargo.toml` | 依存追加 | 低 |
| `tests/cli_args.rs` | help テストに export/import 検証追加 | 低 |
| `tests/cli_status.rs` | run() 呼び出しに verify: false 追加（4箇所） | 低 |

### 新規ファイル

| ファイル | 内容 |
|---------|------|
| `src/cli/export.rs` | エクスポートサブコマンド + ExportResult/ExportError |
| `src/cli/import_index.rs` | インポートサブコマンド + ImportResult/ImportError |
| `src/indexer/snapshot.rs` | ExportMeta 構造体 + 読み書き |
| `tests/cli_export.rs` | エクスポート統合テスト |
| `tests/cli_import.rs` | インポート統合テスト |
| `tests/e2e_export_import.rs` | export → import → search E2Eテスト |

### 間接影響

- 既存の `index`, `search`, `update`, `clean`, `embed`, `context`, `config` コマンド: **影響なし**
- CI/CDパイプライン: **変更不要**（tar/flate2 は純Rust、クロスコンパイル問題なし）
- export 成果物の出力先: `.commandindex/` 外（ユーザー指定パス）のため clean コマンドに影響なし

### import 後の既存コマンドとの整合性

- `update`: import 後に実行可能。index_root 書き換え済みのため差分検出が正常動作する（統合テストで検証必須）
- `clean`: import したインデックスも通常通り削除可能
- `search`: import 後の検索が正常動作する（E2Eテストで検証必須）

## 12. テスト戦略

### 新規テスト

| テストファイル | テスト内容 | 種別 |
|--------------|-----------|------|
| `tests/cli_export.rs` | export 基本動作、NotInitialized エラー | 統合 |
| | config.local.toml がエクスポートに含まれないことの検証 | セキュリティ |
| | embeddings.db のデフォルト除外と --with-embeddings | 統合 |
| `tests/cli_import.rs` | import 基本動作 | 統合 |
| | 既存インデックスありで --force なしのエラー | 統合 |
| | 既存インデックスありで --force のインポート | 統合 |
| | パストラバーサル検出テスト（`../`, 絶対パス） | セキュリティ |
| | シンボリックリンクエントリ拒否テスト | セキュリティ |
| | ハードリンクエントリ拒否テスト | セキュリティ |
| | 圧縮爆弾検出テスト（サイズ上限超過） | セキュリティ |
| | export_format_version 不一致時のエラー | 統合 |
| | コミットハッシュ不一致時の警告 | 統合 |
| `tests/e2e_export_import.rs` | export → import → search の E2E フロー | E2E |
| | import 後に update が正常動作するか | E2E |
| | import 後に tantivy インデックスがオープンできるか | E2E |
| `tests/e2e_verify.rs` | 正常インデックスの verify パス | E2E |
| | 破損インデックスの verify エラー検出 | E2E |

### 既存テスト修正

| テストファイル | 修正内容 |
|--------------|---------|
| `tests/cli_args.rs` | `help_flag_shows_usage` に export/import の検証追加 |
| `tests/cli_status.rs` | `run()` 呼び出しに `verify: false` 追加（4箇所） |

## 13. 品質基準

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |
