# 設計方針書 - Issue #66 Phase 5 E2E統合テスト

## 1. 概要

Phase 5のSemantic Search、Hybrid Retrieval、Reranking機能を通したE2E統合テストを作成する。テストは「ライブラリAPI層（Ollama不要）」と「CLI層（Ollama依存）」の2層構成で設計する。

## 2. アーキテクチャ概要

### 2.1 テスト対象のモジュール関係

```
CLI層（CLI経由で間接テスト）
├── src/cli/search.rs       # run_semantic_search(), try_hybrid_search(), try_rerank()
│                             ※ try_rerank は非公開関数 → CLI経由で間接テスト
├── src/cli/embed.rs        # embed コマンド
└── src/cli/context.rs      # context コマンド

ライブラリAPI層（テストから直接呼び出し可能な公開API）
├── src/indexer/symbol_store.rs  # SymbolStore::open(), create_tables(),
│                                  insert_embeddings(), search_similar(), count_embeddings()
├── src/search/hybrid.rs         # rrf_merge() ※ pub fn
├── src/embedding/mod.rs         # EmbeddingProvider trait, create_provider()
└── src/embedding/store.rs       # EmbeddingStore (embeddings.db)
```

**注意**: `try_rerank()`は`src/cli/search.rs`内の非公開関数(`fn`、`pub`なし)であり、統合テストから直接呼び出せない。Rerankingテストは`commandindex search --rerank`のCLI経由で間接的にテストする。

### 2.2 Embedding保存先の2系統

| 保存先 | DB | 本番書込元 | 読取元 |
|--------|-----|--------|--------|
| EmbeddingStore | `.commandindex/embeddings.db` | `embed`, `index --with-embedding` | `embed` |
| SymbolStore | `.commandindex/symbols.db` | **本番経路では直接書込なし** | `search --semantic`, `search` (hybrid) |

**重要**: `embed` / `index --with-embedding` は `EmbeddingStore` にのみ保存する。`SymbolStore::insert_embeddings()` は公開APIだが、本番CLIフローでは使用されていない。本Issueでは**テストfixture投入専用**として `SymbolStore::insert_embeddings()` を利用する。

**テスト設計への影響**:
- semantic/hybrid検索テスト → SymbolStore(symbols.db)にテストfixture投入（本番経路とは異なる）
- embed/indexの保存確認 → embeddings.dbを検証

### 2.3 SymbolStoreの初期化手順

統合テストでは`SymbolStore::open_in_memory()`は利用不可(`#[cfg(test)]`修飾のため)。ファイルベースで初期化する:

```rust
// テストでのSymbolStore初期化パターン
let symbols_db = commandindex_dir.join("symbols.db");
let store = commandindex::indexer::symbol_store::SymbolStore::open(&symbols_db).unwrap();
store.create_tables().unwrap();  // open()後にcreate_tables()が必須
```

## 3. テスト層構成と責務

### 3.1 ライブラリAPI層テスト（Ollama不要・CI常時実行）

| テスト | 対象API | 検証内容 |
|--------|---------|----------|
| Embedding挿入確認 | `SymbolStore::insert_embeddings()` | 固定embedding投入→count_embeddings確認 |
| Semantic Search基本 | `SymbolStore::search_similar()` | コサイン類似度による順序検証 |
| Semantic Searchフィルタ | `search_similar()` の結果まで検証 | コサイン類似度結果の取得確認（フィルタ適用は非公開API内のため、CLI経由で外形確認） |
| Hybrid RRF統合 | `commandindex::search::hybrid::rrf_merge()` | BM25+Semantic結果の統合順序 |
| --no-semantic | CLI経由 `search --no-semantic` | hybrid経路を通らないことの確認 |
| embedding未生成 | CLI経由 `search` | embeddingなしでエラーなく動作 |

### 3.2 CLI層テスト（Ollama不要・CI常時実行）

| テスト | コマンド | 検証内容 |
|--------|---------|----------|
| embed Ollama停止時 | `embed` | 非ゼロ終了 + stderrエラー |
| Reranking --rerank受理 | `search --rerank` | CLI引数が受け付けられること（Ollamaなしフォールバック） |
| Reranking --rerank-top受理 | `search --rerank --rerank-top 5` | CLI引数が受け付けられること |
| context正常動作 | `context` | embedding存在下でContext Pack生成 |

### 3.3 環境依存テスト（ローカルOllama状態に依存・`#[ignore]`）

| テスト | コマンド | 前提条件 | 検証内容 |
|--------|---------|----------|----------|
| Hybrid自動切替 | `search <query>` | Ollama起動中 + embedding存在 | hybridモードで動作 |
| BM25フォールバック | `search <query>` | Ollama停止 + embedding存在 | hybrid→BM25フォールバック（成功終了、stderr警告あり） |

### 3.4 除外（既存テストで担保済み）

- Semantic Search排他制御 → `tests/cli_args.rs` の `search_semantic_conflicts_*` テスト群
- Reranking排他制御 → `tests/cli_args.rs` の `search_rerank_conflicts_*` テスト群

## 4. テストデータ設計

### 4.1 テストフィクスチャ

```
temp_dir/
├── guide.md          # 日本語、tags: [rust, tutorial] ← tag/typeフィルタ検証用
├── api.md            # 英語、tags: [api, http]       ← 複数結果の順序検証用
└── .commandindex/    # commandindex index で自動生成
    ├── symbols.db    # SymbolStore（テストfixture投入先）
    ├── embeddings.db # EmbeddingStore（embedコマンドテスト時に検証対象）
    └── config.toml   # CLI層テストで必要な場合のみ create_test_config() で追加
```

※ 上記は最大構成。ライブラリAPI層テストでは symbols.db のみ使用する場合もある。

テストデータは2ファイルに絞り実行時間を抑制する。各ファイルが必要な理由:
- `guide.md`: 日本語検索テスト、tagフィルタ検証
- `api.md`: 英語検索テスト、複数結果の類似度順序検証

### 4.2 固定Embedding設計

テスト用に4次元の固定ベクトルを使用。全テストで同一次元を使用し、`search_similar()`の次元フィルタと整合させる。

```rust
// 定数はテストファイル内で定義（テスト専用のため common に置かない）
const TEST_DIMENSION: usize = 4;
const QUERY_VEC: [f32; 4] = [1.0, 0.0, 0.0, 0.0];
const SIMILAR_VEC: [f32; 4] = [0.9, 0.1, 0.0, 0.0];   // cosine ≈ 0.994
const DIFFERENT_VEC: [f32; 4] = [0.0, 0.0, 1.0, 0.0];  // cosine = 0.0
```

### 4.3 EmbeddingInfo構造体

```rust
// commandindex::indexer::EmbeddingInfo をそのまま使用
pub struct EmbeddingInfo {
    pub file_path: String,
    pub section_heading: String,
    pub embedding: Vec<f32>,
    pub model_name: String,
    pub file_hash: String,
}
```

## 5. 共通ヘルパー設計

### 5.1 配置方針

ヘルパー関数は内部API型（SymbolStore等）を扱うため、`tests/common/mod.rs`ではなく**各テストファイル内にローカルヘルパーとして定義**する。これにより:
- 既存のcommon/mod.rsのCLI系ヘルパーと責務が混在しない
- commandindex crateの内部型への依存が他テストファイルに波及しない

既存の`tests/common/mod.rs`のCLI系ヘルパー（`cmd()`, `run_index()`, `run_search()`等）はそのまま再利用する。

### 5.2 テストファイル内ヘルパー

全ヘルパーは`Result`を返し、テスト側で`expect("説明")`する形で失敗点を局所化する。

```rust
// tests/e2e_semantic_hybrid.rs 内に定義
use std::error::Error;

const TEST_MODEL: &str = "test-model";
const TEST_HASH: &str = "test-hash";

/// テスト用Markdownファイルとインデックスをセットアップ
/// 「ディレクトリ構造とテストファイルの作成 + index実行」のみ担当
fn setup_semantic_test_dir() -> Result<(tempfile::TempDir, std::path::PathBuf), Box<dyn Error>> {
    // 1. tempfile::tempdir()で安全なランダムディレクトリ作成
    // 2. 2個のMarkdownファイル作成
    // 3. common::run_index() でインデックス構築
    // 4. Ok((TempDir, path))
}

/// SymbolStoreに固定embeddingを挿入する（insert_embeddingsの呼び出しを1箇所に集約）
fn insert_test_embeddings(
    symbols_db_path: &std::path::Path,
    embeddings: &[(String, String, Vec<f32>)],
) -> Result<(), Box<dyn Error>> {
    // 1. SymbolStore::open(path) で開く
    // 2. store.create_tables() でテーブル作成
    // 3. EmbeddingInfoを組み立てて insert_embeddings() を呼び出す
    //    model_name: TEST_MODEL, file_hash: TEST_HASH 固定
    // 4. Ok(())
}

/// テスト用config.tomlを作成（CLIでprovider解決が必要なテストのみ使用）
fn create_test_config(commandindex_dir: &std::path::Path) -> Result<(), Box<dyn Error>> {
    // [embedding]
    // provider = "ollama"
    // endpoint = "http://localhost:11434"  ← 外部宛先への接続を防止
    // ※ ライブラリAPI層テスト(search_similar, rrf_merge)では不要
}
```

### 5.3 各ヘルパーの呼び出し関係

テスト側が明示的に組み合わせる（Compose over Configure）:

```rust
#[test]
fn test_semantic_search_basic() {
    let (dir, path) = setup_semantic_test_dir();     // ファイル作成 + index
    let ci_dir = path.join(".commandindex");
    create_test_config(&ci_dir);                      // config.toml作成
    insert_test_embeddings(                           // embedding挿入
        &ci_dir.join("symbols.db"),
        &[("guide.md", "heading", SIMILAR_VEC.to_vec()), ...],
    );
    // 検索実行 & 検証
}
```

## 6. テストファイル構成

```
tests/
├── common/mod.rs                  # 既存: 変更なし（CLI系ヘルパーのみ）
├── e2e_semantic_hybrid.rs         # 新規: 全12テスト（ライブラリAPI層 + CLI層混在）
└── e2e_embedding.rs               # 既存: 変更なし
```

### 6.1 e2e_semantic_hybrid.rs のテスト構成

```rust
mod common;
use commandindex::indexer::symbol_store::{SymbolStore, EmbeddingInfo};
use commandindex::search::hybrid::rrf_merge;

// === ローカルヘルパー（全てResult返却） ===
fn setup_semantic_test_dir() -> Result<(tempfile::TempDir, std::path::PathBuf), Box<dyn std::error::Error>> { ... }
fn insert_test_embeddings(...) -> Result<(), Box<dyn std::error::Error>> { ... }
fn create_test_config(...) -> Result<(), Box<dyn std::error::Error>> { ... }

// === ライブラリAPI層テスト（Ollama不要） ===
#[test]
fn test_embedding_insert_and_count() { ... }       // シナリオ1

#[test]
fn test_semantic_search_basic() { ... }            // シナリオ3

#[test]
fn test_semantic_search_filter() { ... }           // シナリオ4

#[test]
fn test_rrf_merge_integration() { ... }            // RRF統合

// === CLI層テスト（Ollama不要） ===
#[test]
fn test_embed_without_ollama_fails() { ... }       // シナリオ2

#[test]
fn test_hybrid_no_semantic() { ... }               // シナリオ7

#[test]
fn test_hybrid_no_embeddings() { ... }             // シナリオ8

#[test]
fn test_rerank_fallback_via_cli() { ... }          // シナリオ10（CLI経由）

#[test]
fn test_rerank_top_accepted_via_cli() { ... }      // シナリオ11（CLI経由）

#[test]
fn test_context_with_embeddings() { ... }          // シナリオ13

// === CLI層テスト（Ollama依存） ===
#[test]
#[ignore]
fn test_hybrid_auto_switch() { ... }               // シナリオ6

#[test]
#[ignore]
fn test_hybrid_bm25_fallback() { ... }             // シナリオ9
```

## 7. 設計判断とトレードオフ

### 7.1 ライブラリAPI層 vs CLI層の分離

**判断**: 検索時のクエリembedding生成がOllama依存のため、CLI経由のsemantic/hybridテストは`#[ignore]`とする。

**トレードオフ**:
- (+) CIで安定実行可能
- (-) CLI統合ロジック（hybrid自動切替等）はCIで検証されない
- (対策) ライブラリAPI層で個別ロジック（rrf_merge, search_similar等）を十分カバーする

### 7.2 固定embeddingの直接挿入方式

**判断**: MockProviderが`pub(crate)`で統合テストから利用不可のため、SymbolStoreに直接固定embeddingを挿入する。

**トレードオフ**:
- (+) Ollamaなしでembedding検索をテスト可能
- (-) CLI→embedding生成→保存の統合パスはテストされない
- (対策) CLI統合パスは`#[ignore]`テストでカバー

### 7.3 新規公開API追加なし

**判断**: 本Issue では本体crateの新規公開API追加を行わない。

**理由**: テスト追加のためだけにプロダクションコードの公開面を変更すると、影響範囲が拡大する。必要な場合は別Issueで対応する。

### 7.4 try_rerankのテスト方針

**判断**: `try_rerank()`は非公開関数のため、直接テストではなくCLI経由（`search --rerank`）で間接テストする。

**理由**: 7.3の方針に従い、テストのためにpub化しない。CLI経由で「Ollamaなし環境で--rerankオプション付き検索が元順序を維持して結果を返す」ことを確認する。

### 7.5 ヘルパー関数の配置

**判断**: SymbolStore等の内部API型を扱うヘルパーはテストファイル内に定義し、`tests/common/mod.rs`には追加しない。

**理由**: 既存common/mod.rsはCLI系ヘルパーのみで構成されており、内部API型への依存を持ち込むと他テストファイルへの影響が生じる。

## 8. 影響範囲

### 変更対象

| ファイル | 変更種別 | 内容 |
|----------|----------|------|
| `tests/e2e_semantic_hybrid.rs` | 新規 | E2Eテスト12件 + ローカルヘルパー3件 |

### 影響なし

| ファイル | 理由 |
|----------|------|
| `src/` 配下全て | プロダクションコード変更なし |
| `tests/common/mod.rs` | 変更なし（既存CLI系ヘルパー `cmd()`, `run_index()`, `run_search()` 等を依存先として利用） |
| `tests/e2e_embedding.rs` | 既存テスト変更なし |
| `tests/cli_args.rs` | 排他制御テスト追加なし |
| `Cargo.toml` | dev-dependencies追加なし |
| `.github/workflows/ci.yml` | CI設定変更なし |

## 9. 品質基準

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス（`#[ignore]`除く） |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |

## 10. セキュリティ設計

本Issueはテストコードのみの追加であり、セキュリティリスクは低い。

- テストデータは`tempfile::tempdir()`で安全なランダムディレクトリに作成・`Drop`で自動削除
- 外部サービスへの接続は`#[ignore]`テストのみ（ローカルOllama、localhost:11434）
- `unsafe`コード使用なし
- テストデータにAPIキー・トークン等の秘密情報をハードコードしない
