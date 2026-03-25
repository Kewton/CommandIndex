# マルチステージ設計レビュー サマリーレポート: Issue #159

## 概要
- **Issue**: #159 - before-changeのlimitをIssue単位に変更
- **レビュー日**: 2026-03-25
- **実施ステージ**: Stage 1-4（Stage 5-8はMust Fix対応済みによりスキップ）

## Stage 1: 設計原則レビュー（Claude Opus）

### Must Fix (2件 → 反映済み)
1. total_issuesのセマンティクス曖昧 → 判断6として再定義を追加
2. BTreeMap + Vec二重管理（KISS違反） → HashMapに変更、doc comment追加

### Should Fix (4件 → 反映済み)
- max_docs_per_issueを定数化（KISS）
- rank_by_max_similarityのソートを3段階に
- 既存テスト修正をテスト戦略に追記
- JSON findings最大件数変更の注記追加

## Stage 2: 整合性レビュー（Claude Opus）

### Must Fix (7件 → 全て実装時対応項目)
設計書と現行コードの差分であり、設計書自体の修正は不要。実装フェーズで対応。

## Stage 3: 影響分析レビュー（Claude Opus）

### Must Fix (3件 → 反映済み)
1. E2Eテストアサーション更新（limit=1で最大2件返る）
2. total_issuesセマンティクス変更のbreaking change明記
3. 既存テスト期待値更新

### Should Fix (4件 → 反映済み)
- json.rs手動JSON構築の注意事項追加
- displayed_issues表示形式の定義
- limit=0エッジケース → value_parserで拒否
- テスト戦略にエッジケース追加

## Stage 4: セキュリティレビュー（Claude Opus）

### Must Fix (1件 → 反映済み)
1. --limitにvalue_parser範囲制約追加 → range(1..=1000)

## Stage 5-8: スキップ
設計書のMust Fix 3件（Stage 1: 2件 + Stage 4: 1件）は全て対応済み。2回目レビュー不要と判断。

## 設計方針書の主要な改善点
1. total_issuesのセマンティクス明確化（判断6追加）
2. group_and_limit_by_issue()をHashMapベース+ソート済み前提条件明記に改善
3. MAX_DOCS_PER_ISSUE定数化
4. --limitのvalue_parser範囲制約追加
5. displayed_issuesの表示形式定義
6. テスト戦略にエッジケース追加（limit=0拒否、limit>issue数）

## 最終判定
設計方針書は実装に十分な品質に達しました。次のフェーズ（作業計画立案）に進行可能です。
