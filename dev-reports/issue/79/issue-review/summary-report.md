# マルチステージIssueレビュー サマリーレポート

## Issue: #79 [Feature] チーム向けstatusコマンド拡張（インデックスカバレッジ・統計）

## レビュー実施日: 2026-03-22

## ステージ実施結果

| Stage | 種別 | 実施 | Must Fix | Should Fix | Nice to Have |
|-------|------|------|----------|------------|--------------|
| 0.5 | 仮説検証 | スキップ（Feature Issue） | - | - | - |
| 1 | 通常レビュー（1回目） | Claude opus | 4 | 4 | 3 |
| 2 | 指摘反映（1回目） | Claude sonnet | 8件反映 | - | - |
| 3 | 影響範囲レビュー（1回目） | Claude opus | 3 | 5 | 3 |
| 4 | 指摘反映（1回目） | Claude sonnet | 8件反映 | - | - |
| 5-8 | 2回目レビュー | スキップ | - | - | - |

## スキップ理由（Stage 5-8）
1回目レビュー（Stage 1-4）の全 Must Fix 指摘が Issue 本文に反映済みのため、2回目レビューをスキップ。

## 主要な改善点

### Stage 1（通常レビュー）で特定された課題
1. **IndexState スキーマ**: `last_commit_hash` フィールド追加と後方互換性戦略
2. **EmbeddingStore API**: `count_distinct_files()` メソッドの不在
3. **Git 情報取得**: `git2` vs `git` コマンドの未決定 → `std::process::Command` に確定
4. **ファイルカウント**: Total files / Skipped files のデータソース未定義 → walkdir 走査に確定

### Stage 3（影響範囲レビュー）で特定された追加課題
1. **run() シグネチャ**: `StatusOptions` 構造体導入による既存テスト互換
2. **JSON 出力互換**: `#[serde(skip_serializing_if)]` による後方互換維持
3. **パフォーマンス**: walkdir 走査の除外パターン（.git, node_modules, target/）
4. **Storage 内訳**: `StorageBreakdown` 構造体導入

## Issue 更新状況
- ✅ 実装方針の詳細化（StatusOptions, StorageBreakdown 等の具体的な型設計）
- ✅ 受け入れ基準の強化（設計要件、テスト要件の追加）
- ✅ 影響ファイル・テストファイルの一覧明記
- ✅ CLIオプション排他ルールの定義
- ✅ エラーハンドリングパターンの統一方針
- ✅ 将来の拡張候補セクション新設

## 結論
Issue #79 は2回のレビューサイクルを経て、実装に必要な情報が十分に整理された状態。特に IndexState の後方互換性、run() のシグネチャ変更、エラーハンドリングパターンが明確化されたことで、実装時の手戻りリスクが大幅に低減された。
