# CommandIndex

Git-native knowledge CLI — Markdown・Code・Git を横断し、ローカルで高速に知識を引き出す。

## 概要

CommandIndex は、ローカルで動作するナレッジ検索・文脈取得システムです。
Markdownファイル、ソースコード、Git履歴をもとに、個人および少人数チーム向けの知識検索基盤を提供します。

## インストール

### ビルド

```bash
cargo build --release
```

ビルド成果物は `target/release/commandindex` に生成されます。

### GitHub Release

[Releases](https://github.com/Kewton/CommandIndex/releases) からプラットフォーム別のバイナリをダウンロードできます。

## 使い方

```bash
# インデックスの構築
commandindex index

# 検索
commandindex search "認証の流れ"

# 差分更新
commandindex update

# インデックス状態の確認
commandindex status

# インデックスの削除
commandindex clean

# 2ファイルの影響範囲を比較（コンフリクトリスク検出）
commandindex diff src/auth/jwt.rs src/auth/middleware.rs --format json
```

> **注意:** v0.0.0 時点ではコマンドは未実装です。Phase 1 以降で順次実装されます。

## Embedding（セマンティック検索）

セマンティック検索を利用するには、Ollama でサポートされている embedding モデルが必要です。

### 対応モデル

| モデル | 次元数 | 特徴 |
|---|---|---|
| `qllama/bge-m3:q8_0` | 1024 | デフォルト。多言語対応（日本語に強い） |
| `nomic-embed-text` | 768 | 英語中心 |

### 前提条件

1. [Ollama](https://ollama.com/) をインストール・起動
2. 使用するモデルを事前に pull

```bash
# デフォルトモデル
ollama pull qllama/bge-m3:q8_0

# 英語中心モデル
ollama pull nomic-embed-text
```

### モデル変更手順

`commandindex.toml` の `[embedding]` セクションでモデルを変更した場合、次回の `commandindex embed` または `commandindex index --with-embedding` 実行時に旧モデルの embedding が自動的に削除され、新モデルで再生成されます。

```toml
[embedding]
model = "nomic-embed-text"
```

> **注意:** モデル変更後の再生成にはファイル数に応じた時間がかかります。

### v0.x.x からの移行

v0.x.x 以前からアップグレードした場合、デフォルトモデルが `nomic-embed-text` から `qllama/bge-m3:q8_0` に変更されています。

1. 新しいデフォルトモデルをインストール: `ollama pull qllama/bge-m3:q8_0`
2. `commandindex embed` または `commandindex index --with-embedding` を実行すると、旧モデルの embedding は自動的に削除され、新モデルで再生成されます。
3. 旧モデルを引き続き使用する場合は、`commandindex.toml` に `[embedding]` セクションで `model = "nomic-embed-text"` を指定してください。

## 開発

### 前提条件

- Rust (Edition 2024)

### ビルド・テスト

```bash
# ビルド
cargo build

# テスト
cargo test --all

# 静的解析（ゼロ警告必須）
cargo clippy --all-targets -- -D warnings

# フォーマットチェック
cargo fmt --all -- --check
```

### 品質チェック

| チェック項目 | コマンド | 基準 |
|---|---|---|
| ビルド | `cargo build` | エラー 0 件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告 0 件 |
| テスト | `cargo test --all` | 全テスト PASS |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |

## ライセンス

MIT License
