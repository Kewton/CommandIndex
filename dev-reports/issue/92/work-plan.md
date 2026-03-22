# 作業計画 - Issue #92

## Issue: [Feature] diff サブコマンド（影響範囲の比較・共通ファイル検出）
**Issue番号**: #92
**サイズ**: M
**優先度**: Medium
**依存Issue**: なし（#90 impact は未実装だが依存関係なし）

---

## 詳細タスク分解

### Phase 1: 出力型定義（Output Layer）

- [ ] **Task 1.1**: `DiffResult` 型と `format_diff_results()` を `src/output/mod.rs` に追加
  - 成果物: `src/output/mod.rs`
  - 依存: なし
  - 内容:
    - `DiffResult` 構造体（file_a, file_b, only_a, only_b, overlap）
    - `format_diff_results()` ディスパッチ関数

- [ ] **Task 1.2**: Human形式フォーマッタを `src/output/human.rs` に追加
  - 成果物: `src/output/human.rs`
  - 依存: Task 1.1
  - 内容: `format_diff_human()` 関数

- [ ] **Task 1.3**: JSON形式フォーマッタを `src/output/json.rs` に追加
  - 成果物: `src/output/json.rs`
  - 依存: Task 1.1
  - 内容: `format_diff_json()` 関数（単一JSONオブジェクト、overlap_count付き）

- [ ] **Task 1.4**: Path形式フォーマッタを `src/output/path.rs` に追加
  - 成果物: `src/output/path.rs`
  - 依存: Task 1.1
  - 内容: `format_diff_path()` 関数（overlapのみ出力）

### Phase 2: コアロジック実装（CLI Handler Layer）

- [ ] **Task 2.1**: `src/cli/diff.rs` 新規作成 + `src/cli/mod.rs` にモジュール宣言追加
  - 成果物: `src/cli/diff.rs`, `src/cli/mod.rs`
  - 依存: Task 1.1
  - 内容:
    - `run_diff(files, limit, format)` 関数
    - パスバリデーション（絶対パス拒否、`..` 拒否、同一ファイル拒否）
    - `normalize_path()` による正規化
    - インデックス存在チェック
    - `RelatedSearchEngine::find_related()` 2回呼び出し
    - `HashSet` 集合演算（intersection, difference）
    - 出力委譲

### Phase 3: CLI統合（CLI Layer）

- [ ] **Task 3.1**: `src/main.rs` に `Commands::Diff` バリアント追加
  - 成果物: `src/main.rs`
  - 依存: Task 2.1
  - 内容:
    - `Commands` enum に `Diff` バリアント追加（files, format, limit）
    - マッチアームで `commandindex::cli::diff::run_diff()` 呼び出し
    - `--limit` に `value_parser` で上限設定（1..=10000）

### Phase 4: テスト

- [ ] **Task 4.1**: E2Eテスト `tests/e2e_diff.rs` 新規作成
  - 成果物: `tests/e2e_diff.rs`
  - 依存: Task 3.1
  - テストケース:
    - `diff_overlap_detected` - 共通関連ファイル検出
    - `diff_only_a_only_b_correct` - 片方のみ関連ファイル分類
    - `diff_no_overlap` - 重複なし
    - `diff_json_format` - JSON出力構造
    - `diff_human_format` - Human出力形式
    - `diff_path_format` - Path出力（overlapのみ）
    - `diff_same_file_error` - 同一ファイルエラー
    - `diff_no_index_error` - インデックス未作成エラー

- [ ] **Task 4.2**: 既存テスト更新 `tests/cli_args.rs`
  - 成果物: `tests/cli_args.rs`
  - 依存: Task 3.1
  - 内容: `help_flag_shows_usage` に diff 検証行追加

### Phase 5: 品質チェック

- [ ] **Task 5.1**: 全品質チェック実行
  - `cargo build`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test --all`
  - `cargo fmt --all -- --check`

---

## タスク依存関係

```
Task 1.1 (DiffResult型)
  ├── Task 1.2 (Human formatter)
  ├── Task 1.3 (JSON formatter)
  ├── Task 1.4 (Path formatter)
  └── Task 2.1 (run_diff コアロジック)
       └── Task 3.1 (CLI統合)
            ├── Task 4.1 (E2Eテスト)
            ├── Task 4.2 (既存テスト更新)
            └── Task 5.1 (品質チェック)
```

---

## TDD実装順序

設計方針書に基づき、TDDサイクルで実装する際の推奨順序:

1. **Red**: E2Eテスト（Task 4.1）のテストケースを先に記述（コンパイル不可でOK）
2. **Green**: Task 1.1 → 1.2/1.3/1.4 → 2.1 → 3.1 の順で実装してテスト通過
3. **Refactor**: Task 5.1 で品質チェック

---

## Definition of Done

- [ ] すべてのタスク（Task 1.1〜5.1）が完了
- [ ] `cargo test --all` 全テストパス
- [ ] `cargo clippy --all-targets -- -D warnings` 警告ゼロ
- [ ] `cargo fmt --all -- --check` 差分なし
- [ ] `cargo build` エラーゼロ
