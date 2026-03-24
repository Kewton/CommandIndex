# Issue #125 マルチステージIssueレビュー サマリーレポート

## 概要
| 項目 | 値 |
|---|---|
| Issue | #125: [BUG] --rerank がモデル未検出時にサイレントフォールバックし結果が変わらない |
| レビュー日 | 2026-03-24 |
| ステージ | 全8ステージ完了 |

## 仮説検証結果
全3仮説が **Confirmed**:
1. モデル未検出時、stderrにエラー出力されるが exitコードは0
2. rerankが失敗した場合、出力結果は--rerankなしと完全に同一
3. ユーザーからはrerankが成功したように見える

## レビュー統計

| ステージ | 実行者 | Must Fix | Should Fix | Nice to Have |
|---|---|---|---|---|
| Stage 1: 通常レビュー1回目 | Claude Opus | 3 | 4 | 4 |
| Stage 3: 影響範囲レビュー1回目 | Claude Opus | 4 | 4 | 4 |
| Stage 5: 通常レビュー2回目 | Codex (GPT-5.4) | 3 | 4 | 2 |
| Stage 7: 影響範囲レビュー2回目 | Codex (GPT-5.4) | 4 | 4 | 3 |
| **合計** | | **14** | **16** | **13** |

## 主要な改善点（レビュー前 → 後）

### 1. 実装方針の具体化
- **前**: 改善提案の列挙のみ（exitコード変更 or コメント出力、未決定）
- **後**: Graceful degradation + 明示的フォールバック通知に一本化。`RerankStatus` enum で構造化。

### 2. シグネチャ設計の精緻化
- **前**: 未定義
- **後**: `try_rerank()` → `(Vec<SearchResult>, RerankStatus)`、`rerank()` → `Result<(Vec<RerankResult>, Vec<String>), RerankError>`

### 3. 出力フォーマットの網羅性
- **前**: `<!-- rerank skipped -->` のllmのみ
- **後**: json（メタデータ行 + type判別キー）、llm（コメント）、human/path（stderr）の全フォーマット対応

### 4. エラーハンドリングの一元化
- **前**: `try_rerank()` と `ollama.rs` に `eprintln!` が散在
- **後**: 全 `eprintln!` を `run()` に一元化。RerankError種別ごとのヒントメッセージ定義。

### 5. 影響範囲の明確化
- **前**: 未定義
- **後**: 影響対象ファイル一覧、後方互換性方針、パフォーマンス影響、スコープ外事項を明記。

## Issue最終状態
GitHub Issue #125 が全レビュー指摘を反映した状態で更新済み。
