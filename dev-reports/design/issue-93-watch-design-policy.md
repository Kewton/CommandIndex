# 設計方針書 - Issue #93: ファイル変更監視（watch モード）

## 1. 概要

ファイルシステムの変更を監視し、変更があれば自動で `update`（インクリメンタル更新）を実行する `watch` サブコマンドを追加する。

## 2. システムアーキテクチャ上の位置づけ

```
┌─────────────────────────────────────────────┐
│                  main.rs                     │
│           Commands::Watch                    │
└────────────────┬────────────────────────────┘
                 │
┌────────────────▼────────────────────────────┐
│             cli/watch.rs (新規)              │
│  ┌──────────────────────────────────────┐   │
│  │  run_watch()                          │   │
│  │  ├─ notify::RecommendedWatcher 初期化 │   │
│  │  ├─ ctrlc ハンドラ登録               │   │
│  │  ├─ メインループ                      │   │
│  │  │  ├─ イベント受信                   │   │
│  │  │  ├─ 拡張子・パスフィルタ           │   │
│  │  │  ├─ デバウンス待機                 │   │
│  │  │  └─ run_incremental() 呼び出し    │   │
│  │  └─ グレースフルシャットダウン        │   │
│  └──────────────────────────────────────┘   │
└────────────────┬────────────────────────────┘
                 │ 既存モジュール再利用
    ┌────────────┼────────────┐
    ▼            ▼            ▼
cli/index.rs  parser/     indexer/
run_incremental() ignore.rs  diff.rs
```

## 3. モジュール設計

### 3.1 新規ファイル

| ファイル | 責務 |
|---------|------|
| `src/cli/watch.rs` | watch サブコマンドのオーケストレーション |

**責務分離**（SRP対応）:
- デバウンスロジック: `Debouncer` 構造体として独立（watch.rs 内のプライベート構造体）
- イベントフィルタリング: `is_relevant_event()` 関数として独立
- エラー分類: `WatchError::is_recoverable()` メソッドで一箇所に集約

### 3.2 変更ファイル

| ファイル | 変更内容 |
|---------|---------|
| `src/main.rs` | `Commands::Watch` バリアント追加、match ブランチ追加 |
| `src/cli/mod.rs` | `pub mod watch;` 追加（`workspace` の前、アルファベット順） |
| `Cargo.toml` | `notify`, `ctrlc` 依存追加 |

### 3.3 影響を受けない既存モジュール

`cli/index.rs` の `run_incremental()` はそのまま呼び出すため、既存コードの変更は不要。

### 3.4 制限事項

- workspace 横断の watch は未対応（単一リポジトリのみ）
- 公開型は `run()` 関数と `WatchError` のみ。内部ヘルパーは非公開

## 4. 型設計

### 4.1 CLI オプション（clap derive）

```rust
/// ファイル変更を監視して自動 update
Watch {
    /// インデックス対象パス
    #[arg(long, default_value = ".")]
    path: PathBuf,

    /// デバウンス間隔（秒、最小0.2秒）
    #[arg(long, default_value = "1")]
    debounce: u64,

    /// 更新時に embedding も生成
    #[arg(long)]
    with_embedding: bool,
}
```

注意: `--path` は `short` フラグなし（既存コマンドとの一貫性）

### 4.2 WatchError

```rust
#[derive(Debug)]
pub enum WatchError {
    /// notify crate のエラー
    Notify(notify::Error),
    /// インデックス操作エラー（回復可能/不能を含む）
    Index(IndexError),
    /// I/O エラー
    Io(std::io::Error),
    /// ctrlc ハンドラ登録エラー
    Signal(ctrlc::Error),
}

impl WatchError {
    /// 回復可能なエラーかどうかを判定
    pub fn is_recoverable(&self) -> bool {
        match self {
            WatchError::Index(e) => match e {
                IndexError::IndexNotFound
                | IndexError::SchemaVersionMismatch
                | IndexError::IndexCorrupted(_)
                | IndexError::Config(_) => false,
                _ => true,  // Io, Parse, Writer, Diff 等は回復可能
            },
            WatchError::Notify(_) => false,
            WatchError::Io(_) => true,
            WatchError::Signal(_) => false,
        }
    }
}

impl std::fmt::Display for WatchError { ... }
impl std::error::Error for WatchError { ... }
impl From<IndexError> for WatchError { ... }
impl From<notify::Error> for WatchError { ... }
impl From<std::io::Error> for WatchError { ... }
```

### 4.3 エラー分類

| IndexError バリアント | 分類 | watch での挙動 |
|---------------------|------|---------------|
| `Io`, `Parse`, `Writer`, `Diff`, `CodeParse` | 回復可能 | ログ出力して監視継続 |
| `IndexNotFound`, `SchemaVersionMismatch`, `IndexCorrupted` | 回復不能 | エラー出力して終了 |
| `Manifest`, `Ignore`, `State` | 回復可能 | ログ出力して監視継続 |
| `SymbolStore`, `Embedding`, `EmbeddingStore` | 回復可能 | ログ出力して監視継続 |
| `Config` | 回復不能 | エラー出力して終了 |

## 5. 処理フロー

### 5.1 メインフロー

```
1. 起動時チェック
   ├─ path を canonicalize() で正規化
   ├─ path の存在確認
   ├─ .commandindex/ の存在確認（インデックス構築済みか）
   └─ インデックス未構築 → エラーメッセージ出力して終了

2. 初期化
   ├─ AtomicBool (running) フラグ作成
   ├─ ctrlc ハンドラ登録（running = false に設定、SIGINT + SIGTERM）
   ├─ notify::RecommendedWatcher 作成
   ├─ std::sync::mpsc::channel でイベント受信チャネル作成
   └─ watcher.watch(path, RecursiveMode::Recursive) で監視開始

3. 起動メッセージ表示
   └─ "Watching {path} for changes... (press Ctrl+C to stop)"

4. メインループ (while running.load(Ordering::Relaxed))
   ├─ channel.recv_timeout(100ms) でイベント受信
   ├─ is_relevant_event() でフィルタリング
   │   ├─ .commandindex/ 配下 → 無視
   │   ├─ 対象拡張子以外 → 無視
   │   └─ canonicalize() 後ベースディレクトリ外 → 無視
   ├─ Debouncer にイベント通知
   ├─ Debouncer が発火条件を満たしたら run_incremental() 呼び出し
   │   ├─ 成功 → サマリー表示
   │   ├─ 回復可能エラー → warn! ログ出力して継続
   │   └─ 回復不能エラー → error! 出力して終了
   └─ 前回の run_incremental() 実行中は次のトリガーをスキップ

5. シャットダウン
   └─ "Watch stopped." 表示
```

### 5.2 デバウンスアルゴリズム

```rust
struct Debouncer {
    first_event_time: Option<Instant>,
    last_event_time: Option<Instant>,
    debounce_duration: Duration,
    max_wait: Duration,  // 最大待機時間（デバウンスの5倍）
}

impl Debouncer {
    fn notify_event(&mut self) {
        if self.first_event_time.is_none() {
            self.first_event_time = Some(Instant::now());
        }
        self.last_event_time = Some(Instant::now());
    }

    fn should_trigger(&self) -> bool {
        // 最後のイベントから debounce_duration 経過
        // OR 最初のイベントから max_wait 経過（starvation 防止）
    }

    fn reset(&mut self) { ... }
}
```

**starvation 防止**: 連続的なイベントバーストで更新が無期限に遅延しないよう、`max_wait`（デバウンスの5倍）で強制トリガー。

## 6. 依存関係の追加

### 6.1 Cargo.toml

```toml
notify = "7"                                         # ファイルシステム監視
ctrlc = { version = "3", features = ["termination"] } # SIGINT + SIGTERM ハンドリング
```

### 6.2 選定理由

| crate | 理由 |
|-------|------|
| `notify` v7 | Rust エコシステムで最も成熟したFS監視ライブラリ。RecommendedWatcher でプラットフォーム自動選択（macOS: FSEvents, Linux: inotify, Windows: ReadDirectoryChanges） |
| `ctrlc` v3 | 最小限のSIGINT/SIGTERMハンドリング。`termination` feature で SIGTERM もサポート。AtomicBoolパターンとの組み合わせが容易 |

## 7. 並行性・排他制御設計

### 7.1 tantivy ロック競合

`run_incremental()` は内部で `IndexWriterWrapper::open_existing()` を呼び、tantivy の `.tantivy-writer.lock` を取得する。watch 実行中に別プロセスで `update` / `index` が走ると競合する。

### 7.2 対応方針

```
run_incremental() 呼び出し
  ├─ 成功 → 通常処理
  ├─ Writer(TantivyError) でロック関連
  │   ├─ リトライ（最大3回、指数バックオフ 100ms/200ms/400ms）
  │   ├─ リトライ上限超過 → warn! ログ出力、次回デバウンスサイクルでリトライ
  │   └─ "Another process is using the index, skipping..."
  └─ 回復不能エラー → 終了
```

tantivy のロック取得失敗は `Writer(WriterError)` として伝播するため、このエラーを回復可能として扱い、スキップ＆リトライで対応する。

### 7.3 同時実行の制約

- watch 実行中の `update` / `index` 同時実行は推奨しない（ドキュメントに明記）
- 前回の `run_incremental()` が実行中の場合、新たなトリガーはスキップ

## 8. テスト戦略

### 8.1 単体テスト

| テスト | 内容 |
|--------|------|
| `Debouncer` | タイマーベースのデバウンス処理、starvation 防止の max_wait |
| `is_relevant_event()` | 拡張子チェック、`.commandindex/` 除外、ベースディレクトリ外の除外 |
| `WatchError::is_recoverable()` | 回復可能/回復不能の分類が正しいか |

### 8.2 CLIパーステスト

`tests/cli_args.rs` に Watch サブコマンドのパーステストを追加:
- `commandindex watch` （デフォルトオプション）
- `commandindex watch --path /tmp --debounce 3 --with-embedding`
- help 出力に `watch` が含まれることを検証

### 8.3 統合テスト

`tests/e2e_update.rs` を参考に、tempdir + notify でファイル変更→インデックス更新の E2E テストを検討。ただし、タイミング依存のテストは不安定になりやすいため、手動テスト手順の文書化で補完。

## 9. セキュリティ設計

| 脅威 | 対策 | 優先度 |
|------|------|--------|
| TOCTOU（シンボリックリンク置換） | `run_incremental()` 内でファイルを開く前に `canonicalize()` でベースディレクトリ配下であることを検証 | 高 |
| パストラバーサル | `--path` 受け取り直後に `canonicalize()` で正規化。イベントパスも正規化後にベースディレクトリとの前方一致を検証 | 高 |
| シンボリックリンク追従 | notify の `RecursiveMode::Recursive` はシンボリックリンクを追従しない設定を使用 | 中 |
| リソース枯渇（イベントバースト） | デバウンスで `run_incremental()` の呼び出し頻度を制限。デバウンス最小値 200ms をハードコード | 中 |

## 10. 設計判断とトレードオフ

### 判断1: run_incremental() をそのまま再利用

- **選択**: notify イベント後に `run_incremental()` をフル実行（全ファイルスキャン + 差分検知）
- **理由**: 実装のシンプルさを優先。`run_incremental()` は内部でハッシュベースの差分検知を行うため、変更がないファイルは高速にスキップされる
- **トレードオフ**: 大規模リポジトリでは `walkdir` 全走査が毎回発生するため非効率。将来の最適化として notify イベントのファイルパスを直接利用する軽量パスを検討

### 判断2: daemon モードをスコープ外

- **選択**: フォアグラウンド実行のみ
- **理由**: daemon化はPIDファイル管理、二重起動防止、停止コマンド、プラットフォーム依存処理が複雑。MVPとしてフォアグラウンドで十分
- **トレードオフ**: バックグラウンド実行が必要な場合は `nohup` や `screen`/`tmux` で代替

### 判断3: 同期的な std::sync::mpsc チャネル

- **選択**: `std::sync::mpsc::channel` でイベント受信（async ランタイム不使用）
- **理由**: プロジェクト全体が同期コードであり、非同期ランタイム（tokio等）の導入は過剰。notify v7 は同期チャネルベースのAPIを提供
- **トレードオフ**: 将来的に非同期処理が必要になった場合は要リファクタリング

### 判断4: watch 側のフィルタリングは最小限

- **選択**: watch 側では拡張子チェックと `.commandindex/` 除外のみ。詳細な `.cmindexignore` フィルタリングは `run_incremental()` 内の既存ロジックに委ねる
- **理由**: DRY 原則。フィルタリングロジックの二重管理を避ける
- **トレードオフ**: `.cmindexignore` で無視されるファイルの変更でも `run_incremental()` が呼ばれるが、内部で高速にスキップされる

## 11. 品質基準

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |
