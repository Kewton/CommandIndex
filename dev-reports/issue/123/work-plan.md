# 作業計画: Issue #123 - --with-snippet が空文字列を返す

## Issue概要
**Issue番号**: #123
**タイトル**: [BUG] --with-snippet が空文字列を返す（related/impact）
**サイズ**: M
**優先度**: High
**依存Issue**: なし

## 作業タスク

### Phase 1: コア実装

#### Task 1.1: IndexReaderWrapper に all_indexed_paths() 追加
- **成果物**: `src/indexer/reader.rs`
- **依存**: なし
- **内容**:
  - `all_indexed_paths() -> Result<HashSet<String>, ReaderError>` メソッド追加
  - tantivy の全ドキュメントから path フィールドを取得し HashSet で返す
- **見積り**: 小

#### Task 1.2: SymbolStore に find_all_imports() 追加
- **成果物**: `src/indexer/symbol_store.rs`
- **依存**: なし
- **内容**:
  - `find_all_imports() -> Result<Vec<ImportInfo>, SymbolStoreError>` メソッド追加
  - `SELECT * FROM dependencies` で全依存関係を取得
- **見積り**: 小

#### Task 1.3: パス解決関数の実装
- **成果物**: `src/search/related.rs`
- **依存**: なし
- **内容**:
  - `resolve_import_path(import_path: &str, indexed_paths: &HashSet<String>) -> Option<String>` 追加
  - `path_component_suffix_matches(indexed_path: &str, import_suffix: &str) -> bool` 追加
  - 入力バリデーション（空文字、長さ1024上限）
  - コンポーネント境界チェック付きサフィックスマッチ
  - エイリアス除去（`@/`, `~/`, `./`, `../`）
  - 拡張子補完（.ts, .tsx, .js, .jsx）
  - index.ts パターン対応
- **見積り**: 中

#### Task 1.4: add_relation 共通ヘルパー追加
- **成果物**: `src/search/related.rs`
- **依存**: なし
- **内容**:
  - スコア加算パターンを共通関数化
  - 既存の score_markdown_links, score_import_deps, score_tag_match, score_path_proximity から呼び出し
- **見積り**: 小

#### Task 1.5: RelatedSearchEngine 改修
- **成果物**: `src/search/related.rs`
- **依存**: Task 1.1, 1.2, 1.3, 1.4
- **内容**:
  - `indexed_paths: OnceCell<HashSet<String>>` フィールド追加（`std::cell::OnceCell`、外部クレート不要）
  - `get_indexed_paths()` 遅延初期化メソッド追加
  - `score_import_deps()` を修正:
    - 順方向: resolve_import_path でインポートパス→ファイルパス変換後にスコア加算
    - 逆方向: find_all_imports + resolve_cache で全依存を解決し、targetに一致するものをフィルタ
    - 外部パッケージ（解決失敗）はスキップ
  - 他のscore_*メソッドを add_relation 使用に更新
- **見積り**: 中

### Phase 2: テスト

#### Task 2.1: ユニットテスト追加
- **成果物**: `src/search/related.rs` (#[cfg(test)] モジュール)
- **依存**: Task 1.3
- **テストケース**:
  - `resolve_import_path`:
    - 完全一致パス → Some
    - 相対パス `./utils` → `src/utils.ts`
    - エイリアス `@/components/Button` → `src/components/Button.tsx`
    - 外部パッケージ `react` → None
    - index.tsパターン `@/components/Foo` → `src/components/Foo/index.ts`
    - 空文字列 → None
    - 長すぎる文字列 → None
  - `path_component_suffix_matches`:
    - `auth` は `src/auth.ts` にマッチ
    - `auth` は `src/oauth.ts` にマッチしない（境界チェック）
    - 拡張子補完（.ts, .tsx, .js, .jsx）
- **見積り**: 中

#### Task 2.2: E2Eテスト追加・更新
- **成果物**: `tests/e2e_related_search.rs`, `tests/e2e_impact.rs`
- **依存**: Task 1.5
- **テストケース**:
  - import依存関係 + `--with-snippet` でsnippet非空
  - 結果の file_path が実ファイルパス（完全一致検証）
  - 既存テスト `related_import_dependency_detects_ts_imports` のパス検証強化
  - impact + `--with-snippet` テスト
- **見積り**: 中

### Phase 3: 品質チェック

#### Task 3.1: 全体品質チェック
- **依存**: Task 2.1, 2.2
- **内容**:
  - `cargo build` → エラー0件
  - `cargo clippy --all-targets -- -D warnings` → 警告0件
  - `cargo test --all` → 全テストパス
  - `cargo fmt --all -- --check` → 差分なし

## タスク依存関係

```
Task 1.1 (reader.rs) ──┐
Task 1.2 (symbol_store) ──┤
Task 1.3 (パス解決関数) ──┼── Task 1.5 (RelatedSearchEngine改修) ── Task 2.2 (E2E)
Task 1.4 (add_relation) ──┘                                          ↓
                                                                   Task 3.1 (品質チェック)
Task 1.3 ── Task 2.1 (ユニットテスト) ── Task 3.1
```

## 実行順序

1. Task 1.1 + 1.2 + 1.3 + 1.4（並列実行可能）
2. Task 1.5（上記の統合）
3. Task 2.1 + 2.2（並列実行可能）
4. Task 3.1（最終品質チェック）

## Definition of Done

- [ ] すべてのタスクが完了
- [ ] `cargo test --all` 全パス
- [ ] `cargo clippy --all-targets -- -D warnings` 警告0件
- [ ] `cargo fmt --all -- --check` 差分なし
- [ ] import依存のrelated結果にfile_pathが実ファイルパスで返る
- [ ] `--with-snippet` で相対パスimportのsnippetが非空
- [ ] 外部パッケージが結果に含まれない

## 次のアクション

1. ブランチ: `fix/issue-123-snippet-empty`（現在のブランチ）
2. TDD実装: `/pm-auto-dev 123` で自動開発
3. PR作成: `/create-pr` で自動作成
