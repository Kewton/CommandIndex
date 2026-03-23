# 設計方針書: Issue #109 - 検索クエリのLLM向けガイド (suggest)

## 1. Issue情報

- **Issue番号**: #109
- **タイトル**: 検索クエリのLLM向けガイド (suggest)
- **優先度**: 低
- **概要**: LLM用途で効果的な検索クエリを提案する `suggest` サブコマンドを追加する

## 2. システムアーキテクチャ概要

CommandIndexは以下のレイヤー構成を持つローカルナレッジ検索CLI:

```
┌──────────────────────────────────────────────┐
│  main.rs (clap CLI / Commands enum)          │
├──────────────────────────────────────────────┤
│  cli/* (サブコマンド実装)                      │
│  ┌─────────┬─────────┬─────────┬───────────┐ │
│  │search.rs│context.rs│impact.rs│suggest.rs │ │
│  │         │         │         │  [NEW]    │ │
│  └─────────┴─────────┴─────────┴───────────┘ │
├──────────────────────────────────────────────┤
│  indexer/reader.rs │ search/related.rs       │
│  (BM25検索API)     │ (RelatedSearchEngine)   │
├──────────────────────────────────────────────┤
│  indexer/           │ output/mod.rs           │
│  symbol_store.rs   │ (フォーマット出力)        │
│  (SymbolStore)     │                          │
├──────────────────────────────────────────────┤
│  tantivy (全文検索) │ SQLite (シンボルストア)   │
└──────────────────────────────────────────────┘
```

## 3. レイヤー構成と責務

| レイヤー | モジュール | suggest における責務 |
|---------|-----------|-------------------|
| **CLI定義** | `src/main.rs` | Commands enum に Suggest バリアント追加 (L19-242付近) |
| **CLIロジック** | `src/cli/suggest.rs` [NEW] | 入力バリデーション、戦略生成、出力 |
| **CLIモジュール宣言** | `src/cli/mod.rs` | `pub mod suggest;` 追加 |
| **検索** | `src/indexer/reader.rs` | BM25検索 (`search()` / `search_with_options()`) |
| **関連検索** | `src/search/related.rs` | `RelatedSearchEngine::find_related()` |
| **関連検索（impact用）** | `src/search/related.rs` | `RelatedSearchEngine::find_related()` を直接利用（impact共通化は行わない） |
| **Embedding判定** | `src/indexer/symbol_store.rs` | `SymbolStore::count_embeddings()` |
| **出力** | `src/output/mod.rs` | SuggestResult 出力構造体、フォーマッタ |
| **LLMヘルプ** | `src/cli/help_llm.rs` | commands セクションに suggest 追加 |

## 4. 技術選定

| カテゴリ | 選定技術 | 選定理由 |
|---------|---------|---------|
| 検索エンジン | tantivy (既存) | BM25全文検索でタスク説明文からファイルを推定 |
| 関連度計算 | RelatedSearchEngine (既存) | 5軸スコアリングによる関連ファイル特定 |
| シンボルストア | SQLite / rusqlite (既存) | Embedding状態判定 |
| 出力フォーマット | serde_json (既存) | JSON出力 |
| 新規crate追加 | **なし** | 既存依存のみで実装可能 |

## 5. 設計パターン

### 5.1 コマンドパターン（既存踏襲）

```rust
// src/main.rs - Commands enum 追加
#[derive(Subcommand)]
#[allow(clippy::large_enum_variant)]
enum Commands {
    // ... 既存13コマンド ...

    /// タスク説明文に基づく検索戦略を提案
    Suggest {
        /// タスク説明文
        #[arg(long = "for")]
        for_task: String,

        /// 出力フォーマット
        #[arg(long, value_enum, default_value = "human")]
        format: OutputFormat,
    },
    // NOTE: index_path は Cli 構造体のグローバルオプション cli.index_path を使用
    //       Suggest バリアント内には含めない（既存の Impact/Diff は技術的負債）
}
```

### 5.2 エラー型パターン（既存踏襲: ImpactError と同パターン）

```rust
// src/cli/suggest.rs
use std::fmt;

#[derive(Debug)]
pub enum SuggestError {
    /// 入力バリデーションエラー（空文字、空白のみ、長さ上限超過）
    InvalidInput(String),
    /// インデックス未構築エラー
    IndexNotFound(String),
    /// シンボルDB未発見
    SymbolDbNotFound(String),
    /// 検索エラー
    Reader(crate::indexer::reader::ReaderError),
    /// 関連検索エラー
    RelatedSearch(crate::search::related::RelatedSearchError),
    /// シンボルストアエラー
    SymbolStore(crate::indexer::symbol_store::SymbolStoreError),
    /// 出力エラー（OutputError: Io + Json の union）
    Output(crate::output::OutputError),
}

impl fmt::Display for SuggestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(msg) => write!(f, "Invalid input: {msg}"),
            Self::IndexNotFound(msg) => write!(f, "Index not found: {msg}"),
            Self::SymbolDbNotFound(msg) => write!(f, "Symbol database not found: {msg}"),
            Self::Reader(e) => write!(f, "Search error: {e}"),
            Self::RelatedSearch(e) => write!(f, "Related search error: {e}"),
            Self::SymbolStore(e) => write!(f, "Symbol store error: {e}"),
            Self::Output(e) => write!(f, "Output error: {e}"),
        }
    }
}

impl std::error::Error for SuggestError {}

// From<> 型変換チェーン（既存パターン踏襲）
impl From<crate::indexer::reader::ReaderError> for SuggestError {
    fn from(e: crate::indexer::reader::ReaderError) -> Self { Self::Reader(e) }
}
impl From<crate::search::related::RelatedSearchError> for SuggestError {
    fn from(e: crate::search::related::RelatedSearchError) -> Self { Self::RelatedSearch(e) }
}
impl From<crate::indexer::symbol_store::SymbolStoreError> for SuggestError {
    fn from(e: crate::indexer::symbol_store::SymbolStoreError) -> Self { Self::SymbolStore(e) }
}
impl From<crate::output::OutputError> for SuggestError {
    fn from(e: crate::output::OutputError) -> Self { Self::Output(e) }
}
```

### 5.3 出力構造体パターン（既存 ImpactResult と同パターン）

```rust
// src/output/mod.rs に追加
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct SuggestStep {
    pub command: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SuggestResult {
    pub query: String,          // 入力クエリ（トレーサビリティ用）
    pub has_embeddings: bool,   // Embedding状態
    pub strategy: Vec<SuggestStep>,
}
// NOTE: step番号はstrategyのインデックスから導出（出力時にenumerate()で付与）
```

### 5.4 フォーマッタパターン（既存 format_impact_results と同パターン: writer + OutputError）

```rust
// src/output/mod.rs に追加
use std::io::Write;

pub fn format_suggest_results(
    result: &SuggestResult,
    format: OutputFormat,
    writer: &mut dyn Write,
) -> Result<(), OutputError> {
    match format {
        OutputFormat::Human => format_suggest_human(result, writer),
        OutputFormat::Json => format_suggest_json(result, writer),
        OutputFormat::Path => {
            // Path形式: コマンド文字列を1行1コマンドで出力（パイプライン連携用途）
            for step in &result.strategy {
                writeln!(writer, "{}", step.command)?;
            }
            Ok(())
        }
    }
}

fn format_suggest_human(result: &SuggestResult, writer: &mut dyn Write) -> Result<(), OutputError> {
    writeln!(writer, "Suggested search strategy:")?;
    for (i, step) in result.strategy.iter().enumerate() {
        writeln!(writer, "{}. {} ({})", i + 1, step.command, step.reason)?;
    }
    Ok(())
}

fn format_suggest_json(result: &SuggestResult, writer: &mut dyn Write) -> Result<(), OutputError> {
    let json = serde_json::to_string_pretty(result)
        .map_err(OutputError::Json)?;
    writeln!(writer, "{json}")?;
    Ok(())
}
```

> **変更点**: writer引数による出力先抽象化、OutputError使用、step番号はenumerate()で付与（手動管理廃止）、Path形式もコマンド一覧出力に対応

### 5.5 メインロジック: 戦略生成パイプライン

```rust
// src/cli/suggest.rs
pub fn run_suggest(
    for_task: &str,
    format: OutputFormat,
    index_path: Option<&Path>,
) -> Result<(), SuggestError> {
    // 1. 入力バリデーション
    let query = validate_input(for_task)?;

    // 2. インデックス解決（SearchContext::new() を直接使用）
    let base_path = std::env::current_dir()
        .map_err(|e| SuggestError::IndexNotFound(e.to_string()))?;
    let ctx = SearchContext::new(&base_path, index_path)
        .map_err(|e| SuggestError::IndexNotFound(e.to_string()))?;

    // 3. リソースオープン（1回のみ）
    let reader = IndexReaderWrapper::open(&ctx.index_dir())?;
    let store = SymbolStore::open(&ctx.symbol_db_path())?;
    // NOTE: SymbolStore は src/indexer/symbol_store.rs にある

    // 4. BM25検索 → ファイル単位dedup
    let entry_files = search_entry_files(&reader, &query)?;

    // 5. 戦略生成
    let strategy = if entry_files.is_empty() {
        build_fallback_strategy()
    } else {
        build_strategy(&reader, &store, &entry_files, &ctx)?
    };

    // 6. 出力
    let stdout = std::io::stdout();
    let mut writer = stdout.lock();
    format_suggest_results(&strategy, format, &mut writer)?;
    Ok(())
}
```

### 5.6 BM25結果のファイル単位正規化

```rust
use std::collections::HashMap;

/// BM25検索結果をファイル単位に正規化・重複排除
fn deduplicate_by_file(results: Vec<SearchResult>, limit: usize) -> Vec<(String, f32)> {
    let mut file_scores: HashMap<String, f32> = HashMap::new();
    for result in results {
        let entry = file_scores.entry(result.path.clone()).or_insert(0.0);
        *entry = entry.max(result.score);
    }
    let mut sorted: Vec<(String, f32)> = file_scores.into_iter().collect();
    sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    sorted.truncate(limit);
    sorted
}
```

### 5.7 コマンド文字列生成のサニタイズ

```rust
/// コマンド引数用のサニタイズ（シェルメタ文字をエスケープ）
fn sanitize_for_command_arg(input: &str) -> String {
    // シェルメタ文字を除去またはエスケープ
    input.chars()
        .filter(|c| !matches!(c, '"' | '\'' | '`' | '$' | ';' | '&' | '|' | '>' | '<' | '\n' | '\r'))
        .collect()
}

/// バイナリ名の定数化（DRY: 一箇所管理）
const BINARY_NAME: &str = "commandindexdev";
```

> **セキュリティ**: ユーザー入力(`--for` 引数)やインデックスから取得したファイルパスをコマンド文字列に埋め込む際は、必ず `sanitize_for_command_arg()` を通す。これにより、LLMが出力コマンドをそのまま実行した場合のシェルインジェクションリスクを軽減する。

### 5.8 Embedding状態によるsemantic gating

```rust
/// Embedding構築済みの場合のみsemantic検索ステップを追加
/// NOTE: SymbolStore は src/indexer/symbol_store.rs にある
fn maybe_add_semantic_step(
    steps: &mut Vec<SuggestStep>,
    store: &SymbolStore,
    query: &str,
) {
    if let Ok(count) = store.count_embeddings() {
        if count > 0 {
            let sanitized = sanitize_for_command_arg(query);
            steps.push(SuggestStep {
                command: format!("{BINARY_NAME} search --semantic \"{sanitized}\" --limit 5"),
                reason: "Semantic search for related documents".to_string(),
            });
        }
    }
}
```

## 6. セキュリティ設計

| 脅威 | 対策 | 優先度 |
|------|------|--------|
| コマンドインジェクション | コマンド文字列に埋め込む全パラメータ（クエリ、ファイルパス）は `sanitize_for_command_arg()` でシェルメタ文字を除去。ユーザー入力はBM25クエリとしてのみ使用し、出力コマンドのファイルパスはインデックスから取得した値のみ使用 | 高 |
| パストラバーサル | 出力に含まれるファイルパスはインデックス内の正規化済みパスのみ。ユーザー入力のパスは含めない | 高 |
| 長大入力によるDoS | `--for` 引数の長さ上限500文字を設定 | 中 |
| 不正文字入力 | `validate_input()` で制御文字（ASCII 0x00-0x1F, 0x7F）を含む入力を拒否 | 中 |
| unsafe使用 | 使用禁止（プロジェクト規約準拠） | 中 |

## 7. 設計判断とトレードオフ

### 判断1: ルールベース vs LLM依存
- **選択**: ルールベース（BM25 → related → impact パイプライン）
- **理由**: 外部API依存なし、ローカル完結、オフライン動作可能、予測可能な動作
- **トレードオフ**: LLMベースに比べてクエリ理解の柔軟性が低いが、確実性と速度を優先

### 判断2: 新規サブコマンド vs help-llm拡張
- **選択**: 独立した `suggest` サブコマンドとして新設
- **理由**: help-llm は静的リファレンス、suggest はインデックス内容に基づく動的推奨。責務が異なる
- **トレードオフ**: サブコマンド数が増えるが、命名規則に準拠し単一責任を維持

### 判断3: RelatedSearchEngine 直接利用（impact共通化は行わない）
- **選択**: `RelatedSearchEngine::find_related()` を suggest から直接呼び出す。impact.rs の `aggregate_impact()` の共通化は行わない
- **理由**: aggregate_impact() は impacted_by 集約・入力ファイル除外等の impact 固有ロジックを含み、suggest が必要とする機能と一致しない。無理な共通化は回帰リスクを増大させる
- **トレードオフ**: suggest 独自の集約ロジックが必要になるが、find_related() のシンプルな呼び出しで十分
- **結果**: impact.rs への変更なし → 回帰リスクゼロ

### 判断4: リソース管理方式
- **選択**: tantivy IndexReader と SymbolStore をコマンド実行ごとに1回だけオープンして使い回す
- **理由**: I/O回数削減、パフォーマンス向上
- **実装**: `run_suggest()` の冒頭でオープンし、各段階の関数に参照渡し

### 判断5: 候補0件時の動作
- **選択**: エラーではなくフォールバック戦略を返す
- **理由**: LLMが利用する場合、エラーよりも「何かしらの手がかり」を返す方がユーザビリティが高い
- **フォールバック内容**: インデックスルートの `context` コマンド等、最小限の探索起点を提示

## 8. 影響範囲

### 変更対象ファイル

| ファイル | 変更種別 | 影響度 | 内容 |
|---------|---------|--------|------|
| `src/main.rs` | 変更 | 低 | Commands enum に Suggest 追加、match アーム追加 |
| `src/cli/mod.rs` | 変更 | 低 | `pub mod suggest;` 1行追加 |
| `src/cli/suggest.rs` | **新規** | - | suggestコマンドのメインロジック |
| `src/cli/impact.rs` | **変更なし** | - | suggest は RelatedSearchEngine を直接利用するため impact への変更不要 |
| `src/output/mod.rs` | 変更 | 低 | SuggestStep, SuggestResult 構造体追加、フォーマッタ追加 |
| `src/cli/help_llm.rs` | 変更 | 低 | build_commands() に suggest の CommandInfo 追加 |
| `tests/cli_args.rs` | 変更 | **中** | ヘルプテスト、サブコマンド一覧テスト、help-llm契約テスト（件数13→14）に suggest 追加 |
| `tests/e2e_suggest.rs` | **新規** | - | E2Eテスト |

### 既存機能への影響

- **既存サブコマンド**: 影響なし（suggest は独立した新規コマンド）
- **impact コマンド**: 変更なし（suggest は RelatedSearchEngine を直接利用するため impact への変更不要）
- **help-llm**: JSON契約変更（コマンド件数 13→14）→ 契約テスト更新必須
- **Cargo.toml**: 変更なし

### 内部モジュール依存の増加

```
src/cli/suggest.rs
  ├── src/indexer/reader.rs     (IndexReaderWrapper, SearchOptions, SearchResult)
  ├── src/search/related.rs     (RelatedSearchEngine)
  ├── src/indexer/symbol_store.rs (SymbolStore::count_embeddings)
  ├── src/output/mod.rs         (SuggestResult, OutputFormat, format_suggest_results)
  └── src/cli/search.rs         (SearchContext, resolve_index_path)
```

## 9. 品質基準

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |
| 回帰確認 | `cargo test --all` | 既存テスト全パス（impact.rs 変更なしのため個別回帰不要） |

## 10. テスト戦略

### 単体テスト（src/cli/suggest.rs 内）
- `validate_input()`: 空文字、空白のみ、500文字超過、制御文字含む入力、正常入力
- `sanitize_for_command_arg()`: シェルメタ文字除去のテスト
- `deduplicate_by_file()`: 重複排除、スコア最大値採用、limit適用
- `build_fallback_strategy()`: 全コマンドが実在サブコマンドであること
- `maybe_add_semantic_step()`: count_embeddings > 0 の分岐

### E2Eテスト（tests/e2e_suggest.rs）
- インデックス構築済み → `suggest --for "..."` → 正常出力
- `--format json` → 有効なJSON
- `--format human` → 人間可読
- インデックス未構築 → エラー
- `--for ""` → バリデーションエラー
- `--for "   "` → バリデーションエラー

### CLIパーステスト（tests/cli_args.rs）
- `suggest --help` が正常終了
- トップレベル `--help` のサブコマンド一覧に `suggest` が含まれる

### 回帰テスト
- `cargo test --all` で既存テスト全パス（impact.rs への変更なしのため個別回帰不要）
- `help_llm_contains_all_subcommands` テスト更新（件数 13→14、expected配列に "suggest" 追加）
- `help_flag_shows_usage` テストに `.stdout(predicate::str::contains("suggest"))` 追加
