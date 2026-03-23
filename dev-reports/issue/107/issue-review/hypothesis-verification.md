# 仮説検証レポート - Issue #107

## 検証対象Issue
search結果のデフォルトlimit引き下げ (LLM用途向け)

## 仮説一覧と検証結果

### 仮説1: 現在のデフォルトは20件
- **判定**: ✅ Confirmed
- **根拠**: `src/config/mod.rs:417` で `.unwrap_or(20)` 、`src/main.rs:364` でも `limit.unwrap_or(20).min(1000)` と設定

### 仮説2: `--format llm` 時にデフォルトを5件に自動設定
- **判定**: ⚠️ Partially Confirmed (修正が必要)
- **根拠**: `--format llm` は**存在しない**。現在の `OutputFormat` は `Human`, `Json`, `Path` の3種類のみ (`src/output/mod.rs:31-36`)
- **影響**: Issue の提案内容を修正する必要がある。`--format llm` を新設するか、別のアプローチが必要

### 仮説3: 明示的に `--limit` を指定した場合はそちらを優先
- **判定**: ✅ Confirmed
- **根拠**: `src/main.rs:357-368` で `limit.unwrap_or(config_default)` の形式。CLIで指定された場合は `Some(value)` となり config デフォルトより優先される

### 仮説4: グローバル設定で `llm_default_limit` を追加可能
- **判定**: ✅ Feasible
- **根拠**: `src/config/mod.rs` の `SearchConfig` に `default_limit: usize` フィールドが存在。同様に `llm_default_limit` を追加するのは構造的に可能

## 検証で判明した追加事項

1. **limit の上限制御**: `.min(1000)` で最大1000件に制限されている
2. **LLM関連機能**: `help-llm` コマンドと `--rerank` オプション（LLMベースリランキング）が既存
3. **config優先度**: ローカル > チーム > レガシー の3段階マージ
4. **OutputFormat**: `--format` で `llm` を指定する場合は列挙体に追加が必要

## 推奨事項

Issue の提案「`--format llm` 時はデフォルトを5件に自動設定」について:
- `OutputFormat::Llm` を新設し、LLM向けの出力フォーマットを定義するか
- 既存の `--format json` に対して config で `llm_default_limit` を追加し、LLM用途向けのデフォルトを分離するか
- いずれの方式でも `--limit` 明示指定時はそちらを優先する仕組みは既に存在
