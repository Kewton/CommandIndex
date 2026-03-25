# 作業計画: Issue #168 - issue/before-changeの出力に判断理由のスニペットを付与する

## Issue概要
**Issue番号**: #168
**タイトル**: issue/before-changeの出力に判断理由のスニペットを付与する
**サイズ**: L（型変更15箇所以上、フォーマッタ6関数更新、テスト30箇所以上）
**優先度**: High（プロダクトコア機能）
**依存Issue**: なし
**ブランチ**: `feature/issue-168-snippet-inline`（作成済み）

## 詳細タスク分解

### Phase 1: 型定義・基盤変更

#### Task 1.1: BeforeChangeFinding に snippet フィールド追加
- **成果物**: `src/output/mod.rs`
- **変更内容**: `pub snippet: Option<String>` フィールド追加
- **依存**: なし
- **影響**: before_change.rs 内のコンストラクタ5箇所 + テスト15箇所以上に `snippet: None` 追加が必要（コンパイルエラー解消）

#### Task 1.2: IssueDocumentEntry に snippet フィールド追加
- **成果物**: `src/indexer/knowledge.rs`
- **変更内容**: `pub snippet: Option<String>` フィールド追加
- **依存**: なし
- **影響**: symbol_store.rs の find_documents_by_issue() 1箇所、issue.rs テスト11箇所に `snippet: None` 追加

#### Task 1.3: コンパイルエラー解消（全構築箇所に snippet: None 追加）
- **成果物**: `src/cli/before_change.rs`, `src/cli/issue.rs`, `src/indexer/symbol_store.rs`
- **変更内容**: 全 BeforeChangeFinding / IssueDocumentEntry リテラルに `snippet: None` 追加
- **依存**: Task 1.1, 1.2
- **検証**: `cargo build` パス

### Phase 2: snippet_helper 拡張

#### Task 2.1: 既存 enrich 関数の空→None 変換統一
- **成果物**: `src/cli/snippet_helper.rs`
- **変更内容**: `enrich_impact_with_snippets()` と `enrich_related_with_snippets()` の `Some(fetch_snippet(...))` を `{ let s = fetch_snippet(...); if s.is_empty() { None } else { Some(s) } }` に変更
- **依存**: なし
- **検証**: 既存テストがパスすること

#### Task 2.2: enrich_before_change_with_snippets() 追加
- **成果物**: `src/cli/snippet_helper.rs`
- **変更内容**: 設計書セクション6の関数を追加
- **依存**: Task 1.1

#### Task 2.3: enrich_issue_documents_with_snippets() 追加
- **成果物**: `src/cli/snippet_helper.rs`
- **変更内容**: 設計書セクション6の関数を追加
- **依存**: Task 1.2

### Phase 3: CLI引数追加

#### Task 3.1: before-change に --with-snippet / --snippet-lines / --snippet-chars 追加
- **成果物**: `src/main.rs`
- **変更内容**: BeforeChange enum に3フィールド追加、SnippetOptions 構築ロジック追加
- **依存**: Task 2.2
- **パターン**: impact コマンドの既存パターンに準拠
- **定数**: `KNOWLEDGE_SNIPPET_LINES = 3`, `KNOWLEDGE_SNIPPET_CHARS = 200`

#### Task 3.2: issue に --with-snippet / --snippet-lines / --snippet-chars 追加
- **成果物**: `src/main.rs`
- **変更内容**: Issue enum に3フィールド追加、SnippetOptions 構築ロジック追加
- **依存**: Task 2.3
- **パターン**: Task 3.1 と同じ

### Phase 4: コマンド関数シグネチャ変更・enrich 呼び出し

#### Task 4.1: run_before_change() に snippet_options 追加
- **成果物**: `src/cli/before_change.rs`, `src/main.rs`
- **変更内容**:
  1. run_before_change() シグネチャに `snippet_options: SnippetOptions` 追加
  2. group_and_limit_by_issue() 後に IndexReaderWrapper::open → enrich 呼び出し
  3. main.rs のコール箇所を更新
- **依存**: Task 3.1

#### Task 4.2: issue::run() に snippet_options 追加
- **成果物**: `src/cli/issue.rs`, `src/main.rs`
- **変更内容**:
  1. run() シグネチャに `snippet_options: SnippetOptions` 追加
  2. documents 取得後に IndexReaderWrapper::open → enrich 呼び出し
  3. main.rs のコール箇所を更新
- **依存**: Task 3.2

### Phase 5: 出力フォーマッタ更新

#### Task 5.1: before-change human フォーマッタ
- **成果物**: `src/output/human.rs`
- **変更内容**: format_before_change_human() で doc_title 後に snippet 表示追加
- **依存**: Task 1.1
- **パターン**: impact の snippet 表示パターン（`if let Some(ref snippet) = finding.snippet && !snippet.is_empty()`）

#### Task 5.2: before-change llm フォーマッタ
- **成果物**: `src/output/llm.rs`
- **変更内容**: format_before_change_llm() で title_str 後に `> snippet` 表示追加
- **依存**: Task 1.1

#### Task 5.3: before-change json フォーマッタ
- **成果物**: `src/output/json.rs`
- **変更内容**: format_before_change_json() で snippet フィールドを条件付き追加（impact と同パターン）
- **依存**: Task 1.1

#### Task 5.4: issue human/llm/json/path フォーマッタ
- **成果物**: `src/cli/issue.rs`
- **変更内容**:
  1. format_human(): snippet 表示追加
  2. format_llm(): `> snippet` 表示追加
  3. format_json(): --with-snippet 有無で string[] / object[] を分岐（SnippetOptions を引数に追加）
  4. format_path(): 変更なし
- **依存**: Task 1.2, 4.2

### Phase 6: テスト

#### Task 6.1: 既存テスト修正確認
- **成果物**: 全テストファイル
- **変更内容**: `cargo test --all` で全テストパスを確認
- **依存**: Phase 1-5 全て

#### Task 6.2: before-change フォーマッタ snippet テスト追加
- **成果物**: `src/cli/before_change.rs` (mod tests) または `tests/output_format.rs`
- **テストケース**:
  1. snippet=None の場合: 既存出力と同じ
  2. snippet=Some("text") の場合: human/llm/json で正しく表示

#### Task 6.3: issue フォーマッタ snippet テスト追加
- **成果物**: `src/cli/issue.rs` (mod tests)
- **テストケース**:
  1. snippet=None の場合: 既存出力と同じ
  2. snippet=Some("text") の場合: human/llm で正しく表示
  3. format_json: --with-snippet 未指定で string[]、指定で object[]

#### Task 6.4: CLI引数テスト追加
- **成果物**: `tests/cli_args.rs`
- **テストケース**: `before-change src/auth.rs --with-snippet --snippet-lines 3 --snippet-chars 200` が受理されること

### Phase 7: ドキュメント・仕上げ

#### Task 7.1: help-llm 更新
- **成果物**: `src/cli/help_llm.rs`
- **変更内容**: issue/before-change のコマンド説明に --with-snippet 等のオプションと出力例を追加

#### Task 7.2: 品質チェック
- `cargo build` → エラー0件
- `cargo clippy --all-targets -- -D warnings` → 警告0件
- `cargo test --all` → 全テストパス
- `cargo fmt --all -- --check` → 差分なし

## 実行順序

```
Phase 1 (型定義)
  Task 1.1 → Task 1.2 → Task 1.3 → cargo build 確認
              ↓
Phase 2 (snippet_helper)
  Task 2.1 (並行可) | Task 2.2 | Task 2.3
              ↓
Phase 3 (CLI引数)
  Task 3.1 | Task 3.2
              ↓
Phase 4 (コマンド関数)
  Task 4.1 | Task 4.2
              ↓
Phase 5 (フォーマッタ)
  Task 5.1 | 5.2 | 5.3 | 5.4
              ↓
Phase 6 (テスト)
  Task 6.1 → 6.2 | 6.3 | 6.4
              ↓
Phase 7 (仕上げ)
  Task 7.1 → 7.2
```

## Definition of Done

- [ ] すべてのタスクが完了
- [ ] `cargo build` エラー0件
- [ ] `cargo clippy --all-targets -- -D warnings` 警告0件
- [ ] `cargo test --all` 全テストパス
- [ ] `cargo fmt --all -- --check` 差分なし
- [ ] Issue の受け入れ基準14項目すべて満たす
