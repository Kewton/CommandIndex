# 作業計画書: Issue #7 - index コマンド実装

## 作業ステップ

### Step 1: モジュール骨格作成
- `src/cli/mod.rs` 作成（`pub mod index;`）
- `src/cli/index.rs` 作成（IndexError, IndexSummary, run() のスタブ）
- `src/lib.rs` に `pub mod cli` 追加

### Step 2: CLIオプション変更
- `src/main.rs` の `Commands::Index` に `path: PathBuf` フィールド追加
- `Commands::Index { path }` で `cli::index::run(&path)` を呼び出し
- サマリー表示ロジックを main.rs に実装

### Step 3: IndexError 実装
- IndexError enum（Io, Parse, Writer, State, Manifest, Ignore）
- Display, std::error::Error, From トレイト実装

### Step 4: index コマンド本体実装（cli/index.rs の run()）
- ディレクトリ存在確認
- IgnoreFilter 構築
- walkdir + IgnoreFilter でファイル列挙
- 既存 tantivy ディレクトリ削除
- IndexWriterWrapper::open()
- ファイル毎の parse_file() → SectionDoc 変換 → add_section()
- Manifest / IndexState 生成・保存
- IndexSummary 返却

### Step 5: 既存テスト更新
- `tests/cli_args.rs` の index 関連テスト更新

### Step 6: 新規統合テスト作成
- `tests/cli_index.rs` に11テストケース実装

### Step 7: 品質チェック
- cargo build / clippy / test / fmt 全パス確認

## TDD実装順序
1. テストを先に書く（Red）
2. 最小実装で通す（Green）
3. リファクタリング（Refactor）
