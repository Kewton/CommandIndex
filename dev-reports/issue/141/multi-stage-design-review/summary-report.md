# Issue #141 マルチステージ設計レビュー サマリーレポート

## レビュー実施状況

| Stage | 種別 | エージェント | Must Fix | Should Fix | Nice to Have |
|-------|------|-------------|----------|------------|--------------|
| 1 | 設計原則 | Claude (opus) | 2 | 3 | 3 |
| 2 | 整合性 | Claude (opus) | 3 | 4 | 3 |
| 3 | 影響分析 | Claude (opus) | 2 | 3 | 4 |
| 4 | セキュリティ | Claude (opus) | 0 | 2 | 3 |
| 5 | 設計原則（2回目） | Codex | 2 | 4 | 2 |
| 6 | 指摘反映 | Claude (sonnet) | 2件反映 | - | - |
| 7 | 整合性・影響（2回目） | Codex | 2 | 4 | 2 |
| 8 | 指摘反映 | Claude (sonnet) | 2件反映 | - | - |

## 主要な修正内容

### 1回目レビュー（Stage 1-4）: Must Fix 7件
1. display_label() を output/human.rs に移動（SRP準拠）
2. WhyError::Validation(SearchError) を InvalidArgument(String) に変更
3. file: String を files: Vec<String> に変更（既存パターン統一）
4. WhyError を ImpactError パターンに統一（SymbolDbNotFound追加）
5. index_path は Impact パターンに合わせてコマンド内定義
6. help_llm テスト件数 14→15 更新を明記
7. --help の why 検証追加を明記

### 2回目レビュー（Stage 5-8）: Must Fix 4件
1. インデックスパス解決を resolve_index_path + symbol_db_path() に修正
2. Path形式の出力契約を全フォーマット統一（入力ファイル含む）
3. --index-path をグローバル Cli::index_path に統一（サブコマンド定義から削除）
4. ターミナルインジェクション対策（strip_control_chars）をセキュリティ設計に追加

## 最終設計方針書の状態
- レイヤー構成・責務: 明確
- 設計判断・トレードオフ: 4判断を記録
- データフロー: resolve_index_path 含む完全なフロー
- 型設計: WhyResult / WhyIssueEntry / WhyDocumentEntry + WhyError
- CLI: グローバル index_path 使用、files: Vec<String>
- セキュリティ: パストラバーサル + SQLインジェクション + ターミナルインジェクション対策
- テスト: 単体 + 統合テスト計画
