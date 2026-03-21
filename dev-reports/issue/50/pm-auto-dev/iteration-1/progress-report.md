# 進捗レポート: Issue #50 --related 検索オプション実装

## 実施日: 2026-03-21

## ステータス: 実装完了

---

## 品質チェック結果

| チェック | 結果 |
|---------|------|
| cargo build | OK |
| cargo clippy --all-targets -- -D warnings | 0 warnings |
| cargo test --all | 284 tests passed, 0 failed |
| cargo fmt --all -- --check | No diff |

---

## 変更ファイル一覧

### 新規ファイル (3)
- `src/search/mod.rs` - searchモジュール宣言
- `src/search/related.rs` - RelatedSearchEngine スコアリングロジック
- `tests/e2e_related_search.rs` - E2E関連検索テスト (10テスト)

### 変更ファイル (9)
- `src/lib.rs` - `pub mod search` 追加
- `src/main.rs` - `--related` CLIオプション追加、分岐ロジック
- `src/cli/search.rs` - `run_related_search()`, `SearchError::RelatedSearch` 追加
- `src/indexer/symbol_store.rs` - `find_imports_by_source()`, `find_file_links_by_target()` 追加
- `src/indexer/reader.rs` - `search_by_exact_path()` 追加
- `src/output/mod.rs` - `RelatedSearchResult`, `RelationType`, `format_related_results()` 追加
- `src/output/human.rs` - `format_related_human()` 追加
- `src/output/json.rs` - `format_related_json()` 追加
- `src/output/path.rs` - `format_related_path()` 追加

---

## テスト追加

| テストファイル | 追加テスト数 | 内容 |
|--------------|-------------|------|
| src/indexer/symbol_store.rs | 4 | find_imports_by_source, find_file_links_by_target のユニットテスト |
| src/search/related.rs | 7 | normalize_path, path proximity のユニットテスト |
| tests/e2e_related_search.rs | 10 | E2E関連検索テスト（リンク検出、フォーマット、排他制御、エラー） |

---

## 受け入れ基準の充足状況

| 基準 | 状況 |
|------|------|
| `--related` で指定ファイルの関連ドキュメントが検索できる | OK |
| Markdownリンクによる関連が検出される | OK |
| タグ一致による関連が検出される | OK |
| パス近接性による関連が検出される | OK |
| import依存関係による関連が検出される（双方向） | OK |
| 関連度スコアでソートされる | OK |
| human / json / path 出力形式に対応 | OK |
| `--related` と query / `--symbol` を同時指定した場合にエラー | OK |
| 関連ファイルが見つからない場合に適切なメッセージ | OK |
| `--related` に存在しないファイルを指定した場合にエラー | OK |
| cargo test / clippy / fmt 全パス | OK |

---

## 実装アーキテクチャ

```
CLI (main.rs --related)
  → cli/search.rs::run_related_search()
    → search/related.rs::RelatedSearchEngine
      ├── score_markdown_links() → symbol_store.find_file_links_by_source/target()
      ├── score_import_deps() → symbol_store.find_imports_by_source/target()
      ├── score_tag_match() → reader.search_by_exact_path() + reader.search()
      └── score_path_proximity() → path segment analysis
    → output/mod.rs::format_related_results()
```

### スコアリング重み定数
| 判定基準 | 重み |
|---------|------|
| Markdownリンク | 1.0 |
| import依存関係 | 0.9 |
| タグ一致 | 0.5 × 一致数 |
| パスセグメント類似 | 0.4 |
| 同一ディレクトリ | 0.2 |
| 親ディレクトリ共通 | 0.1 |
