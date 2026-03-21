# 作業計画: Issue #53

## Issue概要

**タイトル**: [Feature] Phase 4 E2E 統合テスト（関連検索・Context Pack検証）

既存の `tests/e2e_related_search.rs`（10テスト）と `tests/e2e_context_pack.rs`（7テスト）に、設計方針書で定義された8つのE2Eテストシナリオを追加する。プロダクションコード変更なし。

### 現状のテスト構成
- `e2e_related_search.rs`: `setup_linked_docs()` + 10テスト関数
- `e2e_context_pack.rs`: `setup_context_docs()` + `parse_context_pack()` + 7テスト関数
- `common/mod.rs`: `cmd()`, `run_index()`, `parse_jsonl()` 等のヘルパー

### 追加対象（8テスト）
| # | ファイル | テスト関数名 |
|---|---|---|
| 1 | e2e_related_search.rs | `related_full_flow_verifies_relation_types` |
| 2 | e2e_related_search.rs | `related_tag_match_detects_shared_tags` |
| 3 | e2e_related_search.rs | `related_directory_proximity_boosts_score` |
| 4 | e2e_related_search.rs | `related_import_dependency_detects_ts_imports` |
| 5 | e2e_related_search.rs | `related_conflicts_with_tag` |
| 6 | e2e_context_pack.rs | `context_pack_entry_fields_are_enriched` |
| 7 | e2e_context_pack.rs | `context_pack_max_tokens_limits_output` |
| 8 | e2e_context_pack.rs | `context_pack_empty_context_for_isolated_file` |

---

## タスク分解

### Phase 1: e2e_related_search.rs テスト追加

#### Task 1.1: `setup_import_chain()` 関数追加
- **目的**: import_dependency 検証用のテストフィクスチャ
- **依存**: なし
- **工数目安**: 小

#### Task 1.2: `related_full_flow_verifies_relation_types` テスト
- **目的**: `relations` 配列内の `markdown_link` 文字列を検証
- **依存**: 既存 `setup_linked_docs()` を再利用
- **工数目安**: 小

#### Task 1.3: `related_tag_match_detects_shared_tags` テスト
- **目的**: `tag_match` オブジェクト（`{"tag_match": [...]}` 形式）の検証
- **依存**: 既存 `setup_linked_docs()` を再利用
- **工数目安**: 小

#### Task 1.4: `related_directory_proximity_boosts_score` テスト
- **目的**: `directory_proximity` 包含とスコアブースト検証
- **依存**: 既存 `setup_linked_docs()` を再利用（同ディレクトリ `docs/` 内配置）
- **工数目安**: 小

#### Task 1.5: `related_import_dependency_detects_ts_imports` テスト
- **目的**: TypeScript import チェーンの `import_dependency` 検出検証
- **依存**: Task 1.1 (`setup_import_chain()`)
- **工数目安**: 中

#### Task 1.6: `related_conflicts_with_tag` テスト
- **目的**: `--related` と `--tag` の排他制御テスト
- **依存**: 既存 `setup_linked_docs()` を再利用
- **工数目安**: 小

### Phase 2: e2e_context_pack.rs テスト追加

#### Task 2.1: `setup_isolated_docs()` 関数追加
- **目的**: 孤立ファイル（リンク・タグ・import なし）の空コンテキスト検証用フィクスチャ
- **依存**: なし
- **工数目安**: 小

#### Task 2.2: `context_pack_entry_fields_are_enriched` テスト
- **目的**: `context` 配列の各エントリの詳細フィールド検証
- **依存**: 既存 `setup_context_docs()` を再利用
- **工数目安**: 中

#### Task 2.3: `context_pack_max_tokens_limits_output` テスト
- **目的**: `--max-tokens` オプションによるトークン制限検証
- **依存**: 既存 `setup_context_docs()` を再利用
- **工数目安**: 小

#### Task 2.4: `context_pack_empty_context_for_isolated_file` テスト
- **目的**: 孤立ファイルの `context` が空配列になることの検証
- **依存**: Task 2.1 (`setup_isolated_docs()`)
- **工数目安**: 小

### Phase 3: 品質チェック

#### Task 3.1: ビルド・lint・テスト実行
- `cargo build`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all`
- `cargo fmt --all -- --check`

---

## 各タスクの詳細（実装ガイド）

### Task 1.1: `setup_import_chain()` 関数

TypeScript ファイル間の import チェーンを構築する。

```rust
fn setup_import_chain() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("create temp dir");
    let src = dir.path().join("src");
    std::fs::create_dir_all(&src).unwrap();

    // main.ts imports from ./helper
    std::fs::write(
        src.join("main.ts"),
        "import { helper } from './helper';\n\nexport function run() {\n  helper();\n}\n",
    ).unwrap();

    // helper.ts exports helper
    std::fs::write(
        src.join("helper.ts"),
        "export function helper() {\n  return 'help';\n}\n",
    ).unwrap();

    // unrelated.ts has no imports/exports to main or helper
    std::fs::write(
        src.join("unrelated.ts"),
        "export function standalone() {\n  return 'alone';\n}\n",
    ).unwrap();

    common::run_index(dir.path());
    dir
}
```

**ポイント**:
- `src/` ディレクトリ内に3ファイル配置
- `main.ts -> helper.ts` の import 関係を作る
- `unrelated.ts` は import 関係なし（ネガティブケース用）

---

### Task 1.2: `related_full_flow_verifies_relation_types`

`setup_linked_docs()` で a.md -> b.md のリンクがあるので、`relations` 配列に `"markdown_link"` 文字列が含まれることを検証。

```rust
#[test]
fn related_full_flow_verifies_relation_types() {
    let dir = setup_linked_docs();
    let output = common::cmd()
        .args(["search", "--related", "docs/a.md", "--format", "json"])
        .current_dir(dir.path())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let results = common::parse_jsonl(&stdout);

    // b.md を見つけて relations に "markdown_link" が含まれることを検証
    let b_result = results.iter().find(|r| {
        r["path"].as_str().map_or(false, |p| p.contains("b.md"))
    }).expect("b.md should be in related results");

    let relations = b_result["relations"].as_array()
        .expect("relations should be an array");
    let has_markdown_link = relations.iter().any(|r| r.as_str() == Some("markdown_link"));
    assert!(has_markdown_link,
        "relations for b.md should contain 'markdown_link', got: {relations:?}");
}
```

**検証内容**:
- `relations` が配列であること
- `"markdown_link"` 文字列がリテラルとして含まれること（`as_str()` で比較）

---

### Task 1.3: `related_tag_match_detects_shared_tags`

a.md (tags: auth security) と b.md (tags: auth) の共有タグ `auth` を検証。

```rust
#[test]
fn related_tag_match_detects_shared_tags() {
    let dir = setup_linked_docs();
    let output = common::cmd()
        .args(["search", "--related", "docs/a.md", "--format", "json"])
        .current_dir(dir.path())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let results = common::parse_jsonl(&stdout);

    let b_result = results.iter().find(|r| {
        r["path"].as_str().map_or(false, |p| p.contains("b.md"))
    }).expect("b.md should be in related results");

    let relations = b_result["relations"].as_array()
        .expect("relations should be an array");

    // TagMatch は {"tag_match": ["auth"]} 形式のオブジェクト
    let tag_match = relations.iter().find(|r| r.is_object() && r.get("tag_match").is_some());
    assert!(tag_match.is_some(),
        "relations should contain a tag_match object, got: {relations:?}");

    let matched_tags = tag_match.unwrap()["tag_match"].as_array()
        .expect("tag_match should be an array");
    let tag_strings: Vec<&str> = matched_tags.iter().filter_map(|t| t.as_str()).collect();
    assert!(tag_strings.contains(&"auth"),
        "tag_match should contain 'auth', got: {tag_strings:?}");
}
```

**検証内容**:
- `relations` 内に `{"tag_match": [...]}` 形式のオブジェクトが存在すること
- `tag_match` 配列内に共有タグ文字列が含まれること

---

### Task 1.4: `related_directory_proximity_boosts_score`

同一ディレクトリ（`docs/`）内のファイル間で `directory_proximity` が relations に含まれ、スコアがブーストされることを検証。

```rust
#[test]
fn related_directory_proximity_boosts_score() {
    let dir = setup_linked_docs();
    let output = common::cmd()
        .args(["search", "--related", "docs/a.md", "--format", "json"])
        .current_dir(dir.path())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let results = common::parse_jsonl(&stdout);

    // b.md は docs/ 内にある（a.md と同一ディレクトリ）ため directory_proximity が付くはず
    let b_result = results.iter().find(|r| {
        r["path"].as_str().map_or(false, |p| p.contains("b.md"))
    }).expect("b.md should be in related results");

    let relations = b_result["relations"].as_array()
        .expect("relations should be an array");
    let has_dir_proximity = relations.iter().any(|r| r.as_str() == Some("directory_proximity"));
    assert!(has_dir_proximity,
        "b.md should have directory_proximity relation, got: {relations:?}");

    // スコアがブーストされている（markdown_link(1.0) + tag_match(0.5) + directory_proximity(0.2) > 1.0）
    let score = b_result["score"].as_f64().expect("score should be a number");
    assert!(score > 1.0,
        "score should be boosted above 1.0 by directory_proximity, got: {score}");
}
```

**検証内容**:
- `relations` に `"directory_proximity"` が含まれること
- スコアが base weight（1.0）を超えていること（ブースト確認）

---

### Task 1.5: `related_import_dependency_detects_ts_imports`

`setup_import_chain()` を使い、TypeScript import による `import_dependency` 関連検出を検証。

```rust
#[test]
fn related_import_dependency_detects_ts_imports() {
    let dir = setup_import_chain();
    let output = common::cmd()
        .args(["search", "--related", "src/main.ts", "--format", "json"])
        .current_dir(dir.path())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let results = common::parse_jsonl(&stdout);

    // helper.ts が関連として検出されるべき
    let helper_result = results.iter().find(|r| {
        r["path"].as_str().map_or(false, |p| p.contains("helper.ts"))
    });
    assert!(helper_result.is_some(),
        "helper.ts should be related to main.ts via import, got paths: {:?}",
        results.iter().filter_map(|r| r["path"].as_str()).collect::<Vec<_>>());

    let relations = helper_result.unwrap()["relations"].as_array()
        .expect("relations should be an array");
    let has_import_dep = relations.iter().any(|r| r.as_str() == Some("import_dependency"));
    assert!(has_import_dep,
        "helper.ts should have import_dependency relation, got: {relations:?}");
}
```

**検証内容**:
- `main.ts` の関連に `helper.ts` が含まれること
- `relations` に `"import_dependency"` 文字列が含まれること

---

### Task 1.6: `related_conflicts_with_tag`

`--related` と `--tag` の排他制御（clap `conflicts_with_all` による）を検証。

```rust
#[test]
fn related_conflicts_with_tag() {
    let dir = setup_linked_docs();
    common::cmd()
        .args([
            "search",
            "--related", "docs/a.md",
            "--tag", "auth",
            "--format", "json",
        ])
        .current_dir(dir.path())
        .assert()
        .failure();
}
```

**検証内容**:
- `--related` と `--tag` を同時指定するとコマンドが失敗（exit code != 0）すること
- clap の `conflicts_with_all` 定義（main.rs L31: `conflicts_with_all = ["query", "symbol", "tag", ...]`）に基づく

---

### Task 2.1: `setup_isolated_docs()` 関数

リンク・タグ・import のいずれも持たない孤立ファイルのフィクスチャ。

```rust
fn setup_isolated_docs() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("create temp dir");
    let docs = dir.path().join("docs");
    std::fs::create_dir_all(&docs).unwrap();

    // isolated.md: タグなし、リンクなし
    std::fs::write(
        docs.join("isolated.md"),
        "# Isolated Page\nThis page has no links, no tags, and no connections.\n",
    ).unwrap();

    // other.md: 別ディレクトリに配置（directory_proximity も発生しない）
    let other = dir.path().join("other");
    std::fs::create_dir_all(&other).unwrap();
    std::fs::write(
        other.join("other.md"),
        "---\ntags: something\n---\n# Other Page\nUnrelated content.\n",
    ).unwrap();

    common::run_index(dir.path());
    dir
}
```

**ポイント**:
- `isolated.md` にフロントマター（tags）を付けない
- 別ディレクトリに `other.md` を配置して directory_proximity を回避

---

### Task 2.2: `context_pack_entry_fields_are_enriched`

`context` 配列の各エントリが `path`, `relation`, `score` フィールドを持ち、さらに `heading`/`snippet`/`symbols` のオプショナルフィールドが適切にセットされることを検証。

```rust
#[test]
fn context_pack_entry_fields_are_enriched() {
    let dir = setup_context_docs();
    let output = common::cmd()
        .args(["context", "docs/a.md"])
        .current_dir(dir.path())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let pack = parse_context_pack(&stdout);

    let context = pack["context"].as_array().expect("context array");
    assert!(!context.is_empty(), "context should not be empty");

    for entry in context {
        // 必須フィールド検証
        assert!(entry.get("path").is_some(), "entry should have 'path'");
        assert!(entry.get("relation").is_some(), "entry should have 'relation'");
        assert!(entry.get("score").is_some(), "entry should have 'score'");

        let path = entry["path"].as_str().unwrap();
        let relation = entry["relation"].as_str().unwrap();
        let score = entry["score"].as_f64().unwrap();

        assert!(!path.is_empty(), "path should not be empty");
        assert!(!relation.is_empty(), "relation should not be empty");
        assert!(score > 0.0, "score should be positive, got: {score}");

        // relation 値が既知の値であること
        let valid_relations = ["linked", "import_dependency", "tag_match", "path_similarity", "directory_proximity"];
        assert!(valid_relations.contains(&relation),
            "relation should be one of {valid_relations:?}, got: '{relation}'");
    }

    // b.md が linked として含まれ、heading が設定されていること
    let b_entry = context.iter().find(|e| {
        e["path"].as_str().map_or(false, |p| p.contains("b.md"))
    });
    if let Some(b) = b_entry {
        assert_eq!(b["relation"].as_str().unwrap(), "linked",
            "b.md should have relation 'linked'");
        // heading はオプションだが、b.md にはある ("# Page B")
        if let Some(heading) = b.get("heading") {
            assert!(heading.as_str().map_or(false, |h| !h.is_empty()),
                "heading should not be empty when present");
        }
    }
}
```

**検証内容**:
- 必須3フィールド（`path`, `relation`, `score`）の存在
- `relation` が既知の文字列値のいずれかであること
- linked 関係の場合に heading が設定されること（条件付き）

---

### Task 2.3: `context_pack_max_tokens_limits_output`

`--max-tokens` オプションで `estimated_tokens` が制限内に収まることを検証。

```rust
#[test]
fn context_pack_max_tokens_limits_output() {
    let dir = setup_context_docs();
    let output = common::cmd()
        .args(["context", "docs/a.md", "--max-tokens", "50"])
        .current_dir(dir.path())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let pack = parse_context_pack(&stdout);

    let summary = pack.get("summary").expect("should have summary");
    let estimated_tokens = summary["estimated_tokens"].as_u64().unwrap();

    // --max-tokens 50 指定時、estimated_tokens が 50 以下であること
    // 注: 最低1エントリは含まれるため、僅かに超える可能性はあるが概ね制限内
    assert!(estimated_tokens <= 100,
        "estimated_tokens should be limited by --max-tokens, got: {estimated_tokens}");

    // --max-tokens なしの場合と比較して context が少ないこと
    let output_unlimited = common::cmd()
        .args(["context", "docs/a.md"])
        .current_dir(dir.path())
        .assert()
        .success();
    let stdout_unlimited = String::from_utf8_lossy(&output_unlimited.get_output().stdout);
    let pack_unlimited = parse_context_pack(&stdout_unlimited);

    let unlimited_context = pack_unlimited["context"].as_array().unwrap();
    let limited_context = pack["context"].as_array().unwrap();

    // 制限あり版のコンテキストが制限なし版以下であること
    assert!(limited_context.len() <= unlimited_context.len(),
        "limited context ({}) should have <= entries than unlimited ({})",
        limited_context.len(), unlimited_context.len());
}
```

**検証内容**:
- `--max-tokens` 指定時に `estimated_tokens` が制限近辺に収まること
- 制限あり版のコンテキスト件数が制限なし版以下であること

---

### Task 2.4: `context_pack_empty_context_for_isolated_file`

孤立ファイルの `context` が空配列になることを検証。

```rust
#[test]
fn context_pack_empty_context_for_isolated_file() {
    let dir = setup_isolated_docs();
    let output = common::cmd()
        .args(["context", "docs/isolated.md"])
        .current_dir(dir.path())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let pack = parse_context_pack(&stdout);

    let context = pack["context"].as_array().expect("context should be an array");
    assert!(context.is_empty(),
        "isolated file should have empty context, got: {context:?}");

    let summary = pack.get("summary").expect("should have summary");
    assert_eq!(summary["total_related"].as_u64().unwrap(), 0,
        "total_related should be 0 for isolated file");
    assert_eq!(summary["included"].as_u64().unwrap(), 0,
        "included should be 0 for isolated file");
}
```

**検証内容**:
- `context` が空配列であること
- `summary.total_related` と `summary.included` が 0 であること

---

## 実装順序と依存関係

```
Phase 1 (e2e_related_search.rs):
  Task 1.1 (setup_import_chain)  ──> Task 1.5 (import_dependency)
  Task 1.2 (relation_types)       ... 独立
  Task 1.3 (tag_match)            ... 独立
  Task 1.4 (directory_proximity)  ... 独立
  Task 1.6 (conflicts_with_tag)   ... 独立

Phase 2 (e2e_context_pack.rs):
  Task 2.1 (setup_isolated_docs) ──> Task 2.4 (empty_context)
  Task 2.2 (entry_fields)         ... 独立
  Task 2.3 (max_tokens)           ... 独立

Phase 3: 全タスク完了後に品質チェック
```

## JSON出力仕様サマリ

### related 検索 (JSONL)
```json
{"path":"docs/b.md","score":1.7,"relations":["markdown_link",{"tag_match":["auth"]},"directory_proximity"]}
```
- `relations`: 配列。文字列（`"markdown_link"`, `"import_dependency"`, `"path_similarity"`, `"directory_proximity"`）またはオブジェクト（`{"tag_match": ["tag1","tag2"]}`）

### context pack (pretty-printed JSON)
```json
{
  "target_files": ["docs/a.md"],
  "context": [
    {
      "path": "docs/b.md",
      "relation": "linked",
      "score": 1.7,
      "heading": "Page B",
      "snippet": "Documentation for module B."
    }
  ],
  "summary": {
    "total_related": 3,
    "included": 2,
    "estimated_tokens": 150
  }
}
```
- `relation`: 文字列（`"linked"`, `"import_dependency"`, `"tag_match"`, `"path_similarity"`, `"directory_proximity"`）

## Definition of Done

- [ ] `tests/e2e_related_search.rs` に 5 テスト + 1 セットアップ関数が追加されている
- [ ] `tests/e2e_context_pack.rs` に 3 テスト + 1 セットアップ関数が追加されている
- [ ] プロダクションコード（`src/`）に変更がないこと
- [ ] `cargo build` がエラー0件で完了すること
- [ ] `cargo clippy --all-targets -- -D warnings` が警告0件であること
- [ ] `cargo test --all` で全テスト（既存 + 新規8テスト）がパスすること
- [ ] `cargo fmt --all -- --check` で差分がないこと
- [ ] 各テストが独立して実行可能であること（`cargo test <test_name>` で単体実行可能）
