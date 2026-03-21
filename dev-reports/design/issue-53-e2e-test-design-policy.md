# 設計方針書: Issue #53 Phase 4 E2E 統合テスト（関連検索・Context Pack検証）

## 1. 概要

### Issue情報

| 項目 | 内容 |
|------|------|
| Issue番号 | #53 |
| タイトル | [Feature] Phase 4 E2E 統合テスト（関連検索・Context Pack検証） |
| ラベル | test |
| 依存Issue | #50（--related検索）, #51（Markdownリンク解析）, #52（Context Pack生成） — 全てマージ済み |

### 設計方針の要約

Phase 4で実装された関連検索（`--related`）とContext Pack（`context`コマンド）の機能を、CLIレベルのE2Eテストで網羅的に検証する。既存テスト（`e2e_related_search.rs`, `e2e_context_pack.rs`）は基本的なフロー検証を担っており、本Issueではそれらとの重複を避けつつ、Issue記載の8シナリオに特化した統合テストを追加する。

---

## 2. テスト構成設計

### 2.1 テストファイル配置

既存の2ファイルに追加テストを配置する方針を採用する。

| ファイル | 役割 | 追加テスト |
|---------|------|-----------|
| `tests/e2e_related_search.rs` | `--related` 検索の E2E テスト | シナリオ 1-4, 7 |
| `tests/e2e_context_pack.rs` | `context` コマンドの E2E テスト | シナリオ 5, 6, 8 |

### 2.2 テストモジュール構成

既存セットアップ関数を最大限再利用し、新規セットアップ関数は必要最小限に抑える。

```
tests/
├── common/mod.rs              # 既存: cmd(), run_index(), parse_jsonl() 等
├── e2e_related_search.rs      # 既存 + 新規テスト追加
│   ├── setup_linked_docs()          # 既存（シナリオ1, 2, 3で再利用）
│   ├── setup_import_chain()         # 新規（import依存関係検証用）
│   └── テスト関数群
└── e2e_context_pack.rs        # 既存 + 新規テスト追加
    ├── setup_context_docs()         # 既存（シナリオ5, 6で再利用）
    ├── setup_isolated_docs()        # 新規（関連なしファイル検証用）
    └── テスト関数群
```

### 2.3 既存テストとの関係

| 既存テスト | 検証範囲 | 新規テストとの差分 |
|-----------|---------|------------------|
| `related_search_finds_linked_files` | MarkdownLink検出の基本確認 | 新規は`relations`配列内の具体的なtype値を検証 |
| `related_search_json_format_has_score_and_relations` | JSON構造の存在確認 | 新規はrelations内のtype文字列を厳密検証 |
| `related_search_conflicts_with_query` | query排他のみ | 新規は--tagとの排他を代表テストとして検証 |
| `context_pack_outputs_valid_json` | JSON構造の基本確認 | 新規はcontext配列内の各フィールド詳細を検証 |
| `context_pack_max_files_limits_output` | --max-files制限 | 新規は--max-tokens制限を検証 |

---

## 3. テストデータ設計

### 3.1 セットアップ関数の設計方針

**DRY原則に基づき、既存セットアップ関数を最大限再利用する。** `setup_tag_match_docs()` は `setup_linked_docs()` とほぼ同一構造のため新設しない。

#### セットアップ関数一覧

| 関数名 | 生成ファイル | 用途 |
|--------|------------|------|
| `setup_linked_docs()` | 既存: docs/a.md(auth,security), b.md(auth), c.md(security), d.md(unrelated) | シナリオ1, 2, 3で再利用 |
| `setup_import_chain()` | src/a.ts, src/b.ts, src/c.ts（importチェーン） | シナリオ4 |
| `setup_context_docs()` | 既存: docs/a.md, b.md + src/c.ts, d.ts | シナリオ5, 6で再利用 |
| `setup_isolated_docs()` | docs/alone.md（リンク・タグなし単独ファイル） | シナリオ8 |

### 3.2 テストデータの構成

#### シナリオ2: タグ一致テストデータ（setup_linked_docs()を再利用）

既存の`setup_linked_docs()`で生成されるファイルを活用:
- a.md(tags: auth, security) で `--related` 実行時
- b.md(tags: auth) が `tag_match` として検出（auth一致）
- c.md(tags: security) が `tag_match` として検出（security一致）
- d.md(tags: unrelated) は検出されない

#### シナリオ3: パス近接性スコアブースト（setup_linked_docs()を再利用）

既存の`setup_linked_docs()`のa.md→b.md（MarkdownLink + 同ディレクトリ）の関係を活用:
- a.mdで`--related`実行時、b.mdに`markdown_link` + `directory_proximity`が付与される
- **比較検証**: 同ディレクトリのb.mdのスコアが、異なるディレクトリに配置した場合のスコアより高いことを検証

#### シナリオ4: import依存関係テストデータ

```
src/
├── a.ts   (import { foo } from './b')
├── b.ts   (import { bar } from './c'; export function foo() {})
└── c.ts   (export function bar() {})
```

- a.tsで`--related`実行時、b.tsが`import_dependency`として検出される

#### シナリオ8: 関連なしファイル（Context Pack）

```
docs/
└── alone.md  (タグなし、リンクなし、独立ファイル)
```

- `context docs/alone.md`実行時、context配列が空配列になることを検証

---

## 4. 各シナリオの設計

### 重要: JSON出力フォーマットの実装仕様

テスト実装時に注意すべきJSON出力の仕様:

| 項目 | 設計方針書の記載（修正後） | 実装の実態 |
|------|------------------------|-----------|
| フィールド名 | `relations` | `src/output/json.rs` L53 で `relations` として出力 |
| RelationType表現 | snake_case文字列 | `markdown_link`, `import_dependency`, `directory_proximity` 等 |
| TagMatchの表現 | オブジェクト形式 | `{"tag_match": ["auth", "security"]}` |
| related検索の出力形式 | JSONL（1行1JSON） | 各行を個別にパース |
| context コマンドの出力形式 | pretty-printed JSON | 全体を1つのJSONとしてパース |
| contextの `--format` | なし | contextコマンドは常にJSON出力 |
| ContextEntry.relation値 | 実装の文字列 | `linked`, `import_dependency`, `tag_match`, `path_similarity`, `directory_proximity` |

### シナリオ1: --related フルフロー

| 項目 | 内容 |
|------|------|
| テスト関数名 | `related_full_flow_verifies_relation_types` |
| セットアップ | `setup_linked_docs()`（既存） |
| 実行コマンド | `search --related docs/a.md --format json` |
| 検証内容 | (1) b.mdとc.mdが結果に含まれる (2) `relations`配列に`"markdown_link"`が**含まれる**（包含チェック） (3) d.mdが結果に含まれない |
| 既存との差分 | 既存はパスの存在のみ検証。新規はrelations配列内のtype値を検証 |

### シナリオ2: タグ一致による関連

| 項目 | 内容 |
|------|------|
| テスト関数名 | `related_tag_match_detects_shared_tags` |
| セットアップ | `setup_linked_docs()`（既存を再利用） |
| 実行コマンド | `search --related docs/a.md --format json` |
| 検証内容 | (1) b.md, c.mdのrelationsに`{"tag_match": [...]}`が**含まれる** (2) matched_tagsが正しい（b.md: ["auth"], c.md: ["security"]） (3) d.mdは検出されない |

### シナリオ3: パス近接性によるスコアブースト

| 項目 | 内容 |
|------|------|
| テスト関数名 | `related_directory_proximity_boosts_score` |
| セットアップ | `setup_linked_docs()`（既存を再利用） |
| 実行コマンド | `search --related docs/a.md --format json` |
| 検証内容 | (1) 同ディレクトリのb.mdのrelationsに`"directory_proximity"`が**含まれる** (2) スコアが`markdown_link`のみの場合より高い（ブースト効果の検証） |

### シナリオ4: import依存関係

| 項目 | 内容 |
|------|------|
| テスト関数名 | `related_import_dependency_detects_ts_imports` |
| セットアップ | `setup_import_chain()`（新規） |
| 実行コマンド | `search --related src/a.ts --format json` |
| 検証内容 | (1) b.tsが検出される (2) relationsに`"import_dependency"`が**含まれる** |

### シナリオ5: Context Pack出力詳細検証

| 項目 | 内容 |
|------|------|
| テスト関数名 | `context_pack_entry_fields_are_enriched` |
| セットアップ | `setup_context_docs()`（既存） |
| 実行コマンド | `context docs/a.md`（--formatオプションなし、常にJSON出力） |
| 検証内容 | context配列の各エントリに (1) path (2) relation (3) score が存在する。relationの値が`"linked"`, `"import_dependency"`, `"tag_match"`, `"path_similarity"`, `"directory_proximity"`のいずれかである |

### シナリオ6: Context Pack --max-tokens 制限

| 項目 | 内容 |
|------|------|
| テスト関数名 | `context_pack_max_tokens_limits_output` |
| セットアップ | `setup_context_docs()`（既存） |
| 実行コマンド | `context docs/a.md --max-tokens 10` |
| 検証内容 | (1) context配列のサイズが制限なし時より小さいか等しい (2) JSON出力が有効 |

### シナリオ7: 排他制御の代表検証

**YAGNI原則に基づき、排他制御テストは代表1本に削減。** clapの`conflicts_with_all`で一括宣言されているため、代表1つの検証で十分。既存テスト（query, symbol）と合わせて3パターンでカバー。

| 項目 | 内容 |
|------|------|
| テスト関数名 | `related_conflicts_with_tag` |
| セットアップ | なし（排他制御はCLIパース時に検出） |
| 実行コマンド | `search --related docs/a.md --tag auth` |
| 検証内容 | exit code非ゼロで失敗すること |

### シナリオ8: 関連なしファイル（context コマンド）

| 項目 | 内容 |
|------|------|
| テスト関数名 | `context_pack_empty_context_for_isolated_file` |
| セットアップ | `setup_isolated_docs()`（新規） |
| 実行コマンド | `context docs/alone.md` |
| 検証内容 | (1) JSON出力が有効 (2) context配列が空配列 (3) summary.total_relatedが0 (4) summary.includedが0 |

---

## 5. 設計判断とトレードオフ

### 5.1 既存ファイルへの追加 vs 新規ファイル作成

**判断: 既存ファイルへの追加**

既存の`e2e_related_search.rs`（218行）と`e2e_context_pack.rs`（197行）はいずれも小規模であり、追加しても管理可能なサイズに収まる。セットアップ関数の再利用メリットが大きい。

### 5.2 セットアップ関数の再利用方針（DRY原則）

**判断: 既存セットアップ関数を最大限再利用**

`setup_tag_match_docs()` は既存の `setup_linked_docs()` とタグ構成・ファイル数がほぼ同一のため新設しない。`setup_dir_proximity_docs()` も `setup_linked_docs()` で十分。新規セットアップ関数は `setup_import_chain()` と `setup_isolated_docs()` の2つのみ。

### 5.3 排他制御テストの削減（YAGNI原則）

**判断: 4テスト → 1テストに削減**

clapの`conflicts_with_all`で一括宣言されているため、個別フィルタごとのテストは冗長。代表1本（--tag）で十分。既存テスト（query, symbol）と合わせて3パターンでカバー。

### 5.4 relation_types検証方法（開放閉鎖原則）

**判断: 包含チェックを採用、排他チェックは行わない**

`assert!(relations.iter().any(|r| r == "markdown_link"))` のような包含チェックに留め、`assert_eq!(relations.len(), N)` のような排他チェックは避ける。将来RelationTypeが追加されてもテストが壊れない。

### 5.5 JSON出力のパース方法

| コマンド | 出力形式 | パース方法 |
|---------|---------|-----------|
| `search --format json` | JSONL（1行1JSON） | 各行を `serde_json::from_str` で個別パース |
| `context` | pretty-printed JSON | 全体を `serde_json::from_str` で一括パース |

---

## 6. 影響範囲

### 変更対象ファイル

| ファイル | 変更内容 |
|---------|---------|
| `tests/e2e_related_search.rs` | 5テスト関数 + 1セットアップ関数追加 |
| `tests/e2e_context_pack.rs` | 3テスト関数 + 1セットアップ関数追加 |

### 変更しないファイル

| ファイル | 理由 |
|---------|------|
| `src/**/*.rs` | テスト専用の変更であり、プロダクションコードの変更は不要 |
| `tests/common/mod.rs` | 既存ユーティリティで十分 |
| `Cargo.toml` | 新規依存crateの追加は不要 |

### 副作用リスク

- テスト追加のみのため、既存機能への影響なし
- 各テストは独立した一時ディレクトリを使用するため、テスト間の干渉なし
- CI実行時間への影響は軽微（各テストはインデックス構築+検索のみで数秒以内）

---

## 7. 品質基準

| チェック項目 | コマンド | 基準 |
|---|---|---|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス（既存+新規） |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |

### テスト実行確認

```bash
# 新規テストのみ実行
cargo test --test e2e_related_search -- related_full_flow_verifies
cargo test --test e2e_related_search -- related_tag_match
cargo test --test e2e_related_search -- related_directory_proximity
cargo test --test e2e_related_search -- related_import_dependency
cargo test --test e2e_related_search -- related_conflicts_with_tag
cargo test --test e2e_context_pack -- context_pack_entry_fields
cargo test --test e2e_context_pack -- context_pack_max_tokens
cargo test --test e2e_context_pack -- context_pack_empty_context

# 全テスト実行
cargo test --all
```
