# 設計方針書: Issue #80 Phase 6 E2E統合テスト

## 1. 概要

Phase 6のチーム向け機能（共有設定、インデックス共有、status拡張）を通したE2E統合テストを作成する。

| 項目 | 内容 |
|------|------|
| Issue | #80 |
| タイプ | テスト追加（プロダクションコード変更なし） |
| スコープ | 7つのE2Eテストシナリオ |
| テストファイル | `tests/e2e_team_workflow.rs` |

## 2. テスト設計方針

### 2.1 既存テストとの差別化

| テストファイル | 役割 | テスト粒度 |
|--------------|------|-----------|
| `tests/cli_status.rs` | status機能の単体・結合テスト | 関数レベル + CLI引数 |
| `tests/cli_export.rs` | export機能の単体テスト | 関数レベル |
| `tests/cli_import.rs` | import機能の単体テスト | 関数レベル |
| `tests/e2e_export_import.rs` | export→import基本フロー | 2機能連携 |
| `tests/e2e_verify.rs` | verify機能のE2E | 単機能CLI |
| **`tests/e2e_team_workflow.rs`** | **チーム運用シナリオの統合フロー** | **複数機能の連携** |

### 2.2 テストパターン

各テストは以下のパターンに従う:

```rust
#[test]
fn scenario_name() {
    // 1. Setup: tempdir + テストデータ配置 + 設定ファイル作成
    let dir = tempfile::tempdir().expect("create temp dir");

    // 2. Act: CLIコマンド実行（assert_cmd経由）
    common::cmd()
        .args(["subcommand", "--path", dir.path().to_str().unwrap()])
        .assert()
        .success();

    // 3. Assert: 出力/状態の検証
}
```

### 2.3 CLI経由テスト vs API直接テスト

- **CLI経由テスト（assert_cmd）**: 全シナリオで採用。実際のユーザー操作を再現。
- **API直接テスト**: export/importのみ `current_dir` 制約回避のため併用可能。

### 2.4 current_dir依存コマンドの制約

以下のコマンドは `--path` オプションを持たず、カレントディレクトリ（`Path::new(".")`）基準で動作する。テストでは `.current_dir(tmp_dir)` の設定が必須。

| コマンド | current_dir依存 | --pathオプション |
|---------|----------------|-----------------|
| `config show` | あり | なし |
| `config path` | あり | なし |
| `export` | あり | なし |
| `import` | あり | なし |
| `search` | あり（config読込） | なし（--path=prefix filter） |
| `index` | なし | あり |
| `status` | なし | あり |
| `clean` | なし | あり |

## 3. テストシナリオ詳細設計

### シナリオ1: 共有設定フルフロー

```
Setup:  commandindex.toml に search.default_limit = 5 を書き込み
        テスト用Markdownファイルを配置
Act:    index → config show
Assert: config show出力に default_limit = 5 が含まれる
```

### シナリオ2: 設定優先順位

```
Setup:  commandindex.toml に search.default_limit = 5
        .commandindex/config.local.toml に search.default_limit = 3
Act:    config show
Assert: default_limit = 3（local が team を上書き）
```

### シナリオ3: config show（APIキーマスク）

```
Setup:  .commandindex/config.local.toml に embedding.api_key = "sk-test123"
Act:    config show
Assert: 出力に "***" が含まれ "sk-test123" が含まれない
```

### シナリオ4: エクスポート/インポートフロー

```
Setup:  テスト用Markdownファイル配置 → index → search確認
Act:    export → clean → import
Assert: import後のsearch結果が元と一致
```

既存 `e2e_export_import.rs` との差別化: search結果の詳細比較（ファイル名・セクション数）

### シナリオ5: status --verify

```
Setup:  index で正常なインデックス作成
Act:    status --verify
Assert: "Verify: OK" が出力される
```

既存 `e2e_verify.rs` との差別化: チーム設定と組み合わせた検証（commandindex.tomlあり環境）

### シナリオ6: status --detail

```
Setup:  テスト用ファイル配置 → index
Act:    status --detail
Assert: Coverage/Storage セクションが出力される
```

### シナリオ7: status --format json

```
Setup:  テスト用ファイル配置 → index
Act:    status --format json --detail
Assert: JSONパース成功、coverage/storage フィールド存在
```

## 4. テストヘルパー設計

### 4.1 既存ヘルパー（tests/common/mod.rs）

| 関数 | 用途 |
|------|------|
| `cmd()` | CLIバイナリのCommand生成 |
| `run_index(path)` | インデックス作成 |
| `run_search_jsonl(path, query)` | 検索+JSONL解析 |
| `run_status_json(path)` | status JSON取得 |
| `run_clean(path)` | インデックス削除 |

### 4.2 新規ヘルパー

```rust
/// commandindex.toml をリポジトリルートに作成
fn write_commandindex_toml(base_path: &Path, content: &str) {
    std::fs::write(base_path.join("commandindex.toml"), content)
        .expect("write commandindex.toml");
}

/// .commandindex/config.local.toml を作成
fn write_config_local_toml(base_path: &Path, content: &str) {
    let dir = base_path.join(".commandindex");
    std::fs::create_dir_all(&dir).expect("create .commandindex");
    std::fs::write(dir.join("config.local.toml"), content)
        .expect("write config.local.toml");
}

/// Git リポジトリを初期化（staleness テスト用）
fn init_git_repo(path: &Path) {
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(path)
        .output()
        .expect("git init");
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(path)
        .output()
        .expect("git add");
    std::process::Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(path)
        .output()
        .expect("git commit");
}
```

### 4.3 ヘルパー配置方針

新規ヘルパーは `tests/e2e_team_workflow.rs` 内にローカル関数として定義する。
`tests/common/mod.rs` への追加は最小限にし、既存テストへの影響を避ける。

## 5. 影響範囲

### 変更対象

| ファイル | 変更内容 |
|---------|---------|
| `tests/e2e_team_workflow.rs` | **新規作成**: 7テストシナリオ |

### 影響なし

- プロダクションコード（src/配下）: 変更なし
- 既存テストファイル: 変更なし
- Cargo.toml: dev-dependencies追加なし

## 6. 品質基準

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |

## 7. 設計判断とトレードオフ

| 判断 | 選択 | 理由 |
|------|------|------|
| テストファイル数 | 1ファイルにまとめる | テストバイナリ数増加を抑制、CI時間最適化 |
| ヘルパー配置 | テストファイル内ローカル | common/mod.rs への影響を回避 |
| テスト粒度 | CLI経由のE2E | ユーザー操作を忠実に再現 |
| マルチリポジトリ | スコープ外 | Issue #78未実装 |
| 環境変数テスト | スコープ外 | load_configに環境変数オーバーライド未実装 |

## 8. セキュリティ考慮

- テスト内でAPIキーのダミー値を使用する場合、マスク処理の動作確認に限定
- テストファイルに実際の秘密情報を含めない
- tempfile::tempdir()によるテスト分離で他テストへの干渉を防止
