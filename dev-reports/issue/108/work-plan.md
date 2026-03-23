# 作業計画: Issue #108 - impact/related にコードスニペット付きモード (--with-snippet)

## Issue概要
**Issue番号**: #108
**サイズ**: M（中規模）
**優先度**: Medium
**依存Issue**: なし

## 詳細タスク分解

### Phase 1: データモデル・型定義

#### Task 1.1: SnippetOptions 構造体と snippet_helper モジュール新設
- **成果物**: `src/cli/snippet_helper.rs`, `src/cli/mod.rs`
- **依存**: なし
- **作業内容**:
  - `src/cli/snippet_helper.rs` を新設
  - `SnippetOptions` 構造体（enabled, config）を定義
  - `fetch_snippet()` 関数を実装（IndexReaderWrapper + truncate_body + strip_control_chars）
  - `enrich_impact_with_snippets()` / `enrich_related_with_snippets()` を実装
  - `src/cli/mod.rs` に `pub mod snippet_helper;` を追加

#### Task 1.2: 構造体にsnippetフィールド追加
- **成果物**: `src/output/mod.rs`
- **依存**: なし
- **作業内容**:
  - `RelatedSearchResult` に `pub snippet: Option<String>` を追加
  - `ImpactFileResult` に `pub snippet: Option<String>` を追加（serde アトリビュートなし）

#### Task 1.3: 構造体構築箇所の修正（snippet: None 追加）
- **成果物**: `src/cli/impact.rs`, `src/cli/context.rs`, `src/search/related.rs`
- **依存**: Task 1.2
- **作業内容**:
  - `src/cli/impact.rs` の `aggregate_impact()` (行228) → `snippet: None` 追加
  - `src/cli/context.rs` の `collect_related_context()` (行171) → `snippet: None` 追加
  - `src/search/related.rs` の `find_related()` (行114) → `snippet: None` 追加
- **検証**: `cargo build` でコンパイル通過を確認

### Phase 2: CLIオプション追加

#### Task 2.1: Search サブコマンドに --with-snippet 追加
- **成果物**: `src/main.rs`
- **依存**: Task 1.1
- **作業内容**:
  - Search サブコマンド定義に `with_snippet: bool` フィールド追加
  - Search の実行分岐で `SnippetOptions` 構築
  - `run_related_search()` / `run_related_search_from_stdin()` への引数追加

#### Task 2.2: Impact サブコマンドに --with-snippet / --snippet-lines / --snippet-chars 追加
- **成果物**: `src/main.rs`
- **依存**: Task 1.1
- **作業内容**:
  - Impact サブコマンド定義に `with_snippet`, `snippet_lines`, `snippet_chars` フィールド追加
  - `snippet_lines` / `snippet_chars` に `value_parser(1..)` 制約追加
  - Impact の実行分岐で `SnippetOptions` 構築（config.toml デフォルト値解決含む）
  - `run_impact()` への引数追加

### Phase 3: コアロジック統合

#### Task 3.1: run_impact() にスニペット取得処理を追加
- **成果物**: `src/cli/impact.rs`
- **依存**: Task 1.1, Task 2.2
- **作業内容**:
  - `run_impact()` のシグネチャに `snippet_options: SnippetOptions` を追加
  - `aggregate_impact()` 後に `enrich_impact_with_snippets()` を呼び出し

#### Task 3.2: changed_since.rs の run_impact() 呼び出しを修正
- **成果物**: `src/cli/changed_since.rs`
- **依存**: Task 3.1
- **作業内容**:
  - `run_impact()` 呼び出し (行42) に `SnippetOptions::default()` を追加

#### Task 3.3: run_related_search() にスニペット取得処理を追加
- **成果物**: `src/cli/search.rs`
- **依存**: Task 1.1, Task 2.1
- **作業内容**:
  - `run_related_search()` のシグネチャに `snippet_options: SnippetOptions` を追加
  - 検索結果取得後に `enrich_related_with_snippets()` を呼び出し

#### Task 3.4: run_related_search_from_stdin() にスニペット取得処理を追加
- **成果物**: `src/cli/search.rs`
- **依存**: Task 1.1, Task 2.1
- **作業内容**:
  - `run_related_search_from_stdin()` のシグネチャに `snippet_options: SnippetOptions` を追加
  - 検索結果取得後に `enrich_related_with_snippets()` を呼び出し

- **検証**: `cargo build` でコンパイル通過を確認

### Phase 4: 出力フォーマッタ対応

#### Task 4.1: JSON 出力に snippet フィールドを追加
- **成果物**: `src/output/json.rs`
- **依存**: Task 1.2
- **作業内容**:
  - `format_related_json()`: snippet が Some の場合のみ snippet フィールドを追加（JSONL 形式維持）
  - `format_impact_json()`: snippet が Some の場合のみ snippet フィールドを追加（単一JSON 形式維持）
  - `if let Some(obj) = json_value.as_object_mut()` パターンで安全に追加

#### Task 4.2: Human 出力にスニペット表示を追加
- **成果物**: `src/output/human.rs`
- **依存**: Task 1.2
- **作業内容**:
  - `format_related_human()`: snippet がある場合、パス行の下にインデント + dimmed で表示
  - `format_impact_human()`: 同様にスニペットをインデント表示

### Phase 5: ヘルプ・ドキュメント更新

#### Task 5.1: help-llm 更新
- **成果物**: `src/cli/help_llm.rs`
- **依存**: なし
- **作業内容**:
  - search の `key_options` に `--with-snippet` を追加
  - impact の `key_options` に `--with-snippet`, `--snippet-lines`, `--snippet-chars` を追加

#### Task 5.2: AFTER_HELP テキスト更新
- **成果物**: `src/cli/impact.rs`, `src/cli/search.rs`
- **依存**: なし
- **作業内容**:
  - impact の IMPACT_AFTER_HELP に `--with-snippet` の使用例を追加
  - search の SEARCH_AFTER_HELP に `--with-snippet` の使用例を追加

### Phase 6: テスト

#### Task 6.1: 既存テストの修正（コンパイルエラー解消）
- **成果物**: `tests/output_format.rs`
- **依存**: Task 1.2
- **作業内容**:
  - `make_impact_result()` 等の ImpactFileResult 構築に `snippet: None` を追加
  - RelatedSearchResult 構築箇所に `snippet: None` を追加

#### Task 6.2: --with-snippet の e2e テスト追加
- **成果物**: `tests/e2e_impact.rs`, `tests/e2e_related_search.rs`
- **依存**: Phase 3, Phase 4 完了
- **作業内容**:
  - impact --with-snippet --format json のテスト（snippet フィールドが含まれること）
  - impact --format json のテスト（snippet フィールドが含まれないこと = 後方互換性）
  - search --related --with-snippet --format json のテスト
  - search --related --format json のテスト（後方互換性）
  - impact --with-snippet --format human のテスト（スニペット表示あり）
  - impact --with-snippet --format path のテスト（スニペット無視）

#### Task 6.3: CLI引数テスト
- **成果物**: `tests/cli_args.rs`
- **依存**: Task 2.1, Task 2.2
- **作業内容**:
  - --with-snippet のヘルプ表示検証
  - --snippet-lines 0 が拒否されることのテスト（Impact）

### Phase 7: 品質チェック・回帰確認

#### Task 7.1: 品質チェック
- **作業内容**:
  - `cargo build` → エラー0件
  - `cargo clippy --all-targets -- -D warnings` → 警告0件
  - `cargo test --all` → 全テストパス
  - `cargo fmt --all -- --check` → 差分なし

#### Task 7.2: 回帰確認
- **作業内容**:
  - `cargo test e2e_context_pack` → context コマンドの既存仕様維持
  - `cargo test e2e_changed_since` → changed_since の出力維持
  - `cargo test e2e_team_workflow` → config 設定優先順位

## 実行順序（依存関係に基づく）

```
Phase 1: Task 1.1 + Task 1.2 (並列) → Task 1.3
Phase 2: Task 2.1 + Task 2.2 (並列、Task 1.1 依存)
Phase 3: Task 3.1 → Task 3.2, Task 3.3 + Task 3.4 (並列)
Phase 4: Task 4.1 + Task 4.2 (並列)
Phase 5: Task 5.1 + Task 5.2 (並列、いつでも実行可能)
Phase 6: Task 6.1 (Task 1.2 後すぐ) → Task 6.2 + Task 6.3 (Phase 3-4 完了後)
Phase 7: 全 Phase 完了後
```

## TDD 実装順序

TDD で実装する場合の推奨順序:

1. **Task 1.2**: 構造体変更 → Task 6.1: 既存テスト修正 → `cargo test` 通過確認
2. **Task 1.1**: snippet_helper.rs 新設（ユニットテスト付き）
3. **Task 1.3**: 構築箇所修正 → `cargo build` 通過確認
4. **Task 4.1 + 4.2**: フォーマッタ変更（ユニットテスト付き）
5. **Task 2.1 + 2.2**: CLIオプション追加
6. **Task 3.1-3.4**: コアロジック統合
7. **Task 6.2 + 6.3**: e2e テスト追加
8. **Task 5.1 + 5.2**: ヘルプ更新
9. **Task 7.1 + 7.2**: 品質チェック・回帰確認

## Definition of Done

- [ ] すべてのタスクが完了
- [ ] `cargo test --all` 全テストパス
- [ ] `cargo clippy --all-targets -- -D warnings` 警告0件
- [ ] `cargo fmt --all -- --check` 差分なし
- [ ] --with-snippet 未指定時の既存 JSON 出力に変更なし（後方互換性）
- [ ] context コマンドの既存仕様が維持されている
