# 設計方針書: Issue #65 Reranking（検索結果の再順位付け）

## 1. 概要

ハイブリッド検索（BM25 + Semantic）の結果に対して、Cross-Encoder方式でクエリとドキュメントの関連度を再評価し、より精度の高い順位付けを行うReranking機能を実装する。

## 2. システムアーキテクチャ概要

### 現行アーキテクチャ

```
CLI (main.rs) → cli/search.rs → indexer/reader.rs (BM25)
                                → search/hybrid.rs (RRF統合)
                                → embedding/ (Semantic)
                                → output/ (human/json/path)
```

### Reranking追加後のアーキテクチャ

```
CLI (main.rs) → cli/search.rs → indexer/reader.rs (BM25)
                                → search/hybrid.rs (RRF統合)
                                → embedding/ (Semantic)
                                → rerank/ (Cross-Encoder再順位付け) ← 新規
                                → output/ (human/json/path)
```

Rerankingは検索フローの**後段処理**として`src/cli/search.rs`の`run()`関数内に挿入される。

## 3. レイヤー構成と責務

| レイヤー | モジュール | 責務 | 変更内容 |
|---------|-----------|------|---------|
| **CLI** | `src/main.rs` (L26-65) | clapサブコマンド定義 | `--rerank`, `--rerank-top` 追加 + destructuring更新 |
| **CLI Logic** | `src/cli/search.rs` (L109-140) | 検索オーケストレーション | rerank呼び出し追加 |
| **Rerank** | `src/rerank/` (新規) | Rerankingロジック | 新規作成 |
| **Search** | `src/search/` | 検索ロジック（RRF統合） | 変更なし |
| **Indexer** | `src/indexer/reader.rs` (L64-73) | SearchResult型定義 | 変更なし |
| **Embedding** | `src/embedding/mod.rs` (L138-141) | Config定義 | `rerank: Option<RerankConfig>` 追加 |
| **Output** | `src/output/` | 出力フォーマット | 変更なし（score上書きで対応） |

## 4. 新モジュール設計: `src/rerank/`

### 4.1 モジュール構成

> **レビュー反映 (Stage1 M3/YAGNI)**: cohere.rs の空スケルトンは作成しない。RerankProviderトレイトが正しく設計されていれば、OCPにより後から追加可能。

```
src/rerank/
├── mod.rs           # RerankProvider トレイト、型定義、RerankConfig、ファクトリ
└── ollama.rs        # OllamaRerankProvider（/api/generate プロンプト方式）
```

### 4.2 トレイト設計

> **レビュー反映 (Stage1 S3/ISP, Stage5 M3/M4, S1/S2)**: トレイトメソッドは `rerank()` のみ。`provider_name()` は削除（Debug traitで代替）。引数は `&[RerankCandidate]` に変更（借用で十分）。エラーハンドリングの責務分担を明確化。

```rust
// src/rerank/mod.rs

/// Reranking候補
pub struct RerankCandidate {
    pub document_text: String,  // heading + "\n" + body (最大4096文字)
    pub original_index: usize,  // 元の順序（安定ソート用）
}

/// Reranking結果
/// 契約:
/// - index は 0..documents.len() の範囲内であること
/// - 同一 index の重複は不正（InvalidResponse エラー）
/// - 未返却の index は「スコア取得失敗」として扱う（呼び出し元で score=0 を割当）
pub struct RerankResult {
    pub index: usize,   // 元候補のインデックス
    pub score: f32,      // Cross-Encoderスコア (0.0-10.0)
}

/// Rerankingエラー
/// 注: EmbeddingError と類似バリアントを持つが、ドメインが異なるため独立enum。
/// HTTP共通ユーティリティは将来的に crate::util に切り出す。
/// 初期実装では embedding::mod.rs のユーティリティを pub(crate) で共有。
pub enum RerankError {
    NetworkError(String),
    ApiError { status: u16, message: String },
    ModelNotFound(String),
    InvalidResponse(String),
    Timeout,
    ConfigError(String),
}

/// Rerankプロバイダートレイト（最小インターフェース）
///
/// エラーハンドリング責務分担:
/// - Provider側: 個別候補のパース失敗はスコア0としてRerankResultに含めて返す。
///   Provider内で吸収可能な障害はResultに含める。
/// - Provider側: 全体タイムアウト超過時は処理済み候補のみをOkで返す。
/// - Provider側: 接続不可・モデル未発見等の致命的エラーはErrで返す。
/// - Orchestrator(try_rerank)側: Errの場合は元結果を返す（Graceful Degradation）。
///   Okの場合はRerankResultからスコアを適用する。
pub trait RerankProvider: Send + Sync {
    fn rerank(
        &self,
        query: &str,
        documents: &[RerankCandidate],
    ) -> Result<Vec<RerankResult>, RerankError>;
}
```

### 4.3 RerankConfig（rerank/mod.rs に定義）

> **レビュー反映 (Stage3 SF-1, Stage5 M1/M2)**: RerankConfigは `src/rerank/mod.rs` に定義。Configからの参照は `crate::rerank::RerankConfig`。将来的にConfigを共通configモジュールに分離することを検討。RerankProviderTypeからCohereバリアントを削除（YAGNI）。

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct RerankConfig {
    #[serde(default = "default_rerank_model")]
    pub model: String,                    // デフォルト: "llama3"
    #[serde(default = "default_top_candidates")]
    pub top_candidates: usize,            // デフォルト: 20
    #[serde(default = "default_rerank_endpoint")]
    pub endpoint: String,                 // デフォルト: "http://localhost:11434"
    pub api_key: Option<String>,
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,                // デフォルト: 30
}
```

> **注**: 初期実装はOllamaのみのため、provider フィールドとRerankProviderType enumを削除。将来プロバイダーを追加する際にenumとファクトリパターンを導入する。

### 4.4 OllamaRerankProvider

```rust
// src/rerank/ollama.rs

pub struct OllamaRerankProvider {
    client: reqwest::blocking::Client,
    model: String,
    endpoint: String,
    timeout_secs: u64,
}

impl OllamaRerankProvider {
    pub fn new(config: &RerankConfig) -> Self { ... }
}

impl RerankProvider for OllamaRerankProvider {
    fn rerank(&self, query: &str, documents: &[RerankCandidate])
        -> Result<Vec<RerankResult>, RerankError> {
        // 1. 全体タイムアウト (Instant::now() + timeout_secs) を設定
        // 2. 各候補について逐次 POST /api/generate
        // 3. レスポンスから数値を抽出 → 0-10にクランプ
        // 4. タイムアウト超過時は処理済み候補のみ返す
        // 5. パース失敗時はスコア0
    }
}
```

**リクエスト/レスポンス構造体:**

```rust
#[derive(Serialize)]
struct OllamaGenerateRequest {
    model: String,
    prompt: String,
    stream: bool,
    options: OllamaGenerateOptions,
}

#[derive(Serialize)]
struct OllamaGenerateOptions {
    temperature: f32,    // 0.0
    num_predict: u32,    // 10
}

#[derive(Deserialize)]
struct OllamaGenerateResponse {
    response: String,
    done: bool,
}
```

> **レビュー反映 (Stage4 SEC-001/プロンプトインジェクション対策)**: プロンプトテンプレートでは document_text をデリミタ（三重バッククォート）で囲み、ユーザー指示部分とドキュメント引用部分を明確に分離する:
> ```
> Given the query: "<query>"
>
> Rate the relevance of the following document on a scale of 0 to 10:
>
> ```
> <document_text>
> ```
>
> Relevance score (0-10):
> ```

## 5. Config構造体の拡張

### 5.1 変更箇所: `src/embedding/mod.rs`

> **注（暫定配置）**: Config は現在 `src/embedding/mod.rs` に定義されているが、rerank 等の他機能の設定も参照するようになるため、将来的には `src/config.rs` 等の共通モジュールへの分離を検討する。初期実装では既存構造を維持し、`embedding/mod.rs` に暫定的に `rerank` フィールドを追加する。

```rust
// 既存 Config (L138-141) の拡張
use crate::rerank::RerankConfig;  // rerank モジュールから参照

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub embedding: Option<EmbeddingConfig>,
    pub rerank: Option<RerankConfig>,  // 追加（暫定配置）
}
```

### 5.2 後方互換性

- `rerank` フィールドは `Option` のため、既存の `config.toml` に `[rerank]` セクションがなくても `None` としてデシリアライズされる
- `RerankConfig` の全フィールドに `#[serde(default)]` または `Option` が設定されているため、空の `[rerank]` セクションでもパースが成功する
- 既存テスト（`test_config_parse_no_embedding_section`, `test_config_parse_full_toml`）に影響なし
- 既存テストに `config.rerank.is_none()` の明示的アサーションを追加

## 6. CLI引数の拡張

### 6.1 変更箇所: `src/main.rs` (L26-65)

```rust
Commands::Search {
    // ... 既存引数 ...

    /// Enable reranking of search results using Cross-Encoder
    #[arg(long, conflicts_with_all = ["symbol", "related", "semantic"])]
    rerank: bool,

    /// Number of top candidates to rerank (requires --rerank)
    #[arg(long, requires = "rerank", default_value_t = 20)]
    rerank_top: usize,
}
```

> **レビュー反映 (Stage3 MF-2)**: `src/main.rs` L144-202 の Commands::Search マッチアームにも `rerank`, `rerank_top` の destructuring を追加し、`run()` 呼び出しに渡す。

### 6.2 排他制約

| フラグ | `--rerank` との関係 |
|--------|-------------------|
| `--symbol` | conflicts_with（異なる結果型） |
| `--related` | conflicts_with（異なる結果型） |
| `--semantic` | conflicts_with（初期実装スコープ外） |
| `--no-semantic` | 共存可（BM25→Rerank） |
| `--rerank-top` | requires（--rerankが必要） |

## 7. 検索フロー変更

### 7.1 変更箇所: `src/cli/search.rs` `run()` (L109-140)

```rust
pub fn run(
    options: &SearchOptions,
    filters: &SearchFilters,
    format: OutputFormat,
    rerank: bool,           // 追加
    rerank_top: usize,      // 追加
) -> Result<(), SearchError> {
    // 1. rerank有効時、候補取得数を調整: max(options.limit, rerank_top)
    //    → SearchOptions.limit を一時的に上書きするか、
    //      try_hybrid_search に渡す limit を調整
    // 2. 既存の検索フロー実行（BM25 / Hybrid / BM25フォールバック）
    // 3. rerank == true の場合:
    //    a. SearchResult → RerankCandidate 変換（build_document_text使用）
    //    b. try_rerank() 実行
    //    c. スコア上書き + 安定ソートで再ソート
    // 4. .take(options.limit) で最終件数に切り詰め
    // 5. format_results() で出力
}
```

> **レビュー反映 (Stage2 S2/S3)**: try_rerank() は run() 内で final_results 確定後に呼ぶ。try_hybrid_search() 内には組み込まない。BM25-only (no_semantic=true) 時も final_results 確定後に適用される。

### 7.2 Reranking統合関数

> **レビュー反映 (Stage1 S2/SRP)**: try_rerank() は独立関数として切り出し、try_hybrid_search() の肥大化を防ぐ。

```rust
fn try_rerank(
    results: Vec<SearchResult>,
    query: &str,
    rerank_top: usize,
    commandindex_dir: &Path,
) -> Vec<SearchResult> {
    // Graceful Degradation パターン（try_hybrid_searchと同様のパターン）:
    // 1. Config読み込み失敗 → eprintln!("[rerank] ...") + 元結果を返す
    // 2. Provider作成失敗 → eprintln!("[rerank] ...") + 元結果を返す
    // 3. rerank実行失敗 → eprintln!("[rerank] ...") + 元結果を返す
    // 4. 部分失敗 → スコア0 + 安定ソート（元順序維持）
    // 5. タイムアウト → 処理済み候補のみで再ソート
    //
    // body が空文字列の候補:
    //   heading のみで document_text を構成（heading も空なら空文字列のまま送信）
}
```

### 7.3 document_text 組み立て

> **レビュー反映 (Stage3 SF-6, Stage4 SEC-005)**: `text[..MAX_DOC_LEN]` はマルチバイトUTF-8文字でパニックする。既存の `truncate_text()` (embedding/mod.rs L184-190) と同じ `.chars().take()` パターンを使用する。

```rust
fn build_document_text(result: &SearchResult) -> String {
    const MAX_DOC_CHARS: usize = 4096;
    let text = if result.heading.is_empty() {
        result.body.clone()
    } else {
        format!("{}\n{}", result.heading, result.body)
    };
    // UTF-8安全な切り詰め（chars().take() を使用）
    text.chars().take(MAX_DOC_CHARS).collect()
}
```

## 8. 設計判断とトレードオフ

### 判断1: Rerankingモジュールの配置場所

- **選択**: `src/rerank/` を新規作成（`src/search/` や `src/embedding/` の下ではなく独立）
- **理由**: Rerankingは検索でもEmbeddingでもない独立した責務。既存の `EmbeddingProvider` トレイトパターンに倣いつつ、モジュールとしては分離する。`src/embedding/` と同格のトップレベルモジュールとして配置することで、外部API呼び出しの責務を明確にする
- **トレードオフ**: `lib.rs` に `pub mod rerank;` の追加が必要
- **代替案**: `src/search/rerank.rs` に配置する案もあるが、search/ は検索ロジック（RRF統合等）の責務であり、外部API呼び出しは embedding/ と同格の位置が適切

### 判断2: `--semantic` 経路の除外

- **選択**: 初期実装では `--semantic` 経路を除外し、`conflicts_with` に設定
- **理由**: `SemanticSearchResult` と `SearchResult` は異なる型。共通候補型への変換は追加の設計・実装コストがかかる
- **トレードオフ**: `--semantic --rerank` が使えない

### 判断3: 逐次実行（非並列）

- **選択**: 候補ごとの逐次HTTP呼び出し
- **理由**: `reqwest::blocking` 前提でasyncランタイムがない。Ollama `/api/generate` はバッチ入力に対応していないため逐次が必要。初期実装の複雑さを抑える
- **トレードオフ**: パフォーマンスが候補数に比例して低下。`top_candidates` デフォルト20で緩和

### 判断4: スコア上書き（元スコア非保持）

- **選択**: `SearchResult.score` をCross-Encoderスコアで上書き
- **理由**: SearchResult構造体にフィールド追加すると出力フォーマッタ全体（human.rs, json.rs, path.rs）に影響。初期実装はシンプルに。デバッグ時はstderrログで元スコアとrerankスコアの対応を出力可能
- **トレードオフ**: rerank前後のスコア比較が不可能

### 判断5: 全体タイムアウト + 部分結果返却

- **選択**: 30秒タイムアウトで処理済み候補のみ返す
- **理由**: 逐次実行で最悪ケースの応答時間を制御。ユーザー体験の劣化を防ぐ
- **トレードオフ**: 全候補のrerankが完了しない場合がある

### 判断6: Cohere実装の先送り

- **選択**: 初期実装はOllamaのみ。Cohere関連の型・ファイルは一切作成しない（YAGNI）
- **理由**: OCPによりRerankProviderトレイトが正しく設計されていれば後から追加可能
- **トレードオフ**: API対応のRerankerが初期リリースでは使えない

### 判断7: エラー型の独立性（DRY vs ドメイン分離）

- **選択**: `RerankError` を `EmbeddingError` とは独立のenumとして定義。ユーティリティ関数は `pub(crate)` で共有
- **理由**: ドメインが異なるため（Embedding生成 vs スコアリング）、エラー型は独立させる。ただしHTTP関連のユーティリティ（`map_reqwest_error`, `map_status_to_error`）は共有してDRY違反を緩和
- **トレードオフ**: バリアント名は類似するが、将来的にドメイン固有のバリアント追加時に独立性が活きる

## 9. 影響範囲

### 変更ファイル一覧

| ファイル | 変更種別 | 変更内容 |
|---------|---------|---------|
| `src/main.rs` (L26-65) | 修正 | `--rerank`, `--rerank-top` 引数追加 |
| `src/main.rs` (L144-203) | 修正 | Commands::Search destructuring + run()呼び出しにrerank引数追加 |
| `src/lib.rs` | 修正 | `pub mod rerank;` 追加 |
| `src/cli/search.rs` (L109-140) | 修正 | run() シグネチャ変更 + try_rerank()統合 |
| `src/embedding/mod.rs` (L138-141) | 修正 | Config に `rerank` フィールド追加 + `use crate::rerank::RerankConfig` |
| `src/rerank/mod.rs` | 新規 | トレイト・型定義・RerankConfig・ファクトリ |
| `src/rerank/ollama.rs` | 新規 | OllamaRerankProvider |
| `tests/cli_args.rs` | 修正 | 新引数のテスト追加 + conflicts_withテスト |

### 影響なしファイル

| ファイル | 理由 |
|---------|------|
| `src/search/hybrid.rs` | RRF統合ロジック変更なし |
| `src/indexer/reader.rs` | SearchResult型変更なし |
| `src/output/` | コード変更なし。ただし `--rerank` 時はJSON出力の `score` の意味が変わる（BM25/RRFスコア → Cross-Encoderスコア0-10）。利用者への注意が必要 |
| `src/parser/` | 解析ロジック無関係 |
| `Cargo.toml` | 新規依存なし（reqwest既存） |
| `src/cli/context.rs` | SearchErrorの使用は返却のみ、match式なし |

## 10. セキュリティ設計

| 脅威 | 対策 | 優先度 |
|------|------|--------|
| プロンプトインジェクション | document_textをデリミタ（三重バッククォート）で囲んでプロンプトに含める。指示部分とドキュメント引用部分を明確に分離 | 中（ローカル環境のため影響限定的） |
| API Key漏洩 | Cohere API Key は環境変数優先。config.toml 平文保存時は起動時にstderr警告を出力 | 中 |
| 外部データ送信 | `--rerank` はopt-in。Ollama はローカル。Cohere利用時は外部送信される旨を注記 | 低 |
| エラーメッセージ漏洩 | APIレスポンスボディをエラーメッセージに含める際は最大500文字にトランケート | 低 |
| 非TLS通信 | endpoint が localhost 以外かつ http:// の場合、stderr に警告を出力 | 低 |
| unsafe使用 | 禁止（CLAUDE.md規約準拠） | 高 |

## 11. 品質基準

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |

## 12. 依存関係

- **Issue #64** (Hybrid Retrieval): 実装済み（3a8888f）
- **新規Cargo依存**: なし（reqwest, serde_json は既存）
- **モジュール依存方向**: embedding → rerank（Config定義のため）、cli/search → rerank（try_rerank呼び出し）。循環依存なし
