# 設計方針書: Issue #170 - why/issueのJSON出力に日付情報を付与する

## 1. Issue概要

| 項目 | 内容 |
|------|------|
| Issue番号 | #170 |
| タイトル | why/issueのJSON出力に日付情報を付与する |
| 目的 | JSON出力に日付情報を付与し、判断の時系列を追えるようにする |
| スコープ | `why --format json` と `issue --format json` への `date` フィールド追加 |

## 2. システムアーキテクチャ概要

```
[CLI Layer]         [Indexer Layer]           [Storage Layer]
why.rs ──────────> symbol_store.rs ──────────> SQLite (knowledge_edges)
issue.rs ─────────> knowledge.rs ────────────> ファイルシステム (dev-reports/)
                        │
                   [Output Layer]
                   output/mod.rs
                   output/json.rs
```

本変更は全レイヤーに跨る横断的な変更。

## 3. レイヤー構成と責務

| レイヤー | モジュール | 今回の変更内容 |
|---------|-----------|--------------|
| **Indexer** | `src/indexer/knowledge.rs` | `KnowledgeEntry` に `date` フィールド追加、`parse_dev_report_path` で日付抽出、日付取得ユーティリティ関数追加 |
| **Indexer** | `src/indexer/symbol_store.rs` | metadata に `date` 格納、`find_documents_by_issue` / `find_knowledge_related` で date パース |
| **CLI** | `src/cli/issue.rs` | JSON出力をオブジェクト配列形式に変更 |
| **CLI** | `src/cli/why.rs` | `group_knowledge_results` で date を転送 |
| **Output** | `src/output/mod.rs` | `WhyDocumentEntry` に `date` フィールド追加 |

## 4. 設計判断とトレードオフ

### 判断1: 日付の格納場所

**決定**: `knowledge_edges.metadata` の JSON に `date` フィールドを追加

```json
// Before
{"doc_subtype": "design_policy"}

// After
{"doc_subtype": "design_policy", "date": "2026-03-20"}
```

**理由**:
- 既存の metadata JSON を拡張するだけで済む
- ALTER TABLE 不要（metadata は既に TEXT カラム）
- 後方互換性あり（date が存在しない場合は None として扱う）

**却下案**: knowledge_edges テーブルに `date` カラムを ALTER TABLE で追加
- 理由: metadata JSON の拡張で十分であり、スキーマ変更の複雑性を避けられる

### 判断2: 日付取得の優先順位

**決定**: 2段階のフォールバック

1. **ファイル名パターン抽出**: `YYYY-MM-DD-*` の正規表現マッチ
2. **git log フォールバック**: `git log --format=%ai -1 -- <path>`

**理由**:
- ファイル名抽出は高速で安定（I/O 不要）
- git log は全ファイルに対応可能だがプロセス起動コストあり
- 現在日付プレフィックスがあるのは StageReview ファイルのみだが、今後拡張可能

### 判断3: issue --format json の破壊的変更

**決定**: カテゴリ別の値を文字列配列からオブジェクト配列に変更

```json
// Before: "designs": ["path/to/file.md"]
// After:  "designs": [{"file_path": "path/to/file.md", "date": "2026-03-20"}]
```

**理由**:
- 文字列配列にdate情報を埋め込む方法がない
- オブジェクト配列にすることで将来のフィールド追加も容易
- 破壊的変更だが、JSON出力の利用者は限定的（主に開発者ツール）

### 判断4: 日付取得ユーティリティの配置

**決定**: `src/indexer/knowledge.rs` に配置

**理由**:
- ファイルパスの解析ロジック（`parse_dev_report_path`）と同じモジュール
- git log 実行も既に `extract_file_modifies_from_git_log` が存在する
- 新規モジュール作成の必要なし

### 判断5: git log の実行タイミング

**決定**: インデックス時に実行し、metadata に格納

**理由**:
- クエリ時に毎回 git log を実行するのはパフォーマンス上不適切
- インデックス時に一度だけ実行し、結果をDBに永続化
- `scan_dev_reports` / `parse_dev_report_path` の処理フロー内で日付取得

### 判断6: KnowledgeRelatedResult の date 対応

**決定**: 今回の実装スコープに含める

**理由**:
- `why --format json` の出力で date を表示するには `KnowledgeRelatedResult` に date が必要
- `find_knowledge_related` は既に metadata をパースしており、date 追加は小規模な変更

## 5. 日付取得ユーティリティ関数の設計

```rust
use regex::Regex;
use std::process::Command;
use std::sync::LazyLock;

/// ファイル名先頭の YYYY-MM-DD パターン（コンパイル済みキャッシュ）
static DATE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\d{4}-\d{2}-\d{2})").unwrap()
});

/// ファイルパスから日付を取得する
/// 1. ファイル名の先頭 YYYY-MM-DD パターンを正規表現で抽出
/// 2. マッチしなければ git log --format=%ai -1 -- <path> にフォールバック
/// 3. いずれも失敗した場合は None
/// 4. 抽出した日付は chrono::NaiveDate でバリデーション
pub fn extract_date_from_path(file_path: &str, repo_root: &Path) -> Option<String> {
    // Step 1: ファイル名からの日付抽出
    if let Some(date) = extract_date_from_filename(file_path) {
        return Some(date);
    }

    // Step 2: git log フォールバック
    extract_date_from_git_log(file_path, repo_root)
}

/// ファイル名から先頭の YYYY-MM-DD パターンを抽出（^アンカー付き）
fn extract_date_from_filename(file_path: &str) -> Option<String> {
    let filename = Path::new(file_path).file_name()?.to_str()?;
    let date_str = DATE_RE.captures(filename).map(|c| c[1].to_string())?;
    // chrono::NaiveDate でバリデーション（不正な月・日を拒否）
    chrono::NaiveDate::parse_from_str(&date_str, "%Y-%m-%d").ok()?;
    Some(date_str)
}

/// git log から最終コミット日を取得
fn extract_date_from_git_log(file_path: &str, repo_root: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["log", "--format=%ai", "-1", "--", file_path])
        .current_dir(repo_root)
        .output()
        .map_err(|e| {
            tracing::debug!("git log failed for {}: {}", file_path, e);
            e
        })
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.trim();
    if line.is_empty() {
        tracing::debug!("git log returned empty for {}", file_path);
        return None;
    }
    // "%ai" format: "2026-03-20 10:30:00 +0900" → "2026-03-20"
    // line.get(..10) で安全にスライス（パニック防止）
    let date_str = line.get(..10)?;
    // chrono::NaiveDate でバリデーション
    chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d").ok()?;
    Some(date_str.to_string())
}
```

> **設計レビュー反映事項**:
> - M1: 正規表現に `^` アンカーを追加し、ファイル名先頭のみマッチ
> - M2: `line[..10]` → `line.get(..10)?` でパニック防止
> - S1: `LazyLock` で正規表現コンパイル結果をキャッシュ
> - S2: `chrono::NaiveDate` で日付バリデーション追加
> - S4: `tracing::debug!` で git log 失敗時のログ出力

## 6. 構造体変更の詳細

### KnowledgeEntry (src/indexer/knowledge.rs:170-175)

```rust
pub struct KnowledgeEntry {
    pub issue_number: String,
    pub file_path: String,
    pub relation: KnowledgeRelation,
    pub doc_subtype: DocSubtype,
    pub date: Option<String>,  // 追加
}
```

### IssueDocumentEntry (src/indexer/knowledge.rs:177-183)

```rust
pub struct IssueDocumentEntry {
    pub file_path: String,
    pub relation: KnowledgeRelation,
    pub doc_subtype: DocSubtype,
    pub date: Option<String>,  // 追加
}
```

### KnowledgeRelatedResult (src/indexer/knowledge.rs:186-193)

```rust
pub struct KnowledgeRelatedResult {
    pub file_path: String,
    pub relation: String,
    pub issue_number: String,
    pub title: Option<String>,
    pub doc_subtype: Option<DocSubtype>,
    pub date: Option<String>,  // 追加
}
```

### WhyDocumentEntry (src/output/mod.rs:410-416)

```rust
pub struct WhyDocumentEntry {
    pub file_path: String,
    pub relation: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc_subtype: Option<DocSubtype>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,  // 追加
}
```

## 7. メタデータ格納・取得フロー

### インデックス時（格納）

> **設計判断**: `parse_dev_report_path` のシグネチャは変更せず、日付取得は `scan_dev_reports` 内で別途行う（責務分離）。
> `scan_dev_reports` の `base_dir` はリポジトリルートであることを前提とする。

```
scan_dev_reports(base_dir)
  → parse_dev_report_path(path)  // KnowledgeEntry 生成（date: None）
  → extract_date_from_path(path, base_dir) // date 取得（別途呼出）
  → entry.date = extracted_date   // KnowledgeEntry に設定
  → insert_knowledge_entries()
    → metadata 構築:
      let mut meta = serde_json::json!({"doc_subtype": entry.doc_subtype.as_str()});
      if let Some(ref d) = entry.date {
          meta["date"] = serde_json::Value::String(d.clone());
      }
```

### クエリ時（取得）

```
find_documents_by_issue()
  → metadata JSON パース
  → let date = parsed.get("date").and_then(|v| v.as_str()).map(|s| s.to_string());
  → IssueDocumentEntry { ..., date }

find_knowledge_related()
  → metadata JSON パース
  → let date = parsed.get("date").and_then(|v| v.as_str()).map(|s| s.to_string());
  → KnowledgeRelatedResult { ..., date }
```

### issue --format json 変更（破壊的）

```rust
// format_json 内: grouped() の戻り値から オブジェクト配列を構築
for (category, docs) in result.grouped() {
    let entries: Vec<serde_json::Value> = docs.iter().map(|doc| {
        serde_json::json!({
            "file_path": doc.file_path,
            "date": doc.date
        })
    }).collect();
    map.insert(category, serde_json::Value::Array(entries));
}
```

> **スコープ**: issue コマンドの `human` / `llm` / `path` フォーマットでは date を表示しない（JSON のみ）。

## 8. 影響範囲

### 変更ファイル一覧

| ファイル | 変更内容 | 影響度 |
|---------|---------|--------|
| `src/indexer/knowledge.rs` | KnowledgeEntry/IssueDocumentEntry/KnowledgeRelatedResult に date 追加、日付取得ユーティリティ追加（`pub(crate)` 可視性） | 高 |
| `src/indexer/symbol_store.rs` | metadata に date 格納、find_documents_by_issue/find_knowledge_related で date パース | 高 |
| `src/output/mod.rs` | WhyDocumentEntry に date 追加 | 中 |
| `src/cli/issue.rs` | JSON出力をオブジェクト配列形式に変更 | 高（破壊的変更） |
| `src/cli/why.rs` | group_knowledge_results で date 転送 | 低 |
| `src/cli/suggest.rs` | KnowledgeDocResult の date フィールド追加に伴うテスト修正 | 低 |
| `src/cli/before_change.rs` | KnowledgeDocResult の date を意図的に無視（スコープ外） | 低 |
| `tests/e2e_issue.rs` | テストデータ・アサーション更新（オブジェクト配列対応） | 中 |

### テスト修正箇所（約40箇所）

- `src/cli/issue.rs` テスト: IssueDocumentEntry 初期化に `date: None` 追加（約8箇所）
- `src/cli/why.rs` テスト: WhyDocumentEntry 初期化に `date: None` 追加（約10箇所）
- `src/indexer/symbol_store.rs` テスト: KnowledgeEntry 初期化に `date: None` 追加（約10箇所以上）
- `src/cli/suggest.rs` テスト: KnowledgeDocResult 初期化に `date: None` 追加（約3箇所）
- `tests/e2e_issue.rs`: JSON スキーマ変更に伴うアサーション更新（オブジェクト配列対応）
- `src/indexer/knowledge.rs` テスト: `extract_date_from_filename` のユニットテスト追加（正常系・異常系）

### データ整合性

- re-index 前の既存 metadata には `date` フィールドが存在しない
- `find_documents_by_issue` / `find_knowledge_related` のパース処理で `date` キーが存在しない場合は `None` として安全に処理
- `commandindexdev index` 再実行で日付情報が再生成される

## 9. セキュリティ設計

| 脅威 | 対策 | 優先度 |
|------|------|--------|
| git log コマンドインジェクション | file_path を引数として直接渡す（シェル経由ではない `Command::new` 使用）+ `validate_git_file_path` 相当のパス検証を適用 | 高 |
| パストラバーサル | 既存の `parse_dev_report_path` のパス正規化を利用 | 中 |
| 不正な日付文字列 | `chrono::NaiveDate` による厳密なバリデーション（月・日範囲チェック含む） | 低 |

## 10. 品質基準

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |

## 11. スコープ外

- `--timeline` オプション（別Issue）
- `human` / `llm` / `path` フォーマットへの日付表示
- バッチ方式の git log 最適化（将来的な改善）
- `before-change` コマンドへの date 追加（将来的な拡張）
- KnowledgeEntry/IssueDocumentEntry/KnowledgeRelatedResult の共通フィールド抽出リファクタリング（M3: 別Issue）
- 破壊的変更のバージョニング・マイグレーションガイド（CHANGELOGで対応）
