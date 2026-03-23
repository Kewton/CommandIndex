# 作業計画: Issue #91 --changed-since オプション

## Issue 概要
**Issue番号**: #91
**タイトル**: [Feature] --changed-since オプション（Git 履歴ベースの変更検索）
**サイズ**: M
**優先度**: Medium
**依存Issue**: #90 impact サブコマンド（マージ済み）

## 詳細タスク分解

### Phase 1: 基盤実装（TDD: テスト先行）

#### Task 1.1: Git 操作モジュール新規作成
- **成果物**: `src/cli/git.rs`, `src/cli/mod.rs` 更新
- **依存**: なし
- **内容**:
  - `GitError` enum 定義
  - `validate_changed_since_input()` バリデーション関数
  - `get_changed_files()` Git 変更ファイル取得関数
  - `pub mod git;` を `src/cli/mod.rs` に追加
- **テスト先行**:
  - `validate_changed_since_input()` の単体テスト（正常系・異常系）
  - 先頭 `-` 拒否、制御文字拒否、256文字超拒否

#### Task 1.2: impact.rs の pub(crate) 化
- **成果物**: `src/cli/impact.rs` 修正
- **依存**: なし
- **内容**:
  - `aggregate_impact()` を `fn` → `pub(crate) fn` に変更
  - `MAX_INPUT_FILES` を `pub(crate)` に変更
- **確認**: 既存テスト（e2e_impact.rs）が壊れないこと

#### Task 1.3: エラー型変換の実装
- **成果物**: `src/cli/search.rs` に追加
- **依存**: Task 1.1, Task 1.2
- **内容**:
  - `From<ImpactError> for SearchError` 実装
  - `From<GitError> for SearchError` 実装

#### Task 1.4: CLI 引数定義の追加
- **成果物**: `src/main.rs` 修正
- **依存**: なし
- **内容**:
  - `changed_since: Option<String>` を Search struct に追加
  - `conflicts_with_all` 設定
  - デストラクチャリングに `changed_since` 追加
- **テスト先行**: `tests/cli_args.rs` に排他制御テスト追加

#### Task 1.5: メイン検索処理の実装
- **成果物**: `src/cli/search.rs` に `run_changed_since_search()` 追加
- **依存**: Task 1.1, 1.2, 1.3, 1.4
- **内容**:
  - Git 変更ファイル取得 → 存在チェック → aggregate_impact() → 出力
  - main.rs に if let 分岐追加（related_stdin の直後、match の直前）

### Phase 2: テスト

#### Task 2.1: CLI 引数テスト
- **成果物**: `tests/cli_args.rs` 追加分
- **内容**:
  - `--changed-since` の受理テスト
  - `--changed-since` × `--query` 排他テスト
  - `--changed-since` × `--symbol` 排他テスト
  - `--changed-since` × `--related` 排他テスト
  - `--changed-since` × `--semantic` 排他テスト
  - `--changed-since` × `--workspace` 排他テスト

#### Task 2.2: Git モジュール単体テスト
- **成果物**: `src/cli/git.rs` 内テスト
- **内容**:
  - `validate_changed_since_input()` テスト
    - 正常: "12 hours ago", "yesterday", "abc1234"
    - 異常: 先頭 `-`, 制御文字, 257文字
  - `get_changed_files()` テスト（tempdir + git init）

#### Task 2.3: E2E テスト
- **成果物**: `tests/e2e_changed_since.rs` (新規), `tests/common/mod.rs` 更新
- **内容**:
  - `git_init_with_commit()` ヘルパー（CI対応: `-c user.name/email`）
  - テストケース:
    1. コミットハッシュでの検索（json 出力）
    2. 変更ファイルなし時のメッセージ
    3. human / path 出力形式
    4. 先頭 `-` 拒否エラー

### Phase 3: 品質チェック・仕上げ

#### Task 3.1: 品質チェック
- `cargo build` - エラー0件
- `cargo clippy --all-targets -- -D warnings` - 警告0件
- `cargo test --all` - 全テストパス
- `cargo fmt --all -- --check` - 差分なし

#### Task 3.2: 最終確認
- 既存テスト（e2e_impact.rs, cli_args.rs 等）の回帰なし確認
- 手動動作確認

## 実装順序

```
Task 1.1 (git.rs)  ─┐
Task 1.2 (impact.rs) ├─> Task 1.3 (From impl) ─┐
Task 1.4 (main.rs)  ─┘                          ├─> Task 1.5 (search.rs)
                                                  │
Task 2.1 (cli_args) ─────────────────────────────┘
Task 2.2 (git unit test) ────────────────────────────> Task 2.3 (E2E)
                                                        │
                                                        └─> Task 3.1 (品質チェック)
                                                            └─> Task 3.2 (最終確認)
```

## TDD 実装戦略

各タスクで **Red → Green → Refactor** サイクル:

1. **Red**: まず失敗するテストを書く
2. **Green**: テストを通す最小限の実装
3. **Refactor**: コード品質改善（clippy 対応含む）

## Definition of Done

- [ ] すべてのタスクが完了
- [ ] `cargo test --all` 全テストパス
- [ ] `cargo clippy --all-targets -- -D warnings` 警告ゼロ
- [ ] `cargo fmt --all -- --check` 差分なし
- [ ] 既存テストの回帰なし
- [ ] 受け入れ基準（Issue #91）全項目クリア

## 次のアクション

1. ブランチ: `feature/issue-91-changed-since`（現在のブランチ）
2. `/pm-auto-dev 91` で TDD 自動開発開始
3. 完了後 `/create-pr` で PR 作成
