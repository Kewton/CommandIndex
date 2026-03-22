# 作業計画書: Issue #89 stdin パイプ入力対応

## Issue概要

**Issue番号**: #89
**タイトル**: [Feature] stdin パイプ入力対応
**サイズ**: M（中）
**優先度**: Medium
**依存Issue**: なし
**ブランチ**: `feature/issue-89-stdin-pipe`（既存）

## 詳細タスク分解

### Phase 1: 基盤実装（stdin 共通ユーティリティ）

- [ ] **Task 1.1**: cli/stdin.rs - StdinError 型定義 + Display/Error 実装
  - 成果物: `src/cli/stdin.rs`
  - 依存: なし
  - 内容: StdinError enum（NotPiped, ReadError, EmptyInput, NoValidPaths, TooManyPaths, InvalidPath）、Display + Error trait 実装

- [ ] **Task 1.2**: cli/stdin.rs - validate_file_path 共通関数
  - 成果物: `src/cli/stdin.rs`
  - 依存: Task 1.1
  - 内容: pub(crate) fn validate_file_path（空チェック, 1024文字上限, null バイト, `..` 禁止, 絶対パス禁止, バックスラッシュ禁止）、normalize_path_prefix（strip_prefix("./")）

- [ ] **Task 1.3**: cli/stdin.rs - read_file_paths_from_stdin + filter_existing_files
  - 成果物: `src/cli/stdin.rs`
  - 依存: Task 1.2
  - 内容: TTY検出（IsTerminal）、stdin.lock().take(MAX_STDIN_BYTES) でバイト上限、1行1パス読み取り、バリデーション、正規化、重複排除、filter_existing_files 共通関数

- [ ] **Task 1.4**: cli/stdin.rs - ユニットテスト
  - 成果物: `src/cli/stdin.rs` (#[cfg(test)] mod tests)
  - 依存: Task 1.3
  - 内容: validate_file_path の各バリデーションルール、normalize_path_prefix、filter_existing_files のテスト

- [ ] **Task 1.5**: cli/mod.rs にモジュール登録
  - 成果物: `src/cli/mod.rs`
  - 依存: Task 1.3
  - 内容: `pub mod stdin;` 追加

### Phase 2: impact サブコマンド実装

- [ ] **Task 2.1**: output/mod.rs - ImpactResult, ImpactFileResult 型定義
  - 成果物: `src/output/mod.rs`
  - 依存: なし
  - 内容: ImpactResult（input_files, impacted_files, total_input_files, total_impacted_files）、ImpactFileResult（file_path, score, relation_types, impacted_by）

- [ ] **Task 2.2**: output/json.rs, human.rs, path.rs - impact フォーマッタ
  - 成果物: `src/output/json.rs`, `src/output/human.rs`, `src/output/path.rs`
  - 依存: Task 2.1
  - 内容: format_impact_json（単一JSONオブジェクト、json!マクロで構築）、format_impact_human、format_impact_path + output/mod.rs に format_impact_results ディスパッチ関数

- [ ] **Task 2.3**: cli/impact.rs - ImpactError + run_impact + aggregate_impact
  - 成果物: `src/cli/impact.rs`
  - 依存: Task 1.3, Task 2.2
  - 内容: ImpactError enum（Display + Error 実装）、run_impact（引数 or stdin からファイル取得、存在チェック、インデックス確認、集約、出力）、aggregate_impact（HashMap集約、最大スコア、union relation_types、impacted_by追跡、入力ファイル除外、ソート）

- [ ] **Task 2.4**: cli/mod.rs + main.rs - Impact サブコマンド登録
  - 成果物: `src/cli/mod.rs`, `src/main.rs`
  - 依存: Task 2.3
  - 内容: `pub mod impact;` 追加、Commands enum に Impact バリアント追加（files: Vec<String>, format: OutputFormat (value_enum + default_value_t), limit: Option<usize>）、match 分岐に Impact ハンドラ追加

### Phase 3: search --related-stdin 実装

- [ ] **Task 3.1**: cli/search.rs - SearchError に Stdin バリアント追加
  - 成果物: `src/cli/search.rs`
  - 依存: Task 1.1
  - 内容: SearchError に Stdin(StdinError) バリアント追加、Display/source 実装更新

- [ ] **Task 3.2**: cli/context.rs - merge_related_results を pub(crate) に変更
  - 成果物: `src/cli/context.rs`
  - 依存: なし
  - 内容: fn merge_related_results → pub(crate) fn merge_related_results、validate_file_path を stdin.rs の共通関数に置き換え

- [ ] **Task 3.3**: cli/search.rs - run_related_search_from_stdin
  - 成果物: `src/cli/search.rs`
  - 依存: Task 1.3, Task 3.1, Task 3.2
  - 内容: stdin からパス読み取り、存在チェック、インデックス確認、各ファイルに find_related 実行、context.rs の merge_related_results で集約、format_related_results で出力

- [ ] **Task 3.4**: main.rs - Search に --related-stdin オプション追加
  - 成果物: `src/main.rs`
  - 依存: Task 3.3
  - 内容: Search に related_stdin: bool 追加（conflicts_with_all に query, symbol, related, semantic, tag, path, file_type, heading, workspace, no_semantic, rerank）、match 分岐で related_stdin を先にチェック、(None,None,None,None) エラーメッセージに --related-stdin 追記

### Phase 4: テスト

- [ ] **Task 4.1**: tests/cli_args.rs - CLI引数テスト更新
  - 成果物: `tests/cli_args.rs`
  - 依存: Task 2.4, Task 3.4
  - 内容: help_flag_shows_usage に impact 追加、impact --help テスト、--related と --related-stdin の排他テスト、--related-stdin と --tag/--no-semantic/--rerank の排他テスト

- [ ] **Task 4.2**: tests/e2e_impact.rs - impact E2E テスト
  - 成果物: `tests/e2e_impact.rs`
  - 依存: Task 2.4
  - 内容: stdin 入力（.write_stdin()）、引数入力、JSON/human/path 出力検証、TTYエラー、空stdin、有効パス0件、関連結果0件、複数ファイルの集約・重複排除

- [ ] **Task 4.3**: tests/e2e_related_search.rs - related-stdin E2E テスト
  - 成果物: `tests/e2e_related_search.rs`
  - 依存: Task 3.4
  - 内容: --related-stdin の stdin 入力、集約ルール（union + 最大スコア）、排他確認

- [ ] **Task 4.4**: tests/output_format.rs - ImpactResult フォーマットテスト
  - 成果物: `tests/output_format.rs`
  - 依存: Task 2.2
  - 内容: ImpactResult の JSON/human/path 各フォーマットの出力検証

### Phase 5: 品質チェック・仕上げ

- [ ] **Task 5.1**: 全体品質チェック
  - 内容: cargo build, cargo clippy --all-targets -- -D warnings, cargo test --all, cargo fmt --all -- --check
  - 依存: Phase 4 全タスク

## タスク依存関係

```
Phase 1 (基盤)          Phase 2 (impact)         Phase 3 (search)
1.1 → 1.2 → 1.3 → 1.4  2.1 → 2.2 → 2.3 → 2.4  3.1 ─┐
         │    1.5              │                  3.2 ─┼→ 3.3 → 3.4
         └───────────────────→ 2.3                     │
         └─────────────────────────────────────────────→ 3.3

Phase 4 (テスト) ← Phase 2 + Phase 3
Phase 5 (品質)   ← Phase 4
```

## 実装順序（推奨）

1. Phase 1 を完了（stdin 共通基盤）
2. Phase 2 と Phase 3 を並行実装（impact と search --related-stdin は独立）
3. Phase 4 テスト作成
4. Phase 5 品質チェック

## 品質チェック項目

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |

## Definition of Done

- [ ] すべてのタスクが完了
- [ ] テストが全パス（既存 + 新規）
- [ ] clippy警告ゼロ
- [ ] cargo fmt 差分なし
- [ ] impact サブコマンドが stdin / 引数 両方で動作
- [ ] search --related-stdin が動作
- [ ] エッジケース（TTY, 空stdin, 有効パス0件, 結果0件）が正しく処理される

## 見積もり

| Phase | タスク数 | 概要 |
|-------|---------|------|
| Phase 1 | 5 | stdin 共通基盤 |
| Phase 2 | 4 | impact サブコマンド |
| Phase 3 | 4 | search --related-stdin |
| Phase 4 | 4 | テスト |
| Phase 5 | 1 | 品質チェック |
| **合計** | **18** | |
