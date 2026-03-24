# Issue #127 マルチステージ設計レビュー サマリーレポート

## 設計方針書
`dev-reports/design/issue-127-suggest-keyword-design-policy.md`

## 実施ステージ

| Stage | 種別 | Must Fix | Should Fix | Nice to Have |
|-------|------|----------|------------|-------------|
| 1 | 設計原則（SOLID/KISS/YAGNI/DRY） | 1 | 3 | 3 |
| 2 | 整合性 | 2 | 2 | 3 |
| 3 | 影響分析 | 2 | 4 | 3 |
| 4 | セキュリティ | 0 | 3 | 3 |
| 5-8 | 2回目レビュー | **スキップ** | - | - |

## 主要な設計変更

### 1. dedup順序の変更（Stage 1 S2 + Stage 3 M1）
- **変更前**: dedup(limit=5) → boost
- **変更後**: dedup(全件) → weight → truncate(5)
- **理由**: BM25上位5件がテストファイルで占有された場合に6位以下のソースファイルが救済されない

### 2. is_test_file() の判定精度改善（Stage 1 M1 + Stage 3 S4）
- **変更前**: `file_name.contains("test")`
- **変更後**: `_test.` / `.test.` / `_spec.` / `.spec.` + `test_` プレフィックス + `/tests/` ディレクトリ
- **理由**: `contest.rs`, `latest.rs`, `test_utils.rs` 等の誤検知防止

### 3. is_doc_file() の判定範囲限定（Stage 3 S3）
- **変更前**: `.md` 拡張子を全てドキュメント扱い
- **変更後**: `dev-reports/`, `docs/`, ルート直下定型ドキュメントのみ
- **理由**: Markdown中心プロジェクトでのsuggest精度低下防止

### 4. 命名変更（Stage 2 S1）
- `TEST_FILE_BOOST` → `TEST_FILE_WEIGHT`
- `apply_file_type_boost` → `apply_file_type_weight`
- **理由**: 実質「減衰」の処理であり「boost」は誤解を招く

### 5. パフォーマンス最適化（Stage 3 S2）
- `to_lowercase()` を `file_type_weight_factor` 内で1回のみ実行
- `partial_cmp` → `total_cmp` でNaN安全性確保

## スキップ理由
Stage 5-8: 1回目レビューのMust Fix 5件は全て設計方針書に反映済み。設計の根本的な問題は解消されており、2回目レビューによる追加の品質向上効果は限定的と判断。
