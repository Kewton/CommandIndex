# 作業計画: Issue #44

## Issue: searchコマンドのスニペット表示行数・文字数をCLIオプションで動的に調整可能にする
**Issue番号**: #44
**サイズ**: S
**優先度**: Medium
**依存Issue**: なし

## 設計方針書
`dev-reports/design/issue-44-snippet-options-design-policy.md`

---

## 詳細タスク分解

### Phase 1: 実装タスク

#### Task 1.1: SnippetConfig 構造体定義
- **ファイル**: `src/output/mod.rs`
- **内容**:
  - `pub struct SnippetConfig { pub lines: usize, pub chars: usize }` を追加
  - `#[derive(Debug, Clone, Copy)]` と `Default` トレイト実装（lines=2, chars=120）
- **依存**: なし

#### Task 1.2: format_human() シグネチャ変更
- **ファイル**: `src/output/human.rs`
- **内容**:
  - `format_human()` に `snippet_config: SnippetConfig` 引数追加
  - import に `use crate::output::SnippetConfig;` 追加
  - 28行目のハードコード値を snippet_config の値に置換
  - 0=無制限の制御ロジックを format_human 側に実装
- **依存**: Task 1.1

#### Task 1.3: format_results() 内の format_human 呼び出し修正
- **ファイル**: `src/output/mod.rs`
- **内容**:
  - `format_results()` 内の `OutputFormat::Human` 分岐で `format_human()` に `SnippetConfig::default()` を渡す
  - format_results のシグネチャは不変
- **依存**: Task 1.2

#### Task 1.4: cli/search.rs run() 修正
- **ファイル**: `src/cli/search.rs`
- **内容**:
  - `run()` に `snippet_config: SnippetConfig` 引数追加
  - import に `SnippetConfig` 追加
  - Human 形式の場合に `format_human()` を直接呼び出し、それ以外は `format_results()` を使用
- **依存**: Task 1.3

#### Task 1.5: main.rs CLIオプション追加
- **ファイル**: `src/main.rs`
- **内容**:
  - `Search` enum に `snippet_lines: usize` と `snippet_chars: usize` フィールド追加
  - destructuring パターンに `snippet_lines`, `snippet_chars` 追加
  - `SnippetConfig` 構築して `run()` に渡す
  - import に `use commandindex::output::SnippetConfig;` 追加
- **依存**: Task 1.4

### Phase 2: テストタスク

#### Task 2.1: 新規テスト追加
- **ファイル**: `tests/output_format.rs`
- **内容**:
  - import に `SnippetConfig` 追加
  - `format_human_to_string()` ヘルパー追加
  - テストケース:
    - `test_snippet_custom_lines`: lines=5 で5行分表示
    - `test_snippet_custom_chars`: chars=50 で50文字切り詰め
    - `test_snippet_lines_zero_unlimited`: lines=0, chars=0 で全文表示
    - `test_snippet_chars_zero_unlimited`: chars=0 で文字数無制限（単一行）
    - `test_snippet_default_unchanged`: デフォルト値で既存動作同一
- **依存**: Task 1.5

#### Task 2.2: 既存テスト回帰確認
- **内容**: `cargo test --all` で全テストパス確認
- **依存**: Task 2.1

### Phase 3: 品質チェック

#### Task 3.1: 品質チェック全パス
- **内容**:
  - `cargo build` — エラー0件
  - `cargo clippy --all-targets -- -D warnings` — 警告0件
  - `cargo test --all` — 全テストパス
  - `cargo fmt --all -- --check` — 差分なし
- **依存**: Task 2.2

---

## 実行順序

```
Task 1.1 (SnippetConfig定義)
  → Task 1.2 (format_human変更)
    → Task 1.3 (format_results内の呼び出し修正)
      → Task 1.4 (cli/search.rs変更)
        → Task 1.5 (main.rs CLIオプション追加)
          → Task 2.1 (新規テスト)
            → Task 2.2 (回帰確認)
              → Task 3.1 (品質チェック)
```

## Definition of Done

- [x] SnippetConfig 構造体が定義されている
- [x] format_human が SnippetConfig を受け取る
- [x] --snippet-lines / --snippet-chars CLIオプションが動作する
- [x] 0=無制限で全文表示される
- [x] デフォルト値で既存動作と同一
- [x] 全テストパス
- [x] clippy 警告0件
- [x] cargo fmt 差分なし
