# 設計方針書: Issue #165 — progress-reportのrelationをhas_progressに変更

## 1. 概要

`KnowledgeRelation` enumに `HasProgress` バリアントを追加し、progress-report.md のrelationを `has_review` から `has_progress` に変更する。

## 2. レイヤー構成と影響範囲

| レイヤー | モジュール | 変更内容 | 影響度 |
|---------|-----------|---------|--------|
| **Indexer** | `src/indexer/knowledge.rs` | enum定義・PatternRule変更（コア変更） | 高 |
| **Indexer** | `src/indexer/symbol_store.rs` | DBパースmatch追加 | 高 |
| **CLI** | `src/cli/issue.rs` | sort_order exhaustive match追加 | 中 |
| **CLI** | `src/cli/before_change.rs` | relation_priority追加 | 中 |
| **Output** | `src/output/human.rs` | display labelフォールバック追加 | 低 |

## 3. 設計判断とトレードオフ

### 判断1: HasProgress を独立バリアントとして追加

- **選択肢A**: HasProgress バリアント追加（採用）
- **選択肢B**: HasReview のままdoc_subtypeで区別（現状維持）
- **判断理由**: relation フィールドだけで意味を判別可能にすることで、JSONコンシューマやDB直接クエリの正確性が向上する。doc_subtype に依存しない設計がクリーン。

### 判断2: sort_order での HasProgress の位置（issue.rs）

```rust
// issue.rs sort_order() — ドキュメント一覧の表示順序（開発フロー順）
KnowledgeRelation::HasDesign => 1,
KnowledgeRelation::HasReview => 2,
KnowledgeRelation::HasWorkplan => 3,
KnowledgeRelation::HasProgress => 4,
KnowledgeRelation::Modifies => 5,
```

- **判断理由**: `issue` コマンドの表示順は開発フロー順（設計→レビュー→作業計画→進捗→変更ファイル）。ユーザーが開発の流れを追えるよう時系列に沿った配置。

### 判断3: before_change.rs の relation_priority での位置

```rust
// before_change.rs relation_priority() — 変更前確認の重要度順（低い値 = 高優先度）
"has_design" => 0,
"has_workplan" => 1,
"has_review" => 2,
"has_progress" => 3,
"modifies" => 4,
```

- **判断理由**: `before-change` コマンドの優先度は「変更前に確認すべき重要度」順。設計→作業計画→レビュー→進捗の順で、進捗レポートは参考情報としてレビューより低い優先度。

> **注意**: issue.rs と before_change.rs で HasReview/HasWorkplan の相対順序が異なるのは意図的。
> - issue.rs: 開発フロー順（Review→Workplan）= ドキュメントの時系列表示
> - before_change.rs: 重要度順（Workplan→Review）= 変更前に参照すべき優先度

### 判断4: マイグレーション戦略

- 既存DBの `has_review` レコード（progress-report）は再インデックス(`ci index`)で自動的に `has_progress` に更新される
- `HasReview` バリアント自体は残るため、IssueReview/DesignReview/StageReview は影響なし
- 明示的なDBマイグレーションスクリプトは不要

## 4. 具体的な変更設計

### 4.1 src/indexer/knowledge.rs

```rust
// enum定義
pub enum KnowledgeRelation {
    HasDesign,
    HasReview,
    HasWorkplan,
    HasProgress,  // 追加
    Modifies,
}

// as_str()
Self::HasProgress => "has_progress",

// parse()
"has_progress" => Some(Self::HasProgress),

// build_pattern_rules() — progress-reportルール
relation: KnowledgeRelation::HasProgress,  // HasReview → HasProgress
```

### 4.2 src/indexer/symbol_store.rs

```rust
// find_documents_by_issue() の relation パース — KnowledgeRelation::parse() に統一（DRY改善）
// Before: ハードコードmatch（has_design/has_review/has_workplan の3パターン）
// After: KnowledgeRelation::parse() を再利用し、None→エラー変換
let relation = crate::indexer::knowledge::KnowledgeRelation::parse(&relation_str)
    .ok_or_else(|| SymbolStoreError::InvalidEmbedding {
        reason: format!("Unknown relation type: {relation_str}"),
    })?;
```

> **DRY改善**: find_knowledge_by_issue() (line 974) は既に parse() を使用。find_documents_by_issue() も統一することで、将来のバリアント追加時に symbol_store.rs の変更が不要になる。
>
> **振る舞い変更の注意**: parse() は `Modifies` も成功として返すが、現在のハードコード match は `modifies` をエラーとする。SQLクエリが `kn_doc.type = 'document'` でフィルタしているため、Modifies ノードは返されず実質的な影響はない。ただし防御的に parse 後に Modifies を除外するフィルタを検討しても良い。

### 4.3 src/cli/issue.rs

```rust
// sort_order()
KnowledgeRelation::HasDesign => 1,
KnowledgeRelation::HasReview => 2,
KnowledgeRelation::HasWorkplan => 3,
KnowledgeRelation::HasProgress => 4,  // 追加
KnowledgeRelation::Modifies => 5,     // 4 → 5
```

### 4.4 src/cli/before_change.rs

```rust
// relation_priority()
"has_design" => 0,
"has_workplan" => 1,
"has_review" => 2,
"has_progress" => 3,  // 追加
"modifies" => 4,      // 3 → 4
_ => 5,               // 4 → 5
```

### 4.5 src/output/human.rs

```rust
// relation_display_label() のフォールバック match ブロック (line 252)
// doc_subtype が Some の場合は subtype.display_label_en() が優先されるが、
// None の場合のフォールバックとして追加
match relation {
    "has_design" => "design",
    "has_review" => "review",
    "has_workplan" => "workplan",
    "has_progress" => "progress",  // 追加
    other => other,
}
```

## 5. テスト変更方針

| ファイル | テスト | 変更内容 |
|---------|-------|---------|
| `src/indexer/knowledge.rs` | `test_parse_progress_report` | `HasReview` → `HasProgress` |
| `src/indexer/knowledge.rs` | `test_knowledge_relation_as_str` | `HasProgress` アサーション追加 |
| `src/indexer/knowledge.rs` | `test_knowledge_relation_parse` | `has_progress` パーステスト追加 |
| `src/indexer/knowledge.rs` | `test_knowledge_relation_display` | `HasProgress` 表示テスト追加 |
| `src/indexer/symbol_store.rs` | `test_find_documents_by_issue_metadata_parsed` | progress-report を `HasProgress` に変更 |
| `src/output/human.rs` | `test_relation_display_label_*` | `has_progress` テストケース追加・更新 |
| `src/cli/before_change.rs` | `test_relation_priority_order` | `has_progress` 優先度アサーション追加 |
| `src/cli/issue.rs` | テストデータ | progress-report の relation を `HasProgress` に変更 |
| `tests/e2e_issue.rs` | テストデータ | progress-report の relation を `HasProgress` に変更 |

## 6. セキュリティ設計

- 本変更はenum値の追加とパターンマッチの拡張のみ
- パストラバーサル、入力検証等のセキュリティリスクなし
- unsafe コード不使用

## 7. 品質基準

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |
