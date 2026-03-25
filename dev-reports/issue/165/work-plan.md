# 作業計画: Issue #165 — progress-reportのrelationをhas_progressに変更

## Issue: ナレッジグラフ: progress-reportのrelationをhas_progressに変更
**Issue番号**: #165
**サイズ**: S
**優先度**: Medium
**依存Issue**: #160（完了済み）

## 詳細タスク分解

### Phase 1: コア変更（knowledge.rs）

- [ ] **Task 1.1**: KnowledgeRelation enum に HasProgress バリアント追加
  - 成果物: `src/indexer/knowledge.rs`
  - 変更箇所: enum定義(L82-87)、as_str()(L90-96)、parse()(L100-107)
  - 依存: なし

- [ ] **Task 1.2**: build_pattern_rules() の progress-report ルール変更
  - 成果物: `src/indexer/knowledge.rs`
  - 変更箇所: PatternRule(L394-401) の relation を HasReview → HasProgress
  - 依存: Task 1.1

### Phase 2: 依存モジュール更新

- [ ] **Task 2.1**: symbol_store.rs DRY リファクタリング
  - 成果物: `src/indexer/symbol_store.rs`
  - 変更箇所: find_documents_by_issue()(L880-889) のハードコードmatchをKnowledgeRelation::parse()に統一
  - 依存: Task 1.1

- [ ] **Task 2.2**: issue.rs sort_order() 更新
  - 成果物: `src/cli/issue.rs`
  - 変更箇所: sort_order()(L98-103) に HasProgress => 4 追加、Modifies => 5 に変更
  - 依存: Task 1.1

- [ ] **Task 2.3**: before_change.rs relation_priority() 更新
  - 成果物: `src/cli/before_change.rs`
  - 変更箇所: relation_priority()(L331-338) に "has_progress" => 3 追加、modifies => 4、_ => 5 に変更
  - 依存: Task 1.1

- [ ] **Task 2.4**: human.rs relation_display_label() 更新
  - 成果物: `src/output/human.rs`
  - 変更箇所: relation_display_label()(L252-257) に "has_progress" => "progress" 追加
  - 依存: Task 1.1

### Phase 3: テスト更新

- [ ] **Task 3.1**: knowledge.rs テスト更新
  - test_parse_progress_report: HasReview → HasProgress
  - test_knowledge_relation_as_str: HasProgress アサーション追加
  - test_knowledge_relation_parse: has_progress パーステスト追加
  - test_knowledge_relation_display: HasProgress 表示テスト追加
  - 依存: Task 1.1, 1.2

- [ ] **Task 3.2**: symbol_store.rs テスト更新
  - test_find_documents_by_issue_metadata_parsed: progress-report を HasProgress に変更
  - 依存: Task 2.1

- [ ] **Task 3.3**: issue.rs テスト更新
  - テストデータの progress-report relation を HasProgress に変更
  - 依存: Task 2.2

- [ ] **Task 3.4**: before_change.rs テスト更新
  - test_relation_priority_order: has_progress 優先度アサーション追加
  - 依存: Task 2.3

- [ ] **Task 3.5**: human.rs テスト更新
  - relation_display_label テストに has_progress ケース追加・更新
  - 依存: Task 2.4

- [ ] **Task 3.6**: e2e_issue.rs テスト更新
  - progress-report テストデータを HasProgress に変更
  - 依存: Task 1.1

### Phase 4: 品質検証

- [ ] **Task 4.1**: 全品質チェック実行
  - `cargo build` / `cargo clippy --all-targets -- -D warnings` / `cargo test --all` / `cargo fmt --all -- --check`

## TDD実装順序

1. まず失敗テストを書く（HasProgress のアサーション）
2. knowledge.rs のenum/parse/as_str を実装（テストパス）
3. PatternRule 変更 + テスト更新
4. 依存モジュール順次更新 + テスト更新
5. 全品質チェック

## 品質チェック項目

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |

## Definition of Done

- [ ] `KnowledgeRelation::HasProgress` が追加されている
- [ ] 再インデックス後、progress-report が `has_progress` で登録される
- [ ] human/LLM出力で引き続き `[progress]` が表示される
- [ ] `before-change` コマンドで progress-report が適切な優先度でソートされる
- [ ] 既存テストが全Pass
- [ ] `cargo clippy --all-targets -- -D warnings` 警告0件
