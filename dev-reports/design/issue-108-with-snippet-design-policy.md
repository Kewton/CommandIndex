# 設計方針書: Issue #108 - impact/related にコードスニペット付きモード (--with-snippet)

## 1. 概要

`impact` / `search --related` の出力にコードスニペットを付加する `--with-snippet` オプションを追加する。tantivy インデックスの body フィールドから軽量にスニペットを取得し、LLM がファイルを再読み取りする必要をなくす。

## 2. システムアーキテクチャ概要

### 変更対象レイヤー

```
[CLI Layer]
  main.rs                  ← --with-snippet, --snippet-lines, --snippet-chars オプション追加
  cli/search.rs            ← run_related_search() にスニペット取得処理追加
  cli/impact.rs            ← run_impact() にスニペット取得処理追加
  cli/changed_since.rs     ← run_impact() シグネチャ変更への追従
  cli/snippet_helper.rs    ← 【新設】スニペット取得共通関数（データ取得層）
  cli/help_llm.rs          ← key_options 更新

[Output Layer]
  output/mod.rs       ← RelatedSearchResult, ImpactFileResult に snippet フィールド追加
  output/human.rs     ← format_related_human, format_impact_human にスニペット表示追加
  output/json.rs      ← format_related_json, format_impact_json にスニペット出力追加
  output/path.rs      ← 変更なし（スニペット無視）

[Search Layer]
  変更なし（RelatedSearchEngine はスコアと関連タイプのみ返す既存仕様を維持）

[Indexer Layer]
  変更なし（search_by_exact_path() を利用するのみ）
```

### データフロー

```
[--with-snippet 指定時]

1. CLI がオプションをパース → SnippetOptions { enabled, config }
2. run_impact() / run_related_search() が関連検索を実行
3. limit 適用後、snippet_options.enabled かつ !matches!(format, OutputFormat::Path) の場合のみスニペット取得
4. cli::snippet_helper::enrich_with_snippets() が各結果の file_path で tantivy を検索
5. 結果構造体の snippet フィールドに Some("...") or Some("") を設定
6. フォーマッタが snippet を出力
```

## 3. 詳細設計

### 3.1 スニペット取得共通モジュール（新設）

**配置**: `src/cli/snippet_helper.rs`

**理由**:
- スニペット取得は `IndexReaderWrapper` を使ったデータアクセス処理であり、出力整形（output 層）の責務ではない
- `cli` 層に配置することで、データ取得の責務を正しいレイヤーに置く（SRP）
- `truncate_body()` / `strip_control_chars()` は `output/mod.rs` から `pub(crate)` でアクセス可能

```rust
// src/cli/snippet_helper.rs

use crate::indexer::reader::IndexReaderWrapper;
use crate::output::{truncate_body, strip_control_chars, SnippetConfig};

/// スニペット取得オプション
#[derive(Debug, Clone)]
pub struct SnippetOptions {
    pub enabled: bool,
    pub config: SnippetConfig,
}

impl Default for SnippetOptions {
    fn default() -> Self {
        Self {
            enabled: false,
            config: SnippetConfig::default(),
        }
    }
}

/// ファイルパスから tantivy インデックスのスニペットを取得する。
/// 取得失敗時は空文字列を返す（エラーで停止しない）。
pub(crate) fn fetch_snippet(
    reader: &IndexReaderWrapper,
    path: &str,
    config: SnippetConfig,
) -> String {
    match reader.search_by_exact_path(path) {
        Ok(docs) => {
            if let Some(first) = docs.first() {
                if !first.body.is_empty() {
                    let truncated = truncate_body(
                        &first.body,
                        config.lines,
                        config.chars,
                    );
                    let cleaned = strip_control_chars(&truncated);
                    if !cleaned.is_empty() {
                        return cleaned;
                    }
                }
            }
            String::new()
        }
        Err(_) => String::new(),
    }
}

/// ImpactFileResult のスニペットを一括付与する。
/// with_snippet=false または format=Path の場合は何もしない。
pub(crate) fn enrich_impact_with_snippets(
    results: &mut [crate::output::ImpactFileResult],
    reader: &IndexReaderWrapper,
    snippet_options: &SnippetOptions,
    format: crate::output::OutputFormat,
) {
    if !snippet_options.enabled || matches!(format, crate::output::OutputFormat::Path) {
        return;
    }
    for file in results.iter_mut() {
        file.snippet = Some(fetch_snippet(reader, &file.file_path, snippet_options.config));
    }
}

/// RelatedSearchResult のスニペットを一括付与する。
pub(crate) fn enrich_related_with_snippets(
    results: &mut [crate::output::RelatedSearchResult],
    reader: &IndexReaderWrapper,
    snippet_options: &SnippetOptions,
    format: crate::output::OutputFormat,
) {
    if !snippet_options.enabled || matches!(format, crate::output::OutputFormat::Path) {
        return;
    }
    for result in results.iter_mut() {
        result.snippet = Some(fetch_snippet(reader, &result.file_path, snippet_options.config));
    }
}
```

**設計ポイント**:
- `SnippetOptions` 構造体で `enabled` と `config` をまとめ、API の意味を明確化
- `enrich_*_with_snippets()` で条件判定とループを共通化（DRY）
- `matches!` マクロで OutputFormat を比較（PartialEq derive 不要）
- `pub(crate)` で crate 内のみに公開

### 3.2 構造体拡張

#### RelatedSearchResult

```rust
// src/output/mod.rs
#[derive(Debug, Clone)]
pub struct RelatedSearchResult {
    pub file_path: String,
    pub score: f32,
    pub relation_types: Vec<RelationType>,
    pub snippet: Option<String>,  // ← 追加
}
```

**注意**: RelatedSearchResult は Serialize を derive していない。JSON 出力は `serde_json::json!` マクロで手動構築するため、serde アトリビュートは不要。snippet フィールドの有無はコードで制御する。

**構築箇所の修正が必要**:
- `src/cli/context.rs` の `merge_related_results()` → `snippet: None` 追加
- `src/search/related.rs` 内の構築箇所（あれば） → `snippet: None` 追加

#### ImpactFileResult

```rust
// src/output/mod.rs
#[derive(Debug, Clone, Serialize)]
pub struct ImpactFileResult {
    pub file_path: String,
    pub score: f32,
    pub relation_types: Vec<String>,
    pub impacted_by: Vec<String>,
    pub snippet: Option<String>,  // ← 追加（serde アトリビュートなし）
}
```

**JSON 出力方針**: `format_impact_json()` は `serde_json::json!` マクロで手動構築しているため、snippet フィールドの条件付き追加はコードで制御する。serde アトリビュートは付けない（YAGNI）。

**構築箇所の修正が必要**:
- `src/cli/impact.rs` の `aggregate_impact()` → `snippet: None` 追加
- `tests/output_format.rs` の `make_impact_result()` → `snippet: None` 追加

#### 設計トレードオフ: 構造体に snippet を埋め込む vs 別管理

**選択**: 構造体に `snippet: Option<String>` を埋め込む
**代替案**: `HashMap<String, String>` (path -> snippet) をフォーマッタに別途渡す
**理由**:
- 実装コストが低く、フォーマッタのシグネチャ変更が最小限
- SRP 違反のリスクはあるが、snippet は「検索結果に付随する表示データ」として許容範囲
- HashMap 方式ではフォーマッタ内で path ルックアップが必要になり複雑化する

### 3.3 CLIオプション追加

#### main.rs - Search サブコマンド

```rust
/// Enable snippet output for related search results
#[arg(long)]
with_snippet: bool,
```

**注意**: `requires = "related"` は付けない。`--related-stdin` との併用もあるため、`--with-snippet` が `--related` / `--related-stdin` なしで使われた場合は実行時に無視する（エラーにしない）。

`--snippet-lines` / `--snippet-chars` は既存オプションを再利用。

#### main.rs - Impact サブコマンド

```rust
/// Enable snippet output for impacted files
#[arg(long)]
with_snippet: bool,

/// Number of snippet lines (default: from config or 2)
#[arg(long, value_parser = clap::value_parser!(usize).range(1..))]
snippet_lines: Option<usize>,

/// Number of snippet characters for single-line body (default: from config or 120)
#[arg(long, value_parser = clap::value_parser!(usize).range(1..))]
snippet_chars: Option<usize>,
```

**バリデーション**: `snippet_lines` / `snippet_chars` は 1 以上。0 は許容しない（0=無制限の特殊扱いをしない、KISS）。

#### Impact の config.toml デフォルト値解決

```rust
// main.rs - Impact 分岐内
let snippet_options = SnippetOptions {
    enabled: with_snippet,
    config: SnippetConfig {
        lines: snippet_lines.unwrap_or_else(|| {
            config.as_ref().map_or(2, |c| c.search.snippet_lines)
        }),
        chars: snippet_chars.unwrap_or_else(|| {
            config.as_ref().map_or(120, |c| c.search.snippet_chars)
        }),
    },
};
```

### 3.4 スニペット取得の組み込み

#### run_impact() の変更

```rust
pub fn run_impact(
    files: &[String],
    format: OutputFormat,
    limit: Option<usize>,
    index_path: Option<&Path>,
    snippet_options: SnippetOptions,  // ← 追加（構造体でまとめる）
) -> Result<(), ImpactError> {
    // ... 既存の reader, store, engine 構築 ...

    let mut result = aggregate_impact(&engine, &valid_files, effective_limit)?;

    // limit 適用後にスニペット一括付与（共通関数）
    enrich_impact_with_snippets(&mut result.impacted_files, &reader, &snippet_options, format);

    // 出力
    output::format_impact_results(&result, format, &mut handle)?;
    Ok(())
}
```

#### changed_since.rs への波及対応

```rust
// src/cli/changed_since.rs
run_impact(
    &changed_files,
    format,
    limit,
    index_path,
    SnippetOptions::default(),  // enabled: false
)?;
```

#### run_related_search() の変更

```rust
pub fn run_related_search(
    // ... 既存パラメータ ...
    snippet_options: SnippetOptions,  // ← 追加
) -> Result<(), SearchError> {
    // ... 既存の検索ロジック ...

    // limit 適用後にスニペット一括付与（共通関数）
    enrich_related_with_snippets(&mut results, &reader, &snippet_options, format);

    output::format_related_results(&results, format, &mut handle)?;
    Ok(())
}
```

#### run_related_search_from_stdin() の変更

```rust
pub fn run_related_search_from_stdin(
    limit: usize,
    format: OutputFormat,
    index_path: Option<&Path>,
    snippet_options: SnippetOptions,  // ← 追加
) -> Result<(), SearchError> {
    // ... 既存のロジック ...

    // limit 適用後にスニペット一括付与（共通関数）
    enrich_related_with_snippets(&mut results, &reader, &snippet_options, format);

    output::format_related_results(&results, format, &mut handle)?;
    Ok(())
}
```

### 3.5 出力フォーマッタの変更

#### format_related_json() - JSONL形式

```rust
// snippet が Some の場合のみフィールドを追加
let mut json_value = serde_json::json!({
    "path": result.file_path,
    "score": result.score,
    "relations": relations,
});
if let Some(ref snippet) = result.snippet {
    if let Some(obj) = json_value.as_object_mut() {
        obj.insert(
            "snippet".to_string(),
            serde_json::Value::String(snippet.clone()),
        );
    }
}
```

#### format_impact_json() - 単一JSON（手動構築）

```rust
let mut file_json = serde_json::json!({
    "file_path": f.file_path,
    "score": f.score,
    "relation_types": f.relation_types,
    "impacted_by": f.impacted_by,
});
if let Some(ref snippet) = f.snippet {
    if let Some(obj) = file_json.as_object_mut() {
        obj.insert(
            "snippet".to_string(),
            serde_json::Value::String(snippet.clone()),
        );
    }
}
```

#### format_related_human() / format_impact_human()

```rust
// snippet がある場合、パスの下にインデント表示
// snippet は補足情報なので path/score より弱いトーンで表示する
if let Some(ref snippet) = result.snippet {
    if !snippet.is_empty() {
        for line in snippet.lines() {
            writeln!(writer, "  {}", line.dimmed())?;
        }
    }
}
```

#### path.rs

変更なし。snippet フィールドは無視される。

### 3.6 help-llm 更新

```rust
// search コマンドの key_options に追加
"--with-snippet  Include code snippets in related search results",

// impact コマンドの key_options に追加
"--with-snippet  Include code snippets in impact results",
"--snippet-lines <N>  Number of snippet lines (default: from config or 2)",
"--snippet-chars <N>  Number of snippet characters (default: from config or 120)",
```

### 3.7 AFTER_HELP 更新

```rust
// impact.rs IMPACT_AFTER_HELP に追加
"  # Impact with code snippets (JSON)\n"
"  commandindex impact src/auth.rs --with-snippet --format json\n"

// search.rs に追加
"  # Related files with snippets\n"
"  commandindex search --related src/auth.rs --with-snippet --format json\n"
```

## 4. 設計判断とトレードオフ

### 判断1: スニペット取得の配置を cli 層にする

**選択**: `src/cli/snippet_helper.rs` に配置
**代替案A**: `src/output/snippet.rs` に配置
**代替案B**: `src/cli/context.rs` に統合
**理由**:
- スニペット取得は `IndexReaderWrapper` を使ったデータアクセス処理であり、output 層（表示整形）の責務ではない（SRP）
- `cli` 層に配置することで、データ取得と表示の責務を分離
- `context.rs` に統合すると impact / search が context に依存する不自然な結合が生じる
- `truncate_body()` / `strip_control_chars()` は `pub(crate)` で cli 層からもアクセス可能

### 判断2: 取得失敗時に空文字列を使用

**選択**: `snippet: Some("")`
**代替案**: `snippet: null`
**理由**:
- 空文字列はシンプルで、JSON 利用側（LLM）が処理しやすい
- `--with-snippet` 指定時はフィールドが常に存在するため、JSON スキーマが安定する

### 判断3: 先頭ドキュメントの body を使用する簡易版

**選択**: `search_by_exact_path()` → `docs.first()` → body
**代替案**: 最適セクション選択
**理由**:
- 実装が単純でバグのリスクが低い（KISS）
- context コマンドの enrich_entry() と同じアプローチで実績あり
- 最適セクション選択は将来の改善課題として分離（YAGNI）

### 判断4: limit 適用後にスニペット取得

**選択**: limit でトリム → スニペット取得
**理由**: 不要なファイルの tantivy 検索を回避。limit=20 なら最大20回の search で済む

### 判断5: SnippetConfig のデフォルト値

**選択**: `config.toml` の `[search]` セクションに従う（未設定時: 2行/120文字）
**理由**: 既存の SnippetConfig と統一。ユーザーが config.toml で全体制御可能

### 判断6: context コマンドは変更しない

**選択**: context コマンドのコードは**本 Issue では一切変更しない**。`context.rs` の `enrich_entry()` は既存のまま維持する
**補足**: `merge_related_results()` 内の `RelatedSearchResult` 構築に `snippet: None` を追加する構造体変更のみ。`enrich_entry()` の relation type ごとの snippet/heading 出し分けロジックには手を加えない
**理由**: context コマンドは既に本番利用されており、仕様変更のリスクが高い。将来的に `fetch_snippet()` を context 内部で呼ぶリファクタリングは可能だが、本 Issue のスコープ外

### 判断7: 構造体に snippet を埋め込む

**選択**: `RelatedSearchResult` / `ImpactFileResult` に `snippet: Option<String>` を追加
**代替案**: `HashMap<String, String>` をフォーマッタに別途渡す
**理由**: 実装コストが低く、フォーマッタのシグネチャ変更が最小限

### 判断8: JSON 構築は手動方式を維持、serde アトリビュートは付けない

**選択**: `serde_json::json!` マクロでの手動構築を維持。ImpactFileResult に `skip_serializing_if` は付けない
**理由**: 手動構築パターンを維持する以上、効かない serde アトリビュートはノイズ（YAGNI）

### 判断9: SnippetOptions 構造体でパラメータをグループ化

**選択**: `enabled` と `config` を `SnippetOptions` にまとめて渡す
**代替案**: `with_snippet: bool` と `snippet_config: SnippetConfig` を個別引数で渡す
**理由**: enabled=false のとき config が意味を持たない半端な状態を避ける。将来のオプション追加にも対応しやすい

### 判断10: --with-snippet は実行時に無視する設計（clap requires なし）

**選択**: `--with-snippet` は `--related` / `--related-stdin` なしで使われた場合、実行時に無視する
**代替案**: `requires = "related"` で clap レベルで制約
**理由**: `--related-stdin` との併用を考慮すると、clap の requires では表現しきれない。実行時に `--related` / `--related-stdin` 経路でのみ snippet 取得処理が走るため、自然に無視される

## 5. 影響範囲

### 変更ファイル

| ファイル | 変更内容 | 影響度 |
|---------|---------|--------|
| `src/cli/snippet_helper.rs` | **新設**: スニペット取得共通関数、SnippetOptions 構造体 | 新規 |
| `src/output/mod.rs` | RelatedSearchResult, ImpactFileResult にフィールド追加 | 中 |
| `src/output/human.rs` | format_related_human, format_impact_human にスニペット表示追加 | 中 |
| `src/output/json.rs` | format_related_json, format_impact_json にスニペット出力追加 | 中 |
| `src/cli/impact.rs` | run_impact() にパラメータ追加・スニペット取得処理、AFTER_HELP 更新 | 中 |
| `src/cli/search.rs` | run_related_search(), run_related_search_from_stdin() にパラメータ追加・スニペット取得 | 中 |
| `src/cli/changed_since.rs` | run_impact() シグネチャ変更への追従（SnippetOptions::default()） | 低 |
| `src/cli/context.rs` | merge_related_results() の RelatedSearchResult 構築に snippet: None 追加 | 低 |
| `src/main.rs` | Impact/Search サブコマンドに --with-snippet オプション追加、SnippetOptions 構築 | 低 |
| `src/cli/help_llm.rs` | key_options 更新 | 低 |
| `src/lib.rs` | mod snippet_helper 宣言 | 低 |

### テスト更新

| テストファイル | 変更内容 |
|---------------|---------|
| `tests/output_format.rs` | ImpactFileResult, RelatedSearchResult 構築に `snippet: None` 追加 |
| `tests/e2e_impact.rs` | --with-snippet テスト追加 |
| `tests/e2e_related_search.rs` | --with-snippet テスト追加 |
| `tests/cli_args.rs` | --with-snippet のヘルプ表示検証 |

### 回帰確認

| テストファイル | 確認内容 |
|---------------|---------|
| `tests/e2e_context_pack.rs` | context コマンドの既存仕様が維持されること |
| `tests/e2e_changed_since.rs` | changed_since.rs 修正後、既存出力（JSON/human/path）が変わらないこと。ImpactFileResult 構造変更の波及確認 |
| `tests/e2e_team_workflow.rs` | config.toml 設定優先順位が正しいこと |

## 6. セキュリティ設計

| 脅威 | 対策 | 優先度 |
|------|------|--------|
| パストラバーサル | `search_by_exact_path()` はインデックス内パスのみ検索。ファイルシステム直接アクセスなし | 低リスク |
| メモリ枯渇 | limit（デフォルト20）適用後にスニペット取得。truncate_body() で本文サイズ制限。--snippet-lines/--snippet-chars は 1 以上に制限 | 低リスク |
| unsafe 使用 | なし | - |
| 機密情報漏洩 | **本変更により、従来は path のみだった impact/related の出力にファイル本文の断片が含まれるようになり、情報露出の度合いが一段上がる**。`--with-snippet` は明示的な opt-in のため、意図しない露出は限定的。`.commandindexignore` に機密ファイルを含めることを推奨 | 中リスク（運用対応） |

## 7. 品質基準

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |

## 8. 非機能要件

- 新規 crate 追加なし、既存依存（tantivy, serde, clap）のみで実装
- `--with-snippet` 未指定時は既存のパフォーマンス特性に影響なし
- `OutputFormat::Path` では snippet 取得処理自体をスキップ
