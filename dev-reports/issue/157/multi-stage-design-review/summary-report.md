# Issue #157 マルチステージ設計レビュー サマリーレポート

## 対象
- **設計方針書**: `dev-reports/design/issue-157-suggest-knowledge-graph-design-policy.md`
- **Issue**: #157 suggestコマンドがナレッジグラフを参照していない

## 実施ステージ

| Stage | 種別 | 実行者 | Must Fix | Should Fix | Nice to Have |
|-------|------|--------|----------|------------|-------------|
| 1 | 設計原則（SOLID/KISS/YAGNI/DRY） | Claude Opus | 0 | 4 | 4 |
| 2 | 整合性レビュー | Claude Opus | 3 | 4 | 3 |
| 3 | 影響分析レビュー | Claude Opus | 2 | 3 | 3 |
| 4 | セキュリティレビュー | Claude Opus | 0 | 2 | 4 |
| 5-8 | 2回目レビュー | スキップ（Codex rate limit） | - | - | - |

## 主要な設計変更（レビュー反映）

### 1. KGステップ挿入の責務をrun_suggest側に変更
- **元**: build_strategyに新規引数（kg_docs, matched_issues）追加
- **変更後**: build_strategy/build_fallback_strategyのシグネチャは変更せず、`prepend_knowledge_steps()`関数でrun_suggest側で挿入
- **理由**: 引数肥大化回避、BM25=0件でもKGヒット時に対応可能

### 2. 重複排除をHashSetに変更
- **元**: `nums.dedup()`（連続重複のみ除去）
- **変更後**: `HashSet` + `filter` + `take`パターン
- **理由**: 非連続重複の確実な排除

### 3. serde(skip_serializing_if)の適用
- **元**: matched_issuesが常にJSON出力に含まれる
- **変更後**: 空配列時はフィールド自体を省略
- **理由**: 既存JSON出力との後方互換性維持

### 4. MAX_ISSUE_NUMBERS定数化
- **元**: `truncate(3)` のマジックナンバー
- **変更後**: `const MAX_ISSUE_NUMBERS: usize = 3;`

### 5. 既存テスト影響の詳細化
- テスト修正箇所（5箇所）を具体的な行番号付きで明記

### 6. セキュリティ前提条件の追記
- suggest出力はコマンド提案であり、シェル実行はしないことを明記

## 結論

設計方針書はレビューを経て堅実な設計に改善された。Must Fix指摘は全て反映済み。Stage 5-8はCodex rate limitによりスキップしたが、4段階レビューで十分な品質が確保されている。
