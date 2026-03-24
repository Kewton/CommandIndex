# マルチステージ設計レビュー サマリーレポート

## Issue #140: `commandindex issue <number>` コマンドの実装

## レビュー実施日: 2026-03-24

## ステージ実施結果

| Stage | レビュー種別 | 実行エージェント | Must Fix | Should Fix | Nice to Have |
|---|---|---|---|---|---|
| 1 | 設計原則（SOLID/KISS/YAGNI/DRY） | Claude (opus) | 2 | 3 | 3 |
| 2 | 整合性 | Claude (opus) | 3 | 5 | 3 |
| 3 | 影響分析 | Claude (opus) | 3 | 4 | 3 |
| 4 | セキュリティ | Claude (opus) | 0 | 3 | 3 |
| 1-4反映 | - | Claude (sonnet) | 全件反映 | - | - |
| 5 | 設計原則（2回目） | Codex (gpt-5.4) | 3 | 4 | 2 |
| 6 | 指摘反映（2回目） | Claude (sonnet) | 全件反映 | - | - |
| 7 | 整合性・影響分析（2回目） | Codex (gpt-5.4) | 3 | 4 | 2 |
| 8 | 指摘反映（2回目） | Claude (sonnet) | 全件反映 | - | - |

## 主要な改善ポイント

### 1回目レビュー（Claude Opus 4段階）
1. **DRY違反修正**: IssueDocumentEntry に既存 KnowledgeRelation/DocSubtype enum を再利用
2. **SRP改善**: IssueDocumentsResult を cli/issue.rs 内に配置（output/mod.rs の凝集度維持）
3. **KISS**: 出力フォーマッタを cli/issue.rs にインライン実装（suggest パターン準拠）
4. **整合性**: CLIディスパッチパターン修正、Display トレイト追加
5. **テスト波及**: help-llm テスト（コマンド数14→15）の更新を明記
6. **Serialize**: IssueDocumentEntry に Serialize derive 追加

### 2回目レビュー（Codex gpt-5.4）
1. **エラーハンドリング強化**: NotFound/CorruptedMetadata バリアント追加、silent skip 禁止
2. **SRP改善**: display_label() を cli/issue.rs 内ヘルパーに移動
3. **DRY**: grouped() ヘルパーで分類ロジック集約
4. **整合性修正**: resolve_commandindex_dir() のアクセス可能性問題を修正
5. **未インデックス検出**: symbols.db 存在確認を明示的に行う方針追加
6. **Serialize波及**: KnowledgeRelation/DocSubtype への Serialize 追加を明記

## 設計方針書の最終状態

- 型設計: 既存 enum 再利用、Serialize 対応
- SQLクエリ: ソートはRust側、LIMIT 100 付き
- エラー処理: NotFound + CorruptedMetadata + SymbolStore + Output の4バリアント、From実装付き
- 出力: cli/issue.rs 内インライン、grouped() で分類
- セキュリティ: パラメータバインド、strip_control_chars、LIMIT、metadata破損検知
- テスト: cli_args.rs 回帰 + help-llm回帰 + E2E 8ケース

## 結論

8段階レビュー（Claude Opus 4回 + Codex 2回 + 反映2回）により設計方針書の品質は大幅に向上。
当初はString型のDTO、SQLソート、NotFoundなしの設計だったが、レビューを経て型安全性、DRY、エラーハンドリング、実装可能性がすべて改善された。
設計方針書は実装着手可能な状態。
