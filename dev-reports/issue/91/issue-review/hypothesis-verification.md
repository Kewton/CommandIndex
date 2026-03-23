# 仮説検証レポート - Issue #91

## 検証日: 2026-03-23

| # | 仮説 | 判定 | 根拠 |
|---|------|------|------|
| 1 | impact (#90) の内部ロジックを共用できるか | **Confirmed** | `aggregate_impact()`, `RelatedSearchEngine::find_related()`, stdin サポートが実装済み |
| 2 | --related 検索ロジックが存在するか | **Confirmed** | `RelatedSearchEngine` が `/src/search/related.rs` に完全実装済み（多ソーススコアリング） |
| 3 | CLI clap 定義で search に --changed-since を追加できるか | **Confirmed** | `conflicts_with_all` を使った柔軟な構造。新しい Option 追加可能 |
| 4 | human / json / path 出力形式が実装済みか | **Confirmed** | 全3形式が SearchResult, RelatedResult, ImpactResult, SemanticResult, SymbolResult に実装済み |
| bonus | Git diff インフラが存在するか | **Confirmed** | `count_files_changed_since()`, `run_git()`, `validate_commit_hash()` が `/src/cli/status/git_info.rs` に存在 |

## 実装可能性評価

**HIGHLY FEASIBLE** - 全基盤コンポーネントが存在。

### 活用可能なコードパス
1. `/src/cli/status/git_info.rs` - git diff ロジック
2. `/src/search/related.rs` - 関連ファイル検索
3. `/src/cli/impact.rs` - 集約パターン
4. `/src/output/` - 出力フォーマット
5. `/src/main.rs` - CLI引数定義
