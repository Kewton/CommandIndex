# 仮説検証レポート - Issue #116

## Issue概要
search/related/impact のJSON出力のLLMプロンプト最適化

## 仮説検証結果

### 仮説1: 「searchのJSON出力にはLLMにとって不要な情報が大量に含まれる」
**判定: Confirmed**

JSON出力に含まれるLLM不要フィールド:
- `score` (f32) — 関連度スコア
- `heading_level` (u64) — マークダウンレベル
- `line_start` (u64) — 行番号
- related/impact の `relations` 構造体、`impacted_by` 配列など

### 仮説2: 「`--format llm` の実装が必要」
**判定: Confirmed（既に実装済み）**

Issue #104 で `OutputFormat::Llm` は既に実装完了:
- `src/output/mod.rs:34` に `Llm` variant
- `src/output/llm.rs` に format_llm, format_related_llm, format_impact_llm 実装済み
- search/related/impact/symbol/diff/semantic/workspace/suggest 全コマンドで `--format llm` 利用可能

### 仮説3: 「出力フォーマット処理の所在」
**判定: Confirmed**

`src/output/` に集約:
- `mod.rs`: format_results, format_related_results, format_impact_results でディスパッチ
- `llm.rs`: LLM向けMarkdown形式出力（スコア・メタデータ除去）
- `json.rs`: JSON形式出力（全フィールド含む）

## 結論

`--format llm` は既に実装済み。Issue #116 の本質は、この実装の**最適化**（出力サイズのさらなる削減、LLMが消費しやすい形式への改善）にある。
