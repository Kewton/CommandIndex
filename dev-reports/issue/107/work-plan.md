# 作業計画書 - Issue #107

## Issue: search結果のデフォルトlimit引き下げ (LLM用途向け)
**Issue番号**: #107
**サイズ**: S
**優先度**: Medium
**依存Issue**: なし
**ブランチ**: `feature/issue-107-default-limit`（既存）

---

## 設計方針サマリー

- **方式**: config `llm_default_limit` 追加
- **LLM判定条件**: `--rerank` フラグのみ
- **デフォルト値**: 5件
- **limit解決**: `SearchConfig::resolve_limit()` で一元管理
- **バリデーション**: `.clamp(1, 1000)`

---

## 詳細タスク分解

### Phase 1: Config層の実装

#### Task 1.1: RawSearchConfig にフィールド追加
- **ファイル**: `src/config/mod.rs`
- **変更箇所**: L90 `RawSearchConfig` 構造体
- **内容**: `pub llm_default_limit: Option<usize>` フィールド追加
- **依存**: なし

#### Task 1.2: SearchConfig にフィールド追加 + Default trait
- **ファイル**: `src/config/mod.rs`
- **変更箇所**: L154-159 `SearchConfig` 構造体
- **内容**:
  - `pub llm_default_limit: usize` フィールド追加
  - `impl Default for SearchConfig` 追加（default_limit:20, llm_default_limit:5, snippet_lines:2, snippet_chars:120）
- **依存**: Task 1.1

#### Task 1.3: resolve_limit メソッド追加
- **ファイル**: `src/config/mod.rs`
- **内容**: `SearchConfig::resolve_limit(&self, cli_limit: Option<usize>, is_rerank: bool) -> usize` メソッド
- **ロジック**: cli_limit優先 → rerank時llm_default_limit → 通常時default_limit、`.clamp(1, 1000)`
- **依存**: Task 1.2

#### Task 1.4: merge_search にマージ処理追加
- **ファイル**: `src/config/mod.rs`
- **変更箇所**: L362-365 `merge_search` 関数
- **内容**: `llm_default_limit: h.llm_default_limit.or(b.llm_default_limit)` 追加
- **依存**: Task 1.1

#### Task 1.5: resolve_config にデフォルト値解決追加
- **ファイル**: `src/config/mod.rs`
- **変更箇所**: L412-424 `resolve_config` 関数
- **内容**: `llm_default_limit: raw.search.as_ref().and_then(|s| s.llm_default_limit).unwrap_or(5)` 追加（snippet_charsの前に配置）
- **依存**: Task 1.1, 1.2

### Phase 2: CLI層の実装

#### Task 2.1: main.rs limit解決ロジック置換
- **ファイル**: `src/main.rs`
- **変更箇所**: L357-368
- **内容**:
  - `SearchConfig::default()` でNoneフォールバック統一
  - `config.resolve_limit(limit, rerank)` で effective_limit 解決
  - workspace検索パス（L383-403）でも同じロジック適用
- **依存**: Task 1.3

#### Task 2.2: CLIヘルプ文字列更新
- **ファイル**: `src/main.rs`
- **変更箇所**: L72
- **内容**: `/// Maximum number of results (default: 20, with --rerank: 5)`
- **依存**: なし

#### Task 2.3: help-llm 更新
- **ファイル**: `src/cli/help_llm.rs`
- **変更箇所**: L281
- **内容**: `--limit <N>  Maximum number of results (default: 20, with --rerank: 5)`
- **依存**: なし

### Phase 3: 既存テスト修正

#### Task 3.1: SearchConfig構造体リテラルの更新
- **ファイル**: `src/config/mod.rs`
- **変更箇所**:
  - L707-711 `test_to_masked_view_masks_api_keys`
  - L741-745 `test_to_masked_view_no_api_keys`
  - L967-971 `test_view_model_serializes_to_toml`
- **内容**: 全箇所に `llm_default_limit: 5` 追加
- **依存**: Task 1.2

### Phase 4: 新規テスト追加

#### Task 4.1: resolve_limit ユニットテスト
- **ファイル**: `src/config/mod.rs`（テストモジュール内）
- **テストケース**:
  - CLI `--limit` 指定時は優先される
  - rerank=true, limit=None → llm_default_limit
  - rerank=false, limit=None → default_limit
  - limit=0 → clamp(1, 1000) で1になる
  - limit=2000 → clamp(1, 1000) で1000になる
- **依存**: Task 1.3

#### Task 4.2: merge_search マージテスト拡張
- **ファイル**: `src/config/mod.rs`
- **変更箇所**: `test_merge_raw_higher_wins` テスト
- **内容**: RawSearchConfigに `llm_default_limit` のマージ検証追加
- **依存**: Task 1.4

#### Task 4.3: resolve_config デフォルト値テスト
- **ファイル**: `src/config/mod.rs`
- **変更箇所**: `test_resolve_config_defaults`, `test_resolve_config_with_values`
- **内容**: `assert_eq!(config.search.llm_default_limit, 5)` 等追加
- **依存**: Task 1.5

#### Task 4.4: config show TOML出力テスト
- **ファイル**: `src/config/mod.rs`
- **変更箇所**: `test_view_model_serializes_to_toml`
- **内容**: `llm_default_limit` がTOML出力に含まれることを検証
- **依存**: Task 3.1

---

## 実装順序

```
Phase 1: Config層 (Task 1.1 → 1.2 → 1.3, 1.4, 1.5 並列)
    ↓
Phase 2: CLI層 (Task 2.1, 2.2, 2.3 並列)
    ↓
Phase 3: 既存テスト修正 (Task 3.1)
    ↓
Phase 4: 新規テスト追加 (Task 4.1, 4.2, 4.3, 4.4 並列)
    ↓
品質チェック (cargo build → clippy → test → fmt)
```

---

## 品質チェック項目

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |

---

## Definition of Done

- [ ] `llm_default_limit` が config に追加（デフォルト: 5）
- [ ] `SearchConfig::resolve_limit()` メソッドが実装されている
- [ ] `--rerank` 指定かつ `--limit` 未指定時に llm_default_limit が使用される
- [ ] `--limit` 明示指定時はそちらが優先される
- [ ] workspace検索でも同じlimit解決ロジックが適用される
- [ ] 既存テストが全て通過
- [ ] 新規テスト（resolve_limit, merge, config show）が追加・通過
- [ ] cargo build / clippy / test / fmt 全パス
