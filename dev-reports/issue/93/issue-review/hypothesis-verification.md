# 仮説検証レポート - Issue #93

## 検証結果サマリー

| 仮説 | 判定 | 詳細 |
|------|------|------|
| `notify` crateでFS監視 | **Confirmed** | 適切な選択。ただし依存追加が必要 |
| デバウンスで連続変更をまとめる | **Confirmed** | `notify` v7にはdebounce機能あり |
| `.cmindexignore`に従ったフィルタリング | **Confirmed** | `IgnoreFilter`が既存実装済み。再利用可能 |
| `--index-path`と組み合わせ可能 | **Partially Confirmed** | `--index-path`自体は未実装(Issue #88)。`--path`は利用可能 |
| `--daemon`でバックグラウンド実行 | **Unverifiable** | daemon化の既存仕組みなし。新規実装必要 |

## 詳細検証

### 1. `.cmindexignore` パーサー → 実装済み
- `src/parser/ignore.rs` に `IgnoreFilter` として完全実装
- `globset::GlobSet` を使用
- デフォルトルール: `node_modules/**`, `target/**`, `dist/**`, `.git/**`, `.commandindex/**`, `*.min.js`, `*.lock`

### 2. `--index-path` → 未実装
- Issue #88 で対応予定
- 現在は `--path` でインデックス対象ディレクトリを指定
- `.commandindex/` は `--path` 直下に固定生成

### 3. `update` サブコマンド → 完全実装済み
- `src/cli/index.rs` の `run_incremental()` (行607-799)
- 差分検知: `src/indexer/diff.rs` の `detect_changes()` でハッシュ比較
- watch機能のコア処理として再利用可能

### 4. `notify` crate → 未追加
- Cargo.toml に未登録
- 新規依存追加が必須

### 5. daemon化 → 既存仕組みなし
- 全CLIコマンドがワンショット実行モデル
- daemon化は新規実装が必要

### 6. 対象ファイル拡張子
- `md`, `ts`, `tsx`, `py` の4種類 (`src/indexer/manifest.rs::FileType::all_extensions()`)
