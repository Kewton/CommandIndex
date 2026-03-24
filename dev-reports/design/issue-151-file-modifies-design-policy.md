# 設計方針書: Issue #151 - ナレッジグラフ fileノードとmodifiesエッジの実装

## 1. Issue概要

| 項目 | 内容 |
|------|------|
| Issue番号 | #151 |
| タイトル | ナレッジグラフ: fileノードとmodifiesエッジの実装 |
| 目的 | `why` / `before-change` / `search --related` がコードファイルからIssue・ドキュメントを辿れるようにする |

## 2. 現状分析

### 2.1 現在のナレッジグラフ構造

```
[issue] --has_design--> [document]  (設計方針書)
[issue] --has_review--> [document]  (レビュー結果)
[issue] --has_workplan--> [document]  (作業計画)
```

- ノードタイプ: `issue`, `document` のみ
- エッジタイプ: `has_design`, `has_review`, `has_workplan` のみ
- ソースコードファイルはナレッジグラフに存在しない

### 2.2 問題点

- `why src/foo.rs` → `find_knowledge_related` は `kn_doc.file_path = ?1` で検索開始するが、ソースコードファイルは `knowledge_nodes` に登録されていないため常に空
- `before-change` は git log 経由で間接的にIssueを特定しているが、ナレッジグラフ単体で完結しない
- `search --related` の `score_knowledge_graph` もドキュメントノードのみ対象

## 3. 設計方針

### 3.1 目標アーキテクチャ

```
[issue] --has_design--> [document]
[issue] --has_review--> [document]
[issue] --has_workplan--> [document]
[issue] --modifies--> [file]          ← NEW
```

### 3.2 採用方式: git logからの一括抽出

git log の全コミットを一括解析し、コミットメッセージ中のIssue番号と変更ファイルを紐づける。

```bash
git log --all --format='COMMIT_START%n%s%n%b%nCOMMIT_END' --name-only
```

**選定理由**:
- 実際のコミット履歴に基づく正確な紐づけ
- `before_change.rs` の `ISSUE_RE` 正規表現が再利用可能
- 1回のgitプロセス起動で全データを取得（パフォーマンス重視）

### 3.3 DBスキーマ方針

既存の `knowledge_nodes` / `knowledge_edges` テーブルをそのまま使用。カラム追加なし、スキーマバージョン変更なし。

```sql
-- file ノード
INSERT INTO knowledge_nodes (type, identifier, file_path)
VALUES ('file', 'src/foo.rs', 'src/foo.rs');

-- modifies エッジ（issue → file）
INSERT INTO knowledge_edges (source_id, target_id, relation)
VALUES (<issue_id>, <file_id>, 'modifies');
```

## 4. 変更設計

### 4.1 KnowledgeRelation 拡張

**ファイル**: `src/indexer/knowledge.rs`

```rust
pub enum KnowledgeRelation {
    HasDesign,
    HasReview,
    HasWorkplan,
    Modifies,       // NEW
}

impl KnowledgeRelation {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::HasDesign => "has_design",
            Self::HasReview => "has_review",
            Self::HasWorkplan => "has_workplan",
            Self::Modifies => "modifies",   // NEW
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "has_design" => Some(Self::HasDesign),
            "has_review" => Some(Self::HasReview),
            "has_workplan" => Some(Self::HasWorkplan),
            "modifies" => Some(Self::Modifies),   // NEW
            _ => None,
        }
    }
}
```

### 4.2 git logからのmodifiesエントリ抽出

**ファイル**: `src/indexer/knowledge.rs`

新規関数: `extract_file_modifies_from_git_log`

```rust
pub struct FileModifiesEntry {
    pub issue_number: String,
    pub file_path: String,
}

pub fn extract_file_modifies_from_git_log(
    repo_path: &Path,
) -> Result<Vec<FileModifiesEntry>, KnowledgeError>
```

**処理フロー**:
1. `git log --all --format='COMMIT_START%n%H%n%s%n%b%nCOMMIT_END' --name-only` を実行
2. コミット単位でパース:
   - subject + body から `ISSUE_RE` でIssue番号を抽出
   - `--name-only` 出力からファイルパスを収集
3. `(issue_number, file_path)` のペアを `HashSet` で重複排除
4. `Vec<FileModifiesEntry>` として返却

**ISSUE_RE の共有化**: `before_change.rs` の `ISSUE_RE` を `knowledge.rs` に移動し、両方から参照する。

**出力制限**: `MAX_GIT_OUTPUT_LINES = 50_000`（リポジトリ全体対象のため、before-changeの5000より大きく設定）

### 4.3 fileノード・modifiesエッジの挿入

**ファイル**: `src/indexer/symbol_store.rs`

新規関数: `insert_file_modifies_entries`

```rust
pub fn insert_file_modifies_entries(
    &self,
    entries: &[FileModifiesEntry],
) -> Result<(), SymbolStoreError>
```

**処理フロー**:
1. トランザクション開始
2. 各エントリについて直接SQLで:
   - `INSERT INTO knowledge_nodes (type, identifier, file_path) VALUES ('issue', ?, NULL) ON CONFLICT(type, identifier) DO NOTHING` → issue_id（`SELECT id` で取得）
   - `INSERT INTO knowledge_nodes (type, identifier, file_path) VALUES ('file', ?, ?) ON CONFLICT(type, identifier) DO NOTHING` → file_id（`SELECT id` で取得）
   - `INSERT INTO knowledge_edges (source_id, target_id, relation) VALUES (?, ?, 'modifies') ON CONFLICT DO NOTHING`
3. コミット

**注**: 既存の `insert_knowledge_entries` は `KnowledgeEntry` / `doc_subtype` 前提のため再利用しない。`insert_file_modifies_entries` は独立した専用関数として直接SQLを実行する。2つの挿入パスが存在する理由は、document系（design/review/workplan）とfile系（modifies）でメタデータ構造が異なるため。

### 4.4 SQLクエリ修正

**ファイル**: `src/indexer/symbol_store.rs`

#### find_knowledge_related の拡張

現在のクエリ（document → issue → sibling documents）に加え、file → issue → documents のパスを追加。

```sql
-- 元のクエリ: document経由
SELECT kn_issue.identifier, ke2.relation, kn_sibling.file_path, kn_issue.title
FROM knowledge_nodes kn_doc
JOIN knowledge_edges ke1 ON ke1.target_id = kn_doc.id
JOIN knowledge_nodes kn_issue ON ke1.source_id = kn_issue.id AND kn_issue.type = 'issue'
JOIN knowledge_edges ke2 ON ke2.source_id = kn_issue.id
JOIN knowledge_nodes kn_sibling ON ke2.target_id = kn_sibling.id
WHERE kn_doc.file_path = ?1
AND kn_sibling.file_path != ?1
```

**変更**: `kn_sibling.type = 'document'` フィルタを削除する。`kn_doc` 側は `file_path` ベースの検索のためtypeフィルタは不要（既にtype不問で動作）。

これにより:
- ソースコードファイル → 同一Issueの設計ドキュメント
- ドキュメント → 同一Issueのソースコードファイル
の両方向の検索が可能になる。

#### find_knowledge_by_issue の拡張

```sql
-- kn_doc.type = 'document' を kn_doc.type IN ('document', 'file') に変更
```

**呼び出し元ごとのfileノード結果の扱い**:
| 呼び出し元 | fileノード結果の扱い |
|-----------|-------------------|
| `before_change.rs` | `find_knowledge_by_issue` 呼び出し直後、**`rank_by_max_similarity` 呼び出し前**に `docs.retain(\|d\| d.relation != KnowledgeRelation::Modifies)` でfileノード結果を除外 |
| `why.rs` | `find_knowledge_related` 経由。fileノードは関連ファイルとして返す（ソースファイル同士の関連を表示） |
| `related.rs` | `find_knowledge_related` 経由。fileノードもスコアリング対象に含める |

**注**: `find_knowledge_by_issue` の戻り値型 `KnowledgeDocResult` は名前がdocument前提だが、fileノードも含まれうることをドキュメントコメントで明記する。

### 4.5 whyコマンドの出力調整

**ファイル**: `src/cli/why.rs`

`WhyDocumentEntry` の `relation` フィールドに `"modifies"` が含まれるようになる。

**modifiesエントリの大量表示対策**: 1つのIssueが多数のファイルをmodifiesしている場合、whyコマンドの出力が膨大になる。対策として `find_knowledge_related` の結果に `LIMIT 100` を追加し、出力件数を制限する。`why.rs` 側でもrelation別にグルーピングし、modifiesは件数のみ表示する（例: `modifies: 42 files`）。

### 4.6 before-changeコマンドの relation_priority 追加

**ファイル**: `src/cli/before_change.rs`

```rust
fn relation_priority(relation: &str) -> u8 {
    match relation {
        "has_design" => 0,
        "has_review" => 1,
        "has_workplan" => 2,
        "modifies" => 3,     // NEW
        _ => 4,
    }
}
```

### 4.7 indexコマンドへの組み込み

**ファイル**: `src/cli/index.rs`

#### Full index (Step 8.5 の後に追加)

```rust
// 8.6. Build file-modifies knowledge graph
{
    let entries = crate::indexer::knowledge::extract_file_modifies_from_git_log(path)?;
    if !entries.is_empty() {
        symbol_store.insert_file_modifies_entries(&entries)?;
    }
}
```

#### Update index (Step 13.5 の後に追加)

最初のバージョンでは差分更新は行わず、full rebuild方式とする。update indexでは file/modifies エントリのクリアと再構築を行う。

```rust
// 13.6. Rebuild file-modifies knowledge graph
{
    symbol_store.clear_file_modifies()?;
    let entries = crate::indexer::knowledge::extract_file_modifies_from_git_log(path)?;
    if !entries.is_empty() {
        symbol_store.insert_file_modifies_entries(&entries)?;
    }
}
```

### 4.8 clear_file_modifies 関数

**ファイル**: `src/indexer/symbol_store.rs`

```rust
/// fileノードは現時点でエッジのtargetとしてのみ使用される前提。
/// 将来source_id側でも参照される場合はクエリの修正が必要。
pub fn clear_file_modifies(&self) -> Result<(), SymbolStoreError> {
    let tx = self.conn.unchecked_transaction()?;
    tx.execute(
        "DELETE FROM knowledge_edges WHERE relation = 'modifies'",
        [],
    )?;
    tx.execute(
        "DELETE FROM knowledge_nodes WHERE type = 'file'
         AND id NOT IN (SELECT target_id FROM knowledge_edges)",
        [],
    )?;
    // modifiesエッジのみ持っていたissueノードの孤立を解消
    tx.execute(
        "DELETE FROM knowledge_nodes WHERE type = 'issue'
         AND id NOT IN (SELECT source_id FROM knowledge_edges)",
        [],
    )?;
    tx.commit()?;
    Ok(())
}
```

## 5. ISSUE_RE 共有化設計

### 現状

`before_change.rs` に `ISSUE_RE` が `LazyLock<Regex>` として定義。

### 方針

`src/indexer/knowledge.rs` に移動し、`pub` にする。`before_change.rs` からは `use crate::indexer::knowledge::ISSUE_RE` で参照。`before_change.rs` の `extract_issues_from_git_log` 内の `ISSUE_RE.captures_iter` ループも `knowledge::extract_issue_numbers` 呼び出しに置き換える。

```rust
// src/indexer/knowledge.rs
pub static ISSUE_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"(?i)(?:#(\d+)|\(#(\d+)\)|fixes\s+#(\d+)|refs\s+#(\d+))")
        .expect("ISSUE_RE is a valid regex literal")
});

pub fn extract_issue_numbers(text: &str) -> Vec<String> {
    ISSUE_RE
        .captures_iter(text)
        .filter_map(|cap| {
            cap.get(1)
                .or(cap.get(2))
                .or(cap.get(3))
                .or(cap.get(4))
                .map(|m| m.as_str().to_string())
        })
        .collect()
}
```

## 6. セキュリティ設計

| 脅威 | 対策 |
|------|------|
| git log出力のインジェクション | 行数制限(MAX_GIT_OUTPUT_LINES=50,000)、git logコマンド引数にユーザー由来の動的値を含めない |
| パストラバーサル | `extract_file_modifies_from_git_log` 内で `validate_file_path` 相当の検査（`..` 禁止、絶対パス禁止、null byte禁止、長さ上限）を適用。不正パスはスキップ |
| 大規模リポジトリでのメモリ消費 | HashSetによる重複排除、行数制限、エントリ数上限(MAX_ENTRIES=100,000) |
| SQLインジェクション | 全SQL実行は `rusqlite` の `params![]` マクロによるパラメータバインディング使用。`format!` による SQL文字列結合は禁止 |
| コマンドインジェクション | git logコマンドは固定引数のみ。`Command::new("git").args([...])` でシェル経由でない直接実行 |

## 7. 設計判断とトレードオフ

| 判断 | 選択 | 理由 | トレードオフ |
|------|------|------|-------------|
| 抽出方式 | git log一括取得 | 1回のgitプロセスで完結、パフォーマンス最適 | メモリ使用量が増加 |
| DB設計 | 既存スキーマ再利用 | カラム追加不要、マイグレーション不要 | metadata活用なし |
| 差分更新 | full rebuild | 初期実装の複雑度を抑える | update時にgit log全解析 |
| ISSUE_RE共有 | knowledge.rsに移動 | 重複排除、単一責任 | before_change.rsの変更が必要 |
| クエリ修正 | typeフィルタ拡張 | 最小限の変更で対応 | file/documentの混在出力 |

## 8. 影響範囲

### 変更対象ファイル

| ファイル | 変更種別 | 変更量 |
|---------|---------|--------|
| `src/indexer/knowledge.rs` | 大規模追加 | ~100行（Modifiesバリアント、ISSUE_RE移動、extract関数） |
| `src/indexer/symbol_store.rs` | 中規模追加 | ~50行（insert_file_modifies_entries、clear_file_modifies、SQLクエリ修正） |
| `src/cli/index.rs` | 小規模追加 | ~20行（Step 8.6, 13.6追加、`IndexError` に `Knowledge(KnowledgeError)` バリアント + `From` 実装追加） |
| `src/cli/before_change.rs` | 小規模修正 | ~10行（ISSUE_RE参照変更、relation_priority追加） |
| `src/cli/why.rs` | 変更なし | 0行（クエリ修正で自動対応） |
| `src/search/related.rs` | 変更なし | 0行（クエリ修正で自動対応） |

### 変更不要だが影響を受けるファイル

| ファイル | 影響 |
|---------|------|
| `src/cli/clean.rs` | symbols.db削除時にfileノードも消えるが既存動作と同じ |
| `symbol_store.rs` の `find_documents_by_issue` | `kn_doc.type='document'` フィルタでmodifiesエッジは返されない。変更不要だが将来的に防御的対応を推奨 |

### テスト追加

| テスト | 内容 |
|--------|------|
| `knowledge.rs` ユニットテスト | `KnowledgeRelation::Modifies` の parse/as_str/display |
| `knowledge.rs` ユニットテスト | `extract_file_modifies_from_git_log` の正常系・異常系 |
| `knowledge.rs` ユニットテスト | `extract_issue_numbers` の各パターン |
| `symbol_store.rs` ユニットテスト | `insert_file_modifies_entries` / `clear_file_modifies` |
| `symbol_store.rs` ユニットテスト | `find_knowledge_related` でfileノード経由の検索 |

## 9. 品質基準

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |
