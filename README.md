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
```

> **注意:** v0.0.0 時点ではコマンドは未実装です。Phase 1 以降で順次実装されます。

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
