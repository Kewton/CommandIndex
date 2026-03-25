# 仮説検証レポート - Issue #168

## 総合判定表

| 仮説 | 判定 | 根拠 |
|---|---|---|
| 1. `issue --format llm` はパスのみ | ✅ Confirmed | issue.rs の format_llm がパスのみ出力 |
| 2. `before-change` の snippet は null | ✅ Confirmed | BeforeChangeFinding に snippet フィールドなし |
| 3. 先頭N文字をスニペットとして付与 | ⚠️ Partially Confirmed | snippet_helper 基盤は存在するが before-change に未適用 |
| 4. has_design で採用方針/結論セクション優先抽出 | ❌ Unverifiable | セクション優先抽出ロジック未実装 |
| 5. has_review で Executive Summary 優先抽出 | ❌ Unverifiable | セクション優先抽出ロジック未実装 |
| 6. has_workplan で Phase一覧要約を抽出 | ❌ Unverifiable | セクション優先抽出ロジック未実装 |
| 7. human/llm でインライン、json で snippet フィールド | ⚠️ Partially Confirmed | フォーマット分岐は存在するが snippet フィールド未定義 |

## 詳細

### 仮説1: issue --format llm はパスのみ (Confirmed)
- `src/cli/issue.rs` の `format_llm()` はファイルパスのみ出力
- `IssueDocumentEntry` は file_path, relation, doc_subtype のみ

### 仮説2: before-change snippet は null (Confirmed)
- `BeforeChangeFinding` (output/mod.rs) に snippet フィールドが存在しない
- human/json/llm どのフォーマットでも snippet は出力されない

### 仮説3: 先頭N文字スニペット (Partially Confirmed)
- `src/cli/snippet_helper.rs` に `fetch_snippet()` が実装済み
- impact コマンドや related 検索で既に使用されている
- tantivy インデックスの body フィールドは STORED で保存済み
- **ただし** before-change/issue コマンドではこのメカニズムが未適用

### 仮説4-6: セクション優先抽出 (Unverifiable)
- 現在のコードにはセクション指定抽出ロジックが存在しない
- fetch_snippet() はパスベースで最初の非空ドキュメントを返すのみ
- 将来の拡張として実装が必要

### 仮説7: フォーマット別表示 (Partially Confirmed)
- human/json/llm/path のフォーマット分岐は実装済み
- snippet フィールド追加後は比較的容易に対応可能

## 実装に必要な主要変更箇所

1. **BeforeChangeFinding** に `snippet: Option<String>` 追加 (output/mod.rs)
2. **IssueDocumentEntry** に `snippet: Option<String>` 追加
3. **before-change** コマンドで snippet_helper を使用して snippet 付与
4. **issue** コマンドで snippet 付与
5. **各フォーマッタ** (human.rs, llm.rs, json.rs) で snippet 表示
