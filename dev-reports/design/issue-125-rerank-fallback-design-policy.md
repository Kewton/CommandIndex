# 設計方針書: Issue #125 - rerankフォールバック通知改善

## 1. 概要

| 項目 | 内容 |
|---|---|
| Issue | #125: [BUG] --rerank がモデル未検出時にサイレントフォールバックし結果が変わらない |
| 方針 | Graceful degradation + 明示的フォールバック通知 |
| 影響レイヤー | Rerank層、CLI層、Output層（間接） |
| 新規依存 | なし |

## 2. システムアーキテクチャにおける位置づけ

```
CLI (main.rs)
  └── search::run()
        ├── BM25検索 (tantivy)
        ├── Hybrid検索 (try_hybrid_search)
        ├── Rerank (try_rerank) ← ★本Issue対象
        │     └── RerankProvider::rerank() (ollama.rs)
        └── Output (format_results)
              ├── json.rs
              ├── llm.rs
              ├── human.rs
              └── path.rs
```

### 現在のデータフロー（問題あり）
```
run() → try_rerank() → provider.rerank()
                          ├─ 成功: Vec<RerankResult>
                          ├─ タイムアウト: 部分結果 + eprintln! (stderr)
                          └─ エラー: Err(RerankError)
                       ├─ Ok: rerankされた Vec<SearchResult>
                       └─ Err: eprintln! (stderr) + 元の Vec<SearchResult>
      → format_results() ← rerankステータス情報なし
```

### 改善後のデータフロー
```
run() → try_rerank() → provider.rerank()
                          ├─ 成功: Ok(Vec<RerankResult>)
                          ├─ デッドライン超過: Err(PartialTimeout { results, scored, total })
                          └─ エラー: Err(RerankError)
                       → (Vec<SearchResult>, RerankStatus)
      → RerankStatus に基づいた出力制御
         ├─ Applied: 通常出力
         ├─ AppliedPartially: 結果出力 + 警告
         └─ Skipped: 元結果出力 + 警告
```

**タイムアウト種別**:
- `RerankError::Timeout`: reqwest の HTTP リクエストタイムアウト（個別リクエスト完全失敗）→ `Skipped`
- `RerankError::PartialTimeout`: 全体デッドライン超過（一部スコアリング完了）→ `AppliedPartially`

## 3. 設計判断とトレードオフ

### 判断1: RerankStatus enum vs Result型

**選択: `RerankStatus` enum をタプルで返す**

```rust
/// Rerankの適用状態（rerank実行経路でのみ使用）
/// 表示責務はCLI層、ドメインエラーはrerank層が担う
#[derive(Debug, Clone, PartialEq)]
pub enum RerankStatus {
    /// Rerank が正常に適用された
    Applied,
    /// Rerank が部分的に適用された（デッドライン超過）
    AppliedPartially { warning: String },
    /// Rerank がスキップされた（エラー発生）
    Skipped { reason: String },
}
```

**設計判断**:
- `NotRequested` は削除（YAGNI）。`run()` は既に `rerank` フラグで分岐しており、ステータスは rerank 実行経路でのみ扱う
- `warnings: Vec<String>` は現時点では不要な一般化のため `warning: String` に簡素化
- **配置場所**: `src/cli/search.rs` 内の private enum として定義。CLI の出力制御専用の表現であり、rerank ドメイン層の API には含めない
- 表示責務はCLI層、ドメインエラーはrerank層が担う

| 代替案 | 却下理由 |
|---|---|
| `Result<Vec<SearchResult>, RerankError>` | フォールバック時にも結果を返す必要があるため不適切 |
| `(Vec<SearchResult>, bool)` | 理由文字列・部分適用を表現できない |
| `Result<(Vec<SearchResult>, RerankStatus), SearchError>` | フォールバック自体はエラーではないため過剰 |

### 判断2: provider層のwarnings伝搬

**選択: `rerank()` の戻り値は `Result<Vec<RerankResult>, RerankError>` を維持し、`RerankError` に `PartialTimeout` バリアントを追加**

- trait の戻り値シグネチャを変更しないため、将来の他プロバイダー実装（Cohere等）に影響しない（OCP準拠）
- タイムアウトによる部分結果は `Err(RerankError::PartialTimeout { results, scored, total })` として返す
- `ollama.rs` 内の `eprintln!` を削除し、`PartialTimeout` エラーで伝搬
- `try_rerank()` で `PartialTimeout` を `RerankStatus::AppliedPartially` に変換

```rust
// RerankError に追加するバリアント
pub enum RerankError {
    // ... 既存バリアント ...
    PartialTimeout {
        results: Vec<RerankResult>,
        scored: usize,
        total: usize,
    },
}
```

| 代替案 | 却下理由 |
|---|---|
| `Result<(Vec<RerankResult>, Vec<String>), RerankError>` | trait境界に対する破壊的変更。全provider実装にwarningsタプルを強制する（OCP違反） |
| `RerankResult` に `warnings` フィールド追加 | 結果レベルの情報ではなく操作レベルの情報 |

### 判断3: JSON出力の後方互換性

**選択: `"type"` フィールドによる異種JSONL**

```jsonl
{"type":"metadata","rerank_applied":false,"rerank_reason":"Model not found: llama3"}
{"path":"src/foo.rs","heading":"...","body":"...","tags":[],"score":1.5}
```

| 代替案 | 却下理由 |
|---|---|
| 各行に `rerank_applied` 追加 | 検索結果スキーマが変わり既存パーサー破壊 |
| 別ファイル/別オプション | 過剰な複雑性 |
| stderrのみ | json消費側でプログラム的に検知できない |

**互換性対策**: `"type"` フィールドで行種別を判別。既存パーサーは `"type"` がないか `"result"` の行を処理すれば後方互換。

### 判断4: 出力制御の責務配置

**選択: `run()` 関数内でフォーマット別出力制御**

- `format_results()` のシグネチャは変更しない（影響範囲最小化）
- `run()` 内で `RerankStatus` に基づき、フォーマット出力の前後にメタデータを挿入
- json: `format_json()` 呼び出し前にメタデータ行を `writeln!`
- llm: `format_llm()` 呼び出し前にコメントを `writeln!`
- human/path: `format_human()/format_path()` 呼び出し後に `eprintln!`

| 代替案 | 却下理由 |
|---|---|
| `format_results()` にステータス引数追加 | 全フォーマッタに波及する大きな変更 |
| output層にメタデータformatter追加 | rerank専用のオーバーエンジニアリング |

### 判断5: ヒントメッセージの設計

**選択: CLI層（`try_rerank()` / ヘルパー関数）でヒントを付加（SRP準拠）**

`RerankError::Display` はエラー事実のみを記述する責務とし、ユーザー向けアクションヒントはCLI層で付加する。

```rust
// RerankError::Display はエラー事実のみ（既存スタイル維持）
impl fmt::Display for RerankError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ModelNotFound(model) => write!(f, "Model not found: {model}"),
            Self::NetworkError(msg) => write!(f, "Network error: {msg}"),
            // ... 他のバリアント（既存実装と同じ）
        }
    }
}

// CLI層でヒントを付加するヘルパー関数（src/cli/search.rs）
fn rerank_error_hint(err: &RerankError) -> &'static str {
    match err {
        RerankError::ModelNotFound(_) => "Run `ollama pull <model>` to install, or set rerank.model in config.",
        RerankError::NetworkError(_) => "Is Ollama running? Try `ollama serve`.",
        RerankError::Timeout => "Check Ollama server load.",
        RerankError::ApiError { .. } => "Check Ollama logs.",
        RerankError::InvalidResponse(_) => "Check model compatibility.",
        RerankError::ConfigError(_) => "Check rerank settings in commandindex.toml.",
        RerankError::PartialTimeout { .. } => "Some candidates were not scored due to timeout.",
    }
}
```

| 代替案 | 却下理由 |
|---|---|
| `RerankError::Display` にヒント埋め込み | エラー型と表示層の責務混在（SRP違反）。既存のSearchError::Displayのヒント埋め込みは技術的負債 |

### 判断6: 出力制御ヘルパーの分離

**選択: `emit_rerank_status()` 関数を `search.rs` 内に切り出し**

`run()` 関数の責務膨張を防ぐため、フォーマット別のrerank状態出力を独立関数に分離する。

```rust
fn emit_rerank_status(
    status: &RerankStatus,
    format: OutputFormat,
    writer: &mut dyn Write,
) -> Result<(), OutputError> {
    // json/llm: writerに書き込み、human/path: stderrに出力
}
```

## 4. 詳細設計

### 4.1 src/rerank/mod.rs の変更

#### RerankStatus enum 追加
```rust
/// Rerankの適用状態
#[derive(Debug, Clone)]
pub enum RerankStatus {
    /// Rerank が正常に適用された
    Applied,
    /// Rerank が部分的に適用された（タイムアウト等）
    AppliedPartially { warning: String },
    /// Rerank がスキップされた（エラー発生）
    Skipped { reason: String },
    /// Rerank が要求されていない
    NotRequested,
}
```

#### RerankProvider trait（変更なし）
```rust
// trait の戻り値シグネチャは維持（OCP準拠）
pub trait RerankProvider {
    fn rerank(
        &self,
        query: &str,
        documents: &[RerankCandidate],
    ) -> Result<Vec<RerankResult>, RerankError>;
}
```

#### RerankError に PartialTimeout バリアント追加
```rust
#[derive(Debug)]
pub enum RerankError {
    NetworkError(String),
    ApiError { status: u16, message: String },
    ModelNotFound(String),
    InvalidResponse(String),
    Timeout,
    ConfigError(String),
    /// タイムアウトにより一部の候補のみスコアリング完了
    PartialTimeout {
        results: Vec<RerankResult>,
        scored: usize,
        total: usize,
    },
}
```

#### RerankError の Display（エラー事実のみ、ヒントなし）
```rust
// 既存のDisplay実装スタイルを維持。ヒントはCLI層で付加。
impl fmt::Display for RerankError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // ... 既存バリアント（変更なし） ...
            Self::PartialTimeout { scored, total, .. } => write!(
                f,
                "Timeout reached after scoring {scored} of {total} candidates"
            ),
        }
    }
}
```

### 4.2 src/rerank/ollama.rs の変更

#### rerank() メソッド
- 戻り値の型は `Result<Vec<RerankResult>, RerankError>` のまま維持
- タイムアウト時の `eprintln!` を削除し、`Err(RerankError::PartialTimeout { ... })` を返す

```rust
fn rerank(&self, query: &str, documents: &[RerankCandidate])
    -> Result<Vec<RerankResult>, RerankError>
{
    let deadline = Instant::now() + Duration::from_secs(self.timeout_secs);
    let mut results = Vec::with_capacity(documents.len());

    for (i, doc) in documents.iter().enumerate() {
        if Instant::now() >= deadline {
            // eprintln! を削除し、PartialTimeout エラーで伝搬
            results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
            return Err(RerankError::PartialTimeout {
                results,
                scored: i,
                total: documents.len(),
            });
        }
        // ... 既存のスコアリング処理 ...
    }

    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    Ok(results)
}
```

### 4.3 src/cli/search.rs の変更

#### try_rerank() 関数
```rust
fn try_rerank(
    results: Vec<SearchResult>,
    query: &str,
    top_n: usize,
    config: &Config,
) -> (Vec<SearchResult>, RerankStatus) {
    // Provider生成
    let provider = match crate::rerank::ollama::create_rerank_provider(&config.rerank) {
        Ok(p) => p,
        Err(e) => {
            return (results, RerankStatus::Skipped {
                reason: e.to_string(),
            });
        }
    };

    // 候補変換
    let candidates = /* ... */;

    // Rerank実行
    // 注意: PartialTimeout は通常の Err パターンより先にマッチさせること
    let (rerank_results, status) = match provider.rerank(query, &candidates) {
        Ok(r) => (r, RerankStatus::Applied),
        Err(RerankError::PartialTimeout { results: partial, scored, total }) => {
            // 空結果の場合はSkippedとして扱う（元結果の消失を防ぐ）
            if partial.is_empty() {
                return (results, RerankStatus::Skipped {
                    reason: format!("Timeout: no candidates scored (0 of {total})"),
                });
            }
            (partial, RerankStatus::AppliedPartially {
                warning: format!("Timeout: scored {scored} of {total} candidates"),
            })
        }
        Err(e) => {
            return (results, RerankStatus::Skipped {
                reason: e.to_string(),
            });
        }
    };

    // 結果再構築（既存ロジック）
    let reranked = /* ... rerank_results を元に SearchResult を再構築 ... */;

    (reranked, status)
}
```

#### 4.3.2 run() 関数の変更

```rust
// Reranking適用
let (final_results, rerank_status) = if rerank {
    let (reranked, status) = try_rerank(
        final_results,
        &effective_options.query,
        rerank_top_resolved,
        config,
    );
    (reranked.into_iter().take(original_limit).collect(), status)
} else {
    (final_results, RerankStatus::NotRequested)
};

// ... (トークン予算処理) ...

// stdout prefix（format_results()の前に出力すること）
if let Some(prefix) = build_rerank_stdout_prefix(&rerank_status, format) {
    writeln!(handle, "{prefix}")?;
}

// 通常の出力
match format {
    OutputFormat::Human => { /* ... */ }
    _ => { output::format_results(&final_results, format, &mut handle, llm_options)?; }
}

// stderr警告（human/pathのみ）
if let Some(msg) = build_rerank_stderr_message(&rerank_status, format) {
    eprintln!("{msg}");
}
```

#### 4.3.3 Rerank出力ヘルパー関数群（責務分離）

**設計理由（stdout/stderr分離）**:
- `json`/`llm`: stdoutに機械可読メタデータを埋め込む（パイプライン消費向け）
- `human`/`path`: stdoutは検索結果のみ（stdout契約を維持）、stderrに人間向け警告を出力

```rust
/// stdout向けメタデータを生成する（json/llmのみ）
fn build_rerank_stdout_prefix(status: &RerankStatus, format: OutputFormat) -> Option<String> {
    match (status, format) {
        (RerankStatus::Skipped { reason }, OutputFormat::Json) => {
            let meta = serde_json::json!({
                "type": "metadata", "rerank_status": "skipped", "rerank_warnings": [reason],
            });
            Some(serde_json::to_string(&meta).unwrap())
        }
        (RerankStatus::AppliedPartially { warning }, OutputFormat::Json) => {
            let meta = serde_json::json!({
                "type": "metadata", "rerank_status": "partial", "rerank_warnings": [warning],
            });
            Some(serde_json::to_string(&meta).unwrap())
        }
        (RerankStatus::Skipped { reason }, OutputFormat::Llm) =>
            Some(format!("<!-- rerank skipped: {reason} -->")),
        (RerankStatus::AppliedPartially { warning }, OutputFormat::Llm) =>
            Some(format!("<!-- rerank warning: {warning} -->")),
        _ => None,
    }
}

/// stderr向け警告メッセージを生成する（human/pathのみ）
fn build_rerank_stderr_message(status: &RerankStatus, format: OutputFormat) -> Option<String> {
    match (status, format) {
        (RerankStatus::Skipped { reason }, OutputFormat::Human | OutputFormat::Path) => {
            let hint = rerank_error_hint_from_reason(reason);
            Some(format!("[rerank] Reranking skipped: {reason}\n[rerank] Hint: {hint}"))
        }
        (RerankStatus::AppliedPartially { warning }, OutputFormat::Human | OutputFormat::Path) =>
            Some(format!("[rerank] Warning: {warning}")),
        _ => None,
    }
}
```

## 5. テスト設計

**テスト環境前提**: フォールバックテストはOllama未起動状態で実行する。

### 5.0 単体テスト

| テストケース | 対象 | 検証内容 |
|---|---|---|
| `test_rerank_error_partial_timeout_display` | `RerankError::PartialTimeout` | Display出力が "Timeout reached after scoring N of M candidates" |
| `test_rerank_error_hint_all_variants` | `rerank_error_hint()` | 全RerankErrorバリアントに対応するヒントが存在 |
| `test_emit_rerank_status_json_skipped` | `emit_rerank_status()` | JSON Skipped時にmetadata行が正しい形式 |
| `test_emit_rerank_status_json_partial` | `emit_rerank_status()` | JSON AppliedPartially時にmetadata行が正しい形式 |
| `test_emit_rerank_status_llm_skipped` | `emit_rerank_status()` | LLM Skipped時にコメントが出力される |

### 5.1 E2Eテスト拡充 (tests/e2e_semantic_hybrid.rs)

| テストケース | 検証内容 |
|---|---|
| `test_rerank_fallback_stderr_message` | Skipped時にstderrに`[rerank] Reranking skipped:`とモデル名が含まれる |
| `test_rerank_fallback_json_metadata` | json出力のJSONL先頭行に`"type":"metadata"`で`rerank_status:"skipped"`が含まれる |
| `test_rerank_fallback_llm_comment` | llm出力に`<!-- rerank skipped:`コメントが含まれる |
| `test_rerank_fallback_exit_code_zero` | フォールバック時もexitコード0 |

### 5.2 テストユーティリティ更新 (tests/common/mod.rs)

```rust
/// JSONL出力からメタデータ行を除外して検索結果のみ返す
pub fn parse_search_jsonl(output: &str) -> Vec<serde_json::Value> {
    parse_jsonl(output)
        .into_iter()
        .filter(|v| v.get("type").and_then(|t| t.as_str()) != Some("metadata"))
        .collect()
}

/// JSONL出力からメタデータ行を取得
pub fn parse_jsonl_metadata(output: &str) -> Option<serde_json::Value> {
    parse_jsonl(output)
        .into_iter()
        .find(|v| v.get("type").and_then(|t| t.as_str()) == Some("metadata"))
}
```

### 5.3 既存テスト影響

metadata行はrerankフォールバック時のみ出力されるため、既存テストへの影響は限定的。ただし `parse_jsonl()` を使用している箇所で将来的にrerank付きテストを追加する場合は `parse_search_jsonl()` を使用すること。

## 6. セキュリティ設計

| 脅威 | 対策 | 優先度 |
|---|---|---|
| ApiErrorレスポンスボディの情報漏洩 | JSON metadata / llm コメントの reason にはサニタイズ済みの一般化メッセージのみ含める。ApiError は "API error (HTTP {status})" のみ。サーバーレスポンス詳細はstderrのみ | 高 |
| stdout出力値の制御文字・改行 | reason やモデル名を stdout (JSON/llm) に出す前に制御文字除去・改行正規化を行う | 中 |
| NetworkErrorのURL漏洩 | エラーメッセージからURLクエリパラメータ部分を除去 | 中 |
| エラーメッセージによる情報漏洩 | ヒントメッセージは一般的なアクション提案のみ。内部パスやAPIキー等は含めない | 中 |
| Ollama APIキーの露出 | RerankConfigのapi_keyはエラーメッセージに含めない（Debug実装でマスク済み） | 中 |

## 7. 品質基準

| チェック項目 | コマンド | 基準 |
|---|---|---|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |

## 8. 影響範囲サマリー

| ファイル | 変更種別 | 影響度 |
|---|---|---|
| `src/rerank/mod.rs` | RerankError::PartialTimeout追加 + RerankStatus enum追加（trait戻り値は変更なし） | 高 |
| `src/rerank/ollama.rs` | タイムアウト時PartialTimeout返却 + eprintln!削除 | 高 |
| `src/cli/search.rs` | try_rerank()変更 + run()出力制御追加 | 高 |
| `src/output/json.rs` | 変更なし | - |
| `src/output/llm.rs` | 変更なし | - |
| `tests/e2e_semantic_hybrid.rs` | テスト拡充 | 中 |
| `tests/common/mod.rs` | ユーティリティ追加 | 低 |
