# 設計方針書: Issue #159 - before-changeのlimitをIssue単位に変更

## 1. 概要

### 対象Issue
- **番号**: #159
- **タイトル**: before-changeのデフォルトlimitがIssue単位ではなくドキュメント単位で切られる
- **種別**: バグ修正（セマンティクス変更を伴うbreaking change）

### 目的
`before-change` コマンドの `--limit` オプションをドキュメント単位からIssue単位に変更し、関連する全Issueの設計制約をAIエージェントに提供できるようにする。

## 2. システムアーキテクチャ概要

### 影響レイヤー

| レイヤー | モジュール | 影響 |
|---------|-----------|------|
| **CLI** | `src/main.rs` | ヘルプ文言更新のみ |
| **CLI Logic** | `src/cli/before_change.rs` | **主要変更対象** |
| **CLI Help** | `src/cli/help_llm.rs` | ヘルプ文言更新のみ |
| **Output** | `src/output/mod.rs` | 構造体変更 |
| **Output** | `src/output/human.rs`, `json.rs`, `llm.rs`, `path.rs` | フォーマッタ対応 |
| **Indexer** | `src/indexer/symbol_store.rs` | **変更不要** |
| **Search** | `src/search/related.rs` | **変更不要** |

### データフロー（変更後）

```
git log → Issue番号抽出
    → find_knowledge_by_issue() → 全ドキュメント取得
    → Modifiesフィルタ
    → セマンティックランキング or フォールバックソート
    → ★ Issue単位グルーピング + 代表ドキュメント選出（NEW）
    → ★ Issue単位limit適用（CHANGED）
    → Issue内ドキュメントをフラット化
    → 出力フォーマッタ
```

## 3. 設計判断とトレードオフ

### 判断1: `--limit` のセマンティクスをIssue数に変更

**選択**: ドキュメント数の上限 → Issue数の上限に変更

**理由**:
- before-changeの目的は「変更前に関連する設計制約を確認する」こと
- 設計制約はIssue単位で存在するため、Issue単位のlimitが自然
- ドキュメント単位limitでは、ドキュメント数が多いIssueが枠を独占する

**トレードオフ**:
- breaking change（`--limit 10` の意味が「10ドキュメント」から「10 Issue」に変わる）
- ただし実使用上はIssue数 < ドキュメント数のため、表示される情報量は増加する方向

### 判断2: 各Issueからの代表ドキュメント選出方式

**選択**: 各Issueから最大2件（設計ポリシー1件 + workplan 1件）を優先選出

**理由**:
- 設計ポリシー（has_design）はAIエージェントが最も必要とする情報
- workplanは実装計画の把握に有用
- reviewは設計/workplanがあれば冗長な場合が多い

**代替案と却下理由**:
- 全ドキュメント表示: 情報量が膨大になりLLMのコンテキストを圧迫
- 1件のみ: workplanの情報が欠落する

### 判断3: Issue間ソート戦略

**選択**: 二段階ソート
1. セマンティックランキング使用時: Issue内最大similarityでIssue間をソート
2. 未使用時: issue_number降順（新しいIssue優先）

**理由**:
- セマンティックランキングがある場合、対象ファイルとの関連度が高いIssueを優先
- ない場合、新しいIssueほど現在の設計判断に影響する可能性が高い

### 判断4: BeforeChangeResult構造体の変更方針

**選択**: フラットなfindings配列を維持し、`displayed_issues` フィールドを追加

**理由**:
- JSON出力の後方互換性を最大限維持
- フォーマッタ側でissue_numberグルーピング表示が可能
- ネスト構造への変更は影響範囲が大きく、本Issueのスコープを超える

**注意**: findings配列の最大件数が `limit * MAX_DOCS_PER_ISSUE` に変わるため、findings数に依存する外部ツールへの影響に注意。

### 判断6: total_issues のセマンティクス明確化

**選択**: `total_issues` を「ナレッジドキュメントが1件以上存在するユニークIssue数」に再定義

**現状**: git logから抽出した全Issue数（ドキュメントが0件のIssueも含む）

**理由**:
- `displayed_issues`（limit適用後のIssue数）との差分がユーザーにとって意味のある情報になる
- 「全8 Issue中3 Issueを表示」のようなページネーション情報が提供可能
- git log由来のIssue数はbefore-changeの文脈では情報価値が低い

### 判断5: relation_priority順序の修正

**選択**: `has_design=0 > has_workplan=1 > has_review=2 > modifies=3`

**現状**: `has_design=0 > has_review=1 > has_workplan=2 > modifies=3`

**理由**:
- workplanは具体的な実装計画を含み、reviewより情報価値が高い
- 代表ドキュメント選出で上位2件を取る場合、design + workplanの組み合わせが最適

## 4. 詳細設計

### 4.1 before_change.rs の変更

#### 新規関数: `group_and_limit_by_issue()`

```rust
use std::collections::HashMap;

/// 各Issueから選出する最大ドキュメント数（design + workplan）
const MAX_DOCS_PER_ISSUE: usize = 2;

/// Issue単位でグルーピングし、limitを適用して代表ドキュメントを選出する。
/// 前提条件: 入力findingsはrank_by_max_similarity()またはfindings_without_ranking()で
/// ソート済みであること。Issue間の順序はfindingsの出現順（=ソート順）で決定される。
fn group_and_limit_by_issue(
    findings: Vec<BeforeChangeFinding>,
    limit: usize,
) -> Vec<BeforeChangeFinding> {
    // 1. Issue単位でグルーピング（出現順=ソート順を保持）
    let mut issue_order: Vec<String> = Vec::new();
    let mut issue_groups: HashMap<String, Vec<BeforeChangeFinding>> = HashMap::new();

    for finding in findings {
        if !issue_groups.contains_key(&finding.issue_number) {
            issue_order.push(finding.issue_number.clone());
        }
        issue_groups
            .entry(finding.issue_number.clone())
            .or_default()
            .push(finding);
    }

    // 2. 各Issue内をrelation_priority順にソート
    for docs in issue_groups.values_mut() {
        docs.sort_by(|a, b| {
            relation_priority(&a.relation).cmp(&relation_priority(&b.relation))
        });
    }

    // 3. Issue単位でlimit適用し、各IssueからMAX_DOCS_PER_ISSUE件を選出
    let mut result: Vec<BeforeChangeFinding> = Vec::new();
    for issue_num in issue_order.iter().take(limit) {
        if let Some(docs) = issue_groups.get(issue_num) {
            result.extend(docs.iter().take(MAX_DOCS_PER_ISSUE).cloned());
        }
    }

    result
}
```

#### rank_by_max_similarity() の変更

Issue間ソートをIssue内最大similarityで行うように変更:

```rust
fn rank_by_max_similarity(
    file_embs: &[EmbeddingRecord],
    docs: &[KnowledgeDocResult],
    embedding_store: &EmbeddingStore,
) -> Vec<BeforeChangeFinding> {
    // ... 既存のドキュメント単位similarity計算 ...

    // Issue単位でmax similarityを集約
    let mut issue_max_sim: BTreeMap<String, f32> = BTreeMap::new();
    for finding in &all_findings {
        let sim = finding.similarity.unwrap_or(f32::NEG_INFINITY);
        let entry = issue_max_sim
            .entry(finding.issue_number.clone())
            .or_insert(f32::NEG_INFINITY);
        if sim > *entry {
            *entry = sim;
        }
    }

    // Issue単位でソート（max similarity降順, issue_number, relation_priority）
    // 3段階ソートにより同一Issueのfindingsが必ず隣接する
    all_findings.sort_by(|a, b| {
        let a_issue_sim = issue_max_sim.get(&a.issue_number).unwrap_or(&f32::NEG_INFINITY);
        let b_issue_sim = issue_max_sim.get(&b.issue_number).unwrap_or(&f32::NEG_INFINITY);
        b_issue_sim.partial_cmp(a_issue_sim)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.issue_number.cmp(&b.issue_number))
            .then_with(|| relation_priority(&a.relation).cmp(&relation_priority(&b.relation)))
    });

    all_findings
}
```

#### findings_without_ranking() の変更

Issue番号降順（新しいIssue優先）に変更:

```rust
fn findings_without_ranking(docs: &[KnowledgeDocResult]) -> Vec<BeforeChangeFinding> {
    let mut findings: Vec<BeforeChangeFinding> = /* ... */;

    // 数値比較でissue_number降順（新しいIssue優先）
    findings.sort_by(|a, b| {
        let a_num = a.issue_number.parse::<u64>().unwrap_or(0);
        let b_num = b.issue_number.parse::<u64>().unwrap_or(0);
        b_num.cmp(&a_num)
            .then_with(|| relation_priority(&a.relation).cmp(&relation_priority(&b.relation)))
    });

    findings
}
```

#### run_before_change() のlimit適用変更

```rust
// 7. Apply limit (Issue単位)
let limited_findings = group_and_limit_by_issue(findings, limit);
```

### 4.2 BeforeChangeResult構造体の変更

```rust
#[derive(Debug, Clone, Serialize)]
pub struct BeforeChangeResult {
    pub file_path: String,
    pub findings: Vec<BeforeChangeFinding>,
    pub total_issues: usize,        // CHANGED: ドキュメントが1件以上あるユニークIssue数
    pub displayed_issues: usize,    // NEW: limit適用後のIssue数
    pub has_embeddings: bool,
}
```

**total_issues の算出変更（breaking change）**:
- 旧: `issues.len()`（git logから抽出した全Issue数、ドキュメント0件含む）
- 新: docsから抽出したユニークIssue数（ドキュメントが1件以上あるIssueのみ）

**displayed_issues の表示形式**:
- human: `"showing 3 of 8 issues (limited by --limit)"`（limit適用時のみ表示）
- llm: `"3/8 issues shown"`
- json: `"displayed_issues": 3` フィールドとして追加
- path: 変更なし

**json.rs の実装注意**: json.rsは `serde_json::json!` マクロで手動フィールド列挙しているため、`displayed_issues` の追加漏れに注意。

### 4.3 help_llm.rs の変更

`--limit` の説明を更新:
```rust
// key_options 内
"--limit <N>  Maximum number of issues to show (default: 10)"
```

### 4.4 main.rs のヘルプ変更

```rust
/// Maximum number of issues to show
#[arg(long, default_value = "10", value_parser = clap::value_parser!(usize).range(1..=1000))]
limit: usize,
```

### 4.4 relation_priority() の修正

```rust
fn relation_priority(relation: &str) -> u8 {
    match relation {
        "has_design" => 0,
        "has_workplan" => 1,  // CHANGED: 1 (was 2)
        "has_review" => 2,    // CHANGED: 2 (was 1)
        "modifies" => 3,
        _ => 4,
    }
}
```

## 5. 影響範囲

### 変更対象ファイル

| ファイル | 変更内容 | 難易度 |
|---------|---------|--------|
| `src/cli/before_change.rs` | group_and_limit_by_issue新設、ランキング変更、relation_priority修正 | 高 |
| `src/main.rs` | --limitヘルプ文言更新 | 低 |
| `src/cli/help_llm.rs` | LLM向けヘルプ更新 | 低 |
| `src/output/mod.rs` | BeforeChangeResult.displayed_issues追加 | 低 |
| `src/output/human.rs` | displayed_issues表示対応 | 低 |
| `src/output/json.rs` | displayed_issuesフィールド追加 | 低 |
| `src/output/llm.rs` | displayed_issues表示対応 | 低 |
| `src/output/path.rs` | 影響なし（doc_path抽出のみ） | なし |
| `tests/e2e_before_change.rs` | テスト更新・追加 | 中 |

### 変更不要ファイル
- `src/indexer/symbol_store.rs`: SQLクエリ変更不要
- `src/search/related.rs`: 影響なし
- `src/cli/why.rs`: 参考実装として参照のみ

## 6. テスト戦略

### ユニットテスト（before_change.rs内）

| テスト | 内容 |
|--------|------|
| `test_group_and_limit_by_issue_basic` | 3 Issue × 3ドキュメント、limit=2で2 Issueの代表ドキュメントが返る |
| `test_group_and_limit_by_issue_max_docs` | max_docs_per_issue=2で各Issue最大2件 |
| `test_group_and_limit_by_issue_preserves_order` | ソート順が保持される |
| `test_relation_priority_order` | has_design < has_workplan < has_review < modifies |
| `test_findings_without_ranking_descending` | issue_number数値降順ソート |
| `test_findings_without_ranking_sort_order` | 既存テスト修正: has_design > has_workplan > has_review の順に更新 |

### E2Eテスト（tests/e2e_before_change.rs）

| テスト | 内容 |
|--------|------|
| `before_change_limit_respected` | 更新: limit=1でIssue数が1以下 |
| `before_change_limit_multiple_issues` | 新規: 複数Issue環境でlimit検証 |
| `before_change_displayed_issues_field` | 新規: JSON出力にdisplayed_issuesが含まれる |
| `before_change_limit_zero_rejected` | 新規: --limit 0 がclapバリデーションで拒否される |
| `before_change_limit_exceeds_issues` | 新規: limit > Issue数の場合に全Issue表示される |

## 7. セキュリティ設計

本変更にセキュリティ上の重大なリスクはなし。既存のパストラバーサル対策、入力バリデーションに変更なし。

### 追加バリデーション
- `--limit` に `value_parser = clap::value_parser!(usize).range(1..=1000)` を追加
  - limit=0: 空結果が返る非直感的挙動を防止
  - limit=usize::MAX: findings最大件数 = limit * MAX_DOCS_PER_ISSUE の過大なメモリ使用を防止
  - `--max-commits` の既存パターン `range(1..=10000)` と統一

## 8. 品質基準

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |
