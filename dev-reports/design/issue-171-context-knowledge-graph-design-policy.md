# 設計方針書: Issue #171 - contextコマンドのナレッジグラフ統合改善

## 1. Issue概要

| 項目 | 内容 |
|------|------|
| Issue番号 | #171 |
| タイトル | contextコマンドにナレッジグラフのエッジを統合する |
| 種別 | 改善（既存実装の最適化） |

### 背景
`context` コマンドにはナレッジグラフ統合が既に実装済みだが、以下の改善が必要:
1. 重みの最適化（0.8が低すぎる可能性）
2. スニペット品質の向上
3. リレーション優先度の改善

## 2. システムアーキテクチャ概要

### 対象レイヤーと責務

| レイヤー | 対象モジュール | 変更内容 |
|---------|---------------|---------|
| **Search** | `src/search/related.rs` | 重み定数の調整 |
| **CLI** | `src/cli/context.rs` | スニペット生成改善、リレーション優先度変更 |
| **Output** | `src/output/mod.rs` | RelationType enum に KG メタデータ構造体を追加 |

### データフロー（変更対象部分）

```
context コマンド
  → collect_related_context()
    → RelatedSearchEngine::find_related()
      → score_knowledge_graph() [重み調整 + メタデータ付加]
        → SymbolStore::find_knowledge_related() [メタデータ取得]
  → build_context_pack()
    → enrich_entry() [doc_subtypeベースのスニペット改善]
      → relation_to_string() [優先度変更]
  → JSON出力
```

## 3. 設計判断とトレードオフ

### 判断1: KNOWLEDGE_GRAPH_WEIGHTの調整

| 選択肢 | 説明 | 採否 |
|--------|------|------|
| A: 0.8 → 0.95 | ImportDependency(0.9)より高く、MarkdownLink(1.0)以下 | **採用** |
| B: 0.8 → 1.0 | MarkdownLinkと同等 | 不採用（リンクが明示的な関連のため上位維持） |
| C: KG枠を--max-filesの20%に予約 | 枠確保方式 | 不採用（スコアベースのマージが自然） |

**理由**: KGエッジは設計文書との関連を示す高品質なシグナルであり、ImportDependency(0.9)より重要な場合が多い。ただしMarkdownLink(1.0)は作者が意図的に張ったリンクであり、最高優先度を維持すべき。

**チューニング根拠**: 0.95はMarkdownLink(1.0)以下かつImportDependency(0.9)より高い唯一の0.05刻み値。テストケースで「KGエントリがImportDependencyより上位に来る」ことをアサーションで保証する。

**影響範囲の認識**: この重み変更はcontext以外のsuggest/why/before-changeコマンドにも影響する。全4コマンドで同一の重み優先順位が適切であることを確認済み（KGエッジは全コマンドで設計文脈として重要）。

### 判断2: RelationType::KnowledgeGraph の拡張方式

| 選択肢 | 説明 | 採否 |
|--------|------|------|
| A: フィールド直接追加 | KnowledgeGraph { issue_number, relation, doc_subtype } | 不採用（SRP/OCP違反） |
| B: 専用メタデータ構造体 | KnowledgeGraph(KnowledgeGraphMeta) | **採用** |
| C: 別の経路でメタデータを渡す | HashMap等で並行伝搬 | 不採用（煩雑） |

**理由（レビュー指摘 Stage1-M1 対応）**: RelationType は出力層の enum であり、KG メタデータを直接持たせると検索ドメインの知識が出力層に漏洩する。専用構造体に切り出すことで、KG固有の拡張が構造体内に閉じ、enum本体への変更を最小化できる。

### 判断3: スニペット生成の改善

| 選択肢 | 説明 | 採否 |
|--------|------|------|
| A: doc_subtypeベースのセクション抽出 | doc_subtypeに応じた見出し抽出 | **採用** |
| B: LLMベースの要約生成 | APIコスト・遅延が大きい | 不採用 |
| C: 現状維持（truncate_body） | 改善なし | 不採用 |

**設計**: KnowledgeRelatedResultのdoc_subtypeを活用し、ドキュメント種別に応じた適切なスニペット抽出を行う:
- `design_policy`: 「## 設計判断」セクションから抽出
- `work_plan`: 「## 作業項目」セクションから抽出
- `issue_review` / `design_review`: summary-report.mdの要約を優先
- その他/不明: 現状のtruncate_bodyをフォールバックとして維持

**セキュリティ**: doc_subtypeはDocSubtype enumでバリデーション済み（既知値のみ）。セクション抽出後も500文字上限を維持する（Stage4-S2対応）。

### 判断4: relation_to_string()の優先度変更

| 選択肢 | 説明 | 採否 |
|--------|------|------|
| A: KnowledgeGraphを3番目に移動 | **採用** |
| B: 複数リレーションを配列で返す | 出力フォーマット変更が大きい | 不採用（別Issue） |
| C: 現状維持（最低優先度） | KGラベルが隠れる問題が解消されない | 不採用 |

**変更後の優先度**:
1. MarkdownLink → "linked"
2. ImportDependency → "import_dependency"
3. **KnowledgeGraph → "knowledge_graph"** ← 6番目から移動
4. TagMatch → "tag_match"
5. PathSimilarity → "path_similarity"
6. DirectoryProximity → "directory_proximity"

**影響範囲の認識（Stage1-M2対応）**: この優先度変更は4コマンド共通のrelation_to_string()に影響する。全コマンドでKGの優先度を上げる意図が正しいことを確認済み。各コマンドのテストで順序を検証する。

## 4. 変更詳細設計

### 4.1 KnowledgeGraphMeta 構造体の新設

```rust
// src/output/mod.rs - KG メタデータ専用構造体（Stage1-M1 対応）
#[derive(Debug, Clone, Default)]
pub struct KnowledgeGraphMeta {
    pub issue_number: Option<String>,
    pub relation: Option<String>,      // "has_design", "modifies" etc.
    pub doc_subtype: Option<String>,   // "design_policy", "work_plan" etc.
}

// RelationType enum
pub enum RelationType {
    MarkdownLink,
    ImportDependency,
    TagMatch { matched_tags: Vec<String> },
    PathSimilarity,
    DirectoryProximity,
    KnowledgeGraph(KnowledgeGraphMeta),  // 専用構造体を使用
}
```

### 4.2 is_knowledge_graph() ヘルパーメソッド

```rust
// src/output/mod.rs - パターンマッチの集約（Stage2-M2 対応）
impl RelationType {
    pub fn is_knowledge_graph(&self) -> bool {
        matches!(self, RelationType::KnowledgeGraph(_))
    }

    pub fn kg_meta(&self) -> Option<&KnowledgeGraphMeta> {
        match self {
            RelationType::KnowledgeGraph(meta) => Some(meta),
            _ => None,
        }
    }
}
```

### 4.3 score_knowledge_graph() の変更

```rust
// src/search/related.rs
pub(crate) fn score_knowledge_graph(
    &self,
    target: &str,
    scores: &mut HashMap<String, (f32, Vec<RelationType>)>,
) -> Result<(), RelatedSearchError> {
    let related = self
        .store
        .find_knowledge_related(target)
        .map_err(RelatedSearchError::SymbolStore)?;
    for result in related {
        let meta = KnowledgeGraphMeta {
            issue_number: Some(result.issue_number.clone()),
            relation: Some(result.relation.clone()),
            doc_subtype: result.doc_subtype.as_ref().map(|d| d.to_string()),
        };
        add_relation(
            scores,
            &result.file_path,
            KNOWLEDGE_GRAPH_WEIGHT,  // 0.8 → 0.95
            RelationType::KnowledgeGraph(meta),
        );
    }
    Ok(())
}
```

### 4.4 enrich_entry() のスニペット改善

```rust
// src/cli/context.rs - enrich_entry() 内
let has_knowledge_graph = relation_types
    .iter()
    .any(|r| r.is_knowledge_graph());

// KnowledgeGraph の場合、doc_subtypeに応じたスニペット抽出
if has_knowledge_graph {
    // KGメタデータからdoc_subtypeを取得
    let kg_meta = relation_types.iter()
        .find_map(|r| r.kg_meta());

    if let Some(meta) = kg_meta {
        if let Some(ref subtype) = meta.doc_subtype {
            // doc_subtypeに応じた適切なセクションを抽出
            // フォールバック: 既存のtruncate_body()
            // 上限: 500文字を維持（セキュリティ対応）
        }
    }
}
```

### 4.5 relation_to_string() の優先度変更

```rust
// src/cli/context.rs - relation_to_string()
// 1. MarkdownLink → "linked"
// 2. ImportDependency → "import_dependency"
// 3. KnowledgeGraph → "knowledge_graph"  ← NEW POSITION
// 4. TagMatch → "tag_match"
// 5. PathSimilarity → "path_similarity"
// 6. DirectoryProximity → "directory_proximity"
```

### 4.6 add_relation() の重複処理（Stage2-M3 対応）

```rust
// src/search/related.rs - add_relation()
// KnowledgeGraph(meta) の場合、discriminant は同じだがメタデータが異なる場合がある
// 既存の discriminant チェックは維持（同一ファイルに対する複数KGエントリはスコア加算のみ）
// メタデータは最初のエントリのものを保持（find_knowledge_related のORDER BY で優先度制御済み）
```

## 5. 影響範囲

### 直接影響

| ファイル | 変更内容 | リスク |
|---------|---------|--------|
| `src/output/mod.rs` L122-130 | KnowledgeGraphMeta構造体新設、RelationType変更 | 高（型変更は広範囲影響） |
| `src/search/related.rs` L16 | KNOWLEDGE_GRAPH_WEIGHT 0.8→0.95 | 中（全4コマンドのランキング変動） |
| `src/search/related.rs` L456-474 | KGメタデータをRelationTypeに付加 | 低 |
| `src/cli/context.rs` L283-310 | スニペット生成改善 | 中（出力変更） |
| `src/cli/context.rs` L361-393 | 優先度変更 | 中（ラベル変更） |

### 間接影響（パターンマッチ更新必須）

| ファイル | 変更内容 |
|---------|---------|
| `src/cli/suggest.rs` | `matches!(r, RelationType::KnowledgeGraph)` → `r.is_knowledge_graph()` |
| `src/cli/why.rs` | 同上 |
| テストファイル | パターンマッチ更新 |

### 共有インフラへの影響

| コマンド | 影響 |
|---------|------|
| `context` | 直接対象 |
| `suggest` | 重み変更でランキング変動 + パターンマッチ更新 |
| `why` | 重み変更でランキング変動 + パターンマッチ更新 |
| `before-change` | 重み変更でランキング変動 |

## 6. テスト戦略

### 新規テスト

| テスト種別 | 対象 | 内容 |
|-----------|------|------|
| ユニットテスト | `related.rs` | score_knowledge_graph()のスコア値検証（KG > ImportDep アサーション） |
| ユニットテスト | `context.rs` | KGエントリのdoc_subtypeベーススニペット生成検証 |
| ユニットテスト | `context.rs` | relation_to_string()の新優先度検証 |
| ユニットテスト | `output/mod.rs` | is_knowledge_graph()、kg_meta()のテスト |
| ユニットテスト | `output/mod.rs` | KnowledgeGraphMeta全フィールドNone時の後方互換テスト |
| E2Eテスト | `context` | KGエッジを持つファイルでcontext実行、"knowledge_graph"エントリ確認 |

### リグレッションテスト

| 対象コマンド | 検証内容 |
|-------------|---------|
| `suggest` | 重み変更後の出力が妥当であること |
| `why` | 重み変更後の出力が妥当であること |
| `before-change` | 重み変更後の出力が妥当であること |
| 既存テスト全件 | `cargo test --all` パス |

## 7. セキュリティ設計

| 脅威 | 対策 | 優先度 |
|------|------|--------|
| パストラバーサル | 既存のファイルパス正規化を維持 | 維持 |
| SQLインジェクション | パラメータバインド（既存）を維持 | 維持 |
| doc_subtype不正値 | DocSubtype enumでバリデーション、不明値はフォールバック | 新規（中） |
| スニペット肥大化 | セクション抽出後も500文字上限を維持 | 新規（中） |

## 8. 品質基準

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |
