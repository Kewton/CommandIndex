# 設計方針書: Issue #150 - ナレッジグラフ review/ ディレクトリ検出

## 1. Issue概要

| 項目 | 値 |
|------|-----|
| Issue番号 | #150 |
| タイトル | ナレッジグラフ: dev-reports/review/ のstage別レビューファイルが未検出 |
| 種別 | 機能追加（既存機能の拡張） |
| 関連Issue | #139 (ナレッジグラフ実装) |

## 2. 設計判断とトレードオフ

### 判断1: DocSubtype の選択

**決定**: 新規 `DocSubtype::StageReview` バリアントを追加する

**代替案と比較**:

| 案 | メリット | デメリット |
|----|---------|-----------|
| A. 既存 `DesignReview` を再利用 | 変更箇所が少ない | 意味が異なるドキュメントを混同 |
| B. 新規 `StageReview` 追加 ✅ | 型安全、将来的にstage別の表示制御が可能 | match arm の追加が必要（4箇所） |
| C. stage別に個別バリアント追加 | 最も細かい粒度 | 過剰設計、バリアント爆発 |

**理由**: Stage別レビューファイルは既存の `IssueReview`（issue-review/summary-report.md）や `DesignReview`（multi-stage-design-review/summary-report.md）とはファイル配置・命名規則が異なる。独立したバリアントにすることで、将来のフィルタリングや表示カスタマイズが容易になる。ただし `display_label()` は「レビュー」を返し、既存レビューと同カテゴリに分類する。

### 判断2: 正規表現パターン

**決定**: `^dev-reports/review/\d{4}-\d{2}-\d{2}-issue(\d+)-[^/]*\.md$`

**理由**:
- 日付プレフィックス（YYYY-MM-DD）は生成ツール（Claude Code スキル）により常に付与される
- `issue{NUMBER}` でIssue番号を抽出（ファイル名ベース。既存パターンはディレクトリベース。ハイフンなし形式は生成ツールの命名規則に準拠）
- `[^/]*\.md$` でパスセパレータを除外しつつ、後続の説明部分とstage番号を許容（防御的プログラミング）

**具体的マッチ例**:
- ✅ `dev-reports/review/2026-02-18-issue299-security-review-stage4.md`
- ✅ `dev-reports/review/2026-03-20-issue525-consistency-review-stage2.md`
- ✅ `dev-reports/review/2024-01-01-issue1234-long-description-with-hyphens.md`
- ❌ `dev-reports/review/no-date-issue100.md`（日付プレフィックスなし）
- ❌ `dev-reports/review/2024-01-01-no-issue-number.md`（issue番号なし）

### 判断3: KnowledgeRelation

**決定**: 既存の `HasReview` を使用

**理由**: Stage別レビューはレビュードキュメントの一種。既存の `IssueReview`, `DesignReview`, `ProgressReport` も全て `HasReview` を使用しており、一貫性が保たれる。

### 判断4: DocSubtype::parse() メソッド追加

**決定**: `DocSubtype::parse(s: &str) -> Option<Self>` メソッドを追加し、`symbol_store.rs` のデシリアライズを委譲する

**理由**: `KnowledgeRelation::parse()` と同じパターン。文字列逆変換ロジックを `DocSubtype` に集約することで DRY 原則を遵守し、新規バリアント追加時の更新漏れを防ぐ。

## 3. 影響範囲

### 変更対象ファイル

| ファイル | 変更内容 | リスク |
|---------|---------|--------|
| `src/indexer/knowledge.rs` | `DocSubtype::StageReview` 追加、`as_str()` 追加、`parse()` メソッド追加、`build_pattern_rules()` にパターン追加 | 低（enum拡張） |
| `src/cli/issue.rs` | `display_label()` に `StageReview` arm 追加、`sort_order()` に `StageReview` arm 追加 | 低（match arm追加） |
| `src/indexer/symbol_store.rs` | `find_documents_by_issue()` のデシリアライズを `DocSubtype::parse()` に委譲（L908-918） | 低（リファクタリング） |
| `tests/e2e_issue.rs` | `setup_issue_test_data()` に `StageReview` エントリ追加、count assertion 更新 | 低（テスト拡張） |

### 変更不要ファイル

| ファイル | 理由 |
|---------|------|
| `src/cli/issue.rs` `grouped()` | `StageReview` は `display_label()` で「レビュー」を返すため、既存カテゴリに自動分類 |
| `src/indexer/knowledge.rs` `KnowledgeRelation` | 既存の `HasReview` を使用 |
| `src/main.rs` | CLI引数変更なし |

### 実装順序の制約

> **重要**: `DocSubtype::StageReview` バリアント追加は、`knowledge.rs`（`as_str()`, `parse()`）、`issue.rs`（`display_label()`, `sort_order()`）、`symbol_store.rs`（デシリアライズ）の3ファイルを**同一コミット**で変更する必要がある。Rust の網羅的 match により、部分的な変更ではコンパイルエラーが発生する。

## 4. 実装詳細

### 4.1 DocSubtype enum 拡張

```rust
// src/indexer/knowledge.rs
pub enum DocSubtype {
    DesignPolicy,
    WorkPlan,
    IssueReview,
    DesignReview,
    ProgressReport,
    StageReview,    // 新規追加
}

impl DocSubtype {
    pub fn as_str(&self) -> &'static str {
        match self {
            // ... 既存 ...
            Self::StageReview => "stage_review",
        }
    }

    /// Parse a doc subtype string from the database. Returns `None` for unknown values.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "design_policy" => Some(Self::DesignPolicy),
            "work_plan" => Some(Self::WorkPlan),
            "issue_review" => Some(Self::IssueReview),
            "design_review" => Some(Self::DesignReview),
            "progress_report" => Some(Self::ProgressReport),
            "stage_review" => Some(Self::StageReview),
            _ => None,
        }
    }
}
```

### 4.2 パターンルール追加

```rust
// src/indexer/knowledge.rs - build_pattern_rules()
// Note: issue{N} uses no hyphen separator, matching the review tool's output naming convention
PatternRule {
    regex: regex::Regex::new(
        r"^dev-reports/review/\d{4}-\d{2}-\d{2}-issue(\d+)-[^/]*\.md$"
    ).expect("invalid regex"),
    doc_subtype: DocSubtype::StageReview,
    relation: KnowledgeRelation::HasReview,
},
```

### 4.3 display_label / sort_order 更新

```rust
// src/cli/issue.rs
fn display_label(subtype: &DocSubtype) -> &'static str {
    match subtype {
        DocSubtype::DesignPolicy => "設計",
        DocSubtype::IssueReview | DocSubtype::DesignReview | DocSubtype::StageReview => "レビュー",
        DocSubtype::WorkPlan => "作業計画",
        DocSubtype::ProgressReport => "進捗レポート",
    }
}

fn sort_order(entry: &IssueDocumentEntry) -> (u8, u8) {
    // ... relation_order 既存 ...
    let subtype_order = match entry.doc_subtype {
        // ... 既存 1-5 ...
        DocSubtype::StageReview => 6,
    };
    (relation_order, subtype_order)
}
```

### 4.4 メタデータデシリアライズ更新

```rust
// src/indexer/symbol_store.rs - find_documents_by_issue() L908-918
// Before: 手動 match で文字列 → DocSubtype 変換
// After: DocSubtype::parse() に委譲
let doc_subtype = DocSubtype::parse(subtype_str).ok_or_else(|| {
    SymbolStoreError::InvalidEmbedding {
        reason: format!("Unknown doc_subtype: {subtype_str}"),
    }
})?;
```

## 5. テスト計画

### 新規テスト（knowledge.rs）

| テスト名 | 検証内容 |
|---------|---------|
| `test_parse_stage_review` | 基本パターンのパース検証: `dev-reports/review/2026-02-18-issue299-security-review-stage4.md` |
| `test_parse_stage_review_multi_digit_issue` | 複数桁Issue番号: `dev-reports/review/2024-01-01-issue1234-test.md` |
| `test_parse_stage_review_hyphenated_desc` | ハイフン含む説明文: `dev-reports/review/2024-01-01-issue42-long-desc-with-hyphens-stage1.md` |
| `test_parse_stage_review_non_matching` | 除外パターン: 日付なし、issue番号なし、.json ファイル等 |
| `test_doc_subtype_parse` | `DocSubtype::parse()` の全バリアント + unknown |

### 既存テスト更新

| テスト名 | 更新内容 |
|---------|---------|
| `test_doc_subtype_as_str` | `StageReview` のアサーション追加 |
| `test_scan_dev_reports_with_temp_dir` | `dev-reports/review/2024-01-15-issue100-design-review-stage1.md` ファイル追加、expected count を 4 に |
| `test_display_label` (issue.rs) | `StageReview` のアサーション追加: `display_label(&DocSubtype::StageReview) == "レビュー"` |
| `test_sort_order` (issue.rs) | `StageReview` のソート順検証追加 |
| `test_grouped` (issue.rs) | `StageReview` エントリ追加、「レビュー」カテゴリに含まれることを検証 |
| E2E: `setup_issue_test_data` (tests/e2e_issue.rs) | `StageReview` エントリ追加、path count assertion を 5→6 に更新 |

## 6. セキュリティ設計

| 脅威 | 対策 | 優先度 |
|------|------|--------|
| パストラバーサル | 既存の `strip_prefix(base_dir)` + 正規表現のアンカー（`^...$`）+ `[^/]*` でパスセパレータ除外 | 低（既存対策+防御強化） |
| 正規表現DoS | パターンは固定文字列で構築、ユーザー入力を含まない | 該当なし |
| シンボリックリンク | `scan_dev_reports()` で `follow_links(false)` 設定済み | 該当なし |

## 7. 品質基準

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |
