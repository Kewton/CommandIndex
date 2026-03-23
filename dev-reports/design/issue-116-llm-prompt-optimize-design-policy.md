# 設計方針書: Issue #116 - search/related/impact のLLMプロンプト最適化

## 1. Issue概要

| 項目 | 内容 |
|------|------|
| Issue番号 | #116 |
| タイトル | search/related/impact のJSON出力のLLMプロンプト最適化 |
| 優先度 | 高 |
| 前提 | Issue #104 で `--format llm` 基本実装済み |

## 2. 対象レイヤーと責務

本Issueの変更の主座は **Output レイヤー** だが、オプション受け渡しのため CLI 層にもシグネチャ変更が波及する。

| レイヤー | モジュール | 変更有無 | 責務 |
|---------|-----------|---------|------|
| **CLI** | `src/main.rs` | 軽微 | `LlmFormatOptions` の組み立て・受け渡し |
| **CLI** | `src/cli/search.rs` | 軽微 | `LlmFormatOptions` の受け渡し |
| **CLI** | `src/cli/impact.rs` | 軽微 | `LlmFormatOptions` の受け取り、`format_impact_results` 呼び出し変更 |
| **Output** | `src/output/mod.rs` | 中 | `LlmFormatOptions` 構造体定義、format_results / format_impact_results のシグネチャ変更 |
| **Output** | `src/output/llm.rs` | **主** | format_llm / format_impact_llm の最適化 |
| **Output** | `src/output/json.rs`, `human.rs`, `path.rs` | なし | - |
| **Parser/Indexer/Search** | - | なし | - |

## 3. 設計判断とトレードオフ

### 判断1: LlmFormatOptions 構造体と適用範囲

**決定**: `LlmFormatOptions` を導入し、**search と impact のディスパッチ関数のみ**に `llm_options: &LlmFormatOptions` 引数を追加する。workspace/semantic/symbol/diff/suggest/related のディスパッチ関数は変更しない。

```rust
/// LLM出力フォーマットの表示制御オプション。
/// search/impact のLLM出力専用。検索・インデックス・データ構造には影響しない。
#[derive(Debug, Clone, Copy)]
pub struct LlmFormatOptions {
    /// bodyのトランケーション行数（None = 無制限 = 現行動作）
    pub max_body_lines: Option<usize>,
}

impl Default for LlmFormatOptions {
    fn default() -> Self {
        Self {
            max_body_lines: None,
        }
    }
}
```

**適用範囲の限定（ISP準拠）**:

| ディスパッチ関数 | llm_options引数 | 理由 |
|-----------------|----------------|------|
| `format_results` | **追加** | search bodyトランケーション・重複除去で使用 |
| `format_impact_results` | **追加** | impacted_by省略で使用（将来拡張を含む） |
| `format_related_results` | 変更なし | LLM出力はファイルパスのみでオプション不要 |
| `format_workspace_results` | 変更なし | 本Issue対象外 |
| `format_semantic_results` | 変更なし | 本Issue対象外 |
| `format_symbol_results` | 変更なし | 本Issue対象外 |
| `format_diff_results` | 変更なし | 本Issue対象外 |
| `format_suggest_results` | 変更なし | 本Issue対象外 |

```rust
// format_results のシグネチャ変更
pub fn format_results(
    results: &[SearchResult],
    format: OutputFormat,
    writer: &mut dyn Write,
    llm_options: &LlmFormatOptions,
) -> Result<(), OutputError> {
    match format {
        OutputFormat::Human => human::format_human(results, writer, SnippetConfig::default()),
        OutputFormat::Json => json::format_json(results, writer),
        OutputFormat::Path => path::format_path(results, writer),
        OutputFormat::Llm => llm::format_llm(results, writer, llm_options),
    }
}

// format_impact_results のシグネチャ変更
pub fn format_impact_results(
    result: &ImpactResult,
    format: OutputFormat,
    writer: &mut dyn Write,
    llm_options: &LlmFormatOptions,
) -> Result<(), OutputError> {
    match format {
        OutputFormat::Human => human::format_impact_human(result, writer),
        OutputFormat::Json => json::format_impact_json(result, writer),
        OutputFormat::Path => path::format_impact_path(result, writer),
        OutputFormat::Llm => llm::format_impact_llm(result, writer, llm_options),
    }
}
```

**トレードオフ**:
- (+) ISP準拠: 未使用引数を不要なAPIに配らない
- (+) 変更範囲が最小限（2関数のみシグネチャ変更）
- (+) 将来の拡張は必要になった時点で他関数にも追加可能
- (-) search と impact で一貫したインターフェースだが、他関数とは非対称

### 判断2: --snippet-lines のLLM適用ルール

**決定**: CLI引数の `--snippet-lines` (Option型) をそのまま `LlmFormatOptions.max_body_lines` に渡す。None の場合はトランケーションしない（現行動作維持）。設定ファイルの `search.snippet_lines` は LLM には適用しない。

```rust
// main.rs での LlmFormatOptions 組み立て
let llm_options = LlmFormatOptions {
    max_body_lines: snippet_lines.map(|v| usize::try_from(v).unwrap_or(usize::MAX)),
};
```

**`--snippet-lines 0` の扱い**: LLMフォーマットでは `0` は「無制限」（トランケーションしない）として扱う。既存のHumanフォーマットでの `0` の意味と一致させる。

```rust
// format_llm内でのトランケーション判定
if let Some(max_lines) = llm_options.max_body_lines {
    if max_lines > 0 {
        // トランケーション実行
    }
    // max_lines == 0 の場合はトランケーションしない（無制限）
}
```

### 判断3: 重複除去の実装

**決定**: `format_llm` 関数から呼び出す前処理関数として実装。フォーマッタの描画ロジックとは分離する。

```rust
/// LLM出力用の前処理: 重複除去（最初の出現を採用）
fn dedup_results<'a>(items: &[&'a SearchResult]) -> Vec<&'a SearchResult> {
    let mut seen = HashSet::new();
    items
        .iter()
        .filter(|item| {
            let key = (&item.path, &item.heading, &item.body);
            seen.insert(key)
        })
        .copied()
        .collect()
}
```

**重複判定キー**: `path + heading + body` 完全一致。scoreは判定に含めない。最初の出現を採用。

### 判断4: impacted_by 省略の実装

**決定**: `format_impact_llm` の表示ロジックのみで実装。データ構造（`ImpactResult`）は変更しない。

```rust
const IMPACTED_BY_DISPLAY_LIMIT: usize = 3;

fn format_impacted_by(impacted_by: &[String]) -> String {
    if impacted_by.len() <= IMPACTED_BY_DISPLAY_LIMIT {
        impacted_by.iter().map(|s| strip_control_chars(s)).collect::<Vec<_>>().join(", ")
    } else {
        let shown: Vec<String> = impacted_by[..IMPACTED_BY_DISPLAY_LIMIT]
            .iter().map(|s| strip_control_chars(s)).collect();
        format!("{}, ... (+{} more)", shown.join(", "), impacted_by.len() - IMPACTED_BY_DISPLAY_LIMIT)
    }
}
```

### 判断5: トランケーション時の安全処理

**決定**: bodyを行数制限で切り詰め時に安全処理を行う。

```rust
fn truncate_body_for_llm(body: &str, max_lines: usize) -> (String, bool) {
    if max_lines == 0 {
        return (body.to_string(), false); // 0は無制限
    }
    let lines: Vec<&str> = body.lines().collect();
    if lines.len() <= max_lines {
        return (body.to_string(), false);
    }
    let truncated = lines[..max_lines].join("\n");
    (truncated, true)
}
```

トランケーション後の安全処理:
- `... (truncated)` マーカー出力
- コードファイルの場合、コードフェンスの閉じ処理

## 4. エラーハンドリング方針

- **新規エラー型は追加しない**: フォーマット最適化は純粋関数として扱い、既存の `OutputError::Io` / `Json` の範囲に留める
- **型変換の安全処理**: `usize::try_from(v).unwrap_or(usize::MAX)` でオーバーフローをクランプ。panicしない
- **不正値の扱い**: `--snippet-lines 0` は「無制限」として処理（既存動作と一致）。負値はclapのusize型で自動拒否

## 5. データフロー

### search --format llm --snippet-lines N のデータフロー

```
CLI args (--snippet-lines N, --format llm)
  ↓
main.rs: LlmFormatOptions { max_body_lines: Some(N) } を構築
  ↓
cli/search.rs: run() → format_results(results, Llm, writer, &llm_options)
  ↓
output/mod.rs: → llm::format_llm(results, writer, &llm_options)
  ↓
output/llm.rs: format_llm()
  1. group_by_path(results)
  2. 各グループ内で dedup_results() → 重複除去
  3. truncate_body_for_llm() + write_body() → body出力
  4. トークン推定コメント出力
```

### impact --format llm のデータフロー

```
CLI args (--format llm)
  ↓
main.rs: LlmFormatOptions::default() を構築
  ↓
output/mod.rs: format_impact_results → llm::format_impact_llm(result, writer, &llm_options)
  ↓
output/llm.rs: format_impact_llm()
  1. impacted_by が IMPACTED_BY_DISPLAY_LIMIT 超 → 省略表記
  2. トークン推定コメント出力
```

## 6. 影響範囲

### 変更対象ファイル

| ファイル | 変更内容 | 影響度 |
|---------|---------|--------|
| `src/output/mod.rs` | `LlmFormatOptions` 構造体定義。`format_results` と `format_impact_results` のシグネチャに `llm_options` 追加 | 中 |
| `src/output/llm.rs` | `format_llm`: 重複除去・トランケーション追加。`format_impact_llm`: impacted_by省略。両関数に `llm_options` 引数追加 | 高 |
| `src/cli/search.rs` | run関数に `llm_options` 引数追加、`format_results` 呼び出し変更 | 低 |
| `src/cli/impact.rs` | `run_impact` 関数に `llm_options` 引数追加、`format_impact_results` 呼び出し変更 | 低 |
| `src/main.rs` | search/impact サブコマンドでの `LlmFormatOptions` 組み立て・`run_impact` への受け渡し | 低 |

### 変更なし

| ファイル | 理由 |
|---------|------|
| `src/output/llm.rs` の workspace/semantic/symbol/diff/related 関数 | 本Issue対象外。引数追加もしない |
| `format_related_results`, `format_workspace_results` 等 | シグネチャ変更なし |
| `src/output/json.rs`, `human.rs`, `path.rs` | 一切変更なし |
| `src/cli/context.rs`, `workspace.rs` | 一切変更なし |
| `src/cli/search.rs` の `run_related_search` / `run_related_search_from_stdin` | related は LLMオプション非対応。回帰確認のみ |
| `src/cli/help_llm.rs` および CLI help 文 | 公開オプション追加なし。変更なし確認 |
| `ImpactResult`, `SearchResult` 等のデータ構造 | 一切変更なし |
| `format_suggest_results` | 変更なし |

### テスト影響範囲

| テストファイル | 影響 | 推定影響テスト数 |
|--------------|------|----------------|
| `tests/output_format.rs` | `format_to_string` ヘルパー（format_results経由）に `&LlmFormatOptions::default()` 追加。`format_impact_to_string` も同様 | ~20個 |
| `tests/cli_args.rs` | 変更不要の見込み | 0個 |
| `tests/e2e_impact.rs` | impact コマンド経由のE2Eテスト。format_impact_results シグネチャ変更の影響確認 | ~4個 |
| `src/cli/impact.rs` 内テスト | run_impact 呼び出しテストがあれば変更 | 要確認 |

### パフォーマンス影響
- 検索・関連性計算・DBアクセスには影響なし
- 出力整形段階の O(n) 相当の追加処理のみ（重複除去・トランケーション）
- 重複除去の HashSet は文字列本体を再確保せず参照キー `(&str, &str, &str)` のみ保持

### 依存関係
- 新規crate追加なし。既存の output/config/CLI 層の内部変更のみ

## 7. テスト戦略

### 新規テストケース

| テスト名 | 検証対象 |
|---------|---------|
| `test_format_llm_truncation` | max_body_lines指定時のbodyトランケーション |
| `test_format_llm_truncation_marker` | truncatedマーカーの付与 |
| `test_format_llm_truncation_code_fence_close` | コードファイルでトランケーション時のフェンス閉じ |
| `test_format_llm_no_truncation_default` | デフォルト（max_body_lines=None）で現行動作維持 |
| `test_format_llm_truncation_zero_means_unlimited` | max_body_lines=Some(0)でトランケーションしない |
| `test_format_llm_dedup` | 同一path+heading+bodyの重複除去 |
| `test_format_llm_no_dedup_different_body` | bodyが異なる場合は除去しない |
| `test_format_impact_llm_impacted_by_truncation` | IMPACTED_BY_DISPLAY_LIMIT超の省略表記 |
| `test_format_impact_llm_impacted_by_no_truncation` | IMPACTED_BY_DISPLAY_LIMIT以下は全表示 |
| `test_e2e_impact_llm_format` | impact --format llm のE2Eテスト |

### 既存テスト更新方針

`format_to_string` / `format_impact_to_string` ヘルパーに `&LlmFormatOptions::default()` 引数を追加。`format_related_to_string` 等は変更不要（シグネチャ変更なし）。既存アサーション自体は変更不要（デフォルトオプションで現行動作維持）。

### 移行基準

既存テストヘルパー以外の呼び出し箇所は、`&LlmFormatOptions::default()` を引数に追加するだけで移行可能。

## 8. セキュリティ設計

| 脅威 | 対策 | 優先度 |
|------|------|--------|
| 制御文字インジェクション | 既存の `strip_control_chars()` を維持 | 中 |
| 大量入力メモリ消費 | 重複除去HashSetは参照のみ保持、O(n)メモリ | 低 |
| LLMプロンプトインジェクション | heading/pathのMarkdown構文文字は既存挙動。本Issueでは新たなMarkdownエスケープは追加しないが、既存より悪化させない。`strip_control_chars()` 維持、トランケーション後もコードフェンスを必ず閉じることを保証 | 低 |

## 9. 品質基準

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |
| 後方互換性 | デフォルトオプションでの出力比較 | 現行と同一 |
