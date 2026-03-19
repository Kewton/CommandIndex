# CI/CD パイプライン設計

## 概要

Anvil と同等の CI/CD パイプラインを GitHub Actions で構築する。

## CI パイプライン（ci.yml）

### トリガー

- `main` ブランチへの push
- `develop` ブランチへの push
- 上記ブランチへの PR

### ジョブ構成

```
fmt ─────────┐
clippy ──────┤
test ────────┤──→ build（依存: fmt, clippy, test が全て PASS）
```

4 ジョブは以下の通り。fmt / clippy / test は並列実行、build はそれらの完了後に実行する。

### ジョブ詳細

#### 1. fmt

```yaml
目的: コードフォーマットの統一
環境: ubuntu-latest
コマンド: cargo fmt --all -- --check
キャッシュ: なし（高速のため不要）
```

#### 2. clippy

```yaml
目的: 静的解析によるコード品質チェック
環境: ubuntu-latest
環境変数: RUSTFLAGS="-D warnings"
コマンド: cargo clippy --all-targets -- -D warnings
キャッシュ: Swatinem/rust-cache@v2
```

#### 3. test

```yaml
目的: 全テストの実行
環境: ubuntu-latest
環境変数: RUSTFLAGS="-D warnings"
コマンド: cargo test --all
キャッシュ: Swatinem/rust-cache@v2
```

#### 4. build

```yaml
目的: リリースビルドの確認
環境: ubuntu-latest
依存: fmt, clippy, test
コマンド: cargo build --release
キャッシュ: Swatinem/rust-cache@v2
```

## リリースパイプライン（release.yml）

### トリガー

- `v*` パターンのタグ push（例: `v0.1.0`）

### ビルドマトリクス

| ターゲット | バイナリ名 |
|---|---|
| `x86_64-unknown-linux-gnu` | `commandindex-linux-amd64` |
| `aarch64-unknown-linux-gnu` | `commandindex-linux-arm64` |
| `x86_64-apple-darwin` | `commandindex-darwin-amd64` |
| `aarch64-apple-darwin` | `commandindex-darwin-arm64` |

### フロー

1. チェックアウト
2. Rust ツールチェーン + ターゲットインストール
3. クロスコンパイルツールインストール（aarch64-linux 用）
4. リリースビルド
5. gzip パッケージング
6. アーティファクトアップロード
7. GitHub Release 自動作成（auto-generated notes）

### 権限

```yaml
permissions:
  contents: write  # Release 作成に必要
```

## ブランチ保護ルール

### main

- Direct push 禁止
- PR 必須
- CI 全ジョブ PASS 必須
- 1+ レビュー必須

### develop

- Direct push 禁止
- PR 必須
- CI 全ジョブ PASS 必須

## 品質チェックコマンド一覧

開発者がローカルで実行する品質チェック。CI と同じ基準。

```bash
# フォーマットチェック
cargo fmt --all -- --check

# 静的解析（ゼロ警告）
cargo clippy --all-targets -- -D warnings

# テスト
cargo test --all

# リリースビルド
cargo build --release
```
