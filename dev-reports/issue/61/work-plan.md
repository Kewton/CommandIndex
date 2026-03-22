# 作業計画書: Issue #61 Embedding生成基盤（ローカルLLM / API対応）

## Issue概要

**Issue番号**: #61
**タイトル**: [Feature] Embedding生成基盤（ローカルLLM / API対応）
**サイズ**: L（新モジュール追加 + 既存モジュール変更 + 外部依存追加）
**優先度**: High（Phase 5基盤）
**依存Issue**: なし（Phase 4完了済み v0.0.4）
**ブランチ**: `feature/issue-61-embedding-provider`（既存）

## 設計方針書

`dev-reports/design/issue-61-embedding-provider-design-policy.md`

---

## 詳細タスク分解

### Step 1: 依存追加とモジュール骨格（基盤準備）

- [ ] **Task 1.1**: Cargo.toml に reqwest, toml 依存追加
  - 成果物: `Cargo.toml`
  - 依存: なし
  - 詳細:
    ```toml
    reqwest = { version = "0.12", features = ["blocking", "json"] }
    toml = "0.8"
    ```

- [ ] **Task 1.2**: embedding モジュール骨格作成
  - 成果物: `src/embedding/mod.rs`（空のトレイト・エラー型定義）
  - 依存: なし
  - 詳細: EmbeddingProvider trait, EmbeddingError enum, ProviderType enum

- [ ] **Task 1.3**: lib.rs / cli/mod.rs にモジュール宣言追加
  - 成果物: `src/lib.rs`, `src/cli/mod.rs`
  - 依存: Task 1.2
  - 詳細: `pub mod embedding;`, `pub mod embed;`

- [ ] **Task 1.4**: indexer/mod.rs に embeddings_db_path() 追加
  - 成果物: `src/indexer/mod.rs`
  - 依存: なし
  - 詳細: `const EMBEDDINGS_DB_FILE` + `pub fn embeddings_db_path()`

**検証**: `cargo build` 通過、`cargo clippy` 警告0

### Step 2: EmbeddingStore（SQLite格納）

- [ ] **Task 2.1**: EmbeddingStoreError 定義
  - 成果物: `src/embedding/store.rs`
  - 依存: Task 1.2
  - 詳細: Sqlite/Io/SchemaVersionMismatch バリアント、From実装、Display/Error実装

- [ ] **Task 2.2**: EmbeddingStore 実装
  - 成果物: `src/embedding/store.rs`
  - 依存: Task 2.1
  - 詳細:
    - open(), create_tables()（schema_meta + embeddings テーブル + UNIQUE制約 + インデックス）
    - upsert_embedding() （INSERT OR REPLACE）
    - find_by_path(), has_current_embedding(), delete_by_path(), count()
    - #[cfg(test)] open_in_memory()

- [ ] **Task 2.3**: EmbeddingStore ユニットテスト
  - 成果物: `src/embedding/store.rs` 内 #[cfg(test)] mod tests
  - 依存: Task 2.2
  - テスト項目:
    - テーブル作成成功
    - upsert/find/delete のCRUD操作
    - has_current_embedding のキャッシュチェック
    - count()
    - 重複セクション時のupsert動作（UNIQUE制約）

**検証**: `cargo test` 全パス

### Step 3: EmbeddingConfig（設定管理）

- [ ] **Task 3.1**: Config / EmbeddingConfig 型定義
  - 成果物: `src/embedding/mod.rs`
  - 依存: Task 1.2
  - 詳細:
    - ProviderType enum（Ollama, OpenAi）
    - EmbeddingConfig（provider, model, endpoint, api_key）
    - カスタムDebug実装（api_keyマスキング）
    - Default実装
    - resolve_api_key()（環境変数 > config.toml）
    - Config::load()（.commandindex/config.toml）

- [ ] **Task 3.2**: Config ユニットテスト
  - 成果物: `src/embedding/mod.rs` 内テスト
  - 依存: Task 3.1
  - テスト項目:
    - デフォルト値
    - TOML パース（正常系/異常系）
    - 環境変数優先のapi_key解決
    - 不在時のNone返却

**検証**: `cargo test` 全パス

### Step 4: OllamaProvider

- [ ] **Task 4.1**: OllamaProvider 実装
  - 成果物: `src/embedding/ollama.rs`
  - 依存: Task 1.1, Task 3.1
  - 詳細:
    - new(), from_config()
    - embed(): POST {endpoint}/api/embed、バッチサイズ10
    - dimension(): モデル名→次元数マッピング + OnceLock遅延キャッシュ
    - テキスト入力サイズ制限（8192文字）
    - タイムアウト: connect 10秒、request 30秒

- [ ] **Task 4.2**: OllamaProvider ユニットテスト（モック）
  - 成果物: `src/embedding/ollama.rs` 内テスト
  - 依存: Task 4.1
  - テスト項目:
    - from_config() の正常構築
    - dimension() のマッピング動作
    - テキストサイズ制限

**検証**: `cargo test` 全パス

### Step 5: OpenAiProvider

- [ ] **Task 5.1**: OpenAiProvider 実装
  - 成果物: `src/embedding/openai.rs`
  - 依存: Task 1.1, Task 3.1
  - 詳細:
    - new(), from_config()（api_key必須チェック）
    - embed(): POST {endpoint}/v1/embeddings、バッチサイズ100
    - dimension(): モデル名→次元数マッピング + OnceLock
    - テキスト入力サイズ制限（32000文字）
    - config.endpoint を使用（Azure OpenAI対応）

- [ ] **Task 5.2**: OpenAiProvider ユニットテスト（モック）
  - 成果物: `src/embedding/openai.rs` 内テスト
  - 依存: Task 5.1
  - テスト項目:
    - from_config() api_key未設定時のエラー
    - dimension() のマッピング動作

**検証**: `cargo test` 全パス

### Step 6: プロバイダーファクトリ + MockProvider

- [ ] **Task 6.1**: create_provider() ファクトリ関数
  - 成果物: `src/embedding/mod.rs`
  - 依存: Task 4.1, Task 5.1
  - 詳細: ProviderType enumでマッチ

- [ ] **Task 6.2**: MockProvider（テスト用）
  - 成果物: `src/embedding/mod.rs` 内 #[cfg(test)]
  - 依存: Task 1.2
  - 詳細: 固定ベクトルを返すモック実装

**検証**: `cargo test` 全パス

### Step 7: embed サブコマンド

- [ ] **Task 7.1**: cli/embed.rs 実装
  - 成果物: `src/cli/embed.rs`
  - 依存: Task 2.2, Task 3.1, Task 6.1
  - 詳細:
    - EmbedError enum（Display, Error, From実装）
    - EmbedSummary 構造体
    - run(): インデックス存在チェック → config読み込み → プロバイダー生成 → キャッシュチェック → embedding生成 → store保存

- [ ] **Task 7.2**: main.rs に Commands::Embed 追加
  - 成果物: `src/main.rs`
  - 依存: Task 7.1
  - 詳細: Embed { path } サブコマンド追加、match アーム追加

**検証**: `cargo build` 通過、`commandindex embed --help` 動作確認

### Step 8: index/update --with-embedding 対応

- [ ] **Task 8.1**: IndexOptions 構造体追加
  - 成果物: `src/cli/index.rs`
  - 依存: Task 6.1
  - 詳細:
    - IndexOptions { with_embedding: bool } + Default実装
    - run(path, &IndexOptions) / run_incremental(path, &IndexOptions) シグネチャ変更
    - IndexError に Embedding/EmbeddingStore バリアント追加

- [ ] **Task 8.2**: main.rs の Index/Update コマンド引数更新
  - 成果物: `src/main.rs`
  - 依存: Task 8.1
  - 詳細: --with-embedding フラグ追加、IndexOptions 生成

**検証**: `cargo test` 既存テスト全パス

### Step 9: clean --keep-embeddings 対応

- [ ] **Task 9.1**: CleanOptions 構造体追加 + 選択的削除
  - 成果物: `src/cli/clean.rs`
  - 依存: Task 1.4
  - 詳細:
    - CleanOptions { keep_embeddings: bool } + Default実装
    - run(path, &CleanOptions) シグネチャ変更
    - keep_embeddings=true: tantivy/, manifest.json, state.json, symbols.db 個別削除、embeddings.db + config.toml 保持

- [ ] **Task 9.2**: main.rs の Clean コマンド引数更新
  - 成果物: `src/main.rs`
  - 依存: Task 9.1
  - 詳細: --keep-embeddings フラグ追加、出力メッセージ分岐

**検証**: `cargo test` 既存テスト全パス

### Step 10: E2Eテスト

- [ ] **Task 10.1**: embedding E2Eテスト（モック）
  - 成果物: `tests/e2e_embedding.rs`
  - 依存: Task 7.2, Task 8.2, Task 9.2
  - テスト項目:
    - `commandindex embed` の正常実行（モックプロバイダー）
    - `commandindex clean --keep-embeddings` の動作
    - `commandindex embed` インデックス未構築時のエラー
    - CLI helpテキストに embed が表示される

### Step 11: 最終検証 + コード品質

- [ ] **Task 11.1**: 品質チェック全パス
  - `cargo build`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test --all`
  - `cargo fmt --all -- --check`

- [ ] **Task 11.2**: 既存テスト21ファイルの全パス確認

---

## タスク依存関係

```
Step 1 (基盤準備)
  ├── Step 2 (EmbeddingStore)
  ├── Step 3 (Config)
  │   ├── Step 4 (OllamaProvider)
  │   └── Step 5 (OpenAiProvider)
  │       └── Step 6 (ファクトリ + Mock)
  │           ├── Step 7 (embed コマンド)
  │           ├── Step 8 (index --with-embedding)
  │           └── Step 9 (clean --keep-embeddings)
  │               └── Step 10 (E2Eテスト)
  │                   └── Step 11 (最終検証)
```

## 品質チェック項目

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |

## Definition of Done

- [ ] 全11ステップのタスクが完了
- [ ] EmbeddingProvider トレイト + Ollama/OpenAI実装
- [ ] EmbeddingStore (SQLite) CRUD動作
- [ ] config.toml 読み込み動作
- [ ] embed / index --with-embedding / clean --keep-embeddings CLI動作
- [ ] ユニットテスト + E2Eテスト全パス
- [ ] cargo clippy 警告0件
- [ ] cargo fmt 差分なし
- [ ] 既存テスト21ファイル全パス（後方互換性維持）
