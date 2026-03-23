# 設計方針書 - Issue #107: search結果のデフォルトlimit引き下げ (LLM用途向け)

## 1. Issue概要

| 項目 | 内容 |
|------|------|
| Issue番号 | #107 |
| タイトル | search結果のデフォルトlimit引き下げ (LLM用途向け) |
| 優先度 | 中 |
| 種別 | 機能改善 |

### 目的
LLMプロンプト用途でsearch結果が肥大化（82KB/20件）し、Ollamaタイムアウトが発生する問題を解決する。config に `llm_default_limit` を追加し、`--rerank` 使用時のデフォルト件数を5件に引き下げる。

---

## 2. システムアーキテクチャ概要

### レイヤー構成と本Issue変更箇所

| レイヤー | モジュール | 責務 | 変更有無 |
|---------|-----------|------|---------|
| **CLI** | `src/main.rs` | エントリポイント、clapサブコマンド定義 | **変更** |
| **Config** | `src/config/mod.rs` | 設定ファイルの読み込み・マージ・解決 | **変更** |
| **Parser** | `src/parser/` | Markdown・ソースコード解析 | 変更なし |
| **Indexer** | `src/indexer/` | tantivy/SQLiteインデックス操作 | 変更なし |
| **Search** | `src/search/` | 検索ロジック | 変更なし |
| **Output** | `src/output/` | 出力フォーマット（human/json/path） | 変更なし |

---

## 3. 設計方針

### 3.1 方式選定

| 方式 | 概要 | メリット | デメリット | 採否 |
|------|------|---------|-----------|------|
| A: `--format llm` 新設 | OutputFormat列挙体にLlmを追加 | 明示的なフォーマット指定 | 影響範囲大、出力仕様設計が必要 | ❌ 不採用 |
| B: config `llm_default_limit` | SearchConfigにフィールド追加 | 最小変更、後方互換 | LLMコンテキスト判定が必要 | ✅ **採用** |

### 3.2 LLMコンテキスト判定条件

**判定基準**: `--rerank` フラグが指定された場合のみ

**理由**:
- `--rerank` はLLM（Ollama）を使用する明示的なフラグ
- 最小スコープで意図しない動作変更を防止
- 将来的に他のLLM関連フラグ追加時に段階的に拡張可能

### 3.3 limit解決の優先度

```
1. CLI `--limit` 明示指定 → 最優先
2. --rerank 指定時 → config.search.llm_default_limit (デフォルト: 5)
3. 通常時 → config.search.default_limit (デフォルト: 20)
4. config読み込み失敗時 → rerank時は5、通常時は20（SearchConfig::default()で統一）
```

### 3.4 SRP/DRY改善: resolve_limitメソッド

limit解決ロジックをSearchConfigに集約し、main.rsの責務を軽減する。

```rust
impl SearchConfig {
    /// limit解決ロジックを一元管理
    pub fn resolve_limit(&self, cli_limit: Option<usize>, is_rerank: bool) -> usize {
        let raw = match cli_limit {
            Some(l) => l,
            None if is_rerank => self.llm_default_limit,
            None => self.default_limit,
        };
        raw.clamp(1, 1000) // 最低1件、最大1000件を保証
    }
}
```

**理由**: main.rsに条件分岐を埋め込むとSRP違反。ハードコードフォールバック値もDefault traitで統一しDRY違反を解消。

---

## 4. 詳細設計

### 4.1 Config層の変更

#### RawSearchConfig（デシリアライズ用）

```rust
// src/config/mod.rs
#[derive(Debug, Default, Deserialize)]
pub struct RawSearchConfig {
    pub default_limit: Option<usize>,
    pub llm_default_limit: Option<usize>,  // 追加
    pub snippet_lines: Option<usize>,
    pub snippet_chars: Option<usize>,
}
```

#### SearchConfig（解決済み設定）

```rust
#[derive(Debug, Clone, Serialize)]
pub struct SearchConfig {
    pub default_limit: usize,       // default: 20
    pub llm_default_limit: usize,   // default: 5（追加）
    pub snippet_lines: usize,       // default: 2
    pub snippet_chars: usize,       // default: 120
}
```

#### merge_search 関数

```rust
fn merge_search(
    base: Option<RawSearchConfig>,
    higher: Option<RawSearchConfig>,
) -> Option<RawSearchConfig> {
    match (base, higher) {
        (None, None) => None,
        (Some(b), None) => Some(b),
        (None, Some(h)) => Some(h),
        (Some(b), Some(h)) => Some(RawSearchConfig {
            default_limit: h.default_limit.or(b.default_limit),
            llm_default_limit: h.llm_default_limit.or(b.llm_default_limit),  // 追加
            snippet_lines: h.snippet_lines.or(b.snippet_lines),
            snippet_chars: h.snippet_chars.or(b.snippet_chars),
        }),
    }
}
```

#### resolve_config 関数

```rust
let search = SearchConfig {
    default_limit: raw.search.as_ref()
        .and_then(|s| s.default_limit)
        .unwrap_or(20),
    llm_default_limit: raw.search.as_ref()
        .and_then(|s| s.llm_default_limit)
        .unwrap_or(5),  // 追加: デフォルト5
    snippet_lines: raw.search.as_ref()
        .and_then(|s| s.snippet_lines)
        .unwrap_or(2),
    snippet_chars: raw.search.and_then(|s| s.snippet_chars)
        .unwrap_or(120),
};
```

### 4.2 CLI層の変更（src/main.rs）

#### limit解決ロジック（resolve_limitメソッド使用）

```rust
// 行357-368 の effective_limit 解決を変更
let config = ctx.as_ref()
    .map(|c| c.config.search.clone())
    .unwrap_or_default();  // SearchConfig::default() でフォールバック

let effective_limit = config.resolve_limit(limit, rerank);
let effective_snippet_lines = snippet_lines.unwrap_or(config.snippet_lines);
let effective_snippet_chars = snippet_chars.unwrap_or(config.snippet_chars);
```

#### SearchConfig の Default trait 実装

```rust
impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            default_limit: 20,
            llm_default_limit: 5,
            snippet_lines: 2,
            snippet_chars: 120,
        }
    }
}
```

**注意**: Noneブランチでも `SearchConfig::default()` を使用し、rerank時のllm_default_limitフォールバック（5件）を正しく適用する。workspace検索パス（main.rs L383-403）でも同じ `config.resolve_limit(limit, rerank)` を使用し、全検索パスで一貫した動作を保証する。

### 4.3 CLIヘルプ更新

#### clap ヘルプ文字列（src/main.rs L72）
```rust
/// Maximum number of results (default: 20, with --rerank: 5)
#[arg(long)]
limit: Option<usize>,
```

#### help-llm の更新（src/cli/help_llm.rs）

searchコマンドの key_options に `--rerank` 時のデフォルト limit 変更情報を追加:
```
--limit <N>  Maximum number of results (default: 20, with --rerank: from config llm_default_limit, default 5)
```

### 4.3 設定ファイル例

```toml
# commandindex.toml
[search]
default_limit = 20        # 通常検索のデフォルト件数
llm_default_limit = 5     # LLM用途（--rerank時）のデフォルト件数
```

---

## 5. 影響範囲

### 変更対象ファイル

| ファイル | 変更内容 | リスク |
|---------|---------|--------|
| `src/config/mod.rs` | RawSearchConfig, SearchConfig, merge_search, resolve_config にフィールド追加 + resolve_limit メソッド + Default trait | 低 |
| `src/main.rs` | effective_limit 解決を resolve_limit() メソッド呼び出しに置換 | 低 |
| `src/cli/help_llm.rs` | search コマンドの --limit 説明にrerank時デフォルト情報追加 | 低 |

### 既存機能への影響

| 機能 | 影響 | 理由 |
|------|------|------|
| 通常search（--rerankなし） | **なし** | limit解決パスが変更されない |
| `--limit` 明示指定 | **なし** | 明示指定が最優先のまま |
| `--format human/json/path` | **なし** | OutputFormat は変更しない |
| `config show` | **自動反映** | SearchConfig が Serialize derive済み |
| 既存config.toml | **互換** | Option<T> で未設定時はデフォルト値 |

### テストへの影響

| テスト | 影響 |
|--------|------|
| SearchConfig構造体リテラル使用テスト | **コンパイルエラー** - llm_default_limitフィールド追加が必要（test_to_masked_view_masks_api_keys, test_to_masked_view_no_api_keys, test_view_model_serializes_to_toml 等） |
| e2e_team_workflow | 既存テストは通過、新フィールドのconfig show反映テスト追加推奨 |
| 新規テスト | resolve_limit, merge_search, config show反映のテスト追加 |

---

## 6. セキュリティ設計

| 脅威 | 対策 | 優先度 |
|------|------|--------|
| 不正なlimit値（0, 負数, 巨大値） | `.clamp(1, 1000)` で最低1件・最大1000件を保証。usize型で負数不可 | 対応済み |
| config改竄 | ローカルファイルのため既存の脅威モデルと同等 | 対応不要 |

---

## 7. 設計判断とトレードオフ

### 判断1: config方式 vs format方式
- **決定**: config方式（`llm_default_limit`）を採用
- **トレードオフ**: LLM専用出力フォーマットは得られないが、最小変更でlimit問題を解決
- **将来**: `--format llm` は別Issueで必要に応じて対応

### 判断2: LLMコンテキスト = --rerank のみ
- **決定**: `--rerank` フラグのみをLLMコンテキストとして判定
- **トレードオフ**: semantic searchなど他のLLM関連機能にはllm_default_limitが適用されない
- **将来**: 必要に応じて判定条件を拡張

### 判断3: デフォルト値 = 5
- **決定**: `llm_default_limit` のデフォルトを5件に設定
- **根拠**: 検証データで20件=82KBが問題、5件で適正サイズ
- **トレードオフ**: ユースケースによっては5件では少ない場合があるが、configで変更可能

---

## 8. 品質基準

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |
