# 作業計画書: Issue #109 - 検索クエリのLLM向けガイド (suggest)

## Issue概要

**Issue番号**: #109
**タイトル**: 検索クエリのLLM向けガイド (suggest)
**サイズ**: M（中規模 - 新規サブコマンド追加、既存モジュールへの破壊的変更なし）
**優先度**: Low
**依存Issue**: なし
**ブランチ**: `feature/issue-109-suggest-queries`（既存）

## 前提条件

- [x] Issueレビュー完了（8段階）
- [x] 設計方針書作成・レビュー完了
- [x] 設計レビュー反映完了

## 詳細タスク分解

### Phase 1: 型定義・基盤（依存なし）

#### Task 1.1: 出力構造体定義
- **成果物**: `src/output/mod.rs` に追加
- **内容**:
  - `SuggestStep` 構造体（command, reason）
  - `SuggestResult` 構造体（query, has_embeddings, strategy）
  - `format_suggest_results()` フォーマッタ（writer + OutputError パターン）
  - human / json / path 3形式対応
- **依存**: なし
- **テスト**: 出力フォーマットの単体テスト

#### Task 1.2: エラー型・入力バリデーション定義
- **成果物**: `src/cli/suggest.rs` 新規作成（前半部分）
- **内容**:
  - `SuggestError` enum（InvalidInput, IndexNotFound, SymbolDbNotFound, Reader, RelatedSearch, SymbolStore, Output）
  - `impl Display`, `impl Error`, `From<>` 変換チェーン
  - `validate_input()` 関数（空文字、trim後空白、500文字超過、制御文字チェック）
  - `sanitize_for_command_arg()` 関数（シェルメタ文字除去）
  - `BINARY_NAME` 定数
- **依存**: なし
- **テスト**: バリデーション・サニタイズの単体テスト

### Phase 2: コアロジック実装

#### Task 2.1: BM25検索・ファイル単位dedup
- **成果物**: `src/cli/suggest.rs` に追加
- **内容**:
  - `search_entry_files()` 関数（BM25検索実行）
  - `deduplicate_by_file()` 関数（SearchResult.path で正規化、最大スコア採用、上位N件）
- **依存**: Task 1.2
- **テスト**: dedup ロジックの単体テスト

#### Task 2.2: 戦略生成ロジック
- **成果物**: `src/cli/suggest.rs` に追加
- **内容**:
  - `build_strategy()` 関数（context → related → impact の順で戦略構成）
  - `build_fallback_strategy()` 関数（BM25結果0件時のフォールバック）
  - `maybe_add_semantic_step()` 関数（SymbolStore::count_embeddings() による分岐）
  - 各ステップのコマンド文字列はサニタイズ済みファイルパスで構築
- **依存**: Task 2.1
- **テスト**: 戦略生成の単体テスト（dedup, limit, fallback, semantic gating）

#### Task 2.3: メインエントリポイント
- **成果物**: `src/cli/suggest.rs` に `run_suggest()` 関数
- **内容**:
  - 入力バリデーション → SearchContext::new() → リソースオープン → BM25検索 → 戦略生成 → 出力
  - IndexReader と SymbolStore は1回のみオープン
- **依存**: Task 2.1, 2.2, 1.1

### Phase 3: CLI統合

#### Task 3.1: Commands enum 追加
- **成果物**: `src/main.rs`, `src/cli/mod.rs`
- **内容**:
  - Commands enum に Suggest バリアント追加（for_task, format のみ。index_path はグローバル利用）
  - `pub mod suggest;` を cli/mod.rs に追加
  - main() の match ブロックに Suggest アーム追加（cli.index_path を渡す）
- **依存**: Task 2.3

#### Task 3.2: help-llm 更新
- **成果物**: `src/cli/help_llm.rs`
- **内容**:
  - `build_commands()` に suggest の CommandInfo 追加
  - commands セクション必須。use_cases / workflows は任意
- **依存**: Task 3.1

### Phase 4: テスト

#### Task 4.1: 単体テスト（suggest.rs 内 #[cfg(test)]）
- **成果物**: `src/cli/suggest.rs` の tests モジュール
- **テストケース**:
  - `validate_input`: 空文字、空白のみ、500文字超過、制御文字、正常入力
  - `sanitize_for_command_arg`: シェルメタ文字除去
  - `deduplicate_by_file`: 重複排除、スコア最大値、limit適用
  - `build_fallback_strategy`: 実在サブコマンドのみ使用
  - semantic gating: count_embeddings > 0 分岐

#### Task 4.2: CLIパーステスト
- **成果物**: `tests/cli_args.rs` 修正
- **テストケース**:
  - `suggest --help` が正常終了
  - トップレベル `--help` に `suggest` が表示される
  - `help_llm_contains_all_subcommands` テスト更新（件数 13→14、expected に "suggest" 追加）

#### Task 4.3: E2Eテスト
- **成果物**: `tests/e2e_suggest.rs` 新規作成
- **テストケース**:
  - インデックス構築済み → `suggest --for "..."` → 正常出力
  - `--format json` → 有効なJSON（strategy 配列あり）
  - `--format human` → "Suggested search strategy:" 含む出力
  - インデックス未構築 → エラー出力
  - `--for ""` → バリデーションエラー
  - `--for "   "` → バリデーションエラー

### Phase 5: 品質確認

#### Task 5.1: 全品質チェック実行
- **コマンド**:
  ```bash
  cargo build
  cargo clippy --all-targets -- -D warnings
  cargo test --all
  cargo fmt --all -- --check
  ```
- **基準**: 全て0件エラー、全テストパス

## タスク依存関係

```
Task 1.1 (出力構造体) ──────┐
Task 1.2 (エラー型/バリデーション)─┤
                              ├── Task 2.1 (BM25/dedup) ── Task 2.2 (戦略生成) ── Task 2.3 (メインロジック) ── Task 3.1 (CLI統合) ── Task 3.2 (help-llm)
                              │
Task 4.1 (単体テスト) ── TDD: 各Taskと並行
Task 4.2 (CLIテスト) ── Task 3.1 完了後
Task 4.3 (E2Eテスト) ── Task 3.1 完了後
Task 5.1 (品質確認) ── 全Task完了後
```

## TDD実装順序

TDDで実装する場合の推奨順序:

1. **Red**: Task 1.2 の validate_input テスト作成 → **Green**: validate_input 実装
2. **Red**: Task 1.2 の sanitize テスト作成 → **Green**: sanitize 実装
3. **Red**: Task 1.1 の出力フォーマットテスト作成 → **Green**: 出力構造体・フォーマッタ実装
4. **Red**: Task 2.1 の dedup テスト作成 → **Green**: dedup 実装
5. **Red**: Task 2.2 の戦略生成テスト作成 → **Green**: build_strategy, fallback, semantic gating 実装
6. **Red**: Task 2.3 の run_suggest テスト作成 → **Green**: メインロジック実装
7. **Red**: Task 3.1 の CLI統合 → Task 4.2 の CLIパーステスト追加
8. **Red**: Task 3.2 の help-llm更新 → Task 4.2 の help-llm契約テスト更新
9. **Red**: Task 4.3 の E2Eテスト作成 → **Green**: 必要に応じて修正
10. Task 5.1 の品質確認

## 品質チェック項目

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |

## Definition of Done

- [ ] すべてのタスク（Task 1.1 〜 5.1）が完了
- [ ] `cargo test --all` 全テストパス
- [ ] `cargo clippy --all-targets -- -D warnings` 警告ゼロ
- [ ] `cargo fmt --all -- --check` 差分なし
- [ ] 受け入れ基準（Issue #109 記載）全項目を満たす
- [ ] 既存テストに回帰なし

## 変更対象ファイル一覧

| ファイル | 変更種別 | Phase |
|---------|---------|-------|
| `src/output/mod.rs` | 変更 | Phase 1 |
| `src/cli/suggest.rs` | **新規** | Phase 1-2 |
| `src/cli/mod.rs` | 変更 | Phase 3 |
| `src/main.rs` | 変更 | Phase 3 |
| `src/cli/help_llm.rs` | 変更 | Phase 3 |
| `tests/cli_args.rs` | 変更 | Phase 4 |
| `tests/e2e_suggest.rs` | **新規** | Phase 4 |

## リスクと対策

| リスク | 影響 | 対策 |
|--------|------|------|
| BM25検索結果が意図しない粒度 | 戦略の質低下 | dedup のファイル単位正規化で対応。E2Eテストで検証 |
| help-llm 契約テスト更新漏れ | CI失敗 | Task 4.2 で明示的に件数更新 |
| パフォーマンス劣化（パイプライン連鎖） | ユーザー体験低下 | 各段階 limit 5 で制限。必要に応じて調整 |
