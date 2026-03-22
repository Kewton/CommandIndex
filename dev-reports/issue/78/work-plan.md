# 作業計画書: Issue #78 マルチリポジトリ横断検索

## Issue: [Feature] マルチリポジトリ横断検索
**Issue番号**: #78
**サイズ**: L（大規模）
**優先度**: High
**依存Issue**: #76 チーム共有設定ファイル（実装済み）
**ブランチ**: `feature/issue-78-multi-repo`（作成済み）

---

## 実装方針サマリー

- SearchResult構造体は変更しない（compositionパターン）
- WorkspaceConfig/WorkspaceConfigErrorは`src/config/workspace.rs`に配置
- 横断検索オーケストレーションは`src/cli/workspace.rs`に配置
- Phase 1はBM25のみ横断対応（逐次実行）
- 各ステップでビルド・テスト通過を維持

---

## Step 1: 依存追加 + WorkspaceConfig型定義（Config層）

### Task 1.1: Cargo.toml依存追加
- **成果物**: `Cargo.toml`
- **作業**: `dirs` クレート追加（チルダ展開用）
- **依存**: なし

### Task 1.2: WorkspaceConfig / WorkspaceConfigError / WorkspaceWarning 型定義
- **成果物**: `src/config/workspace.rs`（新規）
- **作業**:
  - `WorkspaceConfig`, `WorkspaceDefinition`, `RepositoryEntry`, `ResolvedRepository` 構造体
  - `WorkspaceConfigError` enum（Display, Error trait実装）
  - `WorkspaceWarning` enum（Display trait実装）
  - バリデーション規則定数: `MAX_REPOSITORIES = 50`, `MAX_ALIAS_LENGTH = 64`, `MAX_CONFIG_SIZE = 1MB`
- **依存**: Task 1.1

### Task 1.3: ワークスペース設定ロード関数
- **成果物**: `src/config/workspace.rs`
- **作業**:
  - `load_workspace_config(path: &Path) -> Result<WorkspaceConfig, WorkspaceConfigError>`
  - `resolve_repositories(config: &WorkspaceConfig, base_dir: &Path) -> (Vec<ResolvedRepository>, Vec<WorkspaceWarning>)`
  - `expand_path(path: &str) -> Result<PathBuf, WorkspaceConfigError>` （チルダ展開、$記号拒否）
  - `validate_alias(name: &str) -> Result<(), WorkspaceConfigError>` （ASCII英数字+ハイフン+アンダースコア）
  - シンボリックリンクチェック（clean.rsパターン適用）
  - エイリアス重複/パス重複チェック
  - `.commandindex/` 存在チェック
- **依存**: Task 1.2

### Task 1.4: configモジュール登録
- **成果物**: `src/config/mod.rs`
- **作業**: `pub mod workspace;` 追加
- **依存**: Task 1.2

### Task 1.5: WorkspaceConfig ユニットテスト
- **成果物**: `tests/workspace_config.rs`（新規）
- **テストケース**:
  - 正常系: TOML パース、パス解決（絶対/相対/チルダ）
  - エイリアス省略時のデフォルト値
  - エイリアス重複検出
  - パス重複検出（canonicalize後）
  - リポ数上限超過
  - 不正alias（制御文字、長さ超過）
  - 不正パス（$記号、バッククォート）
  - チルダ展開エッジケース（`~` 単体、`~user` 拒否）
  - TOMLファイルサイズ超過
  - 存在しないパス → WorkspaceWarning
  - シンボリックリンク → WorkspaceWarning
- **依存**: Task 1.3

---

## Step 2: rrf_merge_multiple 汎用化（Search層）

### Task 2.1: rrf_merge_multiple実装
- **成果物**: `src/search/hybrid.rs`
- **作業**:
  - `rrf_merge_multiple(result_lists: &[Vec<SearchResult>], limit: usize) -> Vec<SearchResult>` 新設
  - キー: (path, heading) の2タプル
  - スコア: Σ(1/(K + rank))
  - 既存`rrf_merge`をラッパー化（後方互換維持）
- **依存**: なし

### Task 2.2: rrf_merge_multiple ユニットテスト
- **成果物**: `src/search/hybrid.rs` 内テスト
- **テストケース**:
  - 3リスト以上のマージ
  - 空リスト混在
  - 全リストが同一結果を含む場合
  - 既存rrf_mergeテストがラッパー経由でパス
  - 同一キーのスコア正しい加算
- **依存**: Task 2.1

---

## Step 3: SearchContext導入（CLI層リファクタリング）

### Task 3.1: SearchContext構造体定義
- **成果物**: `src/cli/search.rs`
- **作業**:
  - `SearchContext { base_path, config }` 構造体
  - `from_current_dir()`, `from_path()` コンストラクタ
  - `index_dir()`, `symbol_db_path()` メソッド
- **依存**: なし

### Task 3.2: run()関数のSearchContext化
- **成果物**: `src/cli/search.rs`
- **作業**:
  - run()シグネチャ変更: `ctx: &SearchContext` を第1引数に追加
  - run()内のPath::new(".")をctx.base_pathに置換（2箇所）
  - run()内のload_config呼出を除去（ctx.configを使用）
  - try_hybrid_searchにctx伝播（内部Path::new(".")3箇所も置換）
- **依存**: Task 3.1

### Task 3.3: main.rsのSearchContext統合
- **成果物**: `src/main.rs`
- **作業**:
  - SearchコマンドハンドラでSearchContext::from_current_dir()構築
  - config読込をSearchContext経由に統一
  - effective_limit算出はctx.configから
- **依存**: Task 3.2

### Task 3.4: SearchError拡張
- **成果物**: `src/cli/search.rs`
- **作業**:
  - `SearchError::Workspace(WorkspaceConfigError)` バリアント追加
  - `From<WorkspaceConfigError> for SearchError` 実装
- **依存**: Task 1.2

### Task 3.5: 既存テスト修正・リグレッション確認
- **テスト**: `cargo test --all` 全パス確認
- **依存**: Task 3.3

---

## Step 4: WorkspaceSearchResult + Output層拡張

### Task 4.1: WorkspaceSearchResult定義
- **成果物**: `src/output/mod.rs`
- **作業**:
  - `WorkspaceSearchResult { repository: String, result: SearchResult }` 構造体
- **依存**: なし

### Task 4.2: Output層のワークスペース対応
- **成果物**: `src/output/human.rs`, `src/output/json.rs`, `src/output/path.rs`
- **作業**:
  - `format_workspace_human()`: `[alias] path:line [## heading]` 形式
  - `format_workspace_json()`: `{"repository":"alias","path":"...","heading":"...",...}` 形式
  - `format_workspace_path()`: 重複除去キー(alias, path)
- **依存**: Task 4.1

### Task 4.3: Output ユニットテスト
- **成果物**: `tests/output_format.rs`
- **テストケース**:
  - ワークスペース用各フォーマッタの出力検証
  - 同名ファイルの重複除去（異なるリポ）
- **依存**: Task 4.2

---

## Step 5: CLIオプション追加

### Task 5.1: SearchコマンドにCLIオプション追加
- **成果物**: `src/main.rs`
- **作業**:
  - `--workspace: Option<String>` 追加
  - `--repo: Option<String>` 追加（requires = "workspace"）
  - --symbol/--related/--semanticの`conflicts_with_all`に"workspace"追加
- **依存**: なし

### Task 5.2: Status/UpdateコマンドにCLIオプション追加
- **成果物**: `src/main.rs`
- **作業**:
  - Status/Updateに`--workspace: Option<String>` 追加
- **依存**: なし

### Task 5.3: CLIオプション パーステスト
- **成果物**: `tests/cli_args.rs`
- **テストケース**:
  - --workspace/--repo 正常パース
  - --repo単独指定時のエラー
  - --workspace + --symbol 競合
  - --workspace + --related 競合
  - --workspace + --semantic 競合
- **依存**: Task 5.1

---

## Step 6: 横断検索オーケストレーション

### Task 6.1: run_workspace_search実装
- **成果物**: `src/cli/workspace.rs`（新規）
- **作業**:
  - `run_workspace_search(ws_path, repo_filter, options, filters, format, snippet_config, rerank, rerank_top)`
  - WorkspaceConfig読込 → リポジトリ解決 → --repoフィルタ
  - 各リポで逐次検索（SearchContext::from_path() → run()）
  - pathにaliasプレフィックス付与 → rrf_merge_multiple → WorkspaceSearchResult変換
  - 警告出力（WorkspaceWarning一括処理）
  - 進捗メッセージ（`[1/3] Searching frontend...`）
- **依存**: Step 1-5 全て

### Task 6.2: run_workspace_status実装
- **成果物**: `src/cli/workspace.rs`
- **作業**:
  - ワークスペースstatus表示（Human/JSON対応）
  - 各リポのファイル数・最終更新日・ステータス表示
- **依存**: Task 6.1

### Task 6.3: run_workspace_update実装
- **成果物**: `src/cli/workspace.rs`
- **作業**:
  - 各リポで逐次update（進捗メッセージ付き）
  - エラー時スキップ・続行、エラーサマリー + 非ゼロ終了
- **依存**: Task 6.1

### Task 6.4: main.rsの分岐フロー統合
- **成果物**: `src/main.rs`
- **作業**:
  - Search/Status/Updateのworkspace有無分岐
  - workspace指定時 → workspace.rsの関数呼出
  - 非workspace時 → 既存フロー維持
- **依存**: Task 6.1-6.3

### Task 6.5: cliモジュール登録
- **成果物**: `src/cli/mod.rs`
- **作業**: `pub mod workspace;` 追加
- **依存**: Task 6.1

---

## Step 7: 統合テスト（E2E）

### Task 7.1: ワークスペース横断検索E2Eテスト
- **成果物**: `tests/e2e_workspace.rs`（新規）
- **テストケース**:
  - 3リポでの横断検索
  - --repoフィルタ動作
  - Human/JSON/Path各出力形式
  - 一部リポインデックス未作成時のgraceful degradation
  - 存在しないリポパスのスキップ
  - 後方互換（--workspace未指定時の動作不変）
- **依存**: Step 6

### Task 7.2: ワークスペースstatus/update E2Eテスト
- **成果物**: `tests/e2e_workspace.rs`
- **テストケース**:
  - status --workspace のHuman/JSON出力
  - update --workspace の逐次更新
  - 一部失敗時のエラーハンドリング
- **依存**: Step 6

### Task 7.3: リグレッションテスト確認
- **作業**: 既存テスト全パス確認
- **コマンド**: `cargo test --all`
- **依存**: Step 7

---

## Step 8: 品質チェック・最終確認

### Task 8.1: 品質チェック

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |

---

## 実装順序の依存関係グラフ

```
Step 1 (WorkspaceConfig)  Step 2 (rrf_merge)  Step 4 (Output)  Step 5 (CLI)
    |                         |                     |              |
    +-------------------------+---------------------+--------------+
                              |
                          Step 3 (SearchContext)
                              |
                          Step 6 (オーケストレーション)
                              |
                          Step 7 (E2Eテスト)
                              |
                          Step 8 (品質チェック)
```

**並行実施可能**: Step 1, 2, 4, 5は独立して並行実施可能
**順序必須**: Step 3 → Step 6 → Step 7 → Step 8

---

## Definition of Done

- [ ] 全タスク完了
- [ ] `cargo test --all` 全パス
- [ ] `cargo clippy --all-targets -- -D warnings` 警告ゼロ
- [ ] `cargo fmt --all -- --check` 差分なし
- [ ] ワークスペース横断検索が動作する
- [ ] 後方互換（--workspace未指定時の動作不変）
- [ ] Graceful Degradation（一部リポ不在時のスキップ）
