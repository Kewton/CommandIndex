# 作業計画: Issue #171 - contextコマンドのナレッジグラフ統合改善

## Issue: contextコマンドにナレッジグラフのエッジを統合する
**Issue番号**: #171
**サイズ**: M
**優先度**: Medium
**依存Issue**: なし

## 影響箇所の全量（grep結果）

`RelationType::KnowledgeGraph` の参照箇所（全8箇所）:

| ファイル | 行 | 用途 |
|---------|-----|------|
| `src/output/mod.rs:129` | enum定義 | 変更対象 |
| `src/search/related.rs:470` | score_knowledge_graph() | 変更対象 |
| `src/cli/context.rs:285` | enrich_entry() matches! | 変更対象 |
| `src/cli/context.rs:389` | relation_to_string() matches! | 変更対象 |
| `src/output/human.rs:127` | human出力フォーマット | パターンマッチ更新 |
| `src/output/llm.rs:350` | LLM出力フォーマット | パターンマッチ更新 |
| `src/output/json.rs:96` | JSON出力フォーマット | パターンマッチ更新 |
| `src/cli/impact.rs:292` | impact出力フォーマット | パターンマッチ更新 |

**注**: suggest.rsは`query_knowledge_graph`（独自関数）を使用しており、RelationType::KnowledgeGraphは参照していない。why.rsも参照なし。

---

## Phase 1: 型定義・基盤変更

### Task 1.1: KnowledgeGraphMeta 構造体と RelationType enum の変更
- **ファイル**: `src/output/mod.rs`
- **変更内容**:
  - `KnowledgeGraphMeta` 構造体を新設（Default derive付き）
  - `RelationType::KnowledgeGraph` を `KnowledgeGraph(KnowledgeGraphMeta)` に変更
  - `is_knowledge_graph()` と `kg_meta()` ヘルパーメソッドを追加
- **依存**: なし
- **テスト**: ヘルパーメソッドのユニットテスト

### Task 1.2: パターンマッチ更新（コンパイル通過）
- **ファイル**: 以下の6ファイル
  - `src/search/related.rs:470` → `KnowledgeGraph(KnowledgeGraphMeta { ... })`
  - `src/cli/context.rs:285` → `r.is_knowledge_graph()`
  - `src/cli/context.rs:389` → `rt.is_knowledge_graph()`
  - `src/output/human.rs:127` → `KnowledgeGraph(_)`
  - `src/output/llm.rs:350` → `KnowledgeGraph(_)`
  - `src/output/json.rs:96` → `KnowledgeGraph(_)`
  - `src/cli/impact.rs:292` → `KnowledgeGraph(_)`
- **依存**: Task 1.1
- **検証**: `cargo build` 成功

## Phase 2: 機能改善

### Task 2.1: KNOWLEDGE_GRAPH_WEIGHT の調整
- **ファイル**: `src/search/related.rs:16`
- **変更内容**: `0.8` → `0.95`
- **依存**: Task 1.2
- **テスト**: 重み値のアサーション

### Task 2.2: score_knowledge_graph() にメタデータ付加
- **ファイル**: `src/search/related.rs:456-474`
- **変更内容**: `KnowledgeRelatedResult` の情報を `KnowledgeGraphMeta` に変換して付加
- **依存**: Task 1.2
- **テスト**: score_knowledge_graph() のメタデータ付加検証

### Task 2.3: relation_to_string() の優先度変更
- **ファイル**: `src/cli/context.rs:361-393`
- **変更内容**: KnowledgeGraph を 6番目から3番目（ImportDependencyの次）に移動
- **依存**: Task 1.2
- **テスト**: 優先度の検証テスト

### Task 2.4: enrich_entry() のスニペット改善
- **ファイル**: `src/cli/context.rs:264-358`
- **変更内容**:
  - `kg_meta()` で doc_subtype を取得
  - doc_subtype に応じたセクション抽出（design_policy: 設計判断、work_plan: 作業項目）
  - フォールバック: 既存の truncate_body()
  - 500文字上限を維持
- **依存**: Task 2.2
- **テスト**: doc_subtypeベースのスニペット生成検証

## Phase 3: テスト

### Task 3.1: ユニットテスト追加
- **ファイル**: 各ソースファイル内の `#[cfg(test)]` モジュール
- **テスト項目**:
  - `is_knowledge_graph()` / `kg_meta()` の動作検証
  - `relation_to_string()` の新優先度検証
  - `KnowledgeGraphMeta` 全フィールド None 時の後方互換
- **依存**: Phase 2 完了

### Task 3.2: 既存テストの通過確認
- **コマンド**: `cargo test --all`
- **依存**: Phase 2 完了

## 品質チェック項目

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |

## Definition of Done

- [ ] KnowledgeGraphMeta 構造体が導入され、RelationType enum が更新されている
- [ ] 全8箇所のパターンマッチが更新され、コンパイルが通る
- [ ] KNOWLEDGE_GRAPH_WEIGHT が 0.95 に変更されている
- [ ] score_knowledge_graph() がメタデータを付加している
- [ ] relation_to_string() で KnowledgeGraph が3番目の優先度
- [ ] enrich_entry() で doc_subtype ベースのスニペット抽出が動作する
- [ ] 全テストパス、clippy警告0件
