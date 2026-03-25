# Issue #169 マルチステージ設計レビュー サマリーレポート

## レビュー実施状況

| Stage | 種別 | 実行者 | Must Fix | Should Fix | Nice to Have |
|-------|------|--------|----------|------------|--------------|
| 1 | 設計原則 (SOLID/KISS/YAGNI/DRY) | Claude opus | 0 | 3 | 4 |
| 2 | 整合性 | Claude opus | 3 | 4 | 3 |
| 3 | 影響分析 | Claude opus | 3 | 2 | 3 |
| 4 | セキュリティ | Claude opus | 0 | 3 | 3 |
| 5 | 設計原則（2回目） | Codex (gpt-5.4) | 3 | 4 | 1 |
| 6 | 反映 | Claude sonnet | - | - | - |
| 7 | 整合性・影響分析（2回目） | Codex (gpt-5.4) | 3 | 4 | 1 |
| 8 | 反映 | Claude sonnet | - | - | - |

## 主要な改善点

### 設計原則 (Stage 1, 5)
- IssueListResult ラッパー削除 (YAGNI)
- label取得のN+1クエリ回避（SQLで一括取得）
- IssueListRow (データ層) / IssueListEntry (CLI表示モデル) の責務分離
- run() → run_show() リネームで API 対称性確保
- open_symbol_store() ヘルパーでDRY改善

### 整合性 (Stage 2, 7)
- symbol_db_path() 使用でDBパス取得を一元化
- boolean集計条件にkn_doc.type='document'追加
- suggest.rs のテスト3件も更新対象に追加
- help_llm.rs の具体的変更箇所3点を明記
- cli_args.rs の影響見積もりを修正

### セキュリティ (Stage 4, 7)
- 不正identifier: 警告ログ出力しスキップ（サイレントスキップ禁止）
- エラーメッセージのサニタイズ
- 正規表現 unwrap → expect に変更
- 制御文字除去方針追加

## 最終設計書状態
- セクション数: 13
- テスト戦略: 5カテゴリ（単体2 + E2E + CLI + 回帰）
- セキュリティ対策: 6項目
- 影響ファイル: 7ファイル
