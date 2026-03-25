# 作業計画: Issue #167 suggestコマンドのナレッジグラフ展開制限

## Issue: suggestコマンドのナレッジグラフ展開が過剰（80件提案）
**Issue番号**: #167
**サイズ**: M
**優先度**: High
**依存Issue**: なし
**ブランチ**: `fix/issue-167-suggest-limit`（既存）

## 設計方針書

`dev-reports/design/issue-167-suggest-limit-design-policy.md`

## 詳細タスク分解

### Phase 1: コア実装

#### Task 1.1: KnowledgeRelation::priority() メソッド追加
- **ファイル**: `src/indexer/knowledge.rs`
- **内容**: `KnowledgeRelation` impl に `pub fn priority(&self) -> u8` メソッドを追加
- **依存**: なし
- **テスト**: `test_kg_relation_priority_order`（Task 2.1で作成）

#### Task 1.2: before_change.rs の relation_priority() 互換ラッパー化
- **ファイル**: `src/cli/before_change.rs`
- **内容**: 既存 `relation_priority(&str) -> u8` 関数の内部実装を `KnowledgeRelation::parse(s).map_or(5, |r| r.priority())` に変更
- **依存**: Task 1.1
- **テスト**: 既存テストが通ること（回帰確認）

#### Task 1.3: SuggestKgDoc 構造体定義
- **ファイル**: `src/cli/suggest.rs`
- **内容**: `SuggestKgDoc { issue_number, file_path, relation, doc_subtype }` 構造体追加、`MAX_KG_DOCS_PER_ISSUE = 4` 定数追加
- **依存**: なし

#### Task 1.4: filter_and_limit_kg_docs() 関数実装
- **ファイル**: `src/cli/suggest.rs`
- **内容**: フィルタリング（Modifies/HasProgress/StageReview除外）、priority()ソート、Issue単位グルーピング・制限
- **依存**: Task 1.1, Task 1.3
- **テスト**: Task 2.1 の新規テスト7件

#### Task 1.5: query_knowledge_graph() の変更
- **ファイル**: `src/cli/suggest.rs`
- **内容**: `find_knowledge_by_issue()` → `find_documents_by_issue()` ループ呼び出しに変更、`IssueDocumentEntry → SuggestKgDoc` 変換
- **依存**: Task 1.3
- **use追加**: `IssueDocumentEntry`, `DocSubtype` のインポート

#### Task 1.6: prepend_knowledge_steps() の引数型変更
- **ファイル**: `src/cli/suggest.rs`
- **内容**: 第2引数を `&[KnowledgeDocResult]` → `&[SuggestKgDoc]` に変更、内部ロジックは `issue_number` と `file_path` のみ参照するため大きな変更なし
- **依存**: Task 1.3

#### Task 1.7: run_suggest() の統合
- **ファイル**: `src/cli/suggest.rs`
- **内容**: ステップ5とステップ10の間に `filter_and_limit_kg_docs()` 呼び出しを挿入、`KnowledgeDocResult` の use を整理
- **依存**: Task 1.4, Task 1.5, Task 1.6

### Phase 2: テスト

#### Task 2.1: 新規ユニットテスト（suggest.rs）
- **ファイル**: `src/cli/suggest.rs` (#[cfg(test)])
- **テスト一覧**:
  - `test_filter_removes_modifies`
  - `test_filter_removes_has_progress`
  - `test_filter_keeps_issue_review_removes_stage_review`
  - `test_filter_keeps_design_and_workplan`
  - `test_filter_limits_per_issue`
  - `test_filter_empty_after_all_filtered`
  - `test_kg_relation_priority_order`
- **依存**: Task 1.4

#### Task 2.2: 既存テスト修正（suggest.rs）
- **ファイル**: `src/cli/suggest.rs` (#[cfg(test)])
- **内容**: `test_prepend_knowledge_steps_with_docs`, `_empty`, `_multiple_issues` のテストデータを `SuggestKgDoc` に変更
- **依存**: Task 1.6

#### Task 2.3: 品質チェック
- `cargo build`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all`
- `cargo fmt --all -- --check`

## 実行順序

```
Task 1.1 (knowledge.rs: priority())
  → Task 1.2 (before_change.rs: 互換ラッパー)
  → Task 1.3 (suggest.rs: SuggestKgDoc + 定数)
    → Task 1.4 (suggest.rs: filter_and_limit_kg_docs)
    → Task 1.5 (suggest.rs: query_knowledge_graph変更)
    → Task 1.6 (suggest.rs: prepend_knowledge_steps型変更)
      → Task 1.7 (suggest.rs: run_suggest統合)
        → Task 2.1 (新規テスト)
        → Task 2.2 (既存テスト修正)
          → Task 2.3 (品質チェック)
```

## Definition of Done

- [ ] `KnowledgeRelation::priority()` メソッドが追加されている
- [ ] `before_change.rs` の `relation_priority()` が互換ラッパー化されている
- [ ] `filter_and_limit_kg_docs()` が実装されている
- [ ] suggestコマンドのKGステップ数が制限されている
- [ ] 新規テスト7件 + 既存テスト修正3件が全パス
- [ ] `cargo clippy --all-targets -- -D warnings` で警告0件
- [ ] `cargo test --all` で全テストパス
- [ ] `cargo fmt --all -- --check` で差分なし
