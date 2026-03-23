# 設計方針書: Issue #102 LLM向けヘルプ改善

## 1. 概要

| 項目 | 内容 |
|------|------|
| Issue | #102 [Feature] LLM向けヘルプ改善（自律的なCLI理解・活用を可能にする） |
| 目的 | LLMがhelpを実行するだけでCLIの全機能を理解し、適切なコマンドを選択・実行できるようにする |
| 影響範囲 | CLI層（src/main.rs, src/cli/） |
| リスクレベル | 低（既存機能への影響最小限、新機能追加中心） |

## 2. システムアーキテクチャ概要

```
┌─────────────────────────────────────────────┐
│                   CLI層                      │
│  src/main.rs (Cli, Commands enum)           │
│  src/cli/*.rs (各サブコマンド実装)            │
│  src/cli/help_llm.rs ← 【新規追加】          │
├─────────────────────────────────────────────┤
│                  Parser層                    │
│  src/parser/ (Markdown・コード解析)           │
├─────────────────────────────────────────────┤
│                 Indexer層                     │
│  src/indexer/ (tantivy/SQLite操作)            │
├─────────────────────────────────────────────┤
│                 Search層                      │
│  src/search/ (検索ロジック)                    │
├─────────────────────────────────────────────┤
│                 Output層                      │
│  src/output/ (フォーマット: human/json/path)   │
└─────────────────────────────────────────────┘
```

**今回の変更はCLI層のみに閉じる。** Parser/Indexer/Search/Output層への変更は不要。

## 3. 変更対象モジュールと責務

### 3.1 新規作成

| ファイル | 責務 |
|---------|------|
| `src/cli/help_llm.rs` | help-llmサブコマンドの実装。HelpLlmOutput構造体定義、JSON生成・出力 |

### 3.2 変更対象

| ファイル | 変更内容 |
|---------|---------|
| `src/main.rs` | Commands enumにHelpLlmバリアント追加、各サブコマンドのabout拡充、matchアームにHelpLlm追加 |
| ~~`src/lib.rs`~~ | ~~VERSION定数追加~~ → 不要（help_llm.rs内でenv!マクロ直接使用） |
| `src/cli/mod.rs` | `pub mod help_llm;` 追加 |
| `src/cli/search.rs` | after_help定数追加（Examples/When to use） |
| `src/cli/impact.rs` | after_help定数追加 |
| `src/cli/diff.rs` | after_help定数追加 |
| `src/cli/context.rs` | after_help定数追加 |
| `src/cli/index.rs` | after_help定数追加（INDEX_AFTER_HELP, UPDATE_AFTER_HELP） |
| `src/cli/status/mod.rs` | after_help定数追加 |
| `src/cli/embed.rs` | after_help定数追加 |
| `src/cli/config.rs` | after_help定数追加 |
| `src/cli/export.rs` | after_help定数追加 |
| `src/cli/import_index.rs` | after_help定数追加 |
| `src/cli/watch.rs` | after_help定数追加 |
| `src/cli/clean.rs` | after_help定数追加 |
| `tests/cli_args.rs` | テスト更新（about文言変更対応）、help-llmテスト追加 |
| `tests/e2e_embedding.rs` | テスト更新（about文言変更対応） |

## 4. 設計詳細

### 4.1 help-llm サブコマンド設計

```rust
// src/main.rs - Commands enum への追加
#[derive(Subcommand)]
#[allow(clippy::large_enum_variant)]
enum Commands {
    // ... 既存サブコマンド ...

    /// Show structured JSON help for LLM integration
    #[command(name = "help-llm")]
    HelpLlm,
}
```

**設計判断**: `--help-llm` グローバルフラグではなく **サブコマンド** として実装する。

**理由**:
1. clapのサブコマンド必須バリデーションとの競合が発生しない
2. clapが自動的に`--help`を生成してくれる
3. 他サブコマンドとの組み合わせ問題が発生しない
4. 既存のCommands enumパターンに自然に統合できる

### 4.2 main.rs matchアーム設計

```rust
// main() 内の match cli.command
Commands::HelpLlm => {
    // インデックス不要 - resolve_commandindex_dir を呼ばない
    // Result を返して exit_code パターンに統一（整数値を返す）
    match commandindex::cli::help_llm::run_help_llm() {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("Error: {e}");
            1
        }
    }
}
```

**設計判断**:
1. `resolve_commandindex_dir()` を呼ばない（メタコマンド、インデックス不要）
2. `Result<(), String>` を返して他サブコマンドと同様のexit_codeパターンに統一（テスタビリティ向上）

### 4.3 HelpLlmOutput 構造体設計

```rust
// src/cli/help_llm.rs

use serde::Serialize;

#[derive(Serialize)]
pub struct HelpLlmOutput {
    pub schema_version: &'static str,
    pub tool: &'static str,
    pub version: String,
    pub description: &'static str,
    pub global_options: Vec<GlobalOption>,
    pub use_cases: Vec<UseCaseItem>,
    pub workflows: Vec<Workflow>,
    pub commands: Vec<CommandInfo>,
}

#[derive(Serialize)]
pub struct GlobalOption {
    pub name: &'static str,
    pub flag: &'static str,
    pub description: &'static str,
}

#[derive(Serialize)]
pub struct UseCaseItem {
    pub name: &'static str,
    pub command: &'static str,
}

#[derive(Serialize)]
pub struct Workflow {
    pub name: &'static str,
    pub description: &'static str,
    pub steps: Vec<&'static str>,
}

#[derive(Serialize)]
pub struct CommandInfo {
    pub name: &'static str,
    pub description: &'static str,
    pub when_to_use: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prerequisites: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modes: Option<Vec<SearchMode>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conflicts: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_options: Option<Vec<&'static str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_formats: Option<Vec<&'static str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pipe_support: Option<PipeSupport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subcommands: Option<Vec<&'static str>>,
    pub examples: Vec<&'static str>,
}

#[derive(Serialize)]
pub struct SearchMode {
    pub name: &'static str,
    pub description: &'static str,
    pub example: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_default: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prerequisites: Option<Vec<&'static str>>,
}

#[derive(Serialize)]
pub struct PipeSupport {
    pub stdin: &'static str,
}
```

**設計判断**: 型安全な構造体 + serde::Serialize でJSON生成。

**理由**:
1. コンパイル時にJSON構造の正しさを保証
2. フィールド追加時にコンパイラが未初期化を検出
3. `skip_serializing_if` でオプショナルフィールドをクリーンに出力

### 4.4 run_help_llm() 関数設計

```rust
/// help-llm固有のエラー型
#[derive(Debug)]
pub enum HelpLlmError {
    Serialize(String),
}

impl std::fmt::Display for HelpLlmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HelpLlmError::Serialize(msg) => write!(f, "failed to generate help-llm output: {msg}"),
        }
    }
}

pub fn run_help_llm() -> Result<(), HelpLlmError> {
    let output = build_help_llm_output();
    let json = serde_json::to_string_pretty(&output)
        .map_err(|e| HelpLlmError::Serialize(e.to_string()))?;
    println!("{json}");
    Ok(())
}

fn build_help_llm_output() -> HelpLlmOutput {
    HelpLlmOutput {
        schema_version: "1.0",
        tool: "commandindexdev",
        version: env!("CARGO_PKG_VERSION").to_string(),
        // ... 全フィールドを構築
    }
}
```

**設計判断**:
- デフォルトpretty print出力（LLMにとってトークン数増加は軽微、人間のデバッグ容易性を優先）
- expect()ではなくmatchによる構造化エラーハンドリング（panic時のスタックトレース露出防止）
- env!("CARGO_PKG_VERSION")を直接使用（既存コードのインライン使用パターンとの整合性）

### 4.5 各サブコマンドのabout拡充設計

```rust
// 現在
/// Search the index
Search { ... }

// 変更後
/// Search the index (full-text, --related, --semantic, --changed-since, --symbol)
Search { ... }
```

各サブコマンドの `///` コメント（clapの`about`に変換される）を用途ベースの情報豊富な説明に変更。

### 4.6 after_help 設計パターン

```rust
// src/cli/search.rs に定数として定義
pub const SEARCH_AFTER_HELP: &str = "\
When to use:
  Find relevant documents, code, or symbols across your repository.
  Use --related for impact analysis, --semantic for meaning-based search.

Search modes (mutually exclusive):
  [QUERY]                Full-text keyword search (default)
  --symbol <NAME>        Search for code symbols (functions, structs, etc.)
  --related <FILE>       Find files related to specified file(s)
  --related-stdin        Find related files from stdin paths
  --semantic <QUERY>     Meaning-based search (requires embeddings)
  --changed-since <EXPR> Find content changed since time expression

Examples:
  commandindexdev search \"認証\" --format json          # Full-text search
  commandindexdev search --related src/auth.rs          # Related files
  commandindexdev search --semantic \"login flow\"        # Semantic search
  commandindexdev search --changed-since \"yesterday\"    # Recent changes";

// src/main.rs での使用
/// Search the index (full-text, --related, --semantic, --changed-since, --symbol)
#[command(after_help = commandindex::cli::search::SEARCH_AFTER_HELP)]
Search { ... }
```

**設計判断**: after_help定数を各cliモジュールに配置。

**命名規則**: `<COMMAND_NAME>_AFTER_HELP`（例: `INDEX_AFTER_HELP`, `DIFF_AFTER_HELP`, `IMPACT_AFTER_HELP`）

**理由**: main.rsの肥大化防止。各サブコマンドの実装と近い位置に配置することで保守性向上。

### 4.7 VERSION 管理設計

```rust
// src/cli/help_llm.rs 内で直接使用
version: env!("CARGO_PKG_VERSION").to_string(),
```

**設計判断**: help_llm.rs内で`env!("CARGO_PKG_VERSION")`を直接使用。lib.rsへのVERSION定数追加は行わない。

**理由**: 既存コード（src/indexer/state.rs、src/cli/export.rs）がenv!マクロをインラインで使用しているパターンと整合性を保つ。lib.rsに定数を追加すると既存インライン使用との一貫性が崩れる。

### 4.8 schema_version 運用ルール

| バージョン変更 | トリガー | 例 |
|-------------|---------|-----|
| メジャー（1.0 → 2.0） | フィールド削除、型変更、破壊的変更 | commandsの構造変更 |
| マイナー（1.0 → 1.1） | フィールド追加、新コマンド追加 | 新サブコマンド追加 |

**設計判断**: schema_versionはセマンティックバージョニング形式（"1.0"）とし、消費者（LLM）側でバージョン比較を可能にする。

## 5. テスト設計

### 5.1 新規テスト

#### E2Eテスト（tests/cli_args.rs）

| テスト名 | 検証内容 |
|---------|---------|
| `help_llm_outputs_valid_json` | help-llm出力がserde_json::from_strでパース可能 |
| `help_llm_contains_all_subcommands` | JSON出力のcommands配列に全13サブコマンドが含まれる（help-llm自身は含まない） |
| `help_llm_has_schema_version` | schema_versionフィールドが存在する |
| `help_llm_version_matches_cargo` | versionフィールドがCargo.tomlのバージョンと一致 |

#### ユニットテスト（src/cli/help_llm.rs #[cfg(test)]）

| テスト名 | 検証内容 |
|---------|---------|
| `test_build_output_all_commands_have_examples` | 全CommandInfoのexamplesが最低1つ以上 |
| `test_build_output_all_commands_have_name` | 全CommandInfoのnameが空でない |
| `test_build_output_schema_version` | schema_versionが"1.0"である |
| `test_build_output_serializes_to_json` | build_help_llm_output()の戻り値がJSON化可能 |

### 5.2 既存テスト更新

| テストファイル | テスト名 | 更新内容 |
|-------------|---------|---------|
| tests/cli_args.rs | help_flag_shows_usage | help-llmのcontains検証追加 |
| tests/cli_args.rs | impact_help_shows_usage | about変更に伴うアサーション更新（必要に応じて） |
| tests/cli_args.rs | config_help_shows_subcommands | 同上 |
| tests/cli_args.rs | watch_help_shows_options | 同上 |
| tests/e2e_embedding.rs | embed_help_shows_usage | about変更に伴うアサーション更新（必要に応じて） |

## 6. セキュリティ設計

| 脅威 | 対策 | 優先度 |
|------|------|--------|
| JSON injection | serde_jsonが自動エスケープ処理（手動文字列結合不使用） | 中 |
| 情報漏洩 | help-llmは静的情報のみ出力。インデックス・設定・ファイル内容へのアクセスなし | 低 |
| panic露出 | expect()不使用、match/eprintlnで構造化エラーハンドリング | 中 |
| 制御文字注入 | after_helpテキストは全て&'static strリテラル。実行時の動的生成・ユーザー入力の挿入は行わない | 低 |
| unsafe依存 | help-llmはParser/Indexer/Embedding層を呼び出さないため、既存unsafeコードとの依存関係なし | 低 |

## 7. 設計判断とトレードオフ

| 判断事項 | 選択 | 代替案 | トレードオフ |
|---------|------|--------|------------|
| help-llm実装方式 | サブコマンド | グローバルフラグ | サブコマンドのほうがclapとの親和性が高いが、`--help-llm`のほうが直感的な場合もある |
| JSON生成方式 | 型安全な構造体+serde | 手動文字列結合 | 構造体定義の手間があるが、コンパイル時検証が得られる |
| after_help配置 | 各cliモジュール | main.rs集約 | import文が増えるが、保守性が向上 |
| JSON整形 | pretty print | minified | ファイルサイズ微増だが可読性向上 |
| VERSION管理 | env!マクロ直接使用 | lib.rs定数 | 既存パターン整合性優先。一元管理の利点よりも既存コードとの一貫性を重視 |
| JSON構造 | Vec<GlobalOption>/Vec<UseCaseItem> | Issue本文のフラットObjectMap | LLMのフィールド理解容易性・型安全性優先。出力JSONは意味的に同等 |
| エラー型 | HelpLlmError enum | Result<(), String> | 既存のCLI全モジュールが専用Error enumを使用するパターンに準拠 |

## 8. 影響範囲サマリー

```
影響範囲:
  ✅ CLI層のみ（src/main.rs, src/cli/, src/lib.rs）
  ✅ テスト（tests/cli_args.rs, tests/e2e_embedding.rs）
  ❌ Parser層 - 変更なし
  ❌ Indexer層 - 変更なし
  ❌ Search層 - 変更なし
  ❌ Output層 - 変更なし
  ❌ Cargo.toml - 変更なし（新規crate不要）

リスク:
  低 - 既存機能の動作には影響しない
  既存テストのアサーション文字列更新のみ注意
```

## 9. 品質基準

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |
| help-llm JSON | `commandindexdev help-llm \| python3 -m json.tool` | 有効なJSON |
