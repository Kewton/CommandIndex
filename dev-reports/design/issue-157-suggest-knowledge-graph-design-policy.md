# 設計方針書: Issue #157 suggestコマンドへのナレッジグラフ参照統合

## 1. 概要

### 対象Issue
- **Issue番号**: #157
- **タイトル**: suggestコマンドがナレッジグラフを参照していない
- **スコープ**: suggestコマンドへのナレッジグラフ参照の統合（ストップワード処理は別Issue）

### 目的
suggestコマンドのクエリにIssue番号パターンが含まれる場合、ナレッジグラフから関連文書を取得し、戦略ステップとして優先的に含める。

## 2. システムアーキテクチャ上の位置づけ

```
User Query → [suggest.rs]
                ├── validate_input()       (既存)
                ├── ★ extract_issue_numbers (新規: Issue番号抽出)
                ├── ★ query_knowledge_graph (新規: KG参照)
                ├── BM25検索               (既存)
                ├── セマンティック検索       (既存)
                ├── RRF統合                (既存・変更なし)
                └── 戦略生成               (既存・拡張: KGステップをrun_suggestで先頭挿入)
```

変更は `src/cli/suggest.rs` に閉じる。他のサブコマンド・モジュールへの影響なし。

## 3. レイヤー構成と責務

| レイヤー | モジュール | 本Issue での役割 |
|---------|-----------|-----------------|
| **CLI** | `src/cli/suggest.rs` | **主要変更対象**: ナレッジグラフ参照ロジック追加 |
| **Indexer** | `src/indexer/knowledge.rs` | 再利用: `extract_issue_numbers()`, `ISSUE_RE` |
| **Indexer** | `src/indexer/symbol_store.rs` | 再利用: `find_knowledge_by_issue()`, `KnowledgeDocResult` |
| **Search** | `src/cli/search.rs` | 再利用: `SearchContext.symbol_db_path()` |
| **Output** | `src/output/mod.rs` | 拡張: `SuggestResult` にメタ情報追加 |

### 必要な import 追加（suggest.rs）

```rust
use crate::indexer::knowledge::extract_issue_numbers;
use crate::indexer::symbol_store::{SymbolStore, KnowledgeDocResult};
```

## 4. 設計判断とトレードオフ

### 判断1: マージ方式 — 戦略ステップ独立追加 vs RRF統合

**採用**: 戦略ステップ独立追加

| 観点 | 戦略ステップ独立追加 | RRF統合 |
|------|---------------------|---------|
| 複雑度 | 低（ステップ配列への挿入のみ） | 高（スコア変換・入力形式の統一が必要） |
| 正確性 | 高（ナレッジグラフは正確な関係なので優先表示が妥当） | 中（スコア正規化で順位が変動する可能性） |
| 保守性 | 高（既存RRFロジックに変更不要） | 低（RRF関数の引数・ロジック変更が必要） |

**理由**: ナレッジグラフの結果はスコア付きランキングではなく、Issue番号に紐づく文書の列挙。RRFの入力形式（`(file, score)` ペア）と本質的に異なるため、独立追加が最もシンプルで正確。

### 判断2: 使用API — `find_knowledge_by_issue` vs `find_documents_by_issue`

**採用**: `find_knowledge_by_issue`

| 観点 | `find_knowledge_by_issue` | `find_documents_by_issue` |
|------|--------------------------|--------------------------|
| Modifies対応 | ○（`KnowledgeRelation::parse()` で対応済み） | ×（未対応、エラーになる） |
| 複数Issue対応 | ○（`Vec<String>` を受け取る） | ×（単一Issueのみ） |
| 戻り値 | `KnowledgeDocResult`（file_path, relation, title含む） | `IssueDocumentEntry`（doc_subtype含む） |

**理由**: `find_knowledge_by_issue` は (1) Modifiesリレーションに対応済み、(2) 複数Issue番号を一度に検索可能、(3) unknown relationをスキップする安全な設計。`find_documents_by_issue` は Modifies でエラーになるバグがあり、単一Issueのみ対応。

**注意**: `find_knowledge_by_issue` にはLIMIT句がない（`find_documents_by_issue` にはLIMIT 100がある）。suggest側の `MAX_ISSUE_NUMBERS` (3件) と結果数の実用上の制約により問題ないが、将来的なLIMIT追加を検討。

### 判断3: SymbolStore接続タイミング — Issue番号検出後 vs 常時接続

**採用**: Issue番号検出後にのみ接続

**理由**: Issue番号がクエリに含まれない場合（大多数のケース）にSymbolStoreのオープンコストを回避。遅延評価によりパフォーマンスへの影響を最小化。

### 判断4: SuggestResult拡張 — メタ情報追加

**採用**: `matched_issues` フィールドを追加（空時はJSON出力から省略）

```rust
pub struct SuggestResult {
    pub query: String,
    pub has_embeddings: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub matched_issues: Vec<String>,  // 新規: ナレッジグラフでマッチしたIssue番号
    pub strategy: Vec<SuggestStep>,
}
```

**理由**: JSON出力でLLM連携時にナレッジグラフ参照があったことを判別可能にする。`skip_serializing_if` により空配列時はフィールド自体を省略し、既存JSON出力との後方互換性を維持。

### 判断5: KGステップ挿入の責務 — build_strategy内 vs run_suggest側

**採用**: run_suggest側でKGステップを挿入

**理由**: build_strategyとbuild_fallback_strategyの両方にKG引数を追加すると引数が肥大化する。KGステップの挿入をrun_suggest側（戦略生成後、出力前）で行うことで:
- build_strategy / build_fallback_strategy のシグネチャを変更不要
- BM25=0件（fallback）でもKGヒット時にナレッジグラフステップを含められる
- 関心の分離が明確（KG参照ロジック ↔ BM25/セマンティック戦略生成）

## 5. 詳細設計

### 5.1 処理フロー

```
run_suggest()
  1. validate_input()                         // 既存（コメント番号維持）
  2. SearchContext::new()                      // 既存
  3. IndexReaderWrapper::open() + EmbeddingStore // 既存
  4. ★ extract_issue_numbers(&query)          // 新規: クエリからIssue番号抽出
  5. ★ query_knowledge_graph()                // 新規: ナレッジグラフ参照（Issue番号がある場合のみ）
  6. BM25検索 → ファイル単位dedup             // 既存（旧4）
  7. セマンティック検索                        // 既存（旧5）
  8. RRF統合                                  // 既存（旧6）
  9. build_strategy() / build_fallback_strategy() // 既存（シグネチャ変更なし）
  10. ★ prepend_knowledge_steps()             // 新規: KGステップを戦略先頭に挿入
  11. result.matched_issues = issue_numbers    // 新規: メタ情報設定
  12. 出力                                    // 既存
```

### 5.2 新規関数: `query_knowledge_graph`

```rust
/// ナレッジグラフからIssue関連文書を取得する。
/// symbols.db が存在しない場合や、マッチするIssueがない場合は空のVecを返す。
fn query_knowledge_graph(
    ctx: &SearchContext,
    issue_numbers: &[String],
) -> Vec<KnowledgeDocResult> {
    if issue_numbers.is_empty() {
        return Vec::new();
    }
    // symbols.db 存在チェック
    let db_path = ctx.symbol_db_path();
    if !db_path.exists() {
        return Vec::new();
    }
    // SymbolStore オープン（エラー時は空を返す）
    let store = match SymbolStore::open(&db_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[suggest] knowledge graph query skipped: {e}");
            return Vec::new();
        }
    };
    // クエリ実行（エラー時は空を返す）
    match store.find_knowledge_by_issue(issue_numbers) {
        Ok(results) => results,
        Err(e) => {
            eprintln!("[suggest] knowledge graph query failed: {e}");
            Vec::new()
        }
    }
}
```

**設計ポイント**:
- フォールバック重視: symbols.db非存在・オープン失敗・クエリ失敗いずれも空Vecを返す
- SuggestErrorへのSymbolStoreErrorバリアント追加は不要（エラーを伝播しないため）
- eprintln は `[suggest]` プレフィックス付きで既存のwarning出力（行264）と統一

### 5.3 新規関数: `prepend_knowledge_steps`

KGステップの挿入をrun_suggest側の独立関数として実装:

```rust
/// ナレッジグラフ結果を戦略ステップとして先頭に挿入する。
fn prepend_knowledge_steps(
    strategy: &mut Vec<SuggestStep>,
    kg_docs: &[KnowledgeDocResult],
    matched_issues: &[String],
) {
    let mut kg_steps = Vec::new();
    // Issue番号ごとの issue コマンドステップ
    for issue_num in matched_issues {
        kg_steps.push(SuggestStep {
            command: format!("{BINARY_NAME} issue {issue_num} --format json"),
            reason: format!("Get knowledge graph documents for Issue #{issue_num}"),
        });
    }
    // 各文書の context ステップ
    for doc in kg_docs {
        let quoted_path = shell_quote(&doc.file_path);
        kg_steps.push(SuggestStep {
            command: format!("{BINARY_NAME} context -- {quoted_path} --max-files 5"),
            reason: format!("Get context for Issue #{} related document", doc.issue_number),
        });
    }
    // 先頭に挿入
    kg_steps.append(strategy);
    *strategy = kg_steps;
}
```

**利点**: build_strategy / build_fallback_strategy のシグネチャを変更しないため、既存テストへの影響が最小限。

### 5.4 Issue番号抽出と上限制御

```rust
/// ナレッジグラフ参照時の最大Issue番号数
const MAX_ISSUE_NUMBERS: usize = 3;

// クエリからIssue番号を抽出（最大3件、重複排除）
let issue_numbers: Vec<String> = {
    let nums = extract_issue_numbers(&query);
    let mut seen = std::collections::HashSet::new();
    let unique: Vec<String> = nums.into_iter()
        .filter(|n| seen.insert(n.clone()))
        .take(MAX_ISSUE_NUMBERS)
        .collect();
    unique
};
```

**注意**: `Vec::dedup()` は連続する重複のみ除去するため、`HashSet` + `filter` + `take` パターンで正確な重複排除と上限制御を行う。順序は元の出現順を保持。

### 5.5 SuggestResult 出力への影響

Issue番号がクエリに含まれる場合のJSON出力:
```json
{
  "query": "Issue #299の設計判断を理解したい",
  "has_embeddings": true,
  "matched_issues": ["299"],
  "strategy": [
    {"command": "commandindexdev issue 299 --format json", "reason": "..."},
    {"command": "commandindexdev context -- 'design-policy.md' ...", "reason": "..."},
    ...
  ]
}
```

Issue番号が含まれない場合のJSON出力（`matched_issues` フィールドは省略される）:
```json
{
  "query": "add authentication feature",
  "has_embeddings": true,
  "strategy": [...]
}
```

## 6. エラーハンドリング設計

| シナリオ | 動作 | 理由 |
|---------|------|------|
| symbols.db が存在しない | ナレッジグラフ参照をスキップ | フォールバック動作 |
| SymbolStore::open 失敗 | ナレッジグラフ参照をスキップ（`[suggest]` warning出力） | 堅牢性 |
| find_knowledge_by_issue 失敗 | 空の結果を使用（`[suggest]` warning出力） | 堅牢性 |
| Issue番号がクエリに含まれない | ナレッジグラフ参照をスキップ | 不要な処理回避 |
| マッチするIssueがない（結果0件） | BM25/セマンティック結果のみで戦略生成 | 正常動作 |

**方針**: ナレッジグラフ参照は「ベストエフォート」。失敗しても既存の検索ベース戦略は常に生成される。

## 7. テスト戦略

### 7.1 ユニットテスト（suggest.rs内）

| テスト | 内容 |
|-------|------|
| `test_prepend_knowledge_steps_with_docs` | KG結果がある場合、戦略先頭にissue/contextステップが挿入されること |
| `test_prepend_knowledge_steps_empty` | KG結果が空の場合、戦略が変更されないこと |
| `test_prepend_knowledge_steps_multiple_issues` | 複数Issue番号で各Issueのステップが生成されること |
| `test_issue_number_dedup` | 重複Issue番号がHashSetで正しく排除されること |
| `test_issue_number_max_limit` | MAX_ISSUE_NUMBERS(3)を超える場合にtruncateされること |

### 7.2 既存テストへの影響

`SuggestResult` に `matched_issues: Vec<String>` フィールド追加のため、以下の箇所で `matched_issues: vec![]` を追加:

| ファイル | テスト | 行番号 |
|---------|-------|--------|
| `src/cli/suggest.rs` | `format_human_output` | 行438 |
| `src/cli/suggest.rs` | `format_json_output` | 行462 |
| `src/cli/suggest.rs` | `format_path_output` | 行485 |
| `src/cli/suggest.rs` | `build_fallback_strategy` 戻り値 | 行168, 178 |
| `src/cli/suggest.rs` | `build_strategy` 戻り値 | 行156 |

### 7.3 E2Eテスト（将来的）

symbols.db にテスト用 knowledge edge を挿入し、Issue番号を含むクエリで suggest を実行して、戦略に issue/context ステップが含まれることを検証するテストを追加する。

## 8. セキュリティ設計

| 脅威 | 対策 | 優先度 |
|------|------|--------|
| SQLインジェクション | SymbolStoreはパラメータ化クエリを使用（`params![]`） | 高（既存対策で対応済み） |
| パストラバーサル | shell_quoteでファイルパスをエスケープ | 中（既存対策で対応済み） |
| Issue番号偽装 | extract_issue_numbersは数字のみ抽出（正規表現で制約） | 低 |
| コマンドインジェクション | suggestの出力はコマンド文字列の**提案**であり、プロセス内でのシェル実行はしない。実行前のバリデーションは呼び出し側（LLM等）の責任 | 低 |

## 9. パフォーマンス影響

| 処理 | オーバーヘッド | 条件 |
|------|-------------|------|
| extract_issue_numbers | 無視可能（正規表現マッチ1回） | 常時 |
| SymbolStore::open | 数ms | Issue番号検出時のみ |
| find_knowledge_by_issue | 数ms（インデックス付きクエリ） | Issue番号検出時のみ |

Issue番号がクエリに含まれない場合、追加オーバーヘッドはextract_issue_numbersの正規表現マッチのみ。

## 10. 品質基準

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |

## 11. 変更ファイル一覧

| ファイル | 変更内容 |
|---------|---------|
| `src/cli/suggest.rs` | ナレッジグラフ参照ロジック追加（`query_knowledge_graph`, `prepend_knowledge_steps`関数新規、`run_suggest`拡張、`MAX_ISSUE_NUMBERS`定数追加、import追加） |
| `src/cli/suggest.rs` (テスト) | 既存テスト3箇所に `matched_issues: vec![]` 追加、新規ユニットテスト5件追加 |
| `src/output/mod.rs` | `SuggestResult` に `matched_issues` フィールド追加（`serde(skip_serializing_if)` 付き） |
