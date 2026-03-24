# 作業計画: Issue #150 - ナレッジグラフ review/ ディレクトリ検出

## Issue概要
**Issue番号**: #150
**タイトル**: ナレッジグラフ: dev-reports/review/ のstage別レビューファイルが未検出
**サイズ**: S（小規模 - enum拡張 + パターン追加 + テスト更新）
**優先度**: Medium
**依存Issue**: なし（#139 は完了済み）
**設計方針書**: `dev-reports/design/issue-150-review-detection-design-policy.md`

---

## 詳細タスク分解

### Phase 1: 実装タスク

#### Task 1.1: DocSubtype enum 拡張（knowledge.rs）
- **成果物**: `src/indexer/knowledge.rs`
- **依存**: なし
- **作業内容**:
  1. `DocSubtype` enum に `StageReview` バリアント追加
  2. `DocSubtype::as_str()` に `StageReview => "stage_review"` 追加
  3. `DocSubtype::parse()` メソッド新規追加（全バリアントの文字列→enum変換）
  4. `build_pattern_rules()` に新規パターン追加:
     ```
     ^dev-reports/review/\d{4}-\d{2}-\d{2}-issue(\d+)-[^/]*\.md$
     ```

#### Task 1.2: display_label / sort_order 更新（issue.rs）
- **成果物**: `src/cli/issue.rs`
- **依存**: Task 1.1
- **作業内容**:
  1. `display_label()`: `DocSubtype::StageReview` を `IssueReview | DesignReview` と同じ arm に追加（「レビュー」）
  2. `sort_order()`: `DocSubtype::StageReview => 6` 追加

#### Task 1.3: メタデータデシリアライズ更新（symbol_store.rs）
- **成果物**: `src/indexer/symbol_store.rs`
- **依存**: Task 1.1
- **作業内容**:
  1. `find_documents_by_issue()` の手動 match（L908-918）を `DocSubtype::parse()` 呼び出しに置換

> **注意**: Task 1.1, 1.2, 1.3 は全て同一コミットで変更する（網羅的 match によるコンパイルエラー防止）

### Phase 2: テストタスク

#### Task 2.1: ユニットテスト追加（knowledge.rs）
- **成果物**: `src/indexer/knowledge.rs` #[cfg(test)] mod tests
- **依存**: Task 1.1
- **テストケース**:
  - `test_parse_stage_review`: 基本パース `dev-reports/review/2026-02-18-issue299-security-review-stage4.md`
  - `test_parse_stage_review_multi_digit_issue`: `dev-reports/review/2024-01-01-issue1234-test.md`
  - `test_parse_stage_review_hyphenated_desc`: `dev-reports/review/2024-01-01-issue42-long-desc-with-hyphens-stage1.md`
  - `test_parse_stage_review_non_matching`: 日付なし、issue番号なし、.jsonファイル等
  - `test_doc_subtype_parse`: `DocSubtype::parse()` の全バリアント + unknown

#### Task 2.2: 既存テスト更新（knowledge.rs）
- **成果物**: `src/indexer/knowledge.rs` #[cfg(test)] mod tests
- **依存**: Task 1.1
- **更新内容**:
  - `test_doc_subtype_as_str`: `StageReview` アサーション追加
  - `test_scan_dev_reports_with_temp_dir`: `dev-reports/review/` ファイル追加、count 3→4

#### Task 2.3: 既存テスト更新（issue.rs）
- **成果物**: `src/cli/issue.rs` #[cfg(test)] mod tests
- **依存**: Task 1.2
- **更新内容**:
  - `test_display_label`: `StageReview` → "レビュー" アサーション追加
  - `test_sort_order`: `StageReview` ソート順検証追加
  - `test_grouped`: `StageReview` エントリ追加、「レビュー」カテゴリ検証

#### Task 2.4: E2Eテスト更新
- **成果物**: `tests/e2e_issue.rs`
- **依存**: Task 1.1, 1.2, 1.3
- **更新内容**:
  - `setup_issue_test_data()`: `StageReview` エントリ追加
  - path count assertion: 5→6 に更新

### Phase 3: 品質チェック

#### Task 3.1: 品質検証
- **依存**: 全タスク完了後
- **チェック内容**:
  ```bash
  cargo build
  cargo clippy --all-targets -- -D warnings
  cargo test --all
  cargo fmt --all -- --check
  ```

---

## 品質チェック項目

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |

## Definition of Done

- [x] 設計方針書作成済み
- [x] 設計レビュー完了
- [ ] Task 1.1-1.3: 実装完了（同一コミット）
- [ ] Task 2.1-2.4: テスト追加・更新完了
- [ ] Task 3.1: 品質チェック全パス
- [ ] PR作成・CIチェック通過
