# Changelog

## v0.0.5 — Phase 5: Semantic Extension + スニペットオプション

### Added
- **Embedding生成基盤** (#61): Ollama/OpenAI対応のEmbeddingプロバイダー（EmbeddingProviderトレイト、設定管理）
- **Embeddingストレージ** (#62): SQLite embeddings.db によるベクトル格納・コサイン類似度検索
- **`embed` サブコマンド** (#61 #62): Embedding生成・格納の統合コマンド
- **Semantic Search** (#63): `--semantic` オプションによるEmbeddingベースの意味検索機能
- **Hybrid Retrieval** (#64): 全文検索+Semantic SearchのRRF統合によるハイブリッド検索
- **Reranking** (#65): `--rerank` オプションによるCross-Encoder方式の再順位付け（Ollama /api/generate ベース）
- **スニペット表示オプション** (#44): search結果のスニペット行数・文字数をCLIオプションで制御可能に
- **Phase 5 E2E統合テスト** (#66): Embedding・Semantic Search・Hybrid・Rerankingフロー検証

---

## v0.0.4 — Phase 4: リンク解析・関連検索・Context Pack

### Added
- **Markdownリンク解析・リンクインデックス構築** (#51) (#55): Markdown内リンクの解析とリンク関係のインデックス化
- **`--related` 検索オプション** (#50) (#56): リンク関係に基づく関連ドキュメント検索
- **`context-pack` サブコマンド** (#52) (#57): Context Pack（コンテキストパック）生成機能
- **Phase 4 E2E 統合テスト** (#53) (#58): リンク解析・関連検索・Context Packフロー検証

---

## v0.0.3 — Phase 3: ソースコード解析 (tree-sitter + SQLite)

### Added
- **tree-sitter パーサー基盤** (#35): TypeScript/Python ソースコード解析（関数・クラス・インターフェース抽出）
- **SQLite 補助ストア** (#36): symbols.db によるシンボル情報の構造化格納
- **コード index/update 統合** (#37): TypeScript/Python → tantivy + symbols.db へのインデックス統合
- **`--symbol` 検索オプション** (#38): シンボル名による関数・クラス検索
- **`--type` フィルタ拡張** (#39): typescript/python コードファイル種別フィルタ対応
- **Phase 3 E2E 統合テスト** (#40): コード解析・シンボル検索フロー検証

### Fixed
- **indexer**: import/依存関係を symbols.db に格納する処理を追加

### Chore
- Phase 3 UAT結果追加（#35 #36 #37 #38 #39 #40 #41）

---

## v0.0.2 — Phase 2: 差分更新 (update コマンド)

### Added
- **差分検知エンジン** (#25): manifest比較による変更/追加/削除の検出
- **tantivy 差分更新** (#26): ドキュメント単位でのインデックス追加/変更/削除
- **`update` コマンド改善** (#27): saturating_sub・follow_links・出力メッセージ統一
- **Phase 2 E2E 統合テスト** (#28): updateコマンド差分更新フロー検証
- `/release` スラッシュコマンド追加（worktree + commandmatedev）

### Fixed
- **`update` コマンド**: インデックス未作成時にエラー終了するよう修正

### Chore
- Phase 2 UAT結果追加（#25 #26 #27 #28）

---

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
