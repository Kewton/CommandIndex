# Changelog

## v0.0.1 — Phase 1: Markdown Knowledge MVP

### Added
- **Markdown パーサー** (#3): ファイル走査・heading単位分割・frontmatter/tag抽出・リンク抽出
- **tantivy インデックス基盤** (#4): スキーマ定義・lindera日本語トークナイザー・Writer/Reader
- **.cmindexignore** (#5): glob パターンによるファイル除外・デフォルト除外ルール
- **インデックス状態管理** (#6): manifest.json / state.json による状態追跡
- **`index` コマンド** (#7): Markdown解析 → tantivy格納 → 状態保存
- **出力フォーマッター** (#8): human（色付き）/ json（JSONL）/ path の3形式
- **`search` コマンド** (#9): 全文検索・タグ/パス/見出し/種別フィルタ・日本語対応
- **`status` コマンド** (#10): インデックス状態表示
- **`clean` コマンド** (#11): インデックス削除・再構築
- **E2E 統合テスト** (#12): index → search → status → clean の一連フロー検証
- Claude Code スラッシュコマンド追加（設計レビュー、UAT、オーケストレーション等）

---

## v0.0.0 — 開発環境整備

### Added
- Rust プロジェクト初期化（Cargo.toml, src/main.rs, src/lib.rs）
- CLI スケルトン（clap サブコマンド: index, search, update, status, clean）
- 統合テスト（tests/cli_args.rs — 10テスト）
- CI/CD パイプライン（GitHub Actions: fmt, clippy, test, build）
- リリースパイプライン（4ターゲットクロスビルド）
- GitHub テンプレート（PR, Issue）
- プロジェクトドキュメント（README, CLAUDE.md, COMMANDINDEX.md）
- Claude Code 統合（コマンド、エージェント、プロンプト）
- 企画書（workspace/plan/plan_v0.0.1.md）
- 作業計画（workspace/v0.0.0/）
