# 作業計画: Issue #170 - why/issueのJSON出力に日付情報を付与する

## Issue概要

| 項目 | 内容 |
|------|------|
| **Issue番号** | #170 |
| **タイトル** | why/issueのJSON出力に日付情報を付与する |
| **サイズ** | M |
| **優先度** | Medium |
| **依存Issue** | なし |
| **設計方針書** | dev-reports/design/issue-170-json-date-design-policy.md |

---

## Phase 1: データモデル・型定義

### Task 1.1: KnowledgeEntry に date フィールド追加

- **ファイル**: `src/indexer/knowledge.rs:170-175`
- **変更内容**: `pub date: Option<String>` フィールドを追加
- **依存**: なし

```rust
pub struct KnowledgeEntry {
    pub issue_number: String,
    pub file_path: String,
    pub relation: KnowledgeRelation,
    pub doc_subtype: DocSubtype,
    pub date: Option<String>,  // 追加
}
```

### Task 1.2: IssueDocumentEntry に date フィールド追加

- **ファイル**: `src/indexer/knowledge.rs:179-183`
- **変更内容**: `pub date: Option<String>` フィールドを追加
- **依存**: なし

### Task 1.3: KnowledgeRelatedResult に date フィールド追加

- **ファイル**: `src/indexer/knowledge.rs:187-193`
- **変更内容**: `pub date: Option<String>` フィールドを追加
- **依存**: なし

### Task 1.4: WhyDocumentEntry に date フィールド追加

- **ファイル**: `src/output/mod.rs:411`
- **変更内容**: `#[serde(skip_serializing_if = "Option::is_none")] pub date: Option<String>` 追加
- **依存**: なし

### Task 1.5: KnowledgeDocResult に date フィールド追加

- **ファイル**: `src/indexer/symbol_store.rs:66-71`
- **変更内容**: `pub date: Option<String>` フィールドを追加
- **依存**: なし

---

## Phase 2: 日付取得ユーティリティ

### Task 2.1: extract_date_from_filename 関数実装

- **ファイル**: `src/indexer/knowledge.rs`（新規関数追加）
- **変更内容**:
  - `LazyLock<Regex>` で `^(\d{4}-\d{2}-\d{2})` パターンをキャッシュ
  - ファイル名先頭からの日付抽出
  - `chrono::NaiveDate` でバリデーション
- **依存**: なし

### Task 2.2: extract_date_from_git_log 関数実装

- **ファイル**: `src/indexer/knowledge.rs`（新規関数追加）
- **変更内容**:
  - `validate_git_file_path` でパス検証（既存関数: 行214-219）
  - `git log --format=%ai -1 -- <path>` でコミット日取得
  - `line.get(..10)?` で安全スライス
  - `chrono::NaiveDate` でバリデーション
  - `tracing::debug!` でエラーログ
- **依存**: なし

### Task 2.3: extract_date_from_path 関数実装

- **ファイル**: `src/indexer/knowledge.rs`（新規関数追加）
- **可視性**: `pub(crate)`
- **変更内容**: ファイル名抽出 → git log フォールバックの2段階処理
- **依存**: Task 2.1, 2.2

---

## Phase 3: インデックス時の日付格納

### Task 3.1: scan_dev_reports で日付取得

- **ファイル**: `src/indexer/knowledge.rs:443` (`scan_dev_reports`)
- **変更内容**:
  - `parse_dev_report_path` 後に `extract_date_from_path` を呼び出し
  - `entry.date = extracted_date` で KnowledgeEntry に設定
  - `base_dir` をリポジトリルートとして使用
- **依存**: Task 1.1, 2.3

### Task 3.2: parse_dev_report_path の戻り値で date: None 設定

- **ファイル**: `src/indexer/knowledge.rs:422` (`parse_dev_report_path`)
- **変更内容**: KnowledgeEntry 生成時に `date: None` を明示
- **依存**: Task 1.1

### Task 3.3: insert_knowledge_entries で metadata に date 格納

- **ファイル**: `src/indexer/symbol_store.rs:783-786` (`insert_knowledge_entries`)
- **変更内容**:
  - metadata JSON 構築で `date` フィールドを条件付きで追加
  ```rust
  let mut meta = serde_json::json!({"doc_subtype": entry.doc_subtype.as_str()});
  if let Some(ref d) = entry.date {
      meta["date"] = serde_json::Value::String(d.clone());
  }
  ```
- **依存**: Task 1.1

---

## Phase 4: クエリ時の日付取得

### Task 4.1: find_documents_by_issue で date パース

- **ファイル**: `src/indexer/symbol_store.rs:855-883`
- **変更内容**:
  - metadata JSON から `date` を抽出
  - `let date = parsed.get("date").and_then(|v| v.as_str()).map(|s| s.to_string());`
  - `IssueDocumentEntry { ..., date }` に設定
- **依存**: Task 1.2

### Task 4.2: find_knowledge_related で date パース

- **ファイル**: `src/indexer/symbol_store.rs:1060-1084`
- **変更内容**:
  - metadata JSON から `date` を抽出
  - `KnowledgeRelatedResult { ..., date }` に設定
- **依存**: Task 1.3

### Task 4.3: find_knowledge_by_issue で date パース（KnowledgeDocResult）

- **ファイル**: `src/indexer/symbol_store.rs`
- **変更内容**: KnowledgeDocResult 生成時に `date` フィールド設定
- **依存**: Task 1.5

---

## Phase 5: 出力フォーマット変更

### Task 5.1: why コマンド - group_knowledge_results で date 転送

- **ファイル**: `src/cli/why.rs:122` (`group_knowledge_results`)
- **変更内容**: `KnowledgeRelatedResult.date` → `WhyDocumentEntry.date` に転送
- **依存**: Task 1.3, 1.4

### Task 5.2: issue コマンド - format_json をオブジェクト配列に変更（破壊的変更）

- **ファイル**: `src/cli/issue.rs:193` (`format_json`)
- **変更内容**:
  - カテゴリ別文字列配列 → オブジェクト配列 `{file_path, date}` に変更
  - `grouped()` の戻り値から新形式の JSON を構築
- **依存**: Task 1.2

---

## Phase 6: テスト修正・追加

### Task 6.1: extract_date_from_filename ユニットテスト

- **ファイル**: `src/indexer/knowledge.rs`（テストモジュール内）
- **テストケース**:
  - 正常系: `2026-03-20-issue140-review.md` → `Some("2026-03-20")`
  - 異常系: `issue-140-design-policy.md` → `None`
  - 異常系: `2026-13-45-invalid.md` → `None`（chrono バリデーション）
  - 異常系: `report-for-2026-03-20.md` → `None`（先頭アンカー）
- **依存**: Task 2.1

### Task 6.2: KnowledgeEntry 初期化箇所の修正

- **ファイル**: 複数ファイル
  - `src/indexer/knowledge.rs` テスト内（parse_dev_report_path テスト等）
  - `src/indexer/symbol_store.rs` テスト内（約10箇所）
  - `src/cli/suggest.rs` テスト内（約3箇所: 行607, 638, 657, 663）
- **変更内容**: 全ての KnowledgeEntry 初期化に `date: None` 追加
- **依存**: Task 1.1

### Task 6.3: IssueDocumentEntry 初期化箇所の修正

- **ファイル**: `src/cli/issue.rs` テスト内（約8箇所）
- **変更内容**: `date: None` 追加
- **依存**: Task 1.2

### Task 6.4: WhyDocumentEntry 初期化箇所の修正

- **ファイル**: `src/cli/why.rs` テスト内（約10箇所）
- **変更内容**: `date: None` 追加
- **依存**: Task 1.4

### Task 6.5: KnowledgeDocResult 初期化箇所の修正

- **ファイル**: `src/cli/suggest.rs` テスト内、`src/indexer/symbol_store.rs` テスト内
- **変更内容**: `date: None` 追加
- **依存**: Task 1.5

### Task 6.6: e2e_issue.rs テスト更新

- **ファイル**: `tests/e2e_issue.rs`
- **変更内容**:
  - `setup_issue_test_data`: KnowledgeEntry に `date: None` 追加
  - `issue_json_format`: オブジェクト配列アサーションに変更
  - `issue_progress_report_categorized`: `progress[0]["file_path"].as_str()` に変更
  - 日付フィールド存在確認テスト追加
- **依存**: Task 5.2

### Task 6.7: KnowledgeRelatedResult 初期化箇所の修正

- **ファイル**: 関連テスト内
- **変更内容**: `date: None` 追加
- **依存**: Task 1.3

---

## Phase 7: 品質チェック

### Task 7.1: ビルド・品質チェック

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |

---

## 実行順序

```
Phase 1 (型定義) → Phase 2 (ユーティリティ) → Phase 3 (格納) → Phase 4 (取得)
                                                                      ↓
Phase 6 (テスト修正) ←←←←←←←←←←←←←←←←←←←←←←←←← Phase 5 (出力変更)
                                                                      ↓
                                                              Phase 7 (品質チェック)
```

**TDD アプローチ**: 各 Phase でテストを先に書き、実装を後から行う。

---

## Definition of Done

- [ ] すべてのタスクが完了
- [ ] `cargo test --all` 全テストパス
- [ ] `cargo clippy --all-targets -- -D warnings` 警告0件
- [ ] `cargo fmt --all -- --check` 差分なし
- [ ] `why --format json` 出力に date フィールドが含まれる
- [ ] `issue N --format json` 出力にオブジェクト配列形式で date が含まれる
- [ ] ファイル名日付抽出が正しく動作する
- [ ] git log フォールバックが正しく動作する
- [ ] 日付取得不可の場合に null が返される
