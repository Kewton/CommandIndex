# Issue #168 マルチステージ設計レビュー サマリーレポート

## 実施状況

| Stage | 種別 | 実行者 | Must Fix | Should Fix | Nice to Have | 状態 |
|-------|------|--------|----------|------------|--------------|------|
| 1 | 設計原則 | Claude (opus) | 2 | 4 | 3 | 完了・反映済 |
| 2 | 整合性 | Claude (opus) | 4 | 4 | 3 | 完了・反映済 |
| 3 | 影響分析 | Claude (opus) | 3 | 4 | 4 | 完了・反映済 |
| 4 | セキュリティ | Claude (opus) | 1 | 3 | 3 | 完了・反映済 |
| 5-8 | 2回目 | Codex | - | - | - | スキップ（サーバーエラー） |

## 主要な指摘と反映結果

### 1. enrich関数の空文字列処理統一（全Stageで指摘）
- **問題**: 既存enrich関数はSome("")、新規はNone→契約分裂
- **対応**: 既存関数も空→None変換に統一するリファクタリングを設計書に追記

### 2. snippet_lines/snippet_charsの上限設定（Stage 2,4）
- **問題**: range(1..)で上限なし→メモリ制御不能リスク
- **対応**: range(1..=100)/range(1..=10000)に変更

### 3. IssueDocumentEntry定義場所の確定（Stage 2）
- **問題**: 「knowledge.rs or issue.rs」と曖昧
- **対応**: knowledge.rs L179に確定

### 4. SnippetConfigデフォルト値の注入箇所（Stage 1,2）
- **問題**: lines=3, chars=200のデフォルト値をどこで設定するか不明確
- **対応**: 定数定義（KNOWLEDGE_SNIPPET_LINES/CHARS）+ unwrap_or パターンを明記

### 5. run_before_change()シグネチャ（Stage 2）
- **問題**: index_pathパラメータが設計書から欠落
- **対応**: 既に含まれていることを確認（セクション8で正しく記載済み）

## 設計品質評価

設計方針書は以下の点で適切:
- 既存パターン（enrich_*_with_snippets）への準拠
- 後方互換性（--with-snippet デフォルトオフ）
- tantivy未存在時の非fatalフォールバック
- YAGNI（Phase 2 セクション優先抽出をスコープ外に分離）
- セキュリティ（unsafe不使用、CLI引数上限設定、出力サニタイズ）

## 残存リスク

- issue JSONの条件付きスキーマ（--with-snippet有無で型が変わる）はAPI消費者にとって扱いにくい可能性あり
- 4つのenrich関数のDRY改善（トレイトベース統合）は将来の検討課題
