# マルチステージ設計レビュー サマリーレポート

## Issue: #61 [Feature] Embedding生成基盤（ローカルLLM / API対応）
## 実施日: 2026-03-22

## ステージ実行結果

| Stage | 種別 | エージェント | 状態 | Must Fix | Should Fix |
|-------|------|------------|------|----------|------------|
| 1 | 設計原則レビュー | Claude opus | 完了 | 3 | 4 |
| 2 | 整合性レビュー | Claude opus | 完了 | 4 | 5 |
| 3 | 影響分析レビュー | Claude opus | 完了 | 3 | 5 |
| 4 | セキュリティレビュー | Claude opus | 完了 | 3 | 5 |
| 5-8 | 2回目レビュー | Codex | スキップ | - | - |

## 主要な改善事項（反映済み）

### 設計原則（Stage 1）
- dimension() ハードコード廃止 → OnceLock遅延初期化パターン
- EmbeddingError / EmbeddingStoreError 分離（単一責任）
- Optionsパターン採用（IndexOptions / CleanOptions）
- ProviderType enum導入（型安全性向上）

### 整合性（Stage 2）
- open_in_memory に #[cfg(test)] 属性追加
- EmbeddingStoreError に From<rusqlite::Error> 実装
- EmbedError に Display/Error/From 実装
- embeddings テーブルに UNIQUE 制約追加
- main.rs の match アーム変更コード例追加

### 影響分析（Stage 3）
- keep_embeddings 時の config.toml 保持を明記
- 既存テスト21ファイルはCLI経由のため直接影響なし確認

### セキュリティ（Stage 4）
- api_key の Debug 出力マスキング（カスタム Debug 実装）
- SSRF バリデーション具体化（スキーム制限、url crate）
- テキスト入力サイズ制限追加
- config.toml パーミッション 0600
- タイムアウト具体値明記（connect: 10秒、request: 30秒）

## 設計方針書の状態

- パス: `dev-reports/design/issue-61-embedding-provider-design-policy.md`
- 全Must Fix指摘: 反映完了
