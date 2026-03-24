# 作業計画: Issue #127 - suggest のキーワード部分一致によるスコアリング改善

## Issue概要

| 項目 | 内容 |
|------|------|
| Issue番号 | #127 |
| タイトル | [BUG] suggest の英語入力でキーワード部分一致により無関係ファイルを推薦 |
| サイズ | S |
| 優先度 | Medium |
| 依存Issue | なし |
| ブランチ | `fix/issue-127-suggest-keyword`（作成済み） |

## 設計方針書

`dev-reports/design/issue-127-suggest-keyword-design-policy.md`

## タスク分解

### Phase 1: コアロジック実装（TDD）

#### Task 1.1: 定数追加
- **成果物**: `src/cli/suggest.rs` に `TEST_FILE_WEIGHT`, `DOC_FILE_WEIGHT` 定数を追加
- **依存**: なし
- **工数**: 極小

#### Task 1.2: is_test_file() 実装
- **成果物**: `src/cli/suggest.rs` に `is_test_file()` 関数を追加
- **依存**: なし
- **テストファースト**:
  - `is_test_file_detects_separator_patterns` — `_test.`, `.test.`, `_spec.`, `.spec.` パターン
  - `is_test_file_detects_test_prefix` — `test_helper.rs` 等
  - `is_test_file_detects_tests_directory` — `tests/`, `__tests__/`
  - `is_test_file_ignores_non_test_files` — `contest.rs`, `latest.rs`, `src/auth.rs`
  - `is_test_file_empty_path` — 空パス

#### Task 1.3: is_doc_file() 実装
- **成果物**: `src/cli/suggest.rs` に `is_doc_file()` 関数を追加
- **依存**: なし
- **テストファースト**:
  - `is_doc_file_detects_dev_reports` — `dev-reports/*`
  - `is_doc_file_detects_docs_directory` — `docs/*.md`
  - `is_doc_file_detects_root_docs` — `README.md`, `CHANGELOG.md`
  - `is_doc_file_ignores_source_markdown` — `src/notes.md`
  - `is_doc_file_ignores_source_files` — `src/main.rs`

#### Task 1.4: file_type_weight_factor() 実装
- **成果物**: `src/cli/suggest.rs` に `file_type_weight_factor()` 関数を追加
- **依存**: Task 1.1, 1.2, 1.3
- **テストファースト**:
  - `file_type_weight_factor_values` — テスト→0.3, ドキュメント→0.5, ソース→1.0

#### Task 1.5: apply_file_type_weight() 実装
- **成果物**: `src/cli/suggest.rs` に `apply_file_type_weight()` 関数を追加
- **依存**: Task 1.4
- **テストファースト**:
  - `apply_file_type_weight_reorders` — テストファイル(2.0)よりソースファイル(1.5)が上位
  - `apply_file_type_weight_truncates` — limit=2で3件→2件
  - `apply_file_type_weight_empty_input` — 空入力→空出力

#### Task 1.6: search_entry_files() の変更
- **成果物**: `src/cli/suggest.rs` の `search_entry_files()` を修正
- **依存**: Task 1.5
- **変更内容**:
  - `deduplicate_by_file(results, DEDUP_FILE_LIMIT)` → `deduplicate_by_file(results, BM25_SEARCH_LIMIT)`
  - `Ok(deduped)` → `Ok(apply_file_type_weight(deduped, DEDUP_FILE_LIMIT))`

### Phase 2: 品質検証

#### Task 2.1: 既存テスト確認
- `cargo test --all` で全テストパス確認
- 特に `e2e_suggest_*` テストが安定していることを確認
  - テスト用リポジトリは `docs/a.md`, `docs/b.md` のみ → `is_doc_file()` で true になるが、e2eテストはスコア順序を検証しないため影響なし

#### Task 2.2: Clippy・フォーマット
- `cargo clippy --all-targets -- -D warnings` で警告0件
- `cargo fmt --all -- --check` で差分なし

## 品質チェック項目

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |

## Definition of Done

- [x] 設計方針書に基づく実装完了
- [ ] 新規ユニットテスト14件追加・全パス
- [ ] 既存テスト全パス（e2e含む）
- [ ] cargo clippy 警告0件
- [ ] cargo fmt 差分なし
- [ ] 受け入れ基準（Issue記載の6項目）を満たす

## 実行順序

```
Task 1.1 (定数)
  ↓
Task 1.2 (is_test_file) ──┐
Task 1.3 (is_doc_file)  ──┤  並列可能
  ↓                        ↓
Task 1.4 (file_type_weight_factor)
  ↓
Task 1.5 (apply_file_type_weight)
  ↓
Task 1.6 (search_entry_files変更)
  ↓
Task 2.1 (テスト確認)
Task 2.2 (Clippy・fmt)
```
