# マルチステージ設計レビュー サマリーレポート

## Issue情報
- **Issue番号**: #109
- **タイトル**: 検索クエリのLLM向けガイド (suggest)
- **レビュー実施日**: 2026-03-23

## レビュー概要

| Stage | 種別 | 実行者 | Must Fix | Should Fix | Nice to Have | 状態 |
|-------|-------|--------|----------|------------|--------------|------|
| 1 | 設計原則 (SOLID/KISS/YAGNI/DRY) | Claude opus | 3 | 4 | 3 | 完了・反映済 |
| 2 | 整合性 | Claude opus | 4 | 4 | 3 | 完了・反映済 |
| 3 | 影響分析 | Claude opus | 3 | 4 | 3 | 完了・反映済 |
| 4 | セキュリティ | Claude opus | 1 | 3 | 3 | 完了・反映済 |
| 5-8 | 2回目レビュー | Codex | - | - | - | スキップ（サーバーエラー） |

## 統合Must Fix一覧（重複排除後 7件）

| # | 指摘 | 反映内容 |
|---|------|---------|
| 1 | OutputError/Writer パターン統一 | format_suggest_results に writer 引数追加、OutputError 使用に変更 |
| 2 | SymbolStore モジュールパス修正 | embedding/store.rs → indexer/symbol_store.rs に修正 |
| 3 | SearchContext::new() 直接使用 | resolve_context() を削除、SearchContext::new() に置換 |
| 4 | index_path グローバルオプション利用 | Suggest バリアントから index_path 削除 |
| 5 | impact共通化撤回 | RelatedSearchEngine::find_related() を直接利用、impact.rs 変更なし |
| 6 | help-llm テスト影響度修正 | tests/cli_args.rs 影響度を低→中に変更 |
| 7 | コマンド文字列サニタイズ | sanitize_for_command_arg() 追加、セキュリティ設計に不正文字入力対策追加 |

## 追加改善（Should Fix から採用）

- SuggestError に SymbolStore バリアント追加
- SuggestStep.step 削除（enumerate()で出力時付与）
- SuggestResult に query, has_embeddings フィールド追加
- OutputFormat::Path で1行1コマンド出力に対応
- validate_input() に制御文字チェック追加
- BINARY_NAME 定数追加（DRY）
- 既存コマンド数 12→13 に修正
- human出力を英語に変更（既存パターン準拠）

## 最終状態
- 設計方針書は1回目4段階レビューの全指摘を反映済み
- 2回目レビュー（Codex）はサーバーエラーによりスキップ
- 実装着手可能な品質に到達
