# 作業計画: Issue #79 チーム向けstatusコマンド拡張

## Issue概要

| 項目 | 内容 |
|------|------|
| **Issue番号** | #79 |
| **タイトル** | [Feature] チーム向けstatusコマンド拡張（インデックスカバレッジ・統計） |
| **サイズ** | L |
| **優先度** | Medium |
| **依存Issue** | なし |

## タスク分解

### Phase 1: データモデル・基盤変更

#### Task 1.1: IndexState に last_commit_hash フィールド追加
- **成果物**: `src/indexer/state.rs`
- **依存**: なし
- **内容**:
  - `IndexState` に `last_commit_hash: Option<String>` を追加
  - `#[serde(default)]` 付与
  - `IndexState::new()` で `last_commit_hash: None` に初期化
- **テスト**: 古い JSON（フィールドなし）からのデシリアライズが `None` になることを検証

#### Task 1.2: EmbeddingStore に count_distinct_files() 追加
- **成果物**: `src/embedding/store.rs`
- **依存**: なし
- **内容**:
  - `SELECT COUNT(DISTINCT section_path) FROM embeddings` クエリ
  - 既存の `count()` パターン踏襲
- **テスト**: 空DB → 0、同一ファイル複数セクション → 正しいユニーク数

#### Task 1.3: StatusOptions 構造体の定義
- **成果物**: `src/cli/status.rs`（または `src/cli/status/mod.rs`）
- **依存**: なし
- **内容**:
  - `StatusOptions { detail, coverage, format }` + `Default` トレイト実装
  - `run()` シグネチャを `run(path, &options, writer)` に変更
  - 既存の表示ロジックは変更なし（options のフラグを無視する状態で OK）

#### Task 1.4: status.rs をディレクトリモジュール化 + git_info.rs 作成
- **成果物**: `src/cli/status/mod.rs`, `src/cli/status/git_info.rs`
- **依存**: Task 1.3
- **内容**:
  - `src/cli/status.rs` → `src/cli/status/mod.rs` に移動
  - `src/cli/status/git_info.rs` を新規作成
  - `validate_commit_hash()`, `get_current_commit_hash()`, `get_staleness_info()` を実装
  - `StalenessInfo` 構造体を定義

### Phase 2: 新規型の実装とデータ収集ロジック

#### Task 2.1: CoverageInfo の実装
- **成果物**: `src/cli/status/mod.rs`
- **依存**: Task 1.2, Task 1.4
- **内容**:
  - `CoverageInfo { discoverable_files, indexed_files, skipped_files, embedding_file_count, embedding_model }`
  - `count_discoverable_files()` 関数（walkdir + デフォルト除外 + .cmindexignore）
  - `get_embedding_file_count()` ヘルパー（DB不在時 0 返却パターン）
  - EmbeddingConfig からモデル名取得（config.toml 不在時 "(not configured)"）

#### Task 2.2: StorageBreakdown の実装
- **成果物**: `src/cli/status/mod.rs`
- **依存**: Task 1.4
- **内容**:
  - `StorageBreakdown { tantivy_bytes, symbols_db_bytes, embeddings_db_bytes, other_bytes, total_bytes }`
  - `indexer::index_dir()`, `symbol_db_path()`, `embeddings_db_path()` を活用
  - `compute_storage_breakdown()` 関数

#### Task 2.3: StatusInfo 拡張
- **成果物**: `src/cli/status/mod.rs`
- **依存**: Task 2.1, Task 2.2
- **内容**:
  - StatusInfo に `coverage: Option<CoverageInfo>`, `staleness: Option<StalenessInfo>`, `storage: Option<StorageBreakdown>` 追加
  - `#[serde(skip_serializing_if = "Option::is_none")]` 付与
  - 新規フィールドに `strip_control_chars()` 適用

### Phase 3: CLI統合と表示ロジック

#### Task 3.1: main.rs のCLIオプション追加
- **成果物**: `src/main.rs`
- **依存**: Task 1.3
- **内容**:
  - Commands::Status に `--detail`, `--coverage` フラグ追加
  - `conflicts_with` 設定
  - dispatch 部分で `StatusOptions` 構築 → `run()` 呼び出し

#### Task 3.2: run() の条件分岐（--detail / --coverage）
- **成果物**: `src/cli/status/mod.rs`
- **依存**: Task 2.3, Task 3.1
- **内容**:
  - `options.detail` 時: CoverageInfo + StalenessInfo + StorageBreakdown を全て収集
  - `options.coverage` 時: CoverageInfo のみ収集
  - オプションなし: 既存ロジックのみ（収集スキップ）

#### Task 3.3: Human フォーマット出力の拡張
- **成果物**: `src/cli/status/mod.rs`
- **依存**: Task 3.2
- **内容**:
  - `--detail` 時: 基本情報 + Coverage + Staleness + Storage セクション
  - `--coverage` 時: Coverage セクションのみ
  - オプションなし: 既存出力そのまま

#### Task 3.4: JSON フォーマット出力の拡張
- **成果物**: `src/cli/status/mod.rs`
- **依存**: Task 3.2
- **内容**:
  - StatusInfo の serde Serialize による自動 JSON 生成
  - `skip_serializing_if` により既存互換維持

#### Task 3.5: index.rs に last_commit_hash 設定フロー追加
- **成果物**: `src/cli/index.rs`
- **依存**: Task 1.1, Task 1.4
- **内容**:
  - `run()` の state 保存前で `git_info::get_current_commit_hash()` を呼び出し
  - `state.last_commit_hash = commit_hash;`
  - `run_incremental()` にも同様の処理

### Phase 4: テスト

#### Task 4.1: 既存テストの修正
- **成果物**: `tests/cli_status.rs`, `tests/cli_args.rs`
- **依存**: Task 3.2
- **内容**:
  - `run()` 呼び出しを `StatusOptions::default()` に移行
  - JSON テストを `StatusOptions { format: StatusFormat::Json, ..Default::default() }` に変更

#### Task 4.2: 新規ユニットテスト
- **成果物**: `src/embedding/store.rs`, `src/cli/status/git_info.rs`
- **依存**: Task 1.2, Task 1.4
- **内容**:
  - `test_count_distinct_files_empty`, `test_count_distinct_files_with_data`
  - `test_validate_commit_hash` (有効/無効パターン)
  - `test_state_backward_compat` (古い state.json の読み込み)

#### Task 4.3: 新規統合テスト
- **成果物**: `tests/cli_status.rs`
- **依存**: Task 3.3, Task 3.4
- **内容**:
  - `test_status_detail_human`: --detail の全セクション出力
  - `test_status_detail_json`: --detail --format json の拡張フィールド
  - `test_status_coverage_only`: --coverage のCoverageセクション出力
  - `test_status_default_compatible`: オプションなしの既存互換
  - `test_status_default_json_no_extra_fields`: デフォルト JSON に拡張フィールドなし
  - `test_detail_coverage_conflict`: 排他エラー
  - `test_embedding_count_no_db`: DB不在時 0 返却
  - `test_storage_breakdown`: ストレージ内訳の正確性

### Phase 5: 品質チェック

#### Task 5.1: 全品質チェック実行
- **依存**: Phase 4 完了
- **内容**:
  - `cargo build` — エラー0件
  - `cargo clippy --all-targets -- -D warnings` — 警告0件
  - `cargo test --all` — 全テストパス
  - `cargo fmt --all -- --check` — 差分なし

## タスク依存関係

```
Phase 1 (並列可):
  Task 1.1 ──────────────────────────────┐
  Task 1.2 ──────────────────────────────┤
  Task 1.3 → Task 1.4 ──────────────────┤
                                         │
Phase 2 (Task 1.x 完了後):              │
  Task 2.1 (← 1.2, 1.4) ───────────────┤
  Task 2.2 (← 1.4) ────────────────────┤
  Task 2.3 (← 2.1, 2.2) ──────────────┤
                                        │
Phase 3 (並列可):                       │
  Task 3.1 (← 1.3) ────────────────────┤
  Task 3.2 (← 2.3, 3.1) ──────────────┤
  Task 3.3 (← 3.2) ────────────────────┤
  Task 3.4 (← 3.2) ────────────────────┤
  Task 3.5 (← 1.1, 1.4) ──────────────┤
                                        │
Phase 4 (Phase 3 完了後):               │
  Task 4.1 (← 3.2) ────────────────────┤
  Task 4.2 (← 1.2, 1.4) ──────────────┤
  Task 4.3 (← 3.3, 3.4) ──────────────┤
                                        │
Phase 5:                                │
  Task 5.1 (← Phase 4 完了) ───────────┘
```

## TDD 実装順序（推奨）

1. **Task 1.1** → テスト: serde 後方互換テスト
2. **Task 1.2** → テスト: count_distinct_files ユニットテスト
3. **Task 1.3** → テスト: StatusOptions::default() で既存テスト通過
4. **Task 1.4** → テスト: validate_commit_hash テスト
5. **Task 3.5** → テスト: index 後に state.json に last_commit_hash が記録される
6. **Task 2.1** → テスト: CoverageInfo 構築（embedding_count_no_db 含む）
7. **Task 2.2** → テスト: StorageBreakdown 計算
8. **Task 2.3** → テスト: StatusInfo JSON 出力の skip_serializing_if
9. **Task 3.1** → テスト: CLI args パース（排他テスト含む）
10. **Task 3.2** → テスト: run() の条件分岐
11. **Task 3.3** → テスト: Human フォーマット出力
12. **Task 3.4** → テスト: JSON フォーマット出力
13. **Task 4.1-4.3** → 残りのテスト追加
14. **Task 5.1** → 品質チェック

## Definition of Done

- [ ] 全タスク完了
- [ ] `cargo build` エラー0件
- [ ] `cargo clippy --all-targets -- -D warnings` 警告0件
- [ ] `cargo test --all` 全テストパス
- [ ] `cargo fmt --all -- --check` 差分なし
- [ ] 既存テスト全パス（後方互換維持）
- [ ] 新規テスト全パス
