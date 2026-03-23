# 設計レビュー サマリーレポート - Issue #93

## 実施ステージ

| Stage | 種別 | 実行者 | 状態 | Must Fix |
|-------|------|--------|------|----------|
| 1 | 設計原則 (SOLID/KISS/YAGNI/DRY) | Claude (opus) | 完了 | 3件 |
| 2 | 整合性 | Claude (opus) | 完了 | 4件 |
| 3 | 影響分析 | Claude (opus) | 完了 | 3件 |
| 4 | セキュリティ | Claude (opus) | 完了 | 2件 |
| 5-8 | 2回目レビュー | Codex | スキップ | - |

## 主な改善点

### Stage 1: 設計原則
- SRP: Debouncer 構造体、is_relevant_event()、is_recoverable() に責務分離
- DRY: WatchError::is_recoverable() でエラー分類を一箇所に集約

### Stage 2: 整合性
- --path から short フラグ削除（既存コマンドとの一貫性）
- workspace 非対応を制限事項に明記
- Cargo.toml 依存追加、mod.rs 宣言、Commands enum 追加を明記

### Stage 3: 影響分析
- CLI help テストへの watch 追加
- 前回 run_incremental() 実行中のスキップ制御

### Stage 4: セキュリティ
- TOCTOU 対策: canonicalize() によるベースディレクトリ検証
- パストラバーサル対策: パス正規化と前方一致検証
- リトライポリシー: 最大3回、指数バックオフ
- SIGTERM 対応: ctrlc termination フィーチャー
