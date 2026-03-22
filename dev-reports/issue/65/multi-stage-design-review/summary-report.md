# マルチステージ設計レビュー サマリーレポート - Issue #65

## 実施日: 2026-03-22

## レビュー概要

| Stage | 種別 | エージェント | Must Fix | Should Fix | Nice to Have |
|-------|------|-------------|----------|------------|--------------|
| 1 | 設計原則 | Claude (opus) | 3 | 4 | 3 |
| 2 | 整合性 | Claude (opus) | 3 | 5 | 3 |
| 3 | 影響分析 | Claude (opus) | 5 | 6 | 4 |
| 4 | セキュリティ | Claude (opus) | 2 | 3 | 3 |
| 1-4反映 | - | Claude (sonnet) | 19件反映 | - | - |
| 5 | 設計原則2回目 | Codex (gpt-5.4) | 4 | 5 | 3 |
| 6 | 反映 | Claude (sonnet) | 7件反映 | - | - |
| 7 | 整合性・影響2回目 | Codex (gpt-5.4) | 4 | 4 | 3 |
| 8 | 反映 | Claude (sonnet) | 6件反映 | - | - |

## 主要な改善ポイント

### DRY/YAGNI
- cohere.rs 空スケルトン削除
- RerankProviderType enum からCohereバリアント削除
- providerフィールド削除（初期はOllamaのみ）

### API設計
- RerankProviderトレイト最小化（rerank()のみ、provider_name()削除）
- 引数を `&[RerankCandidate]` に変更（借用で十分）
- RerankResultの契約明記（index範囲、重複、未返却の扱い）
- エラーハンドリング責務分担をトレイトdocに明文化

### セキュリティ
- プロンプトインジェクション対策（デリミタ分離）
- API Key平文保存警告
- エラーメッセージトランケート
- 非localhost HTTP通信警告

### 整合性
- build_document_text のUTF-8安全切り詰め（chars().take()）
- Config暫定配置の明記
- 出力モジュールのscore意味変更を明記
