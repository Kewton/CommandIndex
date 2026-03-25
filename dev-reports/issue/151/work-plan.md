# 作業計画: Issue #151 - ナレッジグラフ fileノードとmodifiesエッジの実装

## Issue概要

| 項目 | 内容 |
|------|------|
| **Issue番号** | #151 |
| **タイトル** | ナレッジグラフ: fileノードとmodifiesエッジの実装 |
| **サイズ** | M |
| **優先度** | High |
| **依存Issue** | #139 (ナレッジグラフ実装 - 完了済み) |

---

## 詳細タスク分解

### Phase 1: リファクタリング（ISSUE_RE共有化）

リスク軽減のため、新機能追加前にリファクタリングを分離。

#### Task 1.1: ISSUE_RE と extract_issue_numbers を knowledge.rs に移動

**成果物**: `src/indexer/knowledge.rs`, `src/cli/before_change.rs`
**依存**: なし
**変更内容**:
1. `before_change.rs` の `ISSUE_RE` (LazyLock<Regex>) を `knowledge.rs` に移動し `pub` にする
2. `knowledge.rs` に `pub fn extract_issue_numbers(text: &str) -> Vec<String>` を新設
3. `before_change.rs` の `extract_issues_from_git_log` 内の `ISSUE_RE.captures_iter` ループを `knowledge::extract_issue_numbers` 呼び出しに置換:
   ```rust
   for num in crate::indexer::knowledge::extract_issue_numbers(&line) {
       issues.insert(num);
   }
   ```
4. テスト: `extract_issue_numbers` のユニットテスト（`#123`, `(#123)`, `fixes #123`, `refs #123` の各パターン）
5. 既存テスト全パス確認: `cargo test --all`

**品質ゲート**: この時点で `cargo test --all` + `cargo clippy` 全パス

---

### Phase 2: 型定義・データモデル

#### Task 2.1: KnowledgeRelation に Modifies バリアント追加

**成果物**: `src/indexer/knowledge.rs`
**依存**: Task 1.1
**変更内容**:
1. `KnowledgeRelation` enum に `Modifies` バリアント追加
2. `as_str()` に `Self::Modifies => "modifies"` 追加
3. `parse()` に `"modifies" => Some(Self::Modifies)` 追加
4. `Display` 実装への反映（存在する場合）
5. テスト: parse/as_str のラウンドトリップテスト

#### Task 2.2: FileModifiesEntry 構造体定義

**成果物**: `src/indexer/knowledge.rs`
**依存**: なし
**変更内容**:
```rust
pub struct FileModifiesEntry {
    pub issue_number: String,
    pub file_path: String,
}
```

---

### Phase 3: コアロジック実装

#### Task 3.1: extract_file_modifies_from_git_log 関数実装

**成果物**: `src/indexer/knowledge.rs`
**依存**: Task 1.1, Task 2.2
**変更内容**:
1. `pub fn extract_file_modifies_from_git_log(repo_path: &Path) -> Result<Vec<FileModifiesEntry>, KnowledgeError>`
2. `git log --all --format='COMMIT_START%n%s%n%b%nCOMMIT_END' --name-only` 実行
3. BufReader で行単位処理（MAX_GIT_OUTPUT_LINES = 50,000）
4. コミット単位パース → `extract_issue_numbers` で Issue番号抽出
5. ファイルパスのバリデーション（`..` 禁止、絶対パス禁止、null byte禁止）
6. `HashSet<(String, String)>` で重複排除（MAX_ENTRIES = 100,000）
7. テスト: 正常系（Issue番号付きコミット）、異常系（Issue番号なし、不正パス）

#### Task 3.2: insert_file_modifies_entries 関数実装

**成果物**: `src/indexer/symbol_store.rs`
**依存**: Task 2.2
**変更内容**:
1. `pub fn insert_file_modifies_entries(&self, entries: &[FileModifiesEntry]) -> Result<(), SymbolStoreError>`
2. トランザクション内で issue node + file node の INSERT OR IGNORE + SELECT id
3. modifies エッジの INSERT OR IGNORE
4. 全SQL は `params![]` マクロ使用
5. テスト: 正常挿入、重複挿入、空エントリ

#### Task 3.3: clear_file_modifies 関数実装

**成果物**: `src/indexer/symbol_store.rs`
**依存**: Task 3.2
**変更内容**:
1. `pub fn clear_file_modifies(&self) -> Result<(), SymbolStoreError>`
2. `unchecked_transaction` 使用
3. modifiesエッジ削除 → 孤立fileノード削除 → 孤立issueノード削除
4. テスト: クリア後にfileノード/modifiesエッジが0件

---

### Phase 4: SQLクエリ修正

#### Task 4.1: find_knowledge_related のクエリ修正

**成果物**: `src/indexer/symbol_store.rs`
**依存**: Task 3.2
**変更内容**:
1. `kn_sibling.type = 'document'` フィルタを削除
2. `LIMIT 100` を追加（大量結果対策）
3. テスト: fileノード経由での関連ドキュメント検索が動作すること

#### Task 4.2: find_knowledge_by_issue のクエリ修正

**成果物**: `src/indexer/symbol_store.rs`
**依存**: Task 3.2
**変更内容**:
1. `kn_doc.type = 'document'` を `kn_doc.type IN ('document', 'file')` に変更
2. `KnowledgeDocResult` のドキュメントコメントにfileノードを含む旨を追記
3. テスト: fileノードが結果に含まれること

---

### Phase 5: コマンド統合

#### Task 5.1: index コマンドへの組み込み

**成果物**: `src/cli/index.rs`
**依存**: Task 3.1, Task 3.2, Task 3.3
**変更内容**:
1. `IndexError` に `Knowledge(KnowledgeError)` バリアント + `From` 実装追加
2. Full index: Step 8.5 の後に Step 8.6 を追加
3. Update index: Step 13.5 の後に Step 13.6 を追加（clear + rebuild方式）

#### Task 5.2: before_change コマンドの調整

**成果物**: `src/cli/before_change.rs`
**依存**: Task 4.2
**変更内容**:
1. `find_knowledge_by_issue` 呼び出し直後、`rank_by_max_similarity` 前に retain フィルタ追加
2. `relation_priority` に `"modifies" => 3` 追加、`_ => 4` に変更

#### Task 5.3: why コマンドの出力調整

**成果物**: `src/cli/why.rs`
**依存**: Task 4.1
**変更内容**:
1. modifies relation のグルーピング表示（件数のみ表示: `modifies: 42 files`）
2. human/json/llm 各フォーマットでの modifies 表示確認

---

### Phase 6: テスト・品質

#### Task 6.1: 既存テスト全パス確認

**依存**: Phase 5 完了
**コマンド**:
```bash
cargo test --all
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

#### Task 6.2: e2eテスト追加（オプション）

**成果物**: `tests/` 配下
**依存**: Phase 5 完了
**変更内容**:
- git リポジトリ作成 → コミット（Issue番号含む） → index → `why src/foo.rs` → 関連ドキュメント表示の検証

---

## 実装順序

```
Phase 1: Task 1.1 (ISSUE_RE共有化)
   ↓
Phase 2: Task 2.1 + Task 2.2 (型定義) [並列可]
   ↓
Phase 3: Task 3.1 + Task 3.2 + Task 3.3 (コアロジック)
   ↓
Phase 4: Task 4.1 + Task 4.2 (クエリ修正) [並列可]
   ↓
Phase 5: Task 5.1 → Task 5.2 + Task 5.3 (統合)
   ↓
Phase 6: Task 6.1 + Task 6.2 (テスト・品質)
```

## 品質チェック項目

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |

## Definition of Done

- [x] ISSUE_RE が knowledge.rs に移動済み
- [ ] KnowledgeRelation::Modifies が追加済み
- [ ] extract_file_modifies_from_git_log が実装済み
- [ ] insert_file_modifies_entries / clear_file_modifies が実装済み
- [ ] find_knowledge_related / find_knowledge_by_issue のクエリ修正済み
- [ ] index コマンドで file-modifies 構築が動作
- [ ] before_change コマンドで modifies エントリがフィルタされる
- [ ] why コマンドで modifies 表示が制御される
- [ ] cargo test --all 全パス
- [ ] cargo clippy --all-targets -- -D warnings 警告0件
- [ ] cargo fmt --all -- --check 差分なし
