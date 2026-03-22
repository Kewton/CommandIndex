# 作業計画: Issue #80 Phase 6 E2E統合テスト

## 作業概要

| 項目 | 内容 |
|------|------|
| Issue | #80 |
| ブランチ | feature/issue-80-e2e-tests |
| 変更ファイル | tests/e2e_team_workflow.rs（新規） |
| プロダクションコード変更 | なし |

## 作業ステップ

### Step 1: テストファイル作成とヘルパー関数

`tests/e2e_team_workflow.rs` を新規作成。以下のヘルパー関数を定義:
- `write_commandindex_toml(path, content)`
- `write_config_local_toml(path, content)`
- `setup_test_markdown(path)` - テスト用Markdownファイル配置

### Step 2: シナリオ1 - 共有設定フルフロー

```rust
#[test]
fn e2e_team_config_full_flow()
```
- commandindex.toml作成 → index → config show → 設定反映確認

### Step 3: シナリオ2 - 設定優先順位

```rust
#[test]
fn e2e_config_priority()
```
- commandindex.toml + config.local.toml → config show → local優先確認

### Step 4: シナリオ3 - config show APIキーマスク

```rust
#[test]
fn e2e_config_show_api_key_masked()
```
- config.local.tomlにapi_key設定 → config show → マスク確認

### Step 5: シナリオ4 - Export/Import統合フロー

```rust
#[test]
fn e2e_export_import_search_flow()
```
- index → search → export → clean → import → search → 結果比較

### Step 6: シナリオ5 - status --verify

```rust
#[test]
fn e2e_status_verify_with_team_config()
```
- commandindex.toml + index → status --verify → OK確認

### Step 7: シナリオ6-7 - status --detail / --format json

```rust
#[test]
fn e2e_status_detail()

#[test]
fn e2e_status_json_detail()
```

### Step 8: 品質チェック

- `cargo test --all` 全パス
- `cargo clippy --all-targets -- -D warnings` 警告0件
- `cargo fmt --all -- --check` 差分なし

## 注意事項

- config show, export, import, searchはcurrent_dir依存。`.current_dir()`必須
- index, status, cleanは`--path`オプションあり
- 環境変数 `COMMANDINDEX_OPENAI_API_KEY` のテスト干渉に注意
