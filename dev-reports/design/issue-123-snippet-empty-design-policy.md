# 設計方針書: Issue #123 - --with-snippet が空文字列を返す（related/impact）

## 1. Issue概要

| 項目 | 内容 |
|------|------|
| Issue番号 | #123 |
| タイトル | [BUG] --with-snippet が空文字列を返す（related/impact） |
| 深刻度 | 高 |
| 種別 | バグ修正 |

### 根本原因
`score_import_deps()` (related.rs:181) が `imp.target_module`（インポートパス形式: `@/components/Foo`）をscores HashMapのキーに使用。tantivyインデックスには実ファイルパス（`src/components/Foo.tsx`）で保存されているため、`fetch_snippet()` の `search_by_exact_path()` が完全一致せず空文字列を返す。

## 2. システムアーキテクチャ概要

### レイヤー構成と本修正の位置

```
CLI Layer (src/cli/)
  ├── index.rs        → インポート情報をSQLiteに保存（target_module未解決のまま）
  ├── search.rs       → related検索呼び出し
  ├── impact.rs       → impact検索呼び出し（aggregate_impact）
  └── snippet_helper.rs → fetch_snippet（search_by_exact_pathで取得）

Search Layer (src/search/)  ← 修正対象
  └── related.rs      → score_import_deps でインポートパス→ファイルパス変換が必要

Indexer Layer (src/indexer/)
  ├── reader.rs       → search_by_exact_path（STRING完全一致）+ all_indexed_paths追加
  ├── symbol_store.rs → dependencies テーブル + find_all_imports追加
  ├── schema.rs       → path: STRING|STORED
  └── writer.rs       → tantivyへの書き込み

Output Layer (src/output/)
  └── mod.rs          → RelatedSearchResult, ImpactFileResult（影響なし）
```

## 3. 設計方針

### 3.1 採用方針: 案A - score_import_deps 内でのパス解決

**理由**:
- 最も局所的な変更で安全性が高い
- 既存スキーマ変更不要
- 既存テストへの影響が最小

### 3.2 パス解決関数（独立関数として切り出し）

パス解決ロジックはSRP原則に従い、`related.rs` 内のトップレベル独立関数として実装する（`normalize_path` と同粒度）。

#### resolve_import_path

```rust
/// インポートパスをtantivyインデックスのファイルパスに解決する。
/// 解決できない場合（外部パッケージ等）は None を返す。
fn resolve_import_path(
    import_path: &str,
    indexed_paths: &HashSet<String>,
) -> Option<String> {
    // 入力バリデーション
    if import_path.is_empty() || import_path.len() > 1024 {
        return None;
    }

    // 1. 完全一致チェック（既にファイルパスの場合）
    if indexed_paths.contains(import_path) {
        return Some(import_path.to_string());
    }

    // 2. エイリアス/相対パスプレフィックス除去
    let normalized = import_path
        .trim_start_matches("@/")
        .trim_start_matches("~/")
        .trim_start_matches("./")
        .trim_start_matches("../");

    // 3. マッチング優先順位（厳密→緩い順）
    //    a. プレフィックス置換: @/xxx → src/xxx + 拡張子補完
    //    b. サフィックスマッチ（パスコンポーネント境界チェック付き）
    //    c. /index.ts パターン

    // 候補をコンポーネント境界付きサフィックスマッチでフィルタ
    let candidates: Vec<&String> = indexed_paths.iter()
        .filter(|p| path_component_suffix_matches(p, normalized))
        .collect();

    // 最初のマッチを返す（複数候補の複雑なタイブレークは不要）
    candidates.into_iter().next().cloned()
}
```

#### path_component_suffix_matches（コンポーネント境界チェック付き）

```rust
/// パスコンポーネント境界を考慮したサフィックスマッチ。
/// 'auth' が 'oauth' にマッチする誤検知を防ぐ。
fn path_component_suffix_matches(indexed_path: &str, import_suffix: &str) -> bool {
    let extensions = [".ts", ".tsx", ".js", ".jsx"];

    // 拡張子を除去してステムを取得（Path::file_stem相当）
    let stem = extensions.iter()
        .find(|ext| indexed_path.ends_with(*ext))
        .map(|ext| &indexed_path[..indexed_path.len() - ext.len()])
        .unwrap_or(indexed_path);

    // コンポーネント境界チェック: /suffix または完全一致
    let matches_at_boundary = |path: &str, suffix: &str| -> bool {
        path == suffix
            || path.ends_with(&format!("/{}", suffix))
    };

    matches_at_boundary(stem, import_suffix)
        || matches_at_boundary(indexed_path, import_suffix)
        // index.ts パターン: @/components/Foo → src/components/Foo/index.ts
        || matches_at_boundary(stem, &format!("{}/index", import_suffix))
}
```

### 3.3 インデックス済みパスリストの取得

`IndexReaderWrapper` に全パスリストを返すメソッドを追加。戻り値は `HashSet<String>` で完全一致チェックを高速化。

```rust
// src/indexer/reader.rs
pub fn all_indexed_paths(&self) -> Result<HashSet<String>, ReaderError> {
    // tantivyの全ドキュメントからpathフィールドを取得し重複排除
    // STRINGフィールドのため term dictionary API も利用可能だが、
    // 388ファイル規模では全ドキュメント走査+HashSetでO(N)で十分
}
```

### 3.4 キャッシュ戦略

`RelatedSearchEngine` のフィールドに `OnceCell<HashSet<String>>` で遅延キャッシュ。`find_related()` 初回呼び出し時に1回だけ取得し、以降再利用する。これにより `aggregate_impact` で複数回 `find_related` が呼ばれても全パスリスト取得は1回のみ。

```rust
pub struct RelatedSearchEngine<'a> {
    reader: &'a IndexReaderWrapper,
    store: &'a SymbolStore,
    indexed_paths: OnceCell<HashSet<String>>,  // 追加
}

impl<'a> RelatedSearchEngine<'a> {
    fn get_indexed_paths(&self) -> Result<&HashSet<String>, RelatedSearchError> {
        self.indexed_paths.get_or_try_init(|| {
            self.reader.all_indexed_paths()
                .map_err(|e| RelatedSearchError::IndexError(e.to_string()))
        })
    }
}
```

### 3.5 score_import_deps の修正

```rust
pub(crate) fn score_import_deps(
    &self,
    target: &str,
    scores: &mut HashMap<String, (f32, Vec<RelationType>)>,
) -> Result<(), RelatedSearchError> {
    let indexed_paths = self.get_indexed_paths()?;
    let mut resolve_cache: HashMap<String, Option<String>> = HashMap::new();

    // 順方向: このファイルがimportしているモジュール
    let imports = self.store.find_imports_by_source(target)?;
    for imp in &imports {
        let resolved = resolve_cache
            .entry(imp.target_module.clone())
            .or_insert_with(|| resolve_import_path(&imp.target_module, indexed_paths));

        if let Some(file_path) = resolved {
            add_relation(scores, file_path, IMPORT_DEP_WEIGHT, RelationType::ImportDependency);
        }
        // 解決失敗（外部パッケージ等）はスキップ
    }

    // 逆方向: このファイルをimportしているファイル
    let all_imports = self.store.find_all_imports()?;
    for imp in &all_imports {
        let resolved = resolve_cache
            .entry(imp.target_module.clone())
            .or_insert_with(|| resolve_import_path(&imp.target_module, indexed_paths));

        if let Some(file_path) = resolved {
            if file_path == target {
                add_relation(scores, &imp.source_file, IMPORT_DEP_WEIGHT, RelationType::ImportDependency);
            }
        }
    }

    Ok(())
}
```

### 3.6 共通ヘルパー: add_relation

スコア加算パターンの重複排除（DRY原則）:

```rust
fn add_relation(
    scores: &mut HashMap<String, (f32, Vec<RelationType>)>,
    path: &str,
    weight: f32,
    relation: RelationType,
) {
    let entry = scores.entry(path.to_string()).or_insert((0.0, Vec::new()));
    entry.0 += weight;
    if !entry.1.iter().any(|r| std::mem::discriminant(r) == std::mem::discriminant(&relation)) {
        entry.1.push(relation);
    }
}
```

### 3.7 外部パッケージの扱い

`resolve_import_path()` が `None` を返したインポート（`react`, `next/router` 等）はscoresに追加しない。結果に含まれないため、snippetの取得も発生しない。

### 3.8 エラーハンドリング

- `all_indexed_paths()` エラー時: `RelatedSearchError` を伝搬（`find_related` 全体が失敗）
- `resolve_import_path()` 解決失敗: `None` 返却（エラーではない）
- `find_all_imports()` エラー時: `RelatedSearchError` を伝搬

## 4. 変更対象ファイル

| ファイル | 変更内容 | 影響度 |
|---------|---------|--------|
| `src/search/related.rs` | `resolve_import_path`, `path_component_suffix_matches`, `add_relation` 追加、`score_import_deps` 修正、`RelatedSearchEngine` に `OnceCell` フィールド追加 | **高** |
| `src/indexer/reader.rs` | `all_indexed_paths()` メソッド追加 | 中 |
| `src/indexer/symbol_store.rs` | `find_all_imports()` メソッド追加 | 低 |
| `tests/e2e_related_search.rs` | import依存+snippet テスト追加、パス検証強化 | 中 |
| `tests/e2e_impact.rs` | impact+snippet テスト追加 | 低 |

## 5. 変更しないファイル

| ファイル | 理由 |
|---------|------|
| `src/cli/snippet_helper.rs` | score_import_deps修正で連鎖解決 |
| `src/cli/index.rs` | インデックス保存ロジックは変更不要 |
| `src/output/mod.rs` | 構造体変更なし |
| `src/parser/typescript.rs` | パーサは変更不要 |

## 6. 設計判断とトレードオフ

### 判断1: 案A（検索時解決）vs 案B（インデックス時解決）

| 観点 | 案A（採用） | 案B |
|------|-----------|-----|
| 変更範囲 | related.rs + reader.rs + symbol_store.rs | index.rs + symbol_store.rs + マイグレーション |
| スキーマ変更 | 不要 | 必要 |
| パフォーマンス | 検索時にO(N)マッチング | インデックス時に1回解決 |
| 互換性 | 完全互換 | 既存インデックス再構築必要 |

**判断**: 初期リリースでは案Aを採用。パフォーマンスが問題になれば将来的に案Bへ移行。

### 判断2: コンポーネント境界付きサフィックスマッチ vs 厳密パス解決

厳密なパスエイリアス解決（tsconfig.json の paths 読み取り等）は複雑。
コンポーネント境界付きサフィックスマッチは `auth` が `oauth` に誤マッチする問題を防ぎつつ、シンプルに実装できる。

**判断**: コンポーネント境界チェック付きサフィックスマッチを採用。

### 判断3: 外部パッケージの扱い

`react`, `next/router` 等はtantivyインデックスに存在しない。

**判断**: 解決できないインポートパスはscoresに追加しない（スキップ）。結果に含まれない。

### 判断4: 逆方向ルックアップの実装方式

`find_imports_by_target()` はtarget_module完全一致でインポートパスにマッチしない。

**判断**: `find_all_imports()` で全依存関係を取得し、resolve_cacheを使ってtarget_moduleを解決、targetと一致するものをフィルタする。`symbol_store.rs` に `find_all_imports()` メソッドを追加。

### 判断5: キャッシュ方式

**判断**: `RelatedSearchEngine` に `OnceCell<HashSet<String>>` を追加。`aggregate_impact` 等で複数回 `find_related` が呼ばれても1回のみ取得。テスト時にもコンストラクタ経由で注入可能。

## 7. セキュリティ設計

| 脅威 | 対策 | 優先度 |
|------|------|--------|
| パストラバーサル | インデックス済みパスリストのみを候補（外部パス生成しない） | 高 |
| サフィックスマッチ誤マッチ | パスコンポーネント境界チェック（`/suffix` or 完全一致） | 高 |
| 入力バリデーション | resolve_import_path冒頭で長さ・空文字チェック | 中 |
| DoS（メモリ） | indexed_paths はOnceCell で1回のみ取得、resolve_cacheで重複排除 | 中 |

## 8. テスト方針

### ユニットテスト（related.rs #[cfg(test)]）
- `resolve_import_path`: 相対パス、エイリアス、外部パッケージ、index.tsパターン
- `path_component_suffix_matches`: 境界チェック（auth vs oauth）、拡張子補完
- `add_relation`: スコア加算・重複排除

### E2Eテスト
- import依存関係 + `--with-snippet` でsnippet非空
- 結果のfile_pathが実ファイルパス（完全一致検証）
- impact + `--with-snippet` テスト

## 9. 品質基準

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |

## 10. 受け入れ基準（Issueより）

1. 相対パスインポートの結果で snippet が非空
2. エイリアスインポート（`@/xxx`）でも snippet が非空
3. 外部パッケージは結果に含まれない（スキップ）
4. json/llm 両フォーマットで正しく出力
5. related検索結果の file_path が実ファイルパス
6. impact検索結果も実ファイルパス
7. `cargo test --all` 全パス
8. `cargo clippy` 警告0件
9. import依存関係のE2E+ユニットテスト追加
