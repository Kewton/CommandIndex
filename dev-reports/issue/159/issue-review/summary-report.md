# マルチステージIssueレビュー サマリーレポート: Issue #159

## 概要
- **Issue**: #159 - before-changeのデフォルトlimitがIssue単位ではなくドキュメント単位で切られる
- **レビュー日**: 2026-03-25
- **実施ステージ**: Stage 0.5, 1, 2, 3, 4（Stage 5-8はCodexタイムアウトによりスキップ）

## 仮説検証結果（Phase 0.5）

| 仮説 | 判定 |
|---|---|
| `--limit` がドキュメント単位で適用される | **Confirmed** |
| Issue #104が7件消費し#112が3件でlimit到達 | **Confirmed** |
| whyコマンドは全Issue表示できる | **Confirmed** |

根本原因: `before_change.rs` L408で `findings.into_iter().take(limit)` がドキュメント単位でカット。

## Stage 1: 通常レビュー（Claude Opus）

### Must Fix (2件 → 対応済み)
1. 受け入れ基準が明示されていない → 6項目の受け入れ基準を追加
2. 改善案が4つ並列で優先順位が不明 → 推奨案（案1+案4）を選定

### Should Fix (4件 → 対応済み)
- whyコマンドとの設計差異分析
- セマンティックランキングとの相互作用
- E2Eテストケース追加
- コード参照の正確性

## Stage 3: 影響範囲レビュー（Claude Opus）

### Must Fix (5件 → 対応済み)
1. before_change.rs L408のlimit適用ロジック変更
2. main.rs --limitヘルプテキスト更新
3. help_llm.rs LLM向けヘルプ更新
4. BEFORE_CHANGE_AFTER_HELP ヘルプ例文更新
5. E2Eテスト before_change_limit_respected 更新

### Should Fix (4件 → 対応済み)
- BeforeChangeResult にdisplayed_issues情報追加
- relation_priority順序修正
- rank_by_max_similarity() のIssue単位集約
- 出力フォーマッタのIssue単位グルーピング表示

## Stage 5-8: スキップ
Codex via commandmatedev がタイムアウト（600s超過）。1回目レビューで全Must Fix指摘が反映済み。

## Issue更新状況
- **Stage 2**: 受け入れ基準追加、改善方針統合
- **Stage 4**: 影響範囲テーブル追加、relation_priority受け入れ基準追加、後方互換性方針明記

## 最終判定
Issue #159は実装に必要な情報が十分に整備されました。次のフェーズ（設計方針書作成）に進行可能です。
