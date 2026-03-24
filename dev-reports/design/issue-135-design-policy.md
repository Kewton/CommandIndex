# 設計方針書 - Issue #135: embedding生成のバッチサイズ拡大による高速化

## 1. 概要

Ollama embedding APIのバッチサイズを10→50に拡大し、SQLiteトランザクションバッチ化を導入して、embedding生成を3-5倍高速化する。

## 2. 対象レイヤーと責務

| レイヤー | モジュール | 変更内容 | 責務 |
|---------|-----------|---------|------|
| **Embedding** | `src/embedding/ollama.rs` | BATCH_SIZE, REQUEST_TIMEOUT_SECS定数変更 | OllamaへのHTTPバッチリクエスト制御 |
| **Embedding Store** | `src/embedding/store.rs` | `execute_in_transaction()` メソッド追加 | SQLiteトランザクション制御 |
| **CLI (embed)** | `src/cli/embed.rs` | upsertループをトランザクションで囲む | embed コマンドのオーケストレーション |
| **CLI (index)** | `src/cli/index.rs` | upsertループをトランザクションで囲む | index --with-embedding のオーケストレーション |

## 3. 設計判断とトレードオフ

### 判断1: BATCH_SIZE = 50

| 選択肢 | メリット | デメリット | 判定 |
|--------|---------|----------|------|
| 30 | VRAM安全 | 効果限定的（3.3倍削減） | △ |
| **50** | **5倍削減、OpenAI(100)の半分で安全** | **VRAM使用量増加** | **採用** |
| 100 | 最大効果 | OllamaのVRAMリスク大 | × |

**理由:** OpenAIが100で問題ないため、ローカルOllamaでもその半分の50は安全圏。VRAM問題発生時はコード変更で30に調整（定数変更のため再ビルド要）。

### 判断2: REQUEST_TIMEOUT_SECS = 60

| 選択肢 | メリット | デメリット | 判定 |
|--------|---------|----------|------|
| 30（維持） | 早期エラー検知 | バッチサイズ50でタイムアウトリスク | × |
| **60** | **バッチサイズ50に十分なマージン** | **異常系の待機時間増加** | **採用** |
| 120 | 過剰マージン | 接続エラー時のUX悪化 | × |

**理由:** CONNECT_TIMEOUT_SECS=10は維持（接続エラーは素早く検知）。REQUEST_TIMEOUT_SECSのみ60に引き上げ。

### 判断3: トランザクションAPI設計 — raw SQL方式

| 選択肢 | メリット | デメリット | 判定 |
|--------|---------|----------|------|
| rusqlite::Transaction | 型安全 | `Connection::transaction(&mut self)` が `&mut Connection` を要求。`EmbeddingStore` は `conn` を `&self` で保持しているため呼び出し不可 | × |
| **raw SQL (execute_batch)** | **`Connection::execute_batch(&self)` は `&self` で呼び出し可能。既存API無変更** | **手動エラー管理** | **採用** |
| バッチupsert API | カプセル化 | 柔軟性低下 | × |

**理由:** `rusqlite::Connection::transaction()` は `&mut Connection` を要求するが、`EmbeddingStore` の全メソッドは `&self`（不変借用）を使用しており、`&mut self` への変更は全呼び出し元に波及する。`Connection::execute_batch()` は `&self` で呼び出し可能なため、raw SQL方式を採用。

### 判断4: エラー時はROLLBACK（部分コミットしない）

| 選択肢 | メリット | デメリット | 判定 |
|--------|---------|----------|------|
| 部分コミット | 成功分を保存 | `has_current_embedding()` キャッシュと矛盾。一部section欠損が回復不能 | × |
| **ROLLBACK** | **キャッシュ整合性維持。再実行で自動回復** | **成功分も破棄される** | **採用** |

**理由:** `has_current_embedding()` はパス+ハッシュでファイル全体の処理済みを判定する。部分コミットすると、一部section欠損時に再実行してもスキップされ回復不能になる。

### 判断5: orphan sections対策 — トランザクション内でDELETE+INSERT

ファイルのセクション数が減少した場合、upsert（INSERT OR REPLACE）だけでは古いセクションが残る。トランザクション内で先に `delete_by_path` を実行してから全セクションをINSERTすることで、アトミックに置換する。

## 4. 詳細設計

### 4.1 T1: バッチサイズ拡大（ollama.rs）

```rust
// 変更前
const BATCH_SIZE: usize = 10;
const REQUEST_TIMEOUT_SECS: u64 = 30;

// 変更後
const BATCH_SIZE: usize = 50;
const REQUEST_TIMEOUT_SECS: u64 = 60;
```

**影響:** `embed()` メソッド内の `texts.chunks(BATCH_SIZE)` のチャンクサイズが変わるのみ。外部API変更なし。バッチ動作の自動テストは不可（Ollamaサーバーが必要）。手動テストで検証。

### 4.2 T2: SQLiteトランザクションバッチ化

#### 4.2.1 EmbeddingStore::execute_in_transaction（store.rs）

```rust
/// ファイル単位のトランザクション内でクロージャを実行する。
/// クロージャが Ok を返した場合のみ COMMIT、Err の場合は ROLLBACK。
pub fn execute_in_transaction<F, T>(&self, f: F) -> Result<T, EmbeddingStoreError>
where
    F: FnOnce(&Self) -> Result<T, EmbeddingStoreError>,
{
    self.conn.execute_batch("BEGIN")?;
    match f(self) {
        Ok(value) => {
            self.conn.execute_batch("COMMIT")?;
            Ok(value)
        }
        Err(e) => {
            // ROLLBACK失敗時は元のエラーを優先して返す
            let _ = self.conn.execute_batch("ROLLBACK");
            Err(e)
        }
    }
}
```

**API契約:**
- `Ok` → COMMIT（COMMIT失敗時はSQLiteが暗黙的にROLLBACKし、Errを返す）
- `Err` → ROLLBACK（ROLLBACK失敗時は元のエラーを優先して返す。ROLLBACK失敗はSQLiteの接続異常を意味し、次の操作で検出される）
- ネスト呼び出し: SQLiteは`BEGIN`中に`BEGIN`を発行するとエラー（`cannot start a transaction within a transaction`）を返す。意図しないネストは明確なエラーとして検出される。
- **ストア層はログ出力を行わない**: エラー情報は構造化エラーとして返し、ログ出力はCLI層に委ねる。

#### 4.2.2 embed.rs のupsertループ変更

```rust
// 変更前（embed.rs L183-200）:
// 個別upsert、section単位でエラースキップ
for (section, embedding) in sections.iter().zip(embeddings.iter()) {
    if let Err(e) = store.upsert_embedding(
        &entry.path, &section.heading, embedding,
        dimension, model, &entry.hash,
    ) {
        eprintln!("Warning: failed to store embedding for {}#{}: {e}",
            entry.path, section.heading);
        failed += 1;
        continue;
    }
    generated += 1;
}

// 変更後: ファイル単位トランザクション
// 件数検証 + DELETE + INSERT でアトミックに全セクションを置換
if sections.len() != embeddings.len() {
    eprintln!("Warning: section/embedding count mismatch for {}: {} sections, {} embeddings",
        entry.path, sections.len(), embeddings.len());
    failed += sections.len() as u64;
} else {
    match store.execute_in_transaction(|store| {
        store.delete_by_path(&entry.path)?;
        for (section, embedding) in sections.iter().zip(embeddings.iter()) {
            store.upsert_embedding(
                &entry.path, &section.heading, embedding,
                dimension, model, &entry.hash,
            )?;  // エラー時は即座にトランザクションをROLLBACK
        }
        Ok(sections.len() as u64)
    }) {
        Ok(count) => generated += count,
        Err(e) => {
            eprintln!("Warning: failed to store embeddings for {}: {e}", entry.path);
            failed += sections.len() as u64;
    }
}
```

**振る舞い変更:** エラー時のカウント粒度がsection単位からファイル単位に変更。現行コードでは1 sectionの失敗で `failed += 1` だが、変更後は1 sectionの失敗でファイル全体がROLLBACKされ `failed += sections.len()` となる。これは `has_current_embedding()` キャッシュとの整合性を保つための意図的な変更。

#### 4.2.3 index.rs のupsertループ変更

```rust
// 変更前（index.rs generate_embeddings_for_manifest L910-924）:
// 個別upsert、エラー時はeprn出力のみ（カウンタなし）
for (section, embedding) in sections.iter().zip(embeddings.iter()) {
    if let Err(e) = store.upsert_embedding(
        &entry.path, &section.heading, embedding,
        dimension, model, &entry.hash,
    ) {
        eprintln!("Warning: failed to store embedding for {}#{}: {e}",
            entry.path, section.heading);
    }
}

// 変更後: ファイル単位トランザクション
// 件数検証 + DELETE + INSERT
if sections.len() != embeddings.len() {
    eprintln!("Warning: section/embedding count mismatch for {}: {} sections, {} embeddings",
        entry.path, sections.len(), embeddings.len());
} else if let Err(e) = store.execute_in_transaction(|store| {
    store.delete_by_path(&entry.path)?;
    for (section, embedding) in sections.iter().zip(embeddings.iter()) {
        store.upsert_embedding(
            &entry.path, &section.heading, embedding,
            dimension, model, &entry.hash,
        )?;
    }
    Ok(())
}) {
    eprintln!("Warning: failed to store embeddings for {}: {e}", entry.path);
}
```

**注意:** index.rsはembed.rsと異なりgenerated/failedカウンタを持たない。エラー時はeprintln出力のみで続行。

## 5. 影響範囲

### 変更ファイル
| ファイル | 変更種別 | テスト |
|---------|---------|--------|
| `src/embedding/ollama.rs` | 定数変更 | バッチ動作は手動テストで検証（Ollamaサーバー必要） |
| `src/embedding/store.rs` | メソッド追加 | 新規テスト追加（commit/rollback/アトミック性） |
| `src/cli/embed.rs` | ロジック変更 | 既存テスト＋手動テスト |
| `src/cli/index.rs` | ロジック変更 | 既存テスト＋手動テスト |

### コード変更なし・回帰確認対象

以下はコード変更対象ではないが、埋め込みDB生成方式の変更により間接的に影響を受けるため回帰確認が必要。

| ファイル | 依存種別 | 確認事項 |
|---------|---------|---------|
| `src/search/` | 読み取り依存 | 埋め込み再生成後の検索結果が正常か |
| `src/cli/search.rs` | 読み取り依存 | search コマンドの動作確認 |
| `src/cli/suggest.rs` | 読み取り依存 | suggest コマンドの動作確認 |
| `src/cli/status/` | 読み取り依存 | status コマンドの統計表示が正常か |
| `src/embedding/openai.rs` | 変更なし | BATCH_SIZE=100は維持 |
| `src/embedding/mod.rs` | 変更なし | トレイト定義変更なし |
| `tests/e2e_semantic_hybrid.rs` | テスト依存 | EmbeddingStoreを使用（テストパス確認要） |

## 6. テスト計画

### 新規テスト（store.rs）
1. `test_execute_in_transaction_commit` — 正常時にCOMMITされデータが永続化
2. `test_execute_in_transaction_rollback` — エラー時にROLLBACKされデータが残らない
3. `test_execute_in_transaction_multiple_upserts` — 複数upsertがアトミックに処理される
4. `test_execute_in_transaction_deletes_orphan_sections` — DELETE+INSERTでorphan sectionが消えること
5. `test_execute_in_transaction_count_mismatch_preserves_data` — 件数不一致時に既存データが壊れないこと（CLI層テスト）

### 既存テスト
- `cargo test --all` 全パス確認（tests/e2e_semantic_hybrid.rs含む）
- `cargo clippy --all-targets -- -D warnings` 警告0件
- `cargo fmt --all -- --check` 差分なし

## 7. セキュリティ設計

本変更に機密性・完全性への影響なし。
- unsafe使用なし
- 外部入力の追加なし
- SQLインジェクションリスクなし（パラメータバインディング使用）
- raw SQL文字列（"BEGIN"/"COMMIT"/"ROLLBACK"）はハードコードリテラル
- **可用性リスク:** BATCH_SIZE=50への拡大により、ローカルOllamaのVRAM/CPU負荷が増加する。リソース逼迫時はリクエスト処理の遅延・タイムアウトが発生しうる。REQUEST_TIMEOUT_SECS=60への引き上げにより、異常時の待機時間も増加する。

## 8. 品質基準

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |
