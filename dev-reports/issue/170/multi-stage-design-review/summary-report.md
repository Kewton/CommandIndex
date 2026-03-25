# Issue #170 マルチステージ設計レビュー サマリーレポート

## 概要
- **Issue**: #170 why/issueのJSON出力に日付情報を付与する
- **設計方針書**: dev-reports/design/issue-170-json-date-design-policy.md
- **実施日**: 2026-03-25
- **ステージ**: Stage 1-4 完了、Stage 5-8 スキップ（CommandMateサーバー停止）

## レビュー結果サマリー

| Stage | 種別 | Must Fix | Should Fix | Nice to Have |
|-------|------|----------|------------|--------------|
| 1 | 設計原則 | 3 | 4 | 3 |
| 2 | 整合性 | 3 | 4 | 3 |
| 3 | 影響分析 | 3 | 4 | 3 |
| 4 | セキュリティ | 0 | 2 | 3 |
| **合計** | | **9** | **14** | **12** |

## 主要な指摘と対応

### Stage 1: 設計原則（SOLID/KISS/YAGNI/DRY）
- **M1**: 正規表現アンカー不足 → `^` 追加で対応
- **M2**: `line[..10]` パニック可能性 → `line.get(..10)?` で対応
- **M3**: 構造体間フィールド重複 → 別Issueでリファクタリング
- **S1**: Regex毎回コンパイル → `LazyLock` でキャッシュ
- **S2**: 日付バリデーション不足 → `chrono::NaiveDate` 追加

### Stage 2: 整合性
- **M1**: `parse_dev_report_path` のシグネチャ変更問題 → 責務分離（`scan_dev_reports` 内で別途日付取得）
- **M2**: `insert_knowledge_entries` の metadata 構築未記載 → コード例追記
- **M3**: `format_json` 変更コード未記載 → 実装例追記

### Stage 3: 影響分析
- **M1**: `suggest.rs` の影響漏れ → 変更ファイル一覧に追加
- **M2**: `symbol_store.rs` テスト漏れ → テスト修正箇所追加（約10箇所）
- **M3**: 既存metadata互換性 → データ整合性セクション追加

### Stage 4: セキュリティ
- Must Fix なし
- **S1**: `validate_git_file_path` 相当のパス検証追加
- **S2**: 関数可視性を `pub(crate)` に制限

## 設計方針書の改善箇所

1. ユーティリティ関数: `^` アンカー、`LazyLock` キャッシュ、`chrono` バリデーション、`tracing` ログ
2. メタデータフロー: `scan_dev_reports` 内での責務分離、具体的なmetadata構築コード
3. 影響範囲: `suggest.rs`、`before_change.rs`、`symbol_store.rs` テスト追加
4. データ整合性: re-index前のNone処理方針
5. セキュリティ: パス検証、関数可視性制限

## 結論

設計方針書は4段階のレビューで大幅に改善された。セキュリティ上の深刻な問題はなく、実装に進められる状態。
