# 設計方針書: Issue #105 — context コマンドのトークン数制御の実効化

## 1. 概要

`context` コマンドの `--max-tokens` オプションを実効的に機能させ、LLMコンテキストウィンドウへの最適化を可能にする。

### 対象Issue
- **Issue番号**: #105
- **タイトル**: context コマンドのトークン数制御の実効化
- **優先度**: 高

## 2. 現状分析

### 既存実装
| 項目 | 現状 | 課題 |
|------|------|------|
| トークン推定 | `text.len() / 4`（バイト数ベース） | 日本語テキストで過大評価 |
| 推定対象 | snippetのみ | path/relation/score/heading/symbolsが含まれない |
| トークン制限 | greedy方式でエントリ全体をスキップ | 部分含有なし |
| snippet制限 | 固定500文字/10行 | トークン予算と連動していない |
| 最初のエントリ | max_tokens超過でも必ず含まれる | 動的縮約なし |
| 入力バリデーション | max_tokens/max_filesに上限なし | 不合理な値を受け入れる |

### 処理フロー（現状）
```
run_context
  → collect_related_context（関連ファイル収集）
  → build_context_pack
      → max_files でトリム
      → 各エントリを enrich_entry（固定500文字/10行でsnippet生成）
      → max_tokens でgreedy制限（snippetのみ計算）
  → format_context_pack（JSON出力）
```

## 3. 設計方針

### 3.1 トークン推定の改善

#### 変更対象: `estimate_tokens` 関数

**現在の実装** (`src/cli/context.rs:375-377`):
```rust
fn estimate_tokens(text: &str) -> usize {
    text.len() / 4
}
```

**新しい実装**:
```rust
/// トークン数を概算する（文字数 / 4、最低1トークン）
fn estimate_tokens(text: &str) -> usize {
    let count = text.chars().count();
    if count == 0 { 0 } else { (count / 4).max(1) }
}
```

**設計判断**:
- `text.len()`（バイト数）→ `text.chars().count()`（文字数）に変更
- UTF-8でのバイト数と文字数の乖離（日本語: 3byte/文字）を解消
- 非空文字列は最低1トークンとみなす
- O(1)→O(n)の性能変化あるが、snippet最大500文字×20エントリで無視可能
- score は数値固定1トークンとして扱う（JSON出力での実際のトークン消費は3-4トークン程度だが、近似として許容）

#### 変更対象: `build_context_pack` 関数のトークン計算

**エントリトークン推定（メタデータ + snippet 統合）**:

DRY原則に基づき、メタデータ部分とsnippet部分を分離して算出する単一関数として設計する。

```rust
/// ContextEntryのメタデータ部分（snippet以外）の推定トークン数を算出
fn estimate_entry_meta_tokens(entry: &ContextEntry) -> usize {
    let mut total = 0;
    total += estimate_tokens(&entry.path);
    total += estimate_tokens(&entry.relation);
    total += 1; // score は固定1トークン
    if let Some(h) = &entry.heading {
        total += estimate_tokens(h);
    }
    if let Some(syms) = &entry.symbols {
        for sym in syms {
            total += estimate_tokens(sym);
        }
    }
    total
}

/// ContextEntry全体の推定トークン数を算出（メタデータ + snippet）
fn estimate_entry_tokens(entry: &ContextEntry) -> usize {
    let meta = estimate_entry_meta_tokens(entry);
    let snippet = entry
        .snippet
        .as_ref()
        .map(|s| estimate_tokens(s))
        .unwrap_or(0);
    meta + snippet
}
```

### 3.2 部分含有戦略

#### トークン⇔文字数変換ヘルパー

```rust
/// トークン数を文字数予算に変換（estimate_tokensの逆変換）
fn tokens_to_char_budget(tokens: usize) -> usize {
    tokens * 4
}
```

#### 新関数: `truncate_snippet_for_char_budget`

**設計**（アンダーフロー対策済み）:
```rust
/// snippet を先頭と末尾に比率で切り詰める際の定数
const HEAD_RATIO: usize = 3;
const TOTAL_PARTS: usize = 5;
/// 省略マーカー "..." の文字数
const ELLIPSIS_LEN: usize = 3;

/// 文字数予算に収まるようsnippetを動的に切り詰める
/// 先頭と末尾を保持し、中間を省略する戦略
/// budget_chars: 文字数ベースの予算（トークンではない）
/// 戻り値が空文字列の場合、呼び出し側で snippet = None に正規化する
fn truncate_snippet_for_char_budget(snippet: &str, budget_chars: usize) -> String {
    let chars: Vec<char> = snippet.chars().collect();
    if chars.len() <= budget_chars {
        return snippet.to_string();
    }
    if budget_chars == 0 {
        return String::new();
    }
    // 省略マーカー + 最低1文字ずつ = 最低5文字必要
    // それ未満なら先頭のみで切り詰め（省略マーカーなし）
    if budget_chars < ELLIPSIS_LEN + 2 {
        return chars[..budget_chars].iter().collect();
    }
    let content_budget = budget_chars - ELLIPSIS_LEN;
    let head_chars = (content_budget * HEAD_RATIO) / TOTAL_PARTS;
    let tail_chars = content_budget - head_chars;
    let head: String = chars[..head_chars].iter().collect();
    let tail: String = chars[chars.len() - tail_chars..].iter().collect();
    format!("{head}...{tail}")
}
```

**設計判断**:
- `truncate_body`（既存関数）のシグネチャは変更しない（`output/human.rs`で使用）
- 行ベースではなく文字数ベースで切り詰め（トークン予算と直結）
- 先頭60%+末尾40%の比率（`HEAD_RATIO`/`TOTAL_PARTS` 定数で明示）
- 省略マーカー `...` で切り詰めを明示
- **アンダーフロー対策**: `budget_chars < 5` の場合は先頭のみで切り詰め（省略マーカーなし）

### 3.3 build_context_pack の改修

**処理フロー（新設計）— 全エントリ統一縮約ロジック**:

KISS原則に基づき、最初のエントリだけ特別扱いする非対称ロジックを排し、全エントリに統一的な縮約ルールを適用する。

```
build_context_pack
  → max_files でトリム
  → 各エントリを enrich_entry
  → max_tokens 適用（全エントリ統一ルール）:
      1. メタデータ分のトークン数を算出
      2. メタデータだけで残予算を超過 → スキップ（ただし最初のエントリのみ例外として含める）
      3. 残予算内でsnippetを truncate_snippet_for_char_budget で動的縮約
      4. 縮約後のトークン数を累積に加算
```

**コード概要**:
```rust
fn build_context_pack(
    target_files: &[String],
    merged: &[RelatedSearchResult],
    max_files: usize,
    max_tokens: Option<usize>,
    reader: &IndexReaderWrapper,
    store: &SymbolStore,
) -> Result<ContextPack, SearchError> {
    let total_related = merged.len();
    let limited = &merged[..merged.len().min(max_files)];

    let mut entries = Vec::new();
    let mut token_total: usize = 0;

    for result in limited {
        let mut entry = enrich_entry(/* ... */);

        if let Some(max_tok) = max_tokens {
            let meta_tokens = estimate_entry_meta_tokens(&entry);

            // メタデータだけで残予算超過 → スキップ（最初のエントリのみ例外）
            if token_total + meta_tokens > max_tok && !entries.is_empty() {
                continue; // 次のエントリを試す（breakではなくcontinue）
            }

            // snippet動的縮約
            let remaining = max_tok.saturating_sub(token_total + meta_tokens);
            let snippet_budget = tokens_to_char_budget(remaining);
            if let Some(s) = &entry.snippet {
                let truncated = truncate_snippet_for_char_budget(s, snippet_budget);
                // 空文字列はNoneに正規化
                entry.snippet = if truncated.is_empty() { None } else { Some(truncated) };
            }

            token_total += estimate_entry_tokens(&entry);
        }

        entries.push(entry);
    }

    let included = entries.len();
    let estimated_tokens = if max_tokens.is_some() {
        token_total
    } else {
        entries.iter().map(|e| estimate_entry_tokens(e)).sum()
    };

    Ok(ContextPack {
        target_files: target_files.to_vec(),
        context: entries,
        summary: ContextSummary {
            total_related,
            included,
            estimated_tokens,
        },
    })
}
```

**設計判断（変更点）**:
- **全エントリ統一ロジック**: 最初のエントリだけの分岐を廃止。KISS原則準拠
- **continue vs break**: 大きなエントリをスキップして次の小さなエントリを含められるようcontinueを採用。トークン活用率が向上
- **最初のエントリ例外**: `!entries.is_empty()` の条件で最低1エントリを保証（メタデータ超過時も含む）
- **estimated_tokens算出の効率化**: max_tokens指定時はループ内のtoken_totalをそのまま使用（2重計算回避）
- **Ok(...).map(...)パターン廃止**: included を先に計算して直接代入。KISS準拠
- **enumerateの不使用**: 最初エントリのi==0分岐を廃止したためenumerate不要

### 3.4 エラーハンドリング方針

`enrich_entry` 内の index/symbol lookup 失敗（`reader.search_by_exact_path`、`store.find_imports_by_source/target`）は、既存の `if let Ok(...)` パターンを維持し、**best effort でフィールド欠落を許容**する。

- **heading/snippet/symbols 取得失敗**: 該当フィールドを `None` としてエントリを生成（estimated_tokens はメタデータ分のみ）
- **pack 全体を壊す条件**: `IndexReaderWrapper::open` や `SymbolStore::open` の失敗のみ `SearchError` として伝播（既存動作）
- **今回の変更での影響**: フィールド欠落時は estimated_tokens が小さくなるが、トークン予算制御の安全側（過少推定）に倒れるため許容

### 3.5 CLIヘルプ・入力バリデーションの更新

**src/main.rs** — clapヘルプテキスト・バリデーション:
```rust
/// Estimated token limit (approx. 1 token per 4 chars)
#[arg(long, value_parser = clap::value_parser!(usize).range(1..=1_000_000))]
max_tokens: Option<usize>,

/// Maximum number of related files to include
#[arg(long, default_value = "20", value_parser = clap::value_parser!(usize).range(1..=1000))]
max_files: usize,
```

**src/cli/context.rs** — CONTEXT_AFTER_HELP:
```
commandindexdev context src/a.rs src/b.rs --max-tokens 8000
  (Limits total estimated tokens. Estimation: approx. 1 token per 4 chars)
```

## 4. 影響範囲

### 変更対象ファイル

| ファイル | 変更内容 |
|---------|---------|
| `src/cli/context.rs` | `estimate_tokens`改修、`estimate_entry_meta_tokens`新設、`estimate_entry_tokens`新設、`truncate_snippet_for_char_budget`新設、`build_context_pack`改修（ループ構造変更含む）、`CONTEXT_AFTER_HELP`更新 |
| `src/main.rs` | `--max-tokens` のヘルプテキスト更新、`max_tokens`/`max_files` の value_parser 追加 |
| `src/cli/help_llm.rs` | `--max-tokens` の key_options 説明・例文更新 |
| `tests/e2e_context_pack.rs` | 実用的なmax_tokensテスト追加、部分含有テスト追加、既存テスト改修 |

### 変更しないファイル

| ファイル | 理由 |
|---------|------|
| `src/output/mod.rs` | ContextPack/ContextEntry/ContextSummaryの構造は変更不要 |
| `src/output/context_pack.rs` | JSON出力フォーマットは変更不要 |
| `src/output/human.rs` | truncate_bodyは変更不要（context.rs内に専用関数を新設） |

### JSON互換性
- フィールド名・型は変更なし
- `--max-tokens` 利用時の `included`、`estimated_tokens`、`snippet` 内容は変わり得る（推定方式の変更による）

## 5. セキュリティ設計

| 脅威 | 対策 | 優先度 |
|------|------|--------|
| 巨大入力によるDoS | `max_files`（デフォルト20、上限1000）、`max_tokens`（上限1,000,000） | 中 |
| 不合理な入力値 | clap `value_parser` で `max_tokens` に 1..=1,000,000、`max_files` に 1..=1000 の範囲制約 | 中 |
| パストラバーサル | 既存の `validate_file_paths` による入力検証を維持 | 高 |
| unsafe使用 | 使用しない | 高 |
| 整数アンダーフロー | `truncate_snippet_for_char_budget` で `saturating_sub` およびガード条件を使用 | 中 |

## 6. 設計判断とトレードオフ

### 判断1: バイト数→文字数ベースへの変更
- **選択**: `text.chars().count() / 4`
- **代替案**: tiktoken等の外部クレートによる正確なトークン計算
- **トレードオフ**: 精度は近似的だが、外部依存なしでシンプル。LLMモデルごとのトークナイザー差は許容
- **根拠**: Issueの「1トークン ≈ 4文字 の近似でも可」の要件を満たす

### 判断2: 全エントリ統一縮約ロジック
- **選択**: 全エントリに対して統一的な縮約ルールを適用（KISS原則準拠）
- **代替案**: 最初のエントリのみsnippet動的縮約、2番目以降はエントリ全体スキップ
- **トレードオフ**: やや実装は増えるが、非対称な分岐条件が不要になりコードがシンプル
- **根拠**: 全エントリで同じルール（snippet縮約→メタデータ超過ならスキップ）が明快

### 判断3: continue vs break
- **選択**: トークン超過時は `continue`（次のエントリを試す）
- **代替案**: `break`（即座に終了）
- **トレードオフ**: continueの方がトークン活用率が高いが、全エントリを走査する
- **根拠**: max_files上限（デフォルト20、最大1000）でエントリ数が限定されるためパフォーマンス影響なし

### 判断4: truncate_body vs 専用関数
- **選択**: `context.rs` 内に `truncate_snippet_for_char_budget` を新設
- **代替案**: `truncate_body` のシグネチャを拡張
- **トレードオフ**: コード重複の可能性あるが、`human.rs` への影響を完全回避
- **根拠**: 影響範囲の最小化を優先

### 判断5: 先頭+末尾の比率
- **選択**: 先頭60% + 末尾40%（`HEAD_RATIO=3`, `TOTAL_PARTS=5` 定数で明示）
- **代替案**: 50:50、先頭のみ
- **トレードオフ**: import文やクラス定義は冒頭に集中するため先頭を多めに
- **根拠**: コードファイルの構造的特性に基づく

## 7. テスト方針

### 単体テスト（context.rs内）
1. `estimate_tokens` — ASCII、日本語、空文字列、混合テキスト
2. `estimate_entry_meta_tokens` — 全フィールドあり/なしの各パターン
3. `estimate_entry_tokens` — メタデータ + snippet の合算検証
4. `truncate_snippet_for_char_budget` — 予算内/超過/0予算/短文/境界値（budget_chars=1〜5）/空snippet→None正規化
5. `tokens_to_char_budget` — 基本変換テスト

### E2Eテスト（tests/e2e_context_pack.rs）
1. 十分に長いfixtureを用い、max_tokensによってincluded減少またはsnippet縮約が実際に発生するテスト
2. 最初のエントリ例外（メタデータ超過時）のテスト
3. 既存テスト `context_pack_max_tokens_limits_output` の改修（max-tokens=1 のテストはメタデータ含む新ロジックで挙動が変わるため、実用的な値に変更）
4. `--max-tokens` と `--max-files` の value_parser 範囲制約テスト
5. snippet/symbols 取得失敗時にpack生成が継続するケース（best effort動作確認）
6. snippet 縮約後に空→None正規化されるケース

## 8. 品質基準

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |
