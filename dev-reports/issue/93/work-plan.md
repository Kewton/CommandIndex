# 作業計画書 - Issue #93: ファイル変更監視（watch モード）

## Issue概要

**Issue番号**: #93
**タイトル**: [Feature] ファイル変更監視（watch モード）
**サイズ**: L（大）
**優先度**: Medium
**依存Issue**: なし（#88 --index-path は本Issue スコープ外）

## 詳細タスク分解

### Phase 1: 基盤（依存関係・型定義）

- [ ] **Task 1.1**: Cargo.toml に依存追加
  - 成果物: `Cargo.toml`
  - 内容: `notify = "7"`, `ctrlc = { version = "3", features = ["termination"] }` を追加
  - 依存: なし

- [ ] **Task 1.2**: WatchError 型定義
  - 成果物: `src/cli/watch.rs`
  - 内容:
    - `WatchError` enum（Notify, Index, Io, Signal）
    - `is_recoverable()` メソッド
    - `Display`, `Error`, `From` トレイト実装
  - 依存: Task 1.1

- [ ] **Task 1.3**: CLI オプション定義
  - 成果物: `src/main.rs`, `src/cli/mod.rs`
  - 内容:
    - `Commands::Watch { path, debounce, with_embedding }` を enum に追加
    - `src/cli/mod.rs` に `pub mod watch;` 追加
  - 依存: Task 1.2

### Phase 2: コアロジック実装

- [ ] **Task 2.1**: イベントフィルタリング関数
  - 成果物: `src/cli/watch.rs`
  - 内容:
    - `is_relevant_event(path, base_dir)` 関数
    - `.commandindex/` 除外、対象拡張子チェック（md, ts, tsx, py）
    - `canonicalize()` によるベースディレクトリ検証
  - 依存: Task 1.2

- [ ] **Task 2.2**: Debouncer 構造体
  - 成果物: `src/cli/watch.rs`
  - 内容:
    - `Debouncer` 構造体（first_event_time, last_event_time, debounce_duration, max_wait）
    - `notify_event()`, `should_trigger()`, `reset()` メソッド
    - starvation 防止: max_wait（デバウンスの5倍）で強制トリガー
  - 依存: なし

- [ ] **Task 2.3**: メイン watch ループ
  - 成果物: `src/cli/watch.rs`
  - 内容:
    - `pub fn run(path, debounce, with_embedding) -> Result<(), WatchError>`
    - 起動チェック（path存在、インデックス構築済み確認）
    - `notify::RecommendedWatcher` + `mpsc::channel` 初期化
    - `ctrlc` + `AtomicBool` によるシグナルハンドリング
    - メインループ: イベント受信 → フィルタ → デバウンス → `run_incremental()` 呼び出し
    - エラー分類による回復可能/回復不能の処理分岐
    - ロック競合時のリトライ（最大3回、指数バックオフ）
    - グレースフルシャットダウン
  - 依存: Task 2.1, Task 2.2

- [ ] **Task 2.4**: main.rs の match ブランチ追加
  - 成果物: `src/main.rs`
  - 内容:
    - `Commands::Watch { path, debounce, with_embedding }` の match ブランチ
    - `watch::run()` 呼び出しとエラーハンドリング
  - 依存: Task 2.3

### Phase 3: テスト

- [ ] **Task 3.1**: WatchError 単体テスト
  - 成果物: `src/cli/watch.rs`（モジュール内テスト）
  - 内容:
    - `is_recoverable()` のテスト（各 IndexError バリアントの分類確認）

- [ ] **Task 3.2**: イベントフィルタリング単体テスト
  - 成果物: `src/cli/watch.rs`（モジュール内テスト）
  - 内容:
    - `.commandindex/` 配下は無視
    - 対象拡張子のみ通過
    - 対象外拡張子は無視

- [ ] **Task 3.3**: Debouncer 単体テスト
  - 成果物: `src/cli/watch.rs`（モジュール内テスト）
  - 内容:
    - デバウンス時間経過後にトリガー
    - デバウンス時間内はトリガーしない
    - max_wait による starvation 防止

- [ ] **Task 3.4**: CLI パーステスト
  - 成果物: `tests/cli_args.rs`
  - 内容:
    - `commandindex watch` デフォルトオプション
    - `commandindex watch --path /tmp --debounce 3 --with-embedding`
    - help 出力に `watch` が含まれることを検証

### Phase 4: 品質チェック・最終調整

- [ ] **Task 4.1**: 品質チェック
  - 内容: `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all`, `cargo fmt --all -- --check`

- [ ] **Task 4.2**: 既存テスト影響確認
  - 内容: 全既存テスト（32ファイル）がパスすることを確認

## 実装順序

```
Task 1.1 (Cargo.toml)
  → Task 1.2 (WatchError)
    → Task 1.3 (CLI定義)
      → Task 2.1 (フィルタ) + Task 2.2 (Debouncer) ← 並列可
        → Task 2.3 (メインループ)
          → Task 2.4 (main.rs match)
            → Task 3.1-3.4 (テスト) ← 並列可
              → Task 4.1-4.2 (品質チェック)
```

## 品質チェック項目

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |

## Definition of Done

- [ ] すべてのタスクが完了
- [ ] cargo test --all 全パス
- [ ] cargo clippy 警告ゼロ
- [ ] cargo fmt 差分なし
- [ ] 受け入れ基準11項目すべて満たす
