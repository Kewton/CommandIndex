# マルチステージ設計レビュー サマリーレポート

## Issue: #78 [Feature] マルチリポジトリ横断検索
## レビュー日: 2026-03-22

---

## レビュー結果概要

| Stage | 種別 | モデル | Must Fix | Should Fix | Nice to Have |
|-------|------|--------|----------|------------|-------------|
| 1 | 設計原則（1回目） | Opus | 4 | 5 | 4 |
| 2 | 整合性（1回目） | Opus | 4 | 5 | 4 |
| 3 | 影響分析（1回目） | Opus | 5 | 7 | 4 |
| 4 | セキュリティ（1回目） | Opus | 3 | 5 | 4 |
| 5 | 設計原則（2回目） | Opus* | 3 | 5 | 3 |
| 7 | 整合性・影響（2回目） | Opus* | 3 | 5 | 4 |

*Codex接続不可のためOpus代替

### 指摘総数: Must Fix 22件 / Should Fix 32件 / Nice to Have 23件
### 全Must Fix: 反映済み

---

## 主要な設計変更（レビュー反映）

### 1. SearchResult不変 → WorkspaceSearchResult composition（Stage 1 M4）
- SearchResult構造体は変更しない（OCP準拠）
- `WorkspaceSearchResult { repository: String, result: SearchResult }` を新設
- 型はsrc/output/mod.rsに配置（逆依存回避）

### 2. WorkspaceConfig配置分離（Stage 1 M1）
- 設定型: `src/config/workspace.rs`
- オーケストレーション: `src/cli/workspace.rs`

### 3. rrf_merge汎用化（Stage 1 M3）
- `rrf_merge_multiple(ranked_lists: &[Vec<SearchResult>], limit)` を新設
- 既存`rrf_merge`はラッパー化
- キー衝突対策: マージ前にpathにaliasプレフィックス付与

### 4. run()シグネチャ明確化（Stage 2 M1, Stage 5 M2）
```rust
pub fn run(
    ctx: &SearchContext,
    options: &SearchOptions,
    filters: &SearchFilters,
    format: OutputFormat,
    snippet_config: SnippetConfig,
    rerank: bool,
    rerank_top: Option<usize>,
) -> Result<(), SearchError>
```

### 5. セキュリティ強化（Stage 4 M1-M3）
- パス展開: チルダのみ、$記号・バッククォート拒否
- canonicalize後の.commandindex/存在チェック + stderr警告
- シンボリックリンクチェック（clean.rsパターン適用）
- alias/name: ASCII英数字+ハイフン+アンダースコア、上限64文字
- TOMLファイルサイズ上限1MB

### 6. エラー/警告型の分離（Stage 1 S1）
- `WorkspaceConfigError`: 致命的エラー（パース失敗、重複等）
- `WorkspaceWarning`: 検索続行可能な警告（リポ不在、インデックス未作成）
- validate関数からI/O副作用除去、出力はオーケストレーション層で

---

## 成果物一覧

```
dev-reports/issue/78/multi-stage-design-review/
├── stage1-review-context.json   (設計原則レビュー 1回目)
├── stage1-apply-result.json     (Stage 1-4 指摘反映)
├── stage2-review-context.json   (整合性レビュー 1回目)
├── stage3-review-context.json   (影響分析レビュー 1回目)
├── stage4-review-context.json   (セキュリティレビュー 1回目)
├── stage5-review-context.json   (設計原則レビュー 2回目)
├── stage6-apply-result.json     (Stage 5-7 指摘反映)
├── stage7-review-context.json   (整合性・影響レビュー 2回目)
└── summary-report.md            (本レポート)

dev-reports/design/
└── issue-78-design-policy.md    (更新済み設計方針書)
```
