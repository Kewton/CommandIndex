# 作業計画: Issue #159 - before-changeのlimitをIssue単位に変更

## Issue概要
- **Issue番号**: #159
- **タイトル**: before-changeのデフォルトlimitがIssue単位ではなくドキュメント単位で切られる
- **サイズ**: M（中）
- **優先度**: High（AIエージェントの判断品質に直接影響）
- **依存Issue**: なし
- **ブランチ**: `fix/issue-159-before-change-limit`

## 詳細タスク分解

### Phase 1: コアロジック変更

#### Task 1.1: relation_priority() の修正
- **対象**: `src/cli/before_change.rs`
- **内容**: has_workplanとhas_reviewの優先度を入れ替え
  - has_design=0, has_workplan=1（was 2）, has_review=2（was 1）, modifies=3
- **テスト**: 既存テスト `test_findings_without_ranking_sort_order` の期待値更新
  - has_design > has_workplan > has_review の順に変更
- **依存**: なし

#### Task 1.2: group_and_limit_by_issue() 新設
- **対象**: `src/cli/before_change.rs`
- **内容**:
  - `const MAX_DOCS_PER_ISSUE: usize = 2;` 定数追加
  - `group_and_limit_by_issue(findings, limit)` 関数新設
  - HashMap + issue_order Vec でソート順保持グルーピング
  - 各Issue内をrelation_priority順にソート
  - Issue単位でlimit適用、各IssueからMAX_DOCS_PER_ISSUE件選出
- **テスト**:
  - `test_group_and_limit_by_issue_basic`: 3 Issue × 3ドキュメント、limit=2
  - `test_group_and_limit_by_issue_max_docs`: 各Issue最大2件
  - `test_group_and_limit_by_issue_preserves_order`: ソート順保持
- **依存**: Task 1.1

#### Task 1.3: findings_without_ranking() の変更
- **対象**: `src/cli/before_change.rs`
- **内容**: issue_number昇順 → 数値降順（parse::<u64>()で比較）
- **テスト**: `test_findings_without_ranking_descending` 新設
- **依存**: Task 1.1

#### Task 1.4: rank_by_max_similarity() の変更
- **対象**: `src/cli/before_change.rs`
- **内容**:
  - Issue単位でmax similarity集約（BTreeMap<String, f32>）
  - 3段階ソート: max similarity降順 → issue_number → relation_priority
  - without_scoreソートも降順に統一
- **テスト**: 既存テスト `test_rank_by_max_similarity_with_empty_file_embs` が引き続きパスすることを確認
- **依存**: Task 1.1

#### Task 1.5: run_before_change() のlimit適用変更
- **対象**: `src/cli/before_change.rs`
- **内容**:
  - L408: `findings.into_iter().take(limit)` → `group_and_limit_by_issue(findings, limit)`
  - total_issues算出: `issues.len()` → docsからユニークIssue数を算出
  - displayed_issues算出: limited_findingsからユニークIssue数を算出
  - BeforeChangeResult構築にdisplayed_issuesを追加
- **依存**: Task 1.2, 1.3, 1.4

### Phase 2: 構造体・フォーマッタ変更

#### Task 2.1: BeforeChangeResult構造体変更
- **対象**: `src/output/mod.rs`
- **内容**: `displayed_issues: usize` フィールド追加
- **依存**: なし（Phase 1と並行可能だが、コンパイルにはPhase 1のResult構築変更が必要）

#### Task 2.2: human.rs フォーマッタ更新
- **対象**: `src/output/human.rs`
- **内容**: `"showing {displayed} of {total} issues (limited by --limit)"` 表示追加
- **依存**: Task 2.1

#### Task 2.3: json.rs フォーマッタ更新
- **対象**: `src/output/json.rs`
- **内容**: serde_json::json!マクロに `"displayed_issues"` フィールド追加
- **注意**: 手動JSON構築パターンのため追加漏れに注意
- **依存**: Task 2.1

#### Task 2.4: llm.rs フォーマッタ更新
- **対象**: `src/output/llm.rs`
- **内容**: `"{displayed}/{total} issues shown"` 表示追加
- **依存**: Task 2.1

### Phase 3: CLIヘルプ・バリデーション

#### Task 3.1: main.rs ヘルプ文言+バリデーション更新
- **対象**: `src/main.rs`
- **内容**:
  - `/// Maximum number of findings to show` → `/// Maximum number of issues to show`
  - `value_parser = clap::value_parser!(usize).range(1..=1000)` 追加
- **依存**: なし

#### Task 3.2: help_llm.rs ヘルプ更新
- **対象**: `src/cli/help_llm.rs`
- **内容**: key_optionsの`--limit`を`"--limit <N>  Maximum number of issues to show (default: 10)"` に更新
- **依存**: なし

#### Task 3.3: BEFORE_CHANGE_AFTER_HELP 更新
- **対象**: `src/cli/before_change.rs`
- **内容**: ヘルプ例文にlimitがIssue単位であることを明記
- **依存**: なし

### Phase 4: テスト

#### Task 4.1: 既存E2Eテスト更新
- **対象**: `tests/e2e_before_change.rs`
- **内容**: `before_change_limit_respected` のアサーションをIssue単位に変更
  - findings.len() <= 1 → ユニークissue_number数 <= 1（findings.len()は最大2）
- **依存**: Phase 1-3完了

#### Task 4.2: 新規E2Eテスト追加
- **対象**: `tests/e2e_before_change.rs`
- **内容**:
  - `before_change_limit_multiple_issues`: 複数Issue環境でlimit検証
  - `before_change_displayed_issues_field`: JSON出力にdisplayed_issuesが含まれる
  - `before_change_limit_zero_rejected`: --limit 0 がclapで拒否
  - `before_change_limit_exceeds_issues`: limit > Issue数で全Issue表示
- **依存**: Phase 1-3完了

### Phase 5: 品質チェック

#### Task 5.1: 品質チェック実行
- `cargo build`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all`
- `cargo fmt --all -- --check`

## 実行順序

```
Phase 1 (コアロジック):
  Task 1.1 → Task 1.2 → Task 1.5
           → Task 1.3 ↗
           → Task 1.4 ↗

Phase 2 (構造体・フォーマッタ): ※Task 2.1はPhase 1と並行開始可
  Task 2.1 → Task 2.2
           → Task 2.3
           → Task 2.4

Phase 3 (ヘルプ): ※Phase 1-2と並行可
  Task 3.1, 3.2, 3.3（独立）

Phase 4 (テスト): ※Phase 1-3完了後
  Task 4.1, 4.2

Phase 5 (品質チェック): ※Phase 4完了後
  Task 5.1
```

## Definition of Done

- [ ] すべてのタスク（Task 1.1〜5.1）が完了
- [ ] `cargo test --all` 全テストパス
- [ ] `cargo clippy --all-targets -- -D warnings` 警告ゼロ
- [ ] `cargo fmt --all -- --check` 差分なし
- [ ] `cargo build` エラーゼロ
