# 設計方針書: Issue #167 suggestコマンドのナレッジグラフ展開制限

## 1. 概要

suggestコマンドのナレッジグラフ展開が過剰（80件提案）になる問題を修正する。`prepend_knowledge_steps()` の前段にフィルタリングロジックを追加し、代表文書に絞った提案を生成する。

## 2. 変更対象モジュールと責務

| レイヤー | モジュール | 変更内容 |
|---------|-----------|---------|
| **CLI** | `src/cli/suggest.rs` | フィルタリング関数追加、KG取得API変更、SuggestKgDoc構造体追加 |
| **CLI** | `src/cli/before_change.rs` | `relation_priority()` を `KnowledgeRelation::priority()` に置き換え（DRY改善） |
| **Indexer** | `src/indexer/knowledge.rs` | `KnowledgeRelation::priority()` メソッド追加 |

## 3. 設計判断とトレードオフ

### 判断1: KG文書取得APIの選択

| 選択肢 | メリット | デメリット |
|--------|---------|----------|
| **(A) `find_knowledge_by_issue()` + retain()** | 既存コード変更が最小、複数Issue一括取得可能 | `doc_subtype`がなくfile_pathパターンマッチに依存 |
| **(B) `find_documents_by_issue()` を使用** ← 採用 | `doc_subtype`による型安全なフィルタ、file_path非依存 | 単一Issue用APIのため複数Issue時はループ呼び出し（最大3回） |

**採用理由**: `DocSubtype` enum値で `IssueReview`/`DesignReview`（保持）vs `StageReview`（除外）を直接判定でき、ファイルパス規約変更に対して堅牢。SQLiteアクセス回数増加（最大3回）はローカルDBのため影響軽微。

### 判断2: フィルタリングレイヤーの配置

**採用**: `prepend_knowledge_steps()` の前段（`suggest.rs`内）で実施。

**理由**: `before_change.rs` と同パターン。`find_documents_by_issue()` のSQL変更は不要で、他コマンド（issue, before_change）への影響を回避。

### 判断3: MAX_KG_DOCS_PER_ISSUE の値

**採用**: `MAX_KG_DOCS_PER_ISSUE = 4`

**根拠**: 代表文書4種（design-policy, work-plan, issue-review/summary-report.md, design-review/summary-report.md）と整合。`before_change.rs` の `MAX_DOCS_PER_ISSUE = 2` より大きいが、suggestは調査開始ガイドとしてより多くのコンテキストが有用。

### 判断4: relation_priorityの実装方針

**採用**: `KnowledgeRelation` に `priority()` メソッドを追加し、`suggest.rs` と `before_change.rs` の両方で共通利用する。

**理由**: `before_change.rs` の `relation_priority()` は `&str` ベースで同一の優先度値を定義しており、DRY違反となっている。`KnowledgeRelation::priority()` を `src/indexer/knowledge.rs` に追加することで、優先度定義を一元化する。`before_change.rs` の既存 `relation_priority()` 関数は互換ラッパーとして残し、内部実装を `KnowledgeRelation::parse().map_or(5, |r| r.priority())` に委譲する。これにより未知のrelation値に対するフォールバック（優先度5）を維持する。

### 判断5: find_documents_by_issue()ループ方式の採用根拠

`find_knowledge_by_issue()` + SQLに `metadata`（`doc_subtype`）カラムを追加する方式も検討した。しかし、`find_knowledge_by_issue()` は `issue.rs`・`before_change.rs` でも使用されており、SQL変更やレスポンス型変更がこれらのコマンドに波及する。`find_documents_by_issue()` ループ方式は既存APIを変更せず、`suggest.rs` 内で完結するため、影響範囲を最小化できる。

## 4. 詳細設計

### 4.1 新規定数

```rust
/// ナレッジグラフからのIssue単位最大ドキュメント数
const MAX_KG_DOCS_PER_ISSUE: usize = 4;
```

### 4.2 新規構造体: SuggestKgDoc

`find_documents_by_issue()` は `IssueDocumentEntry`（issue_numberなし）を返すため、suggest用に `issue_number` を付与したDTOを定義する。

```rust
/// suggestコマンド用のKGドキュメントDTO
struct SuggestKgDoc {
    issue_number: String,
    file_path: String,
    relation: KnowledgeRelation,
    doc_subtype: DocSubtype,
}
```

### 4.3 新規関数: filter_and_limit_kg_docs()

```rust
/// ナレッジグラフドキュメントをフィルタリング・Issue単位制限する。
///
/// 1. modifies / has_progress / has_review(StageReview) を除外
/// 2. relation_priority でソート
/// 3. Issue単位にグルーピングし MAX_KG_DOCS_PER_ISSUE 件に制限
///
/// issue_numbersの順序でIssueをグルーピングすることで、入力順を維持する。
/// これにより、呼び出し元が指定したIssue優先順位が結果に反映される。
fn filter_and_limit_kg_docs(docs: Vec<SuggestKgDoc>, issue_numbers: &[String]) -> Vec<SuggestKgDoc> {
    // Step 1: retain() フィルタリング
    let mut filtered: Vec<SuggestKgDoc> = docs.into_iter()
        .filter(|d| {
            match d.relation {
                KnowledgeRelation::Modifies => false,
                KnowledgeRelation::HasProgress => false,
                KnowledgeRelation::HasReview => {
                    // IssueReview, DesignReview のみ保持、StageReview は除外。
                    // ProgressReport は has_progress リレーションで管理されるため
                    // HasReview + ProgressReport の組み合わせは通常発生しないが、
                    // 万一存在した場合は DocSubtype の match で暗黙的に除外される。
                    // これは意図的な設計判断である。
                    matches!(d.doc_subtype, DocSubtype::IssueReview | DocSubtype::DesignReview)
                }
                KnowledgeRelation::HasDesign | KnowledgeRelation::HasWorkplan => true,
            }
        })
        .collect();

    // Step 2: KnowledgeRelation::priority() でソート（sort_by は安定ソート）
    filtered.sort_by(|a, b| {
        a.relation.priority().cmp(&b.relation.priority())
    });

    // Step 3: Issue単位グルーピング + 上限制御
    // issue_numbers の順序を維持してグルーピングする（SF-2対応）
    let mut issue_groups: HashMap<String, Vec<SuggestKgDoc>> = HashMap::new();
    for doc in filtered {
        issue_groups.entry(doc.issue_number.clone()).or_default().push(doc);
    }

    let mut result = Vec::new();
    for issue_num in issue_numbers {
        if let Some(docs) = issue_groups.remove(issue_num) {
            result.extend(docs.into_iter().take(MAX_KG_DOCS_PER_ISSUE));
        }
    }
    result
}
```

### 4.4 KnowledgeRelation::priority() メソッド追加（src/indexer/knowledge.rs）

`KnowledgeRelation` に `priority()` メソッドを追加し、`suggest.rs` と `before_change.rs` で共通利用する。

```rust
impl KnowledgeRelation {
    /// Relation priority for sorting (lower = higher priority).
    /// HasProgress / Modifies はフィルタで除外されることが多いが、
    /// 型の網羅性（exhaustive match）を保証するために優先度を定義している。
    pub fn priority(&self) -> u8 {
        match self {
            Self::HasDesign => 0,
            Self::HasWorkplan => 1,
            Self::HasReview => 2,
            Self::HasProgress => 3,
            Self::Modifies => 4,
        }
    }
}
```

**NH-2: priority()をKnowledgeRelationに配置する理由**: リレーションの優先度はドメイン知識（どのリレーションがより重要か）に基づく判断であり、リレーション型自体に帰属させることで、各利用箇所（suggest.rs, before_change.rs）が独自の優先度定義を持つ必要がなくなる。ドメインルールの一元管理先として `KnowledgeRelation` が適切である。

**変更対象**:
- `src/indexer/knowledge.rs`: `priority()` メソッド追加
- `src/cli/before_change.rs`: 既存の `relation_priority(&str) -> u8` 関数を互換ラッパーとして残す（MF-2対応、下記参照）
- `src/cli/suggest.rs`: `kg_relation_priority()` ローカル関数の代わりに `relation.priority()` を使用

**MF-2: before_change.rsの `relation_priority()` 互換ラッパー**: `before_change.rs` の `relation_priority()` は `&str` ベースのインターフェースであり、未知のrelation値に対するフォールバック（`unknown → 5`）を提供している。`KnowledgeRelation::parse()` + `priority()` への単純置換ではこのフォールバックが失われる。そのため、`relation_priority()` 関数自体は互換ラッパーとして残し、内部実装のみを `KnowledgeRelation` に委譲する形とする。

```rust
// src/cli/before_change.rs — 互換ラッパーとして残す
fn relation_priority(s: &str) -> u8 {
    KnowledgeRelation::parse(s).map_or(5, |r| r.priority())
}
```

これにより、既知のrelation値は `KnowledgeRelation::priority()` の一元定義を使用しつつ、未知値に対してはフォールバック優先度5を返す安全な動作を維持する。

### 4.5 query_knowledge_graph() の変更

```rust
fn query_knowledge_graph(ctx: &SearchContext, issue_numbers: &[String]) -> Vec<SuggestKgDoc> {
    if issue_numbers.is_empty() {
        return Vec::new();
    }

    let db_path = ctx.symbol_db_path();
    if !db_path.exists() {
        return Vec::new();
    }

    // SymbolStore::open() はループ外で1回だけ実行する（DB接続コスト削減）
    let store = match SymbolStore::open(&db_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[suggest] knowledge graph open failed: {e}");
            return Vec::new();
        }
    };

    let mut all_docs = Vec::new();
    for issue_num in issue_numbers {
        // find_documents_by_issue() の呼び出し
        // 個別Issueのエラー時はそのIssueをスキップし、他のIssueの処理を継続する
        match store.find_documents_by_issue(issue_num) {
            Ok(entries) => {
                // IssueDocumentEntry → SuggestKgDoc への変換
                for entry in entries {
                    all_docs.push(SuggestKgDoc {
                        issue_number: issue_num.clone(),
                        file_path: entry.file_path,
                        relation: entry.relation,
                        doc_subtype: entry.doc_subtype,
                    });
                }
            }
            Err(e) => {
                eprintln!("[suggest] knowledge graph query failed for issue {issue_num}: {e}");
                // エラー時はこのIssueをスキップして次のIssueを処理
                continue;
            }
        }
    }
    all_docs
}
```

**SF-3: 部分失敗時の方針**:
- **全Issue失敗**: `query_knowledge_graph()` が空の `Vec` を返す。suggestコマンドはKGなし（ナレッジグラフステップを生成せず）で処理を継続する。BM25検索・セマンティック検索の結果のみで提案を生成する。
- **一部Issue失敗**: 成功したIssueの文書のみを採用し、失敗したIssueはスキップする。失敗したIssueについては `eprintln!` で警告を出力するが、コマンド全体のエラーとはしない。

### 4.6 prepend_knowledge_steps() の変更

`prepend_knowledge_steps()` の引数型を `&[KnowledgeDocResult]` から `&[SuggestKgDoc]` に変更する。

**変更理由（MF-1/SF-1対応）**: `KnowledgeDocResult` には `title: Option<String>` フィールドが必要だが `SuggestKgDoc` はこれを持たない（suggestではtitle不要のため）。`KnowledgeDocResult` への変換時に `title: None` を埋める中間変換コードを挟むよりも、`prepend_knowledge_steps()` が直接 `&[SuggestKgDoc]` を受け取る方がKISSの原則に沿う。

```rust
// prepend_knowledge_steps() のシグネチャ変更
fn prepend_knowledge_steps(
    strategy: &mut Vec<SuggestStep>,
    kg_docs: &[SuggestKgDoc],
    issue_numbers: &[String],
) {
    // SuggestKgDoc のフィールド（issue_number, file_path, relation）を直接参照
    // title は不要（suggestのステップ生成では file_path と relation のみ使用するため）
    // ...
}
```

**既存テスト3件の修正**: `test_prepend_knowledge_steps_with_docs`、`test_prepend_knowledge_steps_empty`、`test_prepend_knowledge_steps_multiple_issues` のテストデータを `KnowledgeDocResult` から `SuggestKgDoc` に変更する。`SuggestKgDoc` は `suggest.rs` 内で定義されるローカル構造体のため、テストも同モジュール内で完結する。変更コストは小さい。

**NH-1: SuggestKgDocにtitleを持たせない理由**: suggestコマンドのステップ生成では、参照すべきファイルパスとリレーション種別のみが必要であり、ドキュメントのタイトルは出力に含めない。titleを保持するとfind_documents_by_issue()の返却値からの追加取得が必要になり、不要な複雑性が生じる。

### 4.7 処理フロー

```
run_suggest()
  → ステップ1-4: 入力バリデーション・インデックス解決・リソースオープン・Issue番号抽出（既存）
  → ステップ5: query_knowledge_graph() で各Issueの文書取得
    → SymbolStore::open() （1回のみ）
    → find_documents_by_issue(issue) × N回（エラー時はスキップ）
    → SuggestKgDoc に変換・結合
  → ステップ5.5（新規挿入）: filter_and_limit_kg_docs(docs, &issue_numbers) でフィルタ・制限
    → Modifies/HasProgress除外、StageReview除外
    → relation.priority() でソート
    → issue_numbers順でグルーピング、Issue単位 MAX_KG_DOCS_PER_ISSUE 件に制限
  → ステップ6-9: BM25検索・セマンティック検索・結果統合・戦略生成（既存）
  → ステップ10: prepend_knowledge_steps() でステップ生成（引数型を &[SuggestKgDoc] に変更）
```

**注記**: `filter_and_limit_kg_docs()` はステップ5（KG参照）とステップ6（BM25検索）の間に挿入する。ステップ10の `prepend_knowledge_steps()` は引数型を `&[SuggestKgDoc]` に変更する（MF-1/SF-1対応）。

**MAX_KG_DOCS_PER_ISSUE と同一relation複数文書の扱い**: 同一Issueに同一relationの文書が複数存在する場合（例: HasReview が IssueReview と DesignReview の2文書）、`relation.priority()` でソート後に `take(MAX_KG_DOCS_PER_ISSUE)` で先頭4件を採用する。同一priority内の順序は安定ソートにより `find_documents_by_issue()` の返却順を維持する。

## 5. 影響範囲

### 影響あり
- `src/cli/suggest.rs`: `query_knowledge_graph()` の変更、新規関数 `filter_and_limit_kg_docs()` 追加、新規構造体 `SuggestKgDoc` 追加
- `src/indexer/knowledge.rs`: `KnowledgeRelation::priority()` メソッド追加
- `src/cli/before_change.rs`: 既存の `relation_priority(&str) -> u8` ローカル関数を `KnowledgeRelation::parse().map_or(5, |r| r.priority())` の互換ラッパーに変更（DRY改善、未知値フォールバック維持）

### 影響なし
- `src/cli/issue.rs`: `find_documents_by_issue()` API自体は変更なし
- `src/indexer/symbol_store.rs`: API変更なし

## 6. テスト戦略

### ユニットテスト（suggest.rs内 #[cfg(test)]）

#### 新規テスト

| テスト | 検証内容 |
|--------|---------|
| `test_filter_removes_modifies` | Modifies リレーション除外 |
| `test_filter_removes_has_progress` | HasProgress リレーション除外 |
| `test_filter_keeps_issue_review_removes_stage_review` | IssueReview/DesignReview保持、StageReview除外 |
| `test_filter_keeps_design_and_workplan` | HasDesign/HasWorkplan保持 |
| `test_filter_limits_per_issue` | MAX_KG_DOCS_PER_ISSUE制限 |
| `test_filter_empty_after_all_filtered` | 全件除外時に空Vec |
| `test_kg_relation_priority_order` | 優先度ソート順の検証 |

#### 既存テスト修正（prepend_knowledge_steps引数型変更対応）

`prepend_knowledge_steps()` の引数型を `&[KnowledgeDocResult]` から `&[SuggestKgDoc]` に変更するため（MF-1/SF-1対応）、以下の既存テスト3件のテストデータを `SuggestKgDoc` に修正する。変更はテストデータの構造体型のみであり、検証内容（ステップ生成結果の確認）は変更しない。

| 既存テスト | 影響 |
|-----------|------|
| `test_prepend_knowledge_steps_with_docs` | テストデータを `SuggestKgDoc` に変更 |
| `test_prepend_knowledge_steps_empty` | テストデータを `SuggestKgDoc` に変更 |
| `test_prepend_knowledge_steps_multiple_issues` | テストデータを `SuggestKgDoc` に変更 |

### E2Eテスト（tests/e2e_suggest.rs）

| テスト | 検証内容 |
|--------|---------|
| `test_suggest_kg_limit` | KGステップ数が上限内 |
| `test_suggest_no_modifies_in_output` | modifies文書がcontext出力に含まれない |

## 7. セキュリティ考慮

- パストラバーサル: `file_path` はSQLiteから取得した既存パスを使用。新たなファイルアクセスは発生しない。
- unsafe: 使用なし。
- SQLインジェクション: `find_documents_by_issue()` は既存APIでパラメータバインドを使用しており、Issue番号の直接SQL埋め込みは行わない。本変更で新たなSQLクエリは追加しない。
- リスク認識: SymbolStore（SQLite）のファイルパスはユーザーが `--index-path` で指定可能だが、これは既存の設計であり本Issueのスコープ外。悪意あるDBファイルへの差し替えリスクは全コマンド共通の課題として別途対応を検討する。

## 8. 品質基準

| チェック | コマンド | 基準 |
|---------|---------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全パス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |
