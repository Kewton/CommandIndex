# Issue #125 マルチステージ設計レビュー サマリーレポート

## 概要
| 項目 | 値 |
|---|---|
| Issue | #125: rerankフォールバック通知改善 |
| レビュー日 | 2026-03-24 |
| ステージ | 全8ステージ完了 |

## レビュー統計

| ステージ | 実行者 | Must Fix | Should Fix | Nice to Have |
|---|---|---|---|---|
| Stage 1: 設計原則 | Claude Opus | 2 | 4 | 3 |
| Stage 2: 整合性 | Claude Opus | 3 | 5 | 3 |
| Stage 3: 影響分析 | Claude Opus | 3 | 4 | 3 |
| Stage 4: セキュリティ | Claude Opus | 1 | 4 | 4 |
| Stage 5: 設計原則2回目 | Codex (GPT-5.4) | 4 | 4 | 3 |
| Stage 7: 整合性2回目 | Codex (GPT-5.4) | 4 | 4 | 3 |
| **合計** | | **17** | **25** | **19** |

## 主要な設計改善

### 1. trait戻り値の非破壊的拡張 (OCP)
- **前**: `Result<(Vec<RerankResult>, Vec<String>), RerankError>` に変更
- **後**: `Result<Vec<RerankResult>, RerankError>` を維持。`PartialTimeout` バリアント追加で部分適用を表現

### 2. ヒントメッセージのCLI層分離 (SRP)
- **前**: `RerankError::Display` にヒント埋め込み
- **後**: `Display` はエラー事実のみ。ヒントは `rerank_error_hint()` で CLI 層付加

### 3. 出力ヘルパーの責務分離 (SRP)
- **前**: `emit_rerank_status()` が writer + stderr の複数責務
- **後**: `build_rerank_stdout_prefix()` と `build_rerank_stderr_message()` に分離

### 4. YAGNI簡素化
- `NotRequested` 削除（rerank フラグで分岐可能）
- `warnings: Vec<String>` → `warning: String`

### 5. セキュリティ強化
- JSON/llm の reason はサニタイズ済み一般化メッセージのみ
- stdout 出力値の制御文字除去・改行正規化
- ApiError はステータスコードのみ。レスポンス詳細は stderr のみ

### 6. RerankStatus の配置
- `src/cli/search.rs` 内の private enum（CLI 出力制御専用）

## 設計方針書の最終状態
`dev-reports/design/issue-125-rerank-fallback-design-policy.md` が全レビュー指摘を反映済み。
