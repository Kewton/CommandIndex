# 作業計画 - Issue #135: embedding生成のバッチサイズ拡大による高速化

## Issue概要
**Issue番号**: #135
**サイズ**: M
**優先度**: High
**依存Issue**: なし

## 詳細タスク分解

### Phase 1: 定数変更（T1）

- [ ] **Task 1.1**: BATCH_SIZE拡大 + タイムアウト調整
  - **ファイル**: `src/embedding/ollama.rs`
  - **変更内容**:
    - `BATCH_SIZE: usize = 10` → `50`
    - `REQUEST_TIMEOUT_SECS: u64 = 30` → `60`
  - **依存**: なし
  - **テスト**: 既存テスト通過確認

### Phase 2: トランザクションAPI追加（T2-Store層）

- [ ] **Task 2.1**: `execute_in_transaction` メソッド追加
  - **ファイル**: `src/embedding/store.rs`
  - **変更内容**: `pub fn execute_in_transaction<F, T>(&self, f: F) -> Result<T, EmbeddingStoreError>` メソッドを追加
  - **API契約**: Ok→COMMIT、Err→ROLLBACK
  - **依存**: なし

- [ ] **Task 2.2**: トランザクション単体テスト追加
  - **ファイル**: `src/embedding/store.rs` (tests モジュール内)
  - **テスト項目**:
    1. `test_execute_in_transaction_commit` — 正常時にCOMMITされデータが永続化
    2. `test_execute_in_transaction_rollback` — エラー時にROLLBACKされデータが残らない
    3. `test_execute_in_transaction_multiple_upserts` — 複数upsertがアトミック
    4. `test_execute_in_transaction_deletes_orphan_sections` — DELETE+INSERTでorphan消滅
  - **依存**: Task 2.1

### Phase 3: CLI層のトランザクション適用（T2-CLI層）

- [ ] **Task 3.1**: embed.rs のupsertループ変更
  - **ファイル**: `src/cli/embed.rs`
  - **変更内容**:
    1. sections/embeddings件数検証を追加
    2. `execute_in_transaction` でDELETE+INSERTをファイル単位でラップ
    3. エラーカウンタの粒度をper-file化
  - **依存**: Task 2.1

- [ ] **Task 3.2**: index.rs のupsertループ変更
  - **ファイル**: `src/cli/index.rs` (`generate_embeddings_for_manifest()`)
  - **変更内容**:
    1. sections/embeddings件数検証を追加
    2. `execute_in_transaction` でDELETE+INSERTをファイル単位でラップ
  - **依存**: Task 2.1

### Phase 4: 品質検証

- [ ] **Task 4.1**: 全品質チェック通過
  - `cargo build` — エラー0件
  - `cargo clippy --all-targets -- -D warnings` — 警告0件
  - `cargo test --all` — 全テストパス
  - `cargo fmt --all -- --check` — 差分なし

## TDD実装順序

設計方針書に基づき、以下の順序でTDD実装を行う:

```
1. Task 2.1 + 2.2: store.rsにexecute_in_transactionを追加（テストファースト）
2. Task 1.1: ollama.rsの定数変更（単純変更）
3. Task 3.1: embed.rsのトランザクション適用
4. Task 3.2: index.rsのトランザクション適用
5. Task 4.1: 全品質チェック
```

## Definition of Done

- [ ] `BATCH_SIZE=50`, `REQUEST_TIMEOUT_SECS=60` に変更済み
- [ ] `execute_in_transaction` メソッドが追加済み
- [ ] `embed.rs` と `index.rs` の両方でトランザクション適用済み
- [ ] 件数不一致検証が追加済み
- [ ] orphan sections対策（DELETE+INSERT）が実装済み
- [ ] `cargo test --all` 全パス
- [ ] `cargo clippy --all-targets -- -D warnings` 警告0件
- [ ] `cargo fmt --all -- --check` 差分なし
