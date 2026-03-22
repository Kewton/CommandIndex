# 設計方針書: Issue #90 impact サブコマンド

## 1. 概要

Git の変更ファイル一覧を入力として受け取り、各ファイルの関連ファイル検索結果を一括で返す `impact` サブコマンドを実装する。既存の `--related` 検索エンジンを内部的に利用し、overlap（共通影響ファイル）検出と統計サマリーを付加する。

## 2. レイヤー構成と責務

| レイヤー | モジュール | 変更内容 |
|---------|-----------|---------|
| **CLI** | `src/main.rs` | Impact コマンド定義（既存）+ CLI help 更新 |
| **CLI** | `src/cli/impact.rs` | impact ロジック（**aggregate_impact 全面書き換え**） |
| **CLI** | `src/cli/stdin.rs` | stdin 読み取り（既存）+ UTF-8 truncation バグ修正 |
| **Output** | `src/output/mod.rs` | ImpactResult 型の再設計（新型定義） |
| **Output** | `src/output/json.rs` | JSON 出力の再実装（Serialize derive 活用） |
| **Output** | `src/output/human.rs` | human 出力を per-file + overlap + summary 構造に全面刷新 |
| **Output** | `src/output/path.rs` | union 出力 + strip_control_chars 追加 |
| **Search** | `src/search/related.rs` | 変更なし（内部利用のみ、INTERNAL_FETCH_LIMIT=1000 で十分。1000件超の related は極めて稀） |

## 3. データモデル設計

### 3.1 現行→新フィールド名マッピング

| 現行フィールド | 新フィールド | 備考 |
|---------------|------------|------|
| `input_files` | `changed_files` | |
| `impacted_files` | `impact: Vec<ImpactPerFile>` | フラット→ネスト |
| `total_input_files` | `summary.changed` | |
| `total_impacted_files` | `summary.total_impacted` | 意味変更: limit前基準 |
| （なし） | `overlap: Vec<String>` | 新規追加 |
| （なし） | `summary.overlap_count` | 新規追加 |

### 3.2 新データモデル（変更後）

```rust
/// impact サブコマンドの結果（Issue #90 仕様準拠）
#[derive(Debug, Clone, Serialize)]
pub struct ImpactResult {
    /// 入力ファイル一覧（changed_files）
    pub changed_files: Vec<String>,
    /// ファイルごとの影響分析結果（impact[]）
    pub impact: Vec<ImpactPerFile>,
    /// 複数入力ファイルから共通して影響を受けるファイル一覧
    pub overlap: Vec<String>,
    /// 統計サマリー
    pub summary: ImpactSummary,
}

/// 入力ファイルごとの関連ファイル一覧
#[derive(Debug, Clone, Serialize)]
pub struct ImpactPerFile {
    /// 入力ファイルパス
    pub file: String,
    /// 関連ファイル一覧（スコア降順）
    pub related: Vec<ImpactRelatedFile>,
}

/// 関連ファイル情報
#[derive(Debug, Clone, Serialize)]
pub struct ImpactRelatedFile {
    pub path: String,
    pub score: f32,
    /// snake_case 文字列（"markdown_link", "import_dependency" 等）
    pub relations: Vec<String>,
}

/// 統計サマリー
#[derive(Debug, Clone, Serialize)]
pub struct ImpactSummary {
    /// 入力ファイル数
    pub changed: usize,
    /// ユニーク影響ファイル総数（limit 前基準）
    pub total_impacted: usize,
    /// overlap 件数
    pub overlap_count: usize,
}
```

### 3.3 設計判断

**ネスト構造 vs フラット構造**: ネスト構造を採用。Issue仕様準拠 + ユーザー直感的。

**relations の型**: `Vec<String>`（snake_case）を採用。tag_match の matched_tags 詳細は impact では省略し、シンプルに "tag_match" 文字列のみとする。

**Serialize derive**: 全新型に `#[derive(Debug, Clone, Serialize)]` を付与し、`format_impact_json` では `serde_json::to_writer_pretty(writer, &result)` を使用して手動 JSON 構築を排除する。

**--limit 意味変更**: 現行は全体 truncate → 新仕様は入力ファイルごとの related 件数上限。**破壊的変更**。

## 4. 処理フロー

### 4.1 aggregate_impact 書き換え仕様

```rust
/// 新 aggregate_impact の擬似コード
fn aggregate_impact(engine, files, limit) -> ImpactResult {
    const INTERNAL_FETCH_LIMIT: usize = 1000;

    let mut per_file_results: Vec<ImpactPerFile> = Vec::new();
    let mut overlap_map: HashMap<String, usize> = HashMap::new();
    let mut all_impacted: HashSet<String> = HashSet::new();
    let input_set: HashSet<&str> = files.iter().map(|f| f.as_str()).collect();

    for file in files {
        let results = engine.find_related(file, INTERNAL_FETCH_LIMIT);

        // 入力ファイルを除外
        let filtered: Vec<_> = results
            .filter(|r| !input_set.contains(r.file_path.as_str()))
            .collect();

        // overlap カウント + ユニーク集計（limit 前）
        for r in &filtered {
            *overlap_map.entry(r.file_path.clone()).or_insert(0) += 1;
            all_impacted.insert(r.file_path.clone());
        }

        // limit 適用して ImpactPerFile 構築
        let related = filtered[..min(filtered.len(), limit)]
            .map(|r| ImpactRelatedFile { ... })
            .collect();

        per_file_results.push(ImpactPerFile { file, related });
    }

    let overlap: Vec<String> = overlap_map
        .filter(|_, count| count >= 2)
        .keys()
        .sorted()
        .collect();

    ImpactResult {
        changed_files: files.to_vec(),
        impact: per_file_results,
        overlap: overlap.clone(),
        summary: ImpactSummary {
            changed: files.len(),
            total_impacted: all_impacted.len(),
            overlap_count: overlap.len(),
        },
    }
}
```

### 4.2 --limit 適用タイミング

```
                      ┌─ summary.total_impacted 計算 ─┐
                      │  overlap 検出（全結果から）     │
find_related(1000) →  ├───────────────────────────────┤
                      │  limit 適用（per-file）        │
                      │  related[].truncate(limit)     │
                      └─ JSON/human/path 出力 ────────┘
```

## 5. 出力形式

### 5.1 JSON

`serde_json::to_writer_pretty(writer, &result)` で直接出力。Serialize derive によりフィールド名は構造体定義に準拠。

### 5.2 Human

```
Impact analysis: 2 changed file(s), 5 impacted file(s)

src/a.rs:
  docs/a.md (score: 0.95) [markdown_link]
  tests/a_test.rs (score: 0.80) [import_dependency]

src/b.rs:
  tests/b_test.rs (score: 0.70) [import_dependency]

Overlap (1 file):
  tests/common.rs

Summary: 2 changed, 5 impacted, 1 overlap
```

### 5.3 Path

全 impacted path の union、重複除去、スコア降順（同一 path が複数入力から出た場合は max スコアを代表値とする）。`strip_control_chars()` を適用してから出力。

## 6. エラーハンドリング

| 状態 | 振る舞い | 終了コード |
|------|---------|-----------|
| stdin 未接続 + 引数なし | エラーメッセージ出力 | 1 |
| 無効パス（`..`, 絶対パス） | Warning → スキップ | 0（有効ファイルあれば） |
| 存在しないファイル | Warning → スキップ | 0（有効ファイルあれば） |
| 全ファイルスキップ | "No valid files" エラー | 1 |
| 一部ファイルが未インデックス | Warning → スキップ、残りで継続 | 0（有効結果あれば） |
| find_related: FileNotFound/FileNotIndexed | Warning → スキップ、他ファイルは継続 | 0 |
| find_related: ReaderError/SymbolStoreError | **即時エラー（fail-fast）** | 1 |
| インデックス未作成 | "Index not found" エラー | 1 |
| シンボルDB未作成 | "Symbol database not found" エラー | 1 |

## 7. セキュリティ設計

| 脅威 | 対策 |
|------|------|
| パストラバーサル | `validate_file_path()` で `..` / 絶対パス / null バイトを拒否 |
| stdin 大量入力 | `MAX_STDIN_BYTES=512KB` / `MAX_INPUT_FILES=500` で制限 |
| 引数大量入力 | `validate_and_normalize()` に `MAX_INPUT_FILES` チェック追加 |
| 制御文字注入 | human/path 出力で `strip_control_chars()` 適用。JSON は serde_json のエスケープに委ねる（値改変回避） |
| UTF-8 境界パニック | `StdinError::InvalidPath` の Display で char 境界安全な truncation に修正 |

## 8. 影響範囲

### 変更対象ファイル

| ファイル | 変更種別 | 影響度 |
|---------|---------|--------|
| `src/output/mod.rs` | ImpactResult/ImpactFileResult 削除 → 新型4つ定義 | **高** |
| `src/cli/impact.rs` | aggregate_impact **全面書き換え** + validate_and_normalize にファイル数制限追加 | **高** |
| `src/output/json.rs` | format_impact_json を Serialize 活用に書き換え | **高** |
| `src/output/human.rs` | per-file + overlap + summary 構造に**全面刷新** | **高** |
| `src/output/path.rs` | union 出力 + strip_control_chars 追加 | **低** |
| `src/main.rs` | CLI help テキスト更新 | **低** |
| `src/cli/stdin.rs` | StdinError Display の UTF-8 truncation 修正 | **低** |
| `tests/e2e_impact.rs` | **全テスト書き換え**（フィールド名・構造変更） | **高** |
| `tests/output_format.rs` | impact 関連単体テスト**全面更新** | **高** |
| `tests/cli_args.rs` | impact help テスト更新（パイプ利用例追加時） | **低** |

### 影響を受けない既存機能

- `search --related` / `search --related-stdin` / `context` / その他サブコマンド: CLI 挙動は不変。共通 output モデル・テストには波及あり

### 制限事項

- `find_related(file, 1000)` の内部上限 1000 件を超える関連ファイルがある場合、summary.total_impacted と overlap が不完全になる可能性がある。実用上は極めて稀。将来必要なら Search 層 API を拡張する。

## 9. テスト戦略

### 9.1 単体テスト（tests/output_format.rs）

- `format_impact_json` : 新 JSON 構造（changed_files, impact, overlap, summary）
- `format_impact_human` : per-file 表示 + overlap セクション + summary 行
- `format_impact_path` : union 出力 + strip_control_chars

### 9.2 E2E テスト（tests/e2e_impact.rs）

既存テストを新フィールド名に全面更新 + 以下の新テスト追加:

- overlap 検出テスト: 2つの入力ファイルが共通する related ファイルを持つケース
- summary 統計値テスト: changed, total_impacted, overlap_count の正確性
- --limit per-file テスト: limit 適用後も summary は全件ベースであること
- 入力ファイル除外テスト: 正しいフィールド名（`path`）で検証

### 9.3 overlap テストデータ

```
a.md → links to b.md, c.md
b.md → links to a.md, c.md
```
- input: [a.md, b.md] → c.md が overlap に出現

## 10. 品質基準

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |
