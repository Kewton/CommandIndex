# 作業計画: Issue #157 suggestコマンドへのナレッジグラフ参照統合

## Issue概要
- **Issue番号**: #157
- **タイトル**: suggestコマンドがナレッジグラフを参照していない
- **サイズ**: S（変更はsuggest.rsとoutput/mod.rsに閉じる）
- **優先度**: Medium
- **依存Issue**: なし

## 作業ブランチ
`fix/issue-157-suggest-kg`（既に作成済み）

## 詳細タスク分解

### Phase 1: 型定義・データモデル変更

#### Task 1.1: SuggestResult に matched_issues フィールド追加
- **成果物**: `src/output/mod.rs`
- **依存**: なし
- **作業内容**:
  - `SuggestResult` 構造体に `matched_issues: Vec<String>` フィールド追加
  - `#[serde(skip_serializing_if = "Vec::is_empty")]` アトリビュート付与
- **テスト**: JSON出力テストで空配列時にフィールド省略を確認

#### Task 1.2: 既存コードの SuggestResult 構築箇所を修正
- **成果物**: `src/cli/suggest.rs`
- **依存**: Task 1.1
- **作業内容**:
  - `build_strategy` 戻り値（行156）に `matched_issues: Vec::new()` 追加
  - `build_fallback_strategy` 戻り値（行178）に `matched_issues: Vec::new()` 追加
- **テスト**: `cargo build` + `cargo test` で既存テスト全パス確認

#### Task 1.3: 既存テストの SuggestResult 構築箇所を修正
- **成果物**: `src/cli/suggest.rs` (テスト部分)
- **依存**: Task 1.1
- **作業内容**:
  - `format_human_output` テスト（行438）に `matched_issues: vec![]` 追加
  - `format_json_output` テスト（行462）に `matched_issues: vec![]` 追加
  - `format_path_output` テスト（行485）に `matched_issues: vec![]` 追加

### Phase 2: コアロジック実装

#### Task 2.1: 定数・import 追加
- **成果物**: `src/cli/suggest.rs`
- **依存**: Phase 1 完了
- **作業内容**:
  - `use crate::indexer::knowledge::extract_issue_numbers;` 追加
  - `use crate::indexer::symbol_store::{SymbolStore, KnowledgeDocResult};` 追加
  - `const MAX_ISSUE_NUMBERS: usize = 3;` 追加

#### Task 2.2: query_knowledge_graph 関数実装
- **成果物**: `src/cli/suggest.rs`
- **依存**: Task 2.1
- **作業内容**:
  - 設計方針書 5.2 に従い `query_knowledge_graph()` 関数を実装
  - symbols.db 非存在時は空Vec返却
  - SymbolStore::open 失敗時は `[suggest]` プレフィックス付きwarning出力 + 空Vec返却
  - find_knowledge_by_issue 失敗時も同様

#### Task 2.3: prepend_knowledge_steps 関数実装
- **成果物**: `src/cli/suggest.rs`
- **依存**: Task 2.1
- **作業内容**:
  - 設計方針書 5.3 に従い `prepend_knowledge_steps()` 関数を実装
  - matched_issues のIssue番号ごとに `issue NNN --format json` ステップ生成
  - kg_docs の各文書に `context -- 'file_path' --max-files 5` ステップ生成
  - 既存戦略ステップの前に挿入

#### Task 2.4: run_suggest 関数の拡張
- **成果物**: `src/cli/suggest.rs`
- **依存**: Task 2.2, 2.3
- **作業内容**:
  - EmbeddingStore オープン後に Issue番号抽出ロジック追加（HashSet重複排除 + MAX_ISSUE_NUMBERS制限）
  - `query_knowledge_graph()` 呼び出し追加
  - 戦略生成後に `prepend_knowledge_steps()` 呼び出し追加
  - `result.matched_issues` に抽出したIssue番号を設定

### Phase 3: テスト実装

#### Task 3.1: prepend_knowledge_steps のユニットテスト
- **成果物**: `src/cli/suggest.rs` (テスト部分)
- **依存**: Task 2.3
- **テストケース**:
  - `test_prepend_knowledge_steps_with_docs`: KG結果あり → 戦略先頭にissue/contextステップ挿入
  - `test_prepend_knowledge_steps_empty`: KG結果空 → 戦略変更なし
  - `test_prepend_knowledge_steps_multiple_issues`: 複数Issue → 各Issueのステップ生成

#### Task 3.2: Issue番号抽出のユニットテスト
- **成果物**: `src/cli/suggest.rs` (テスト部分)
- **依存**: Task 2.4
- **テストケース**:
  - `test_issue_number_dedup`: 重複Issue番号の排除確認
  - `test_issue_number_max_limit`: MAX_ISSUE_NUMBERS超過時のtruncate確認

#### Task 3.3: matched_issues のJSON出力テスト
- **成果物**: `src/cli/suggest.rs` (テスト部分)
- **依存**: Task 1.1
- **テストケース**:
  - matched_issuesが空の場合、JSONに`matched_issues`キーが含まれないこと
  - matched_issuesに値がある場合、JSONに正しく出力されること

### Phase 4: 品質チェック

#### Task 4.1: 全品質チェック実行
- **依存**: Phase 3 完了
- **作業内容**:
  - `cargo build` → エラー0件
  - `cargo clippy --all-targets -- -D warnings` → 警告0件
  - `cargo test --all` → 全テストパス
  - `cargo fmt --all -- --check` → 差分なし

## 品質チェック項目

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |

## Definition of Done

- [x] Phase 1: SuggestResult フィールド追加 + 既存コード修正
- [x] Phase 2: コアロジック実装（query_knowledge_graph, prepend_knowledge_steps, run_suggest拡張）
- [x] Phase 3: ユニットテスト実装（新規5件 + 既存修正3件）
- [x] Phase 4: 全品質チェックパス
- [ ] PR作成・レビュー

## 実装順序サマリー

```
Task 1.1 (SuggestResult変更)
  ├── Task 1.2 (既存コード修正)
  └── Task 1.3 (既存テスト修正)
        └── Task 2.1 (import/定数追加)
              ├── Task 2.2 (query_knowledge_graph)
              ├── Task 2.3 (prepend_knowledge_steps)
              └── Task 2.4 (run_suggest拡張)
                    ├── Task 3.1 (prepend_knowledge_stepsテスト)
                    ├── Task 3.2 (Issue番号抽出テスト)
                    └── Task 3.3 (matched_issues出力テスト)
                          └── Task 4.1 (品質チェック)
```

## 見積もり

- Phase 1: 型定義変更 — 軽微
- Phase 2: コアロジック — 中程度（設計方針書にコード例あり）
- Phase 3: テスト — 軽微（純粋関数のユニットテスト中心）
- Phase 4: 品質チェック — 軽微
