# 作業計画: Issue #63 Semantic Search（意味検索）

## Issue概要

| 項目 | 内容 |
|------|------|
| **Issue番号** | #63 |
| **タイトル** | [Feature] Semantic Search（意味検索） |
| **サイズ** | M |
| **優先度** | High |
| **依存Issue** | #61（マージ済み）、#62（マージ済み） |
| **ブランチ** | `feature/issue-63-semantic-search`（作成済み） |
| **設計方針書** | `dev-reports/design/issue-63-semantic-search-design-policy.md` |

## タスク分解

### Phase 1: 基盤（型定義・エラー型・SymbolStore拡張）

#### Task 1.1: SymbolStore に count_embeddings() 追加
- **ファイル**: `src/indexer/symbol_store.rs`
- **内容**: `pub fn count_embeddings(&self) -> Result<u64, SymbolStoreError>` 新規追加
- **テスト**: 既存テストファイル内に count_embeddings のユニットテスト追加
- **依存**: なし

#### Task 1.2: reader.rs の matches_file_type() を pub(crate) に変更
- **ファイル**: `src/indexer/reader.rs`
- **内容**: `fn matches_file_type` → `pub(crate) fn matches_file_type`
- **テスト**: 既存テストが引き続きパスすることを確認
- **依存**: なし

#### Task 1.3: SearchError 拡張
- **ファイル**: `src/cli/search.rs`
- **内容**:
  - `Embedding(EmbeddingError)` バリアント追加
  - `NoEmbeddings` バリアント追加
  - `Display` 実装（NetworkError → Ollama案内、NoEmbeddings → embed案内）
  - `source()` 実装
  - `From<EmbeddingError>` 実装
- **テスト**: SearchError の Display 出力テスト
- **依存**: なし

#### Task 1.4: SemanticSearchResult 構造体 + format_semantic_results()
- **ファイル**: `src/output/mod.rs`, `src/output/human.rs`, `src/output/json.rs`, `src/output/path.rs`
- **内容**:
  - `SemanticSearchResult` 構造体定義（path, heading, similarity, body, tags, heading_level）
  - `format_semantic_results()` ディスパッチ関数
  - `format_semantic_human()` - `[0.89] path > heading` 形式
  - `format_semantic_json()` - JSONL形式
  - `format_semantic_path()` - パスのみ（重複除去）
- **テスト**: 各フォーマット関数のユニットテスト
- **依存**: なし

### Phase 2: コアロジック（検索関数実装）

#### Task 2.1: enrich_with_metadata() 実装
- **ファイル**: `src/cli/search.rs`
- **内容**:
  - file_pathでグルーピング → search_by_exact_path()バッチ呼び出し
  - section_heading照合（空→first(), 非空→heading完全一致）
  - similarity降順ソート
- **テスト**: section_heading照合のユニットテスト
- **依存**: Task 1.4

#### Task 2.2: apply_semantic_filters() 実装
- **ファイル**: `src/cli/search.rs`
- **内容**:
  - path_prefixフィルタ
  - file_typeフィルタ（`crate::indexer::reader::matches_file_type()` 再利用）
  - tagフィルタ（eq_ignore_ascii_case）
- **テスト**: 各フィルタのユニットテスト
- **依存**: Task 1.2, Task 1.4

#### Task 2.3: run_semantic_search() 実装
- **ファイル**: `src/cli/search.rs`
- **内容**:
  - Config::load() → create_provider()
  - SymbolDbNotFound / IndexNotFound 事前チェック
  - count_embeddings() → NoEmbeddings チェック
  - embed() → first() 安全アクセス
  - search_similar() オーバーサンプリング（limit * 5）
  - enrich_with_metadata() 呼び出し
  - apply_semantic_filters() 呼び出し
  - truncate(limit) + 0件ハンドリング
  - format_semantic_results() 出力
- **テスト**: 統合テスト（SymbolStore + tantivy セットアップ）
- **依存**: Task 1.1, Task 1.3, Task 2.1, Task 2.2

### Phase 3: CLI統合

#### Task 3.1: main.rs に --semantic オプション追加
- **ファイル**: `src/main.rs`
- **内容**:
  - `semantic: Option<String>` フィールド追加
  - `conflicts_with_all = ["query", "symbol", "related", "heading"]`
  - 既存 `--symbol` に `"semantic"` 追加
  - 既存 `--related` に `"semantic"` 追加
  - matchパターン 4変数化
  - Noneケースのエラーメッセージ更新
- **テスト**: CLIパーステスト
- **依存**: Task 2.3

#### Task 3.2: tests/cli_args.rs 更新
- **ファイル**: `tests/cli_args.rs`
- **内容**:
  - `search_requires_query_or_symbol` テスト更新（メッセージに --semantic 含む）
  - 排他テスト: `--semantic` + `--symbol` / `--related` / query / `--heading`
  - 併用テスト: `--semantic` + `--tag` / `--path` / `--type`
- **依存**: Task 3.1

### Phase 4: 統合テスト

#### Task 4.1: tests/semantic_search.rs 新規作成
- **ファイル**: `tests/semantic_search.rs`
- **内容**:
  - テストヘルパー: tantivy + SymbolStore セットアップ
  - 正常系: 類似度ソート確認
  - 異常系: embedding 0件 → NoEmbeddings
  - ポストフィルタテスト
- **依存**: Task 3.1

### Phase 5: 最終検証

#### Task 5.1: 品質チェック
- `cargo build` → エラー0件
- `cargo clippy --all-targets -- -D warnings` → 警告0件
- `cargo test --all` → 全テストパス
- `cargo fmt --all -- --check` → 差分なし

## 実装順序（TDD）

```
Task 1.1 (count_embeddings)     ─┐
Task 1.2 (matches_file_type)    ─┤
Task 1.3 (SearchError拡張)      ─┼─ 並列実装可能
Task 1.4 (SemanticSearchResult) ─┘
         ↓
Task 2.1 (enrich_with_metadata) ─┐
Task 2.2 (apply_semantic_filters)┼─ 並列実装可能
         ↓                       ┘
Task 2.3 (run_semantic_search)
         ↓
Task 3.1 (main.rs CLI統合)
         ↓
Task 3.2 (CLIテスト更新) ─┐
Task 4.1 (統合テスト)     ┼─ 並列実装可能
         ↓               ┘
Task 5.1 (最終検証)
```

## Definition of Done

- [ ] すべてのタスクが完了
- [ ] `cargo test --all` 全パス
- [ ] `cargo clippy --all-targets -- -D warnings` 警告ゼロ
- [ ] `cargo fmt --all -- --check` 差分なし
- [ ] `--semantic` で意味検索が動作
- [ ] human/json/path 出力形式対応
- [ ] 排他制御（--symbol/--related/query/--heading）動作
- [ ] フィルタ（--tag/--path/--type）動作
- [ ] エラーメッセージ（NoEmbeddings, NetworkError）適切
