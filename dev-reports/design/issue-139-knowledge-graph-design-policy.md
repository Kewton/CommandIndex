# 設計方針書: Issue #139 SQLiteベースの簡易ナレッジグラフの実装

## 1. 概要

### 対象Issue
- **Issue番号**: #139
- **タイトル**: SQLiteベースの簡易ナレッジグラフの実装
- **目的**: `dev-reports/` 配下のドキュメント群をIssue単位で構造的に関連付け、`search --related` の検出精度を向上させる

### スコープ
- Issue→文書群（設計書・レビュー・作業計画）の関連付け
- `search --related` へのナレッジグラフスコアリング統合
- **対象外**: `file` ノード、`modifies` エッジ、本文中の `#XXX` 参照、impact/context/suggest への機能追加

## 2. システムアーキテクチャ概要

### レイヤー構成と変更箇所

```
┌──────────────────────────────────────────────────────┐
│ CLI Layer (src/cli/)                                  │
│ ├── index.rs     [変更] フル構築+差分更新にKG処理追加 │
│ ├── impact.rs    [変更] RelationType match追加        │
│ └── context.rs   [変更] if matches! パターン追加      │
├──────────────────────────────────────────────────────┤
│ Search Layer (src/search/)                            │
│ └── related.rs   [変更] KGスコアリング追加            │
├──────────────────────────────────────────────────────┤
│ Indexer Layer (src/indexer/)                           │
│ ├── symbol_store.rs [変更] 新テーブル・CRUDメソッド   │
│ ├── knowledge.rs    [新規] KGパーサー・構築ロジック    │
│ ├── diff.rs         [変更なし]                        │
│ └── manifest.rs     [変更なし]                        │
├──────────────────────────────────────────────────────┤
│ Output Layer (src/output/)                            │
│ ├── mod.rs       [変更] RelationType::KnowledgeGraph  │
│ ├── human.rs     [変更] match アーム追加              │
│ ├── json.rs      [変更] match アーム追加              │
│ └── llm.rs       [変更] match アーム追加              │
└──────────────────────────────────────────────────────┘
```

## 3. 新規モジュール設計: `src/indexer/knowledge.rs`

### 責務
- `dev-reports/` ディレクトリの走査
- ファイル名・パス構造からのノード/エッジ抽出
- SymbolStore への一括投入

### エラー型

```rust
/// ナレッジグラフ処理のエラー型
#[derive(Debug)]
pub enum KnowledgeError {
    /// I/O エラー（ファイル走査、git diff 実行）
    Io(std::io::Error),
    /// SQLite エラー（SymbolStoreError からの変換）
    Store(SymbolStoreError),
    /// パス検証エラー（ベースディレクトリ外のパス）
    PathValidation(String),
}

impl From<std::io::Error> for KnowledgeError { ... }
impl From<SymbolStoreError> for KnowledgeError { ... }

// IndexError への統合
impl From<KnowledgeError> for IndexError { ... }
```

### 主要な型

```rust
/// ナレッジグラフのノード種別
#[derive(Debug, Clone, PartialEq)]
pub enum KnowledgeNodeType {
    Issue,
    Document,
}

/// ナレッジグラフのエッジ種別
#[derive(Debug, Clone, PartialEq)]
pub enum KnowledgeRelation {
    HasDesign,
    HasReview,
    HasWorkplan,
    References,
}

/// ドキュメントのサブタイプ
#[derive(Debug, Clone, PartialEq)]
pub enum DocSubtype {
    DesignPolicy,
    WorkPlan,
    IssueReview,
    DesignReview,
    ProgressReport,
}

/// パース結果
#[derive(Debug)]
pub struct KnowledgeEntry {
    pub issue_number: String,
    pub file_path: String,
    pub relation: KnowledgeRelation,
    pub doc_subtype: DocSubtype,
}

/// search --related の戻り値用構造体
#[derive(Debug)]
pub struct KnowledgeRelatedResult {
    pub file_path: String,
    pub relation: String,
    pub issue_number: String,
}
```

### パスパターン定義（構造化）

パスパターンは拡張性を考慮し、構造化された定義として管理する:

```rust
struct PatternRule {
    regex: Regex,
    doc_subtype: DocSubtype,
    relation: KnowledgeRelation,
}

/// パターンルール一覧（新規ドキュメント種別追加時はここに追加するだけ）
fn build_pattern_rules() -> Vec<PatternRule> {
    vec![
        PatternRule {
            regex: Regex::new(r"^dev-reports/design/issue-(\d+)-.*-design-policy\.md$").unwrap(),
            doc_subtype: DocSubtype::DesignPolicy,
            relation: KnowledgeRelation::HasDesign,
        },
        // ... 他のパターン
    ]
}
```

### 主要な関数

```rust
/// dev-reports/ ディレクトリを走査し、ナレッジエントリを抽出
pub fn scan_dev_reports(base_dir: &Path) -> Vec<KnowledgeEntry>;

/// git diff から dev-reports/ 配下の変更ファイルを抽出
pub fn detect_dev_reports_changes(base_dir: &Path) -> Result<DevReportsChanges, KnowledgeError>;

/// パスパターンからIssue番号とドキュメント種別を抽出
fn parse_dev_report_path(path: &Path) -> Option<KnowledgeEntry>;
```

### パスパターン正規表現

```rust
// dev-reports/design/issue-{N}-*-design-policy.md
static DESIGN_POLICY_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^dev-reports/design/issue-(\d+)-.*-design-policy\.md$").unwrap());

// dev-reports/issue/{N}/issue-review/summary-report.md
static ISSUE_REVIEW_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^dev-reports/issue/(\d+)/issue-review/summary-report\.md$").unwrap());

// dev-reports/issue/{N}/multi-stage-design-review/summary-report.md
static DESIGN_REVIEW_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^dev-reports/issue/(\d+)/multi-stage-design-review/summary-report\.md$").unwrap());

// dev-reports/issue/{N}/work-plan.md
static WORK_PLAN_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^dev-reports/issue/(\d+)/work-plan\.md$").unwrap());

// dev-reports/issue/{N}/pm-auto-dev/*/progress-report.md
static PROGRESS_REPORT_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^dev-reports/issue/(\d+)/pm-auto-dev/.+/progress-report\.md$").unwrap());
```

## 4. SymbolStore 拡張設計

### 責務分離方針

ナレッジグラフ関連の CRUD メソッドは `SymbolStore` に追加するが、**impl ブロックを分離**して管理する。将来的に `KnowledgeStore` として独立させる場合の境界を明確にする。

```rust
// src/indexer/symbol_store.rs

// 既存の impl SymbolStore { ... } はそのまま維持

// ナレッジグラフ関連メソッド（分離 impl ブロック）
impl SymbolStore {
    // --- Knowledge Graph Methods ---
    // 以下のメソッド群は将来 KnowledgeStore 分離時の単位となる
}
```

> **設計判断**: EmbeddingStore（`src/embedding/store.rs`）は完全に別ファイル・別DB接続として分離されている。ナレッジグラフは同一 symbols.db 内のテーブルを使用するため、まずは impl ブロック分離で対応し、テーブル数が増えた時点で KnowledgeStore に分離する。

### スキーマ変更

```rust
// src/indexer/symbol_store.rs
const CURRENT_SYMBOL_SCHEMA_VERSION: u32 = 4;  // 3 → 4
```

`create_tables()` に追加:

```rust
// knowledge_nodes テーブル
tx.execute_batch("
    CREATE TABLE IF NOT EXISTS knowledge_nodes (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        type TEXT NOT NULL,
        identifier TEXT NOT NULL,
        title TEXT,
        file_path TEXT,
        created_at TEXT DEFAULT (datetime('now')),
        updated_at TEXT DEFAULT (datetime('now')),
        UNIQUE(type, identifier)
    );
    CREATE INDEX IF NOT EXISTS idx_kn_type ON knowledge_nodes(type);
    CREATE INDEX IF NOT EXISTS idx_kn_identifier ON knowledge_nodes(identifier);
    CREATE INDEX IF NOT EXISTS idx_kn_file_path ON knowledge_nodes(file_path);

    CREATE TABLE IF NOT EXISTS knowledge_edges (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        source_id INTEGER NOT NULL REFERENCES knowledge_nodes(id) ON DELETE CASCADE,
        target_id INTEGER NOT NULL REFERENCES knowledge_nodes(id) ON DELETE CASCADE,
        relation TEXT NOT NULL,
        metadata TEXT,
        UNIQUE(source_id, target_id, relation)
    );
    CREATE INDEX IF NOT EXISTS idx_ke_source ON knowledge_edges(source_id);
    CREATE INDEX IF NOT EXISTS idx_ke_target ON knowledge_edges(target_id);
    CREATE INDEX IF NOT EXISTS idx_ke_relation ON knowledge_edges(relation);
")?;
```

### 新規CRUDメソッド

```rust
impl SymbolStore {
    /// ナレッジノードを挿入または更新（UPSERT）
    pub fn upsert_knowledge_node(
        &self,
        node_type: &str,
        identifier: &str,
        title: Option<&str>,
        file_path: Option<&str>,
    ) -> Result<i64, SymbolStoreError>;

    /// ナレッジエッジを挿入（UPSERT）
    pub fn upsert_knowledge_edge(
        &self,
        source_id: i64,
        target_id: i64,
        relation: &str,
        metadata: Option<&str>,
    ) -> Result<(), SymbolStoreError>;

    /// ナレッジエントリを一括挿入（トランザクション）
    pub fn insert_knowledge_entries(
        &self,
        entries: &[KnowledgeEntry],
    ) -> Result<(), SymbolStoreError>;

    /// knowledge_nodes / knowledge_edges を全削除
    pub fn clear_knowledge_graph(&self) -> Result<(), SymbolStoreError>;

    /// 指定ファイルパスのナレッジノードを削除（ON DELETE CASCADEでエッジも連鎖削除）
    pub fn delete_knowledge_by_file(&self, file_path: &str) -> Result<(), SymbolStoreError>;

    /// 指定ファイルに関連するドキュメントパスを取得（search --related 用）
    /// ファイルが属するIssueを特定し、同一Issueの全関連ドキュメントを返す
    pub fn find_knowledge_related(
        &self,
        file_path: &str,
    ) -> Result<Vec<KnowledgeRelatedResult>, SymbolStoreError>;
}
```

### delete_by_file() の拡張

```rust
pub fn delete_by_file(&self, file_path: &str) -> Result<(), SymbolStoreError> {
    let tx = self.conn.unchecked_transaction()?;
    tx.execute("DELETE FROM symbols WHERE file_path = ?1", params![file_path])?;
    tx.execute("DELETE FROM dependencies WHERE source_file = ?1", params![file_path])?;
    tx.execute("DELETE FROM file_links WHERE source_file = ?1", params![file_path])?;
    tx.execute("DELETE FROM embeddings WHERE file_path = ?1", params![file_path])?;
    // 追加: ナレッジノード削除（ON DELETE CASCADE でエッジも自動削除）
    tx.execute("DELETE FROM knowledge_nodes WHERE file_path = ?1", params![file_path])?;
    tx.commit()?;
    Ok(())
}
```

## 5. search --related 統合設計

### RelationType 拡張

```rust
// src/output/mod.rs
pub enum RelationType {
    MarkdownLink,
    ImportDependency,
    TagMatch { matched_tags: Vec<String> },
    PathSimilarity,
    DirectoryProximity,
    KnowledgeGraph,  // 新規追加
}
```

### スコアリング追加

```rust
// src/search/related.rs
const KNOWLEDGE_GRAPH_WEIGHT: f32 = 0.8;

impl RelatedSearchEngine {
    fn score_knowledge_graph(
        &self,
        target_path: &str,
        scores: &mut HashMap<String, (f32, Vec<RelationType>)>,
    ) -> Result<(), RelatedSearchError> {
        let related = self.store.find_knowledge_related(target_path)
            .map_err(|e| RelatedSearchError::SymbolStore(e))?;
        for result in related {
            add_relation(
                scores,
                &result.file_path,
                KNOWLEDGE_GRAPH_WEIGHT,
                RelationType::KnowledgeGraph,
            );
        }
        Ok(())
    }
}
```

`find_related()` に追加:

```rust
pub fn find_related(&self, file_path: &str, limit: usize) -> Result<Vec<RelatedSearchResult>, ...> {
    let mut scores = HashMap::new();
    self.score_markdown_links(target_path, &mut scores);
    self.score_import_deps(target_path, &mut scores);
    self.score_knowledge_graph(target_path, &mut scores)?;  // 新規追加
    self.score_tag_match(target_path, &mut scores)?;
    self.score_path_proximity(target_path, &mut scores);
    // ... ソートして返却
}
```

### 出力フォーマッタ更新

| ファイル | 追加するmatch アーム | 表示文字列 |
|----------|---------------------|-----------|
| `human.rs` | `RelationType::KnowledgeGraph => "knowledge".to_string()` | `knowledge` |
| `json.rs` | `RelationType::KnowledgeGraph => json!("knowledge_graph")` | `"knowledge_graph"` |
| `llm.rs` | `RelationType::KnowledgeGraph => "knowledge".to_string()` | `knowledge` |
| `impact.rs` | `RelationType::KnowledgeGraph => "knowledge_graph"` | `knowledge_graph` |
| `context.rs` | `relation_to_string()`: if matches! に KnowledgeGraph 追加 + `build_context_entry()` にも対応追加 | `knowledge_graph` |

> **注意**: `context.rs` は exhaustive match ではなく `if matches!` パターンを使用しているため、新バリアント追加時にコンパイルエラーにならない。暗黙的に無視されるため、明示的な対応が必須。

## 6. CLI フロー設計

### `commandindexdev index`（フル構築）

```
既存フロー:
  scan_files → index_file_and_upsert (各ファイル) → commit → manifest保存

追加フロー（writer.commit() 直後、manifest.save() 前に実行）:
  store.clear_knowledge_graph()
  → knowledge::scan_dev_reports(base_dir)
  → store.insert_knowledge_entries(entries)
  // 具体的な挿入位置: src/cli/index.rs run() 内の
  // writer.commit()? の直後（tantivy commitは確定済み、KGエラーでロールバックされない）
```

### `commandindexdev update`（差分更新）

```
既存フロー:
  detect_changes → deleted/modified/added 処理 → commit

追加フロー（writer.commit() 直後に実行）:
  knowledge::detect_dev_reports_changes(base_dir)
  → 変更ファイルに対応するノード削除 (delete_knowledge_by_file)
  → 追加・変更ファイルからエントリ再構築 (insert_knowledge_entries)
  // 具体的な挿入位置: src/cli/index.rs run_incremental() 内の
  // writer.commit()? の直後
```

## 7. 設計判断とトレードオフ

### 判断1: 独立走査パス vs 既存インデクサ拡張

**選択**: 独立走査パス

**理由**: 既存インデクサは拡張子ベースのファイルフィルタ（md/ts/tsx/py）を使用しており、`dev-reports/` のMarkdownファイルは走査対象に含まれる場合がある。しかしナレッジグラフは**ファイル内容ではなくパス構造**から関連性を抽出するため、既存の内容解析パイプラインに乗せるメリットが薄い。独立走査パスにすることで既存コードへの影響を最小化する。

### 判断2: データ複製 vs 実行時結合

**選択**: 実行時結合（dependencies/file_links のデータをknowledge_edgesに複製しない）

**理由**: データの二重管理はメンテナンスコストを増加させる。`search --related` 実行時に既存テーブルとknowledge_edgesの両方を走査する方式で、パフォーマンスへの影響は軽微（SQLiteの単純SELECT）。

### 判断3: ノードタイプ2種 vs 多種

**選択**: 初回は `issue` と `document` の2種のみ

**理由**: YAGNI原則。`file` ノードと `modifies` エッジは後続Issueで対応。初回はIssue→文書群の関連付けに集中し、価値を早く提供する。

### 判断4: 全削除→再構築 vs ALTERマイグレーション

**選択**: 全削除→再構築（スキーマバージョン不一致時）

**理由**: 既存のスキーマ進化パターンに従う。`clean` → `index` で完全に再構築する方式はシンプルで確実。ALTERマイグレーションは複雑さを増すだけで、symbols.dbの再構築コストは許容範囲。

### 判断5: KnowledgeGraph 重み 0.8 の根拠

**選択**: `KNOWLEDGE_GRAPH_WEIGHT = 0.8`

**理由**: MarkdownLink (1.0) と ImportDependency (0.9) に次ぐ高い信頼度。ナレッジグラフのエッジはファイル名パターンから機械的に抽出されるため誤検出が少ない。TagMatch (0.5) やPathSimilarity (0.4) よりも信頼性が高い。

## 8. 影響範囲

### 変更が必要なファイル

| ファイル | 変更規模 | 変更内容 |
|----------|----------|----------|
| `src/indexer/knowledge.rs` | **新規** | KGパーサー・構築ロジック（~200行想定） |
| `src/indexer/symbol_store.rs` | **大** | スキーマv4、新テーブル、CRUD、delete拡張 |
| `src/indexer/mod.rs` | **小** | `pub mod knowledge;` 追加 |
| `src/search/related.rs` | **中** | KGスコアリング追加 |
| `src/cli/index.rs` | **中** | フル構築・差分更新にKG処理追加 |
| `src/output/mod.rs` | **小** | RelationType バリアント追加 |
| `src/output/human.rs` | **小** | match アーム追加 |
| `src/output/json.rs` | **小** | match アーム追加 |
| `src/output/llm.rs` | **小** | match アーム追加 |
| `src/cli/impact.rs` | **小** | match アーム追加 |
| `src/cli/context.rs` | **小** | match アーム追加 |

### 変更不要なファイル

- `src/indexer/diff.rs` - 既存差分検知はそのまま
- `src/indexer/manifest.rs` - マニフェストは変更不要
- `Cargo.toml` - 追加依存なし（rusqlite既存、regex既存）

## 9. セキュリティ設計

| 脅威 | 対策 | 優先度 |
|------|------|--------|
| パストラバーサル | `canonicalize()` + `starts_with(base_dir)` で検証。シンボリックリンクも解決後にチェック | 高 |
| SQLインジェクション | rusqlite のパラメータバインディング（`?1`）を使用 | 高 |
| 正規表現DoS | コンパイル済み正規表現を `Lazy<Regex>` で保持。パターンは固定文字列のみ | 低 |
| ON DELETE CASCADE | `PRAGMA foreign_keys = ON` が `SymbolStore::open()` で設定済み。テストで CASCADE 動作を明示的に検証 | 中 |

### パストラバーサル対策の実装

```rust
fn validate_path(path: &Path, base_dir: &Path) -> Result<PathBuf, KnowledgeError> {
    let canonical = path.canonicalize()
        .map_err(KnowledgeError::Io)?;
    let canonical_base = base_dir.canonicalize()
        .map_err(KnowledgeError::Io)?;
    if !canonical.starts_with(&canonical_base) {
        return Err(KnowledgeError::PathValidation(
            format!("Path {} is outside base directory", path.display())
        ));
    }
    Ok(canonical)
}
```

## 10. テスト戦略

### ユニットテスト

| テスト対象 | テスト内容 |
|-----------|-----------|
| `parse_dev_report_path()` | 各パスパターンの正常系・異常系 |
| `scan_dev_reports()` | テンポラリディレクトリでの走査テスト |
| `upsert_knowledge_node()` | ノードの挿入・重複時のUPSERT |
| `upsert_knowledge_edge()` | エッジの挿入・カスケード削除 |
| `delete_knowledge_by_file()` | ノード削除とエッジの連鎖削除確認 |

### 統合テスト

| テスト対象 | テスト内容 |
|-----------|-----------|
| `search --related` | KGエッジがスコアリングに反映されることの確認 |
| `index` → `search` | フル構築後にKGベースの関連検索が動作 |
| `update` | dev-reports 変更時にKGが差分更新されること |

### 既存テスト更新

| テスト | 更新内容 |
|--------|----------|
| `test_schema_version_v3` → `test_schema_version_v4` | バージョン値更新 |
| `e2e_related_search` | KnowledgeGraph 追加後のスコア期待値調整 |
| `output_format` テスト | KnowledgeGraph バリアントの表示テスト追加 |

## 11. 品質基準

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |

## 12. 作成日

2026-03-24
