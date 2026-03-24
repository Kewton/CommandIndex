# 仮説検証レポート: Issue #150

## 検証対象の仮説

> ファイル名パースが `dev-reports/design/issue-{NUMBER}-*` と `dev-reports/issue/{NUMBER}/` のパターンのみ対応しており、`dev-reports/review/*-issue{NUMBER}-*` パターンを認識していない。

## 判定: **Confirmed（確認済み）**

## 検証結果

### 現在サポートされているパターン（knowledge.rs: lines 139-176）

`build_pattern_rules()` 関数で定義されている5つの正規表現パターン:

| # | パターン | パス例 |
|---|---------|-------|
| 1 | `^dev-reports/design/issue-(\d+)-.*-design-policy\.md$` | design policy |
| 2 | `^dev-reports/issue/(\d+)/issue-review/summary-report\.md$` | issue review |
| 3 | `^dev-reports/issue/(\d+)/multi-stage-design-review/summary-report\.md$` | design review |
| 4 | `^dev-reports/issue/(\d+)/work-plan\.md$` | work plan |
| 5 | `^dev-reports/issue/(\d+)/pm-auto-dev/.+/progress-report\.md$` | progress report |

### 欠落しているパターン

`dev-reports/review/{DATE}-issue{NUMBER}-{DESCRIPTION}-stage{N}.md` に対応するパターンが**存在しない**。

例:
- `dev-reports/review/2026-02-18-issue299-impact-analysis-review-stage3.md`
- `dev-reports/review/2026-02-18-issue299-security-review-stage4.md`

### 修正対象

- **ファイル**: `src/indexer/knowledge.rs`
- **関数**: `build_pattern_rules()` (line 139)
- **追加すべきパターン**: `^dev-reports/review/\d{4}-\d{2}-\d{2}-issue(\d+)-.*\.md$`
- **リレーション**: `HasReview`

### テストカバレッジ

現在のテスト（knowledge.rs: lines 264-418）は5パターンのみ検証しており、`dev-reports/review/` パターンのテストは存在しない。
