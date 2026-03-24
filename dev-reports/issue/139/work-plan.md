# 作業計画: Issue #139 SQLiteベースの簡易ナレッジグラフの実装

## Issue概要

**Issue番号**: #139
**タイトル**: SQLiteベースの簡易ナレッジグラフの実装
**サイズ**: L
**優先度**: High
**依存Issue**: なし（#134, #135 は完了済み）
**ブランチ**: `feature/issue-139-knowledge-graph`（作成済み）

## 詳細タスク分解

### Phase 1: データモデル・型定義

#### Task 1.1: ナレッジグラフ型定義と正規表現パーサー
- **成果物**: `src/indexer/knowledge.rs`（新規）
- **依存**: なし
- **内容**:
  - `KnowledgeNodeType`, `KnowledgeRelation`, `DocSubtype` enum 定義
  - `KnowledgeEntry`, `KnowledgeRelatedResult` 構造体定義
  - `KnowledgeError` エラー型定義（Io, Store, PathValidation）
  - `PatternRule` 構造体と `build_pattern_rules()` 関数
  - `parse_dev_report_path()` パーサー実装
  - `validate_path()` パストラバーサル対策
  - **テスト**: `parse_dev_report_path` の各パターン正常系・異常系

#### Task 1.2: SymbolStore スキーマ拡張
- **成果物**: `src/indexer/symbol_store.rs`（変更）
- **依存**: なし
- **内容**:
  - `CURRENT_SYMBOL_SCHEMA_VERSION` を 3 → 4 にインクリメント
  - `create_tables()` に `knowledge_nodes` / `knowledge_edges` テーブル追加
  - `test_schema_version_v3` → `test_schema_version_v4` テスト更新
  - **テスト**: テーブル作成確認、スキーマバージョンチェック

#### Task 1.3: SymbolStore CRUD メソッド追加
- **成果物**: `src/indexer/symbol_store.rs`（変更、分離implブロック）
- **依存**: Task 1.2
- **内容**:
  - `upsert_knowledge_node()` ノード挿入/更新
  - `upsert_knowledge_edge()` エッジ挿入/更新
  - `insert_knowledge_entries()` バッチ挿入（トランザクション）
  - `clear_knowledge_graph()` 全削除
  - `delete_knowledge_by_file()` ファイルベース削除
  - `find_knowledge_related()` 関連ドキュメント取得
  - **テスト**: 各CRUDの正常系、ON DELETE CASCADE 動作確認

### Phase 2: ナレッジグラフ構築ロジック

#### Task 2.1: dev-reports 走査・構築ロジック
- **成果物**: `src/indexer/knowledge.rs`（追加）
- **依存**: Task 1.1, Task 1.3
- **内容**:
  - `scan_dev_reports()` ディレクトリ走査 + パターンマッチング
  - `detect_dev_reports_changes()` git diff ベースの差分検出
  - `build_knowledge_graph()` KnowledgeEntry → SymbolStore 投入
  - **テスト**: テンポラリディレクトリでの走査テスト、差分検出テスト

#### Task 2.2: delete_by_file() 拡張
- **成果物**: `src/indexer/symbol_store.rs`（変更）
- **依存**: Task 1.3
- **内容**:
  - `delete_by_file()` に knowledge_nodes DELETE 追加
  - **テスト**: ファイル削除時にノード+エッジが連鎖削除されること

### Phase 3: CLI 統合

#### Task 3.1: index コマンドへの KG 構築統合
- **成果物**: `src/cli/index.rs`（変更）
- **依存**: Task 2.1
- **内容**:
  - `run()` に KG フル構築追加（writer.commit() 直後）
  - `clear_knowledge_graph()` → `scan_dev_reports()` → `insert_knowledge_entries()`
  - **テスト**: index 実行後に knowledge テーブルにデータが存在すること

#### Task 3.2: update コマンドへの KG 差分更新統合
- **成果物**: `src/cli/index.rs`（変更）
- **依存**: Task 2.1, Task 3.1
- **内容**:
  - `run_incremental()` に KG 差分更新追加（writer.commit() 直後）
  - `detect_dev_reports_changes()` → `delete_knowledge_by_file()` → `insert_knowledge_entries()`
  - **テスト**: dev-reports 変更後の update でKGが更新されること

### Phase 4: search --related 統合

#### Task 4.1: RelationType 拡張
- **成果物**: `src/output/mod.rs`, `human.rs`, `json.rs`, `llm.rs`（変更）
- **依存**: なし
- **内容**:
  - `RelationType::KnowledgeGraph` バリアント追加
  - 全出力フォーマッタの match アーム追加
  - **テスト**: output_format テストに KnowledgeGraph の表示テスト追加

#### Task 4.2: impact.rs / context.rs の RelationType 対応
- **成果物**: `src/cli/impact.rs`, `src/cli/context.rs`（変更）
- **依存**: Task 4.1
- **内容**:
  - `impact.rs`: `relation_type_to_string()` に match アーム追加
  - `context.rs`: `relation_to_string()` に if matches! 追加、`enrich_entry()` に KnowledgeGraph 対応
  - **テスト**: 既存 e2e_impact, e2e_context_pack テストの期待値確認・調整

#### Task 4.3: RelatedSearchEngine への KG スコアリング追加
- **成果物**: `src/search/related.rs`（変更）
- **依存**: Task 1.3, Task 4.1
- **内容**:
  - `KNOWLEDGE_GRAPH_WEIGHT = 0.8` 定数追加
  - `score_knowledge_graph()` メソッド実装（Result伝搬）
  - `find_related()` に `score_knowledge_graph()` 呼び出し追加
  - **テスト**: KG エッジがスコアリングに反映されることの確認

### Phase 5: モジュール登録・最終統合

#### Task 5.1: モジュール登録
- **成果物**: `src/indexer/mod.rs`（変更）
- **依存**: Task 2.1
- **内容**:
  - `pub mod knowledge;` 追加

#### Task 5.2: 既存テスト修正・E2Eテスト追加
- **成果物**: `tests/`（変更・追加）
- **依存**: Task 4.3
- **内容**:
  - 既存テストの期待値調整（e2e_related_search, e2e_impact 等）
  - KG 統合テスト追加（index → search --related でKGベース関連が返ること）

#### Task 5.3: 品質チェック・最終確認
- **成果物**: なし
- **依存**: 全タスク
- **内容**:
  - `cargo build` エラー0件
  - `cargo clippy --all-targets -- -D warnings` 警告0件
  - `cargo test --all` 全パス
  - `cargo fmt --all -- --check` 差分なし

## タスク依存関係図

```
Task 1.1 (型定義) ──┐
                     ├── Task 2.1 (走査ロジック) ─── Task 3.1 (index統合) ─── Task 3.2 (update統合)
Task 1.2 (スキーマ) ─┤                                    │
                     │                                     │
Task 1.3 (CRUD) ─────┤                                    │
                     │                                     ▼
Task 2.2 (delete拡張)┘                              Task 5.2 (E2Eテスト)
                                                           │
Task 4.1 (RelationType) ─── Task 4.2 (impact/context) ───┤
                         └── Task 4.3 (KGスコアリング) ────┤
                                                           ▼
Task 5.1 (mod登録)                                   Task 5.3 (品質チェック)
```

## 推奨実装順序

1. **Task 1.1** + **Task 1.2** + **Task 4.1**（並行可能、依存なし）
2. **Task 1.3** + **Task 4.2**（1の完了後）
3. **Task 2.1** + **Task 2.2** + **Task 4.3** + **Task 5.1**
4. **Task 3.1** → **Task 3.2**
5. **Task 5.2** → **Task 5.3**

## 品質チェック項目

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |

## Definition of Done

- [ ] `CURRENT_SYMBOL_SCHEMA_VERSION` = 4
- [ ] `IndexState::CURRENT_SCHEMA_VERSION` = 1（変更なし）
- [ ] `knowledge_nodes` / `knowledge_edges` が index 時に構築される
- [ ] `update` で dev-reports/ 変更の差分更新が動作する
- [ ] `search --related` に `RelationType::KnowledgeGraph` が反映される
- [ ] 全出力フォーマッタ（human/json/llm）で KnowledgeGraph が表示される
- [ ] `impact.rs` / `context.rs` の RelationType 対応完了
- [ ] `delete_by_file()` がナレッジノード/エッジを削除する
- [ ] `cargo test --all` 全パス
- [ ] `cargo clippy --all-targets -- -D warnings` 警告0件
- [ ] `cargo fmt --all -- --check` 差分なし

## 作成日: 2026-03-24
