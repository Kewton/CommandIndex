# v0.0.0 — 開発環境整備

## フェーズの目的

CommandIndex の開発を開始するための基盤を整備する。
Rust プロジェクトの初期化から CI/CD、Claude Code 統合まで、Anvil と同等の開発体験を実現する。

## このフェーズに含まれるもの

- Rust プロジェクト初期化（Cargo.toml, src/, tests/）
- 品質基盤（clippy, fmt, テスト雛形）
- CI/CD パイプライン（GitHub Actions）
- GitHub テンプレート（PR, Issue）
- プロジェクトドキュメント（README, CLAUDE.md, CHANGELOG 等）
- Claude Code 統合（コマンド、エージェント、プロンプト）
- Git ワークフロー整備（ブランチ戦略、保護ルール）

## このフェーズに含まれないもの

- Phase 1 の機能実装（Markdown解析、tantivy インデックス等）
- プロダクションコード
- ユーザ向け機能

## 成果物

このフェーズが完了すると、以下が可能になる。

- `cargo build` / `cargo test` / `cargo clippy` / `cargo fmt` が通る
- GitHub に push すると CI が自動実行される
- feature ブランチ → PR → CI → マージの一連フローが動く
- Claude Code のカスタムコマンド・エージェントが利用可能
- Phase 1 の機能実装にすぐ着手できる状態になる

## 関連ドキュメント

- [dev-environment-plan.md](./dev-environment-plan.md) — 目的・目標・実施順序
- [repository-layout.md](./repository-layout.md) — ディレクトリ構成・モジュール設計
- [ci-cd-plan.md](./ci-cd-plan.md) — CI/CD パイプライン設計
- [claude-code-setup.md](./claude-code-setup.md) — Claude Code 統合計画
- [initial-implementation-slice.md](./initial-implementation-slice.md) — 最初の動作確認スライス
