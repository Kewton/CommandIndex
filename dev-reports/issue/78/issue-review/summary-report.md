# マルチステージIssueレビュー サマリーレポート

## Issue: #78 [Feature] マルチリポジトリ横断検索
## レビュー日: 2026-03-22

---

## レビュー結果概要

| ステージ | 種別 | 実行モデル | Must Fix | Should Fix | Nice to Have |
|---------|------|-----------|----------|------------|-------------|
| 0.5 | 仮説検証 | Claude Sonnet | - | - | - |
| 1 | 通常レビュー（1回目） | Claude Opus | 3 | 5 | 4 |
| 2 | 指摘反映（1回目） | Claude Sonnet | - | - | - |
| 3 | 影響範囲レビュー（1回目） | Claude Opus | 5 | 7 | 5 |
| 4 | 指摘反映（1回目） | Claude Sonnet | - | - | - |
| 5 | 通常レビュー（2回目） | Claude Opus* | 3 | 5 | 3 |
| 6 | 指摘反映（2回目） | Claude Sonnet | - | - | - |
| 7 | 影響範囲レビュー（2回目） | Claude Opus* | 4 | 6 | 4 |
| 8 | 指摘反映（2回目） | Claude Sonnet | - | - | - |

*Codex(commandmatedev)接続不可のため、Claude Opusで代替実行

### 指摘総数: Must Fix 15件 / Should Fix 23件 / Nice to Have 16件
### 全Must Fix/Should Fix: 反映済み

---

## 仮説検証結果

| 仮説 | 判定 | 概要 |
|------|------|------|
| インデックス独立性 | Confirmed | indexer/mod.rsにパス管理関数あり、独立インデックス設計は既存と整合 |
| 結果マージ | Unverifiable | 未実装。新規実装が必要 |
| CLIオプション追加 | Confirmed | clap構造への追加は容易 |
| チーム設定(#76)依存 | Confirmed | 完全実装済み |
| rayon並列化 | Rejected | Cargo.tomlに依存なし。新規追加が必要 |

---

## 主要な指摘事項（ハイライト）

### 1回目レビューの重要指摘
- **M1**: 全検索関数のPath::new(".")ハードコード除去 → SearchContext構造体導入
- **M2**: SearchResultにrepository: Option<String>フィールド追加で後方互換維持
- **M3**: ワークスペース設定ファイルのTOMLスキーマ詳細定義（パス解決、エイリアス、エラーハンドリング）
- **S2**: Phase 1はBM25のみ横断対応、ハイブリッド検索は将来Phase

### 2回目レビューで発見された追加指摘
- **rrf_mergeキー設計**: リポ間マージ時に(repository, path, heading)の3タプルキーが必要
- **セキュリティ**: canonicalize()後の.commandindex/存在チェック、チルダ展開HOME未設定対応
- **SearchResult構築箇所の網羅**: プロダクションコード（enrich_semantic_to_search_results, doc_to_search_result）も修正対象
- **Phase 1非対応コマンド**: embed, context, clean, config show/pathを統一的に定義
- **リポ数上限**: 50リポまで、mmapファイルハンドル上限対策

---

## Issue更新履歴

| ステージ | 追加・変更セクション |
|---------|------------------|
| Stage 2 | 実装前提条件(リファクタリング)セクション新設、TOMLスキーマ定義、検索スコープ定義、status出力仕様 |
| Stage 4 | 影響範囲分析セクション新設、影響ファイル一覧(14ファイル)、受け入れ基準12項目追加 |
| Stage 6 | スコアマージ方式(RRFスタイル)セクション、SearchContextフィールド定義、Phase 1非対応サブコマンド、update --workspaceエラーハンドリング |
| Stage 8 | セキュリティ対策セクション、rrf_merge_cross_repo関数、リポ数上限、WorkspaceConfigError、進捗メッセージ |

---

## 最終Issue品質評価

| 評価項目 | 結果 |
|---------|------|
| 受け入れ基準の網羅性 | ✅ 基本機能・リファクタリング・セキュリティ・テスト・パフォーマンスを網羅 |
| 実装方針の明確性 | ✅ SearchContext構造体、WorkspaceConfig、スコアマージ方式が明確 |
| 影響範囲の分析 | ✅ 14+ファイルの影響と対応方針を定義 |
| セキュリティ考慮 | ✅ パストラバーサル防止、チルダ展開安全性を追加 |
| エッジケース考慮 | ✅ パス重複、エイリアス重複、未インデックスリポ、HOME未設定等 |
| スコープ定義 | ✅ Phase 1非対応コマンド一覧が明確 |

---

## 成果物一覧

```
dev-reports/issue/78/issue-review/
├── original-issue.json           # 元のIssue内容
├── hypothesis-verification.md    # 仮説検証レポート
├── stage1-review-context.json    # 通常レビュー（1回目）
├── stage2-apply-result.json      # 指摘反映（1回目）
├── stage3-review-context.json    # 影響範囲レビュー（1回目）
├── stage4-apply-result.json      # 指摘反映（1回目）
├── stage5-review-context.json    # 通常レビュー（2回目）
├── stage6-apply-result.json      # 指摘反映（2回目）
├── stage7-review-context.json    # 影響範囲レビュー（2回目）
├── stage8-apply-result.json      # 指摘反映（2回目）
└── summary-report.md             # 本レポート
```
