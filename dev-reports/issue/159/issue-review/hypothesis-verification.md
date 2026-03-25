# 仮説検証レポート: Issue #159

## 検証対象の仮説

### 仮説1: `--limit` がドキュメント単位で適用される
**判定: Confirmed**

`src/cli/before_change.rs` 行408:
```rust
let limited_findings: Vec<BeforeChangeFinding> = findings.into_iter().take(limit).collect();
```

`BeforeChangeFinding` はドキュメント単位の構造体であり、`.take(limit)` はドキュメント数でカットしている。

### 仮説2: Issue #104が7件消費し、#112が3件でlimit=10到達
**判定: Confirmed（ロジック上整合）**

- `find_knowledge_by_issue()` は全Issue×全ドキュメントを制限なしで返す
- セマンティックランキング後に `.take(limit)` で切り捨て
- Issueごとのドキュメント数に偏りがある場合、先頭Issueが多くの枠を消費する

### 仮説3: whyコマンドは全Issue表示できる
**判定: Confirmed**

`src/cli/why.rs` ではIssue単位でグループ化(`group_knowledge_results()`)し、limitオプション自体が存在しない。全Issueが常に表示される。

## コードベース照合結果

| 項目 | before-change | why |
|------|---|---|
| limit オプション | あり（デフォルト10） | なし |
| 返り値単位 | BeforeChangeFinding（ドキュメント単位） | WhyIssueEntry（Issue単位） |
| 全Issue表示 | limitで制限 | 制限なし |

## 根本原因

`before-change` の limit は「表示するドキュメント数」を制限するが、「表示するIssue数」の制限メカニズムがない。そのため、ドキュメント数が多いIssueが先に枠を消費し、後続Issueの情報が完全に欠落する。

## 関連ファイル

- CLI引数定義: `src/main.rs` 行262-263
- Limit適用: `src/cli/before_change.rs` 行408
- 知識グラフクエリ: `src/indexer/symbol_store.rs` 行929-991
- Whyコマンド: `src/cli/why.rs` 行72-119
- テスト: `tests/e2e_before_change.rs` 行262-285
