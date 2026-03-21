# 設計方針書 - Issue #52 Context Pack 生成

## 1. 概要

| 項目 | 内容 |
|------|------|
| Issue | #52 [Feature] Context Pack 生成（AI向け文脈パッケージ出力） |
| 目的 | 指定ファイルの関連文脈をAI向け構造化JSONとして出力 |
| 依存 | #50 --related 検索オプション（実装済み） |

## 2. アーキテクチャ概要

### レイヤー構成と変更対象

```
src/
├── main.rs              # [変更] Commands enum に Context variant 追加
├── cli/
│   ├── mod.rs           # [変更] pub mod context; 追加
│   ├── context.rs       # [新規] Context Pack 生成ロジック
│   └── search.rs        # [参照] run_related_search() パターン踏襲
├── search/
│   └── related.rs       # [参照] RelatedSearchEngine.find_related() 再利用
├── output/
│   ├── mod.rs           # [変更] ContextEntry, ContextPack 型定義追加
│   └── context_pack.rs  # [新規] format_context_pack() 実装
└── indexer/
    ├── reader.rs         # [参照] search_by_exact_path() でスニペット取得
    └── symbol_store.rs   # [参照] ImportInfo.imported_names 利用
```

### データフロー

```
CLI引数 (files, max_files, max_tokens)
  ↓
cli/context.rs: run_context()
  ↓
search/related.rs: find_related() × N files
  ↓
マージ (union, スコア最大値, target除外)
  ↓
indexer/reader.rs: search_by_exact_path() でスニペット取得
indexer/symbol_store.rs: find_imports_by_source() でシンボル取得
  ↓
ContextPack 構築
  ↓
output/context_pack.rs: format_context_pack() → stdout JSON
```

## 3. 型設計

### 3.1 CLI引数定義 (src/main.rs)

```rust
/// AI向け関連文脈パッケージを生成
Context {
    /// 対象ファイルパス（複数指定可）
    #[arg(required = true)]
    files: Vec<String>,

    /// 出力する関連ファイルの最大数
    #[arg(long, default_value = "20")]
    max_files: usize,

    /// トークン数概算の上限
    #[arg(long)]
    max_tokens: Option<usize>,
}
```

### 3.2 Context Pack データ構造 (src/output/mod.rs)

```rust
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ContextPack {
    pub target_files: Vec<String>,
    pub context: Vec<ContextEntry>,
    pub summary: ContextSummary,
}

#[derive(Debug, Serialize)]
pub struct ContextEntry {
    pub path: String,
    pub relation: String,
    pub score: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heading: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbols: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct ContextSummary {
    pub total_related: usize,
    pub included: usize,
    pub estimated_tokens: usize,
}
```

### 3.3 Relation型マッピング

**注意**: `RelationType::TagMatch` はデータ付きvariant（`{ matched_tags: Vec<String> }`）であり、パターンマッチ時に考慮が必要。

```rust
fn relation_to_string(relation_types: &[RelationType]) -> String {
    // 最もスコア重みの高い RelationType を選択
    // MarkdownLink → "linked"
    // ImportDependency → "import_dependency"
    // TagMatch { .. } → "tag_match"
    // PathSimilarity → "path_similarity"
    // DirectoryProximity → "directory_proximity"
}
```

優先度: MarkdownLink > ImportDependency > TagMatch > PathSimilarity > DirectoryProximity

## 4. モジュール設計

### 4.1 cli/context.rs

**戻り値型**: `cli::search::SearchError` を再利用（cli/search.rs に定義済み）

```rust
pub fn run_context(
    files: &[String],
    max_files: usize,
    max_tokens: Option<usize>,
) -> Result<(), SearchError>
```

**責務分離**: テスタビリティのため、内部を2フェーズに分割

```rust
// Phase 1: 関連ファイル収集・マージ
fn collect_related_context(...) -> Result<Vec<RelatedSearchResult>, SearchError>

// Phase 2: ContextPack構築（エンリッチ + 制限適用）
fn build_context_pack(...) -> Result<ContextPack, SearchError>
```

**処理フロー:**

1. **入力検証**: 各ファイルパスの空文字列チェック・長さ上限（1024文字）・ファイル数上限（100件）
2. **インデックスオープン**: IndexReaderWrapper + SymbolStore
3. **関連検索**: 各ファイルに対して `engine.find_related()` を実行
4. **マージ**: union マージ、重複はスコア最大値を採用、target_files は除外
5. **制限適用**: `--max-files` でトリム、`--max-tokens` でバイト数/4 概算
6. **エンリッチ**: 各エントリにスニペット・見出し・シンボルを付加（取得失敗時は None）
7. **出力**: `format_context_pack()` で stdout に JSON 出力

### 4.2 マージロジック

```rust
fn merge_related_results(
    results_per_file: Vec<Vec<RelatedSearchResult>>,
    target_files: &[String],
) -> Vec<RelatedSearchResult>
```

- `HashMap<String, (f32, Vec<RelationType>)>` でスコア最大値マージ
- target_files に含まれるパスを除外
- スコア降順ソート

### 4.3 スニペット取得

```rust
fn enrich_entry(
    path: &str,
    relation_types: &[RelationType],
    reader: &IndexReaderWrapper,
    store: &SymbolStore,
) -> ContextEntry
```

| relation | heading | snippet | symbols |
|----------|---------|---------|---------|
| MarkdownLink | 先頭セクションのheading | 先頭セクションのbody（`truncate_body(body, 10, 500)`） | None |
| ImportDependency | None | None | imported_names をパース（カンマ+スペース区切り: `names.split(", ")` ） |
| TagMatch { .. } | 先頭セクションのheading | 先頭セクションのbody（`truncate_body(body, 10, 500)`） | None |
| PathSimilarity | 先頭セクションのheading | None | None |
| DirectoryProximity | 先頭セクションのheading | None | None |

### 4.4 トークン概算

```rust
fn estimate_tokens(text: &str) -> usize {
    text.len() / 4  // バイト数 / 4（英語テキスト基準の概算）
}
```

`--max-tokens` 適用時: 各エントリのスニペットのトークン数を累計し、上限超過時に打ち切り。

### 4.5 出力関数 (output/context_pack.rs)

```rust
pub fn format_context_pack(
    pack: &ContextPack,
    writer: &mut dyn Write,
) -> Result<(), OutputError> {
    serde_json::to_writer_pretty(writer, pack)?;  // From<serde_json::Error> → OutputError::Json
    Ok(())
}
```

**既存 JSONL 出力との差異:**
- JSONL（1行1JSON）ではなく、単一の pretty-printed JSON オブジェクト
- `--format` オプションは不要（常にJSON）

## 5. エラーハンドリング

既存の `SearchError` enum を再利用。`cli/context.rs` から返すエラーは `cli/search.rs` と同じパターン:

- `SearchError::IndexNotFound` - インデックスが存在しない
- `SearchError::InvalidArgument` - ファイルパスが無効、またはファイル数上限超過
- `SearchError::Reader` - tantivy 読み取りエラー
- `SearchError::SymbolStore` - SQLite エラー
- `SearchError::SymbolDbNotFound` - シンボルDB未作成
- `SearchError::SchemaVersionMismatch` - スキーマバージョン不一致
- `SearchError::RelatedSearch` - 関連検索エラー
- `SearchError::Output` - JSON出力エラー

## 6. セキュリティ設計

| 脅威 | 対策 |
|------|------|
| パストラバーサル | `find_related()` 内部で `normalize_path()` が呼ばれる。read-only操作かつインデックス済みファイルのみ対象のため実害は限定的。注意: `normalize_path()` は `..` を単にフィルタ除去するのみで正確なパス解決ではない（別Issue対応） |
| 大量ファイル指定 | `--max-files` デフォルト20（出力制限）。入力ファイル数は `run_context()` 内で100件上限チェック |
| 巨大スニペット | `truncate_body(body, 10, 500)` で本文を切り詰め（既存関数を再利用）。スニペットに `strip_control_chars()` も適用 |
| 機密ファイル露出 | インデックス構築時に `.cmindexignore` で `.env`, `*.pem`, `*.key` 等の機密ファイルを除外する運用を推奨 |
| unsafe | 使用しない |

## 7. 設計判断とトレードオフ

### 判断1: 新サブコマンド vs search オプション拡張

**決定**: 新サブコマンド `context` として実装

**理由**:
- `search --related` は関連ファイル一覧（human/json/path 出力対応）
- `context` はAI向け構造化パッケージ（常にJSON、スニペット・シンボル含む）
- 責務が異なるため、別サブコマンドが適切

### 判断2: 単一JSON vs JSONL

**決定**: 単一JSONオブジェクト（`serde_json::to_writer_pretty`）

**理由**:
- Context Pack は1回の呼び出しで1つの構造化ドキュメントを返す
- AI向け入力としては単一JSONが扱いやすい
- パイプ連携でもJSONパース1回で完結

### 判断3: トークン概算精度

**決定**: バイト数 / 4（簡易概算）

**理由**:
- 正確なトークン計算には tiktoken 等の外部依存が必要
- 概算で十分（`estimated_tokens` フィールド名で概算であることを示す）
- 日本語テキストでは過少評価の可能性があるが、初期実装としては許容

### 判断4: RelatedSearchEngine の直接利用

**決定**: `find_related()` を直接利用し、追加情報は別途取得

**理由**:
- RelatedSearchEngine への破壊的変更を回避
- Context Pack 固有の処理（スニペット取得、マージ）は cli/context.rs に集約
- 単一責任原則の維持

## 8. 影響範囲

### 変更ファイル

| ファイル | 変更内容 | 影響度 |
|---------|---------|--------|
| src/main.rs | Commands::Context 追加、match アーム追加 | 低 |
| src/cli/mod.rs | `pub mod context;` 追加 | 低 |
| src/cli/context.rs | 新規作成 | - |
| src/output/mod.rs | ContextPack/ContextEntry/ContextSummary 型追加 | 低 |
| src/output/context_pack.rs | 新規作成（format_context_pack） | - |
| tests/e2e_context_pack.rs | 新規作成（E2Eテスト） | - |

### 影響なしのファイル

- src/cli/search.rs（変更なし）
- src/search/related.rs（変更なし）
- src/indexer/（変更なし）
- src/parser/（変更なし）
- src/output/json.rs（変更なし）
- src/output/human.rs（変更なし）
- src/output/path.rs（変更なし）
- src/lib.rs（変更なし - cli/mod.rs, output/mod.rs のサブモジュールとして追加されるため）
- 既存テスト（変更なし、ただし tests/cli_args.rs に context の help 検証追加を推奨）

## 9. テスト戦略

### E2Eテスト (tests/e2e_context_pack.rs)

既存の `e2e_related_search.rs` のパターンに準拠:

1. **context_pack_outputs_valid_json** - JSON出力が有効であることを検証
2. **context_pack_includes_target_files** - target_files フィールドの正確性
3. **context_pack_includes_related_context** - context 配列に関連ファイルが含まれる
4. **context_pack_max_files_limits_output** - --max-files 制限の検証
5. **context_pack_max_tokens_limits_output** - --max-tokens 制限の検証
6. **context_pack_multiple_files** - 複数ファイル指定のマージ検証
7. **context_pack_no_self_reference** - target_files がcontext結果に含まれない
8. **context_pack_relation_types** - 全5種類のrelation型の検証
9. **context_pack_summary_fields** - summary フィールドの正確性

### テストデータ

```
temp_dir/
├── docs/
│   ├── a.md     # tags: rust, search / [[b.md]] リンク
│   └── b.md     # tags: rust / [link](../src/c.ts) リンク
└── src/
    ├── c.ts     # import { func } from './d'
    └── d.ts     # export function func()
```

## 10. 品質基準

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |
