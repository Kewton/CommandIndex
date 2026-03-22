# マルチステージIssueレビュー サマリーレポート

## Issue: #89 [Feature] stdin パイプ入力対応

## レビュー実施概要

| Stage | 種別 | 実行エージェント | Must Fix | Should Fix | Nice to Have |
|-------|------|-----------------|----------|------------|-------------|
| 0.5 | 仮説検証 | Claude | - | - | - |
| 1 | 通常レビュー（1回目） | Claude Opus | 2 | 4 | 4 |
| 2 | 指摘反映（1回目） | Claude Sonnet | - | - | - |
| 3 | 影響範囲レビュー（1回目） | Claude Opus | 6 | 5 | 4 |
| 4 | 指摘反映（1回目） | Claude Sonnet | - | - | - |
| 5 | 通常レビュー（2回目） | Codex (gpt-5.4) | 2 | 5 | 2 |
| 6 | 指摘反映（2回目） | Claude Sonnet | - | - | - |
| 7 | 影響範囲レビュー（2回目） | Codex (gpt-5.4) | 0 | 5 | 3 |
| 8 | 指摘反映（2回目） | Claude Sonnet | - | - | - |

## 主要な改善点

### Stage 1-2 で修正された点
- impact サブコマンドの機能定義・出力スキーマを追加
- context との差分を明確化
- 受け入れ基準に出力形式サポートを追加
- 実装規模を「小」→「中」に修正

### Stage 3-4 で修正された点
- 影響範囲セクションを新規追加（CLI層・エラー型・出力フォーマッタ・テスト・依存）
- stdin ユーティリティ配置（cli/stdin.rs）を明記
- 入力ファイル数上限500件を追加
- --related-stdin と --related の相互排他を受け入れ基準に追加

### Stage 5-6 で修正された点
- RelatedSearchEngine が双方向集計であることを明記（「逆依存分析」→「関連ファイル集約分析」）
- search --related-stdin の複数入力時の集約ルール明記（union + 最大スコア）
- --related-stdin 採用理由の明記
- relation_types を既存 snake_case 規約に統一
- パスバリデーション追加（相対パスのみ、.. 禁止）
- エッジケースの挙動テーブル追加
- impact の引数形を明記（引数 or stdin、引数優先）

### Stage 7-8 で修正された点
- バックスラッシュの扱いを「正規化」→「禁止」に統一
- 重複排除を「正規化後のパスで比較」に明確化

## 残存事項（設計フェーズで対応）

- 集約ロジックの共通化方針（context / impact / related-stdin）
- impacted_by の保持コスト・最大件数の検討
- 共通CLIエラー型の設計
- tests/output_format.rs への ImpactResult テスト追加

## 結論

Issue #89 は4段階8ステージのレビューを経て、実装に必要な仕様が十分に明確化されました。Must Fix は全て解決済みです。
