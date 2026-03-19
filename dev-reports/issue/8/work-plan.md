# 作業計画 - Issue #8: 検索結果出力フォーマッター

## 作業ステップ

### Step 1: 依存追加・モジュール骨格
1. `Cargo.toml` に `colored = "2"` 追加
2. `src/output/mod.rs` 作成（OutputFormat enum, OutputError enum, 共通ヘルパー, format_results()）
3. `src/output/human.rs` 作成（空の関数シグネチャ）
4. `src/output/json.rs` 作成（空の関数シグネチャ）
5. `src/output/path.rs` 作成（空の関数シグネチャ）
6. `src/lib.rs` の `pub mod output;` アンコメント
7. `cargo check` でコンパイル確認

### Step 2: テスト作成（TDD Red phase）
1. `tests/output_format.rs` 作成
2. 全12テストケースを記述（全て失敗する状態）
3. `cargo test` で全テスト失敗を確認

### Step 3: path フォーマッター実装（最もシンプル）
1. `format_path()` 実装（重複除去）
2. path関連テストがパスすることを確認

### Step 4: json フォーマッター実装
1. `format_json()` 実装（serde_json::json!() + parse_tags()）
2. json関連テストがパスすることを確認

### Step 5: human フォーマッター実装
1. `format_human()` 実装（colored + truncate_body() + strip_control_chars() + parse_tags()）
2. human関連テストがパスすることを確認

### Step 6: CLI統合
1. `src/main.rs` の Search サブコマンドに `--format` オプション追加
2. 既存テスト通過確認

### Step 7: 品質チェック
1. `cargo build` — エラー0件
2. `cargo clippy --all-targets -- -D warnings` — 警告0件
3. `cargo test --all` — 全テストパス
4. `cargo fmt --all -- --check` — 差分なし
