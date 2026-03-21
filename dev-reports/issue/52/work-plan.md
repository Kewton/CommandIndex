# 作業計画書 - Issue #52 Context Pack 生成

## Issue: [Feature] Context Pack 生成（AI向け文脈パッケージ出力）
**Issue番号**: #52
**サイズ**: M
**優先度**: High
**依存Issue**: #50 --related 検索オプション（実装済み）

---

## Phase 1: 型定義・出力モジュール

### Task 1.1: ContextPack 型定義 (output/mod.rs)
- **成果物**: `src/output/mod.rs` に ContextPack, ContextEntry, ContextSummary 型追加
- **依存**: なし
- **内容**:
  - `ContextPack` 構造体（target_files, context, summary）
  - `ContextEntry` 構造体（path, relation, score, heading?, snippet?, symbols?）
  - `ContextSummary` 構造体（total_related, included, estimated_tokens）
  - すべてに `#[derive(Debug, Serialize)]` を付与
  - `ContextEntry` の Option フィールドに `#[serde(skip_serializing_if = "Option::is_none")]`

### Task 1.2: format_context_pack 実装 (output/context_pack.rs)
- **成果物**: `src/output/context_pack.rs` 新規作成
- **依存**: Task 1.1
- **内容**:
  - `pub fn format_context_pack(pack: &ContextPack, writer: &mut dyn Write) -> Result<(), OutputError>`
  - `serde_json::to_writer_pretty(writer, pack)?;` で単一JSON出力
  - `output/mod.rs` に `pub mod context_pack;` 追加

## Phase 2: CLIサブコマンド定義

### Task 2.1: Commands enum に Context variant 追加 (main.rs)
- **成果物**: `src/main.rs`
- **依存**: なし
- **内容**:
  - `Context { files: Vec<String>, max_files: usize, max_tokens: Option<usize> }` variant 追加
  - match アームに `Commands::Context { files, max_files, max_tokens } =>` 追加
  - `cli::context::run_context(&files, max_files, max_tokens)` 呼び出し

### Task 2.2: cli/mod.rs にモジュール宣言追加
- **成果物**: `src/cli/mod.rs`
- **依存**: なし
- **内容**: `pub mod context;` 追加

## Phase 3: コアロジック実装

### Task 3.1: cli/context.rs - 入力検証 + インデックスオープン
- **成果物**: `src/cli/context.rs` 新規作成
- **依存**: Task 1.1, 1.2, 2.1, 2.2
- **内容**:
  - `run_context()` 関数のスケルトン
  - ファイルパス空文字列チェック・長さ上限（1024文字）・ファイル数上限（100件）
  - IndexReaderWrapper + SymbolStore オープン（run_related_search() パターン踏襲）
  - SearchError を cli::search::SearchError から再利用

### Task 3.2: cli/context.rs - collect_related_context (関連ファイル収集・マージ)
- **成果物**: `src/cli/context.rs`
- **依存**: Task 3.1
- **内容**:
  - 各ファイルに対して `engine.find_related()` 実行
  - `merge_related_results()` で union マージ（スコア最大値、target除外）
  - `HashMap<String, (f32, Vec<RelationType>)>` でマージ
  - スコア降順ソート

### Task 3.3: cli/context.rs - build_context_pack (エンリッチ + 制限適用)
- **成果物**: `src/cli/context.rs`
- **依存**: Task 3.2
- **内容**:
  - `--max-files` でトリム
  - `enrich_entry()` で各エントリにスニペット・見出し・シンボル付加
    - `search_by_exact_path()` で heading/body 取得
    - `truncate_body(body, 10, 500)` + `strip_control_chars()` でスニペット生成
    - `find_imports_by_source()` で imported_names 取得、`split(", ")` でパース
    - 取得失敗時は None
  - `relation_to_string()` で RelationType → String 変換
  - `estimate_tokens()` でトークン概算（bytes / 4）
  - `--max-tokens` でトークン数累計上限超過時に打ち切り
  - ContextPack 構築 + `format_context_pack()` で stdout 出力

## Phase 4: テスト

### Task 4.1: E2Eテスト作成 (tests/e2e_context_pack.rs)
- **成果物**: `tests/e2e_context_pack.rs` 新規作成
- **依存**: Phase 3 完了
- **テストケース**:
  1. `context_pack_outputs_valid_json` - JSON出力の有効性検証
  2. `context_pack_includes_target_files` - target_files フィールドの正確性
  3. `context_pack_includes_related_context` - context 配列に関連ファイルが含まれる
  4. `context_pack_max_files_limits_output` - --max-files 制限
  5. `context_pack_max_tokens_limits_output` - --max-tokens 制限
  6. `context_pack_multiple_files` - 複数ファイル指定のマージ
  7. `context_pack_no_self_reference` - target が結果に含まれない
  8. `context_pack_relation_types` - relation 型の検証
  9. `context_pack_summary_fields` - summary フィールドの正確性

### Task 4.2: CLI引数テスト追加 (tests/cli_args.rs)
- **成果物**: `tests/cli_args.rs`
- **依存**: Task 2.1
- **内容**: help 出力に "context" が含まれることを検証

## Phase 5: 品質チェック

### Task 5.1: 品質チェック実行
- **成果物**: なし
- **依存**: Phase 4 完了
- **内容**:
  - `cargo build` - エラー0件
  - `cargo clippy --all-targets -- -D warnings` - 警告0件
  - `cargo test --all` - 全テストパス
  - `cargo fmt --all -- --check` - 差分なし

---

## TDD実装順序

設計方針書に基づき、TDDで以下の順序で実装:

```
1. 型定義（Task 1.1）→ コンパイル確認
2. 出力関数（Task 1.2）→ ユニットテスト
3. CLI定義（Task 2.1, 2.2）→ CLI引数テスト
4. 入力検証（Task 3.1）→ エラーケーステスト
5. マージロジック（Task 3.2）→ ユニットテスト
6. エンリッチ + 出力（Task 3.3）→ E2Eテスト
7. 品質チェック（Task 5.1）
```

## Definition of Done

- [ ] すべてのタスクが完了
- [ ] E2Eテスト9ケース全パス
- [ ] cargo build / clippy / test / fmt 全パス
- [ ] 既存テスト全パス（リグレッションなし）
