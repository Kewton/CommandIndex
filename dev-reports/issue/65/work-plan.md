# 作業計画書: Issue #65 Reranking（検索結果の再順位付け）

## Issue概要

- **Issue番号**: #65
- **タイトル**: [Feature] Reranking（検索結果の再順位付け）
- **サイズ**: M（新モジュール2ファイル + 既存4ファイル修正）
- **優先度**: Medium
- **依存Issue**: #64 Hybrid Retrieval（実装済み）
- **設計方針書**: `dev-reports/design/issue-65-reranking-design-policy.md`

## 詳細タスク分解

### Phase 1: データモデル・型定義

#### Task 1.1: RerankConfig・型定義（src/rerank/mod.rs）
- **成果物**: `src/rerank/mod.rs`
- **依存**: なし
- **内容**:
  - `RerankConfig` 構造体（model, top_candidates, endpoint, api_key, timeout_secs）
  - `RerankCandidate` 構造体（document_text, original_index）
  - `RerankResult` 構造体（index, score）+ 契約コメント
  - `RerankError` enum（NetworkError, ApiError, ModelNotFound, InvalidResponse, Timeout, ConfigError, ProviderNotImplemented）
  - `RerankProvider` トレイト（rerank メソッドのみ）
  - `build_document_text()` ヘルパー関数（chars().take(4096) で UTF-8安全切り詰め）
  - ファクトリ関数 `create_rerank_provider(config: &RerankConfig) -> Box<dyn RerankProvider>`
- **テスト**:
  - `RerankConfig` のデフォルト値テスト
  - `build_document_text()` のテスト（通常、heading空、長文truncate、日本語マルチバイト）

#### Task 1.2: Config構造体拡張（src/embedding/mod.rs）
- **成果物**: `src/embedding/mod.rs` 修正
- **依存**: Task 1.1
- **内容**:
  - `Config` に `pub rerank: Option<RerankConfig>` フィールド追加
  - `use crate::rerank::RerankConfig;` 追加
- **テスト**:
  - 既存テスト `test_config_parse_no_embedding_section` に `config.rerank.is_none()` アサーション追加
  - 既存テスト `test_config_parse_full_toml` に `config.rerank.is_none()` アサーション追加
  - 新テスト: `[rerank]` セクションありのTOMLパーステスト
  - 新テスト: 空の `[rerank]` セクション（全デフォルト値）のパーステスト

#### Task 1.3: lib.rs モジュール宣言
- **成果物**: `src/lib.rs` 修正
- **依存**: Task 1.1
- **内容**:
  - `pub mod rerank;` 追加

### Phase 2: コアロジック実装

#### Task 2.1: OllamaRerankProvider（src/rerank/ollama.rs）
- **成果物**: `src/rerank/ollama.rs`
- **依存**: Task 1.1
- **内容**:
  - `OllamaRerankProvider` 構造体（client, model, endpoint, timeout_secs）
  - `OllamaRerankProvider::new(config: &RerankConfig) -> Self`
  - `RerankProvider` トレイト実装
    - 全体タイムアウト管理（Instant::now() + timeout_secs）
    - 各候補について逐次 POST /api/generate
    - プロンプトテンプレート（デリミタで document_text を囲む）
    - レスポンスから数値抽出 → 0-10 クランプ
    - パース失敗時はスコア0としてRerankResultに含める
    - タイムアウト超過時は処理済み候補のみOkで返す
  - リクエスト/レスポンス構造体（OllamaGenerateRequest, OllamaGenerateResponse）
- **テスト**:
  - プロンプト生成テスト（テンプレート検証）
  - スコアパーステスト（正常値、範囲外、パース失敗）
  - MockRerankProviderを使ったrerankロジックテスト

#### Task 2.2: try_rerank統合関数（src/cli/search.rs）
- **成果物**: `src/cli/search.rs` 修正
- **依存**: Task 2.1
- **内容**:
  - `try_rerank()` 関数実装（Graceful Degradation パターン）
    - Config読み込み → Provider作成 → rerank実行
    - 各ステップでエラー時は eprintln!("[rerank] ...") + 元結果返却
    - rerank成功時: スコア上書き + 安定ソートで再ソート
    - 未返却indexはスコア0
  - `run()` シグネチャ変更（rerank: bool, rerank_top: usize 追加）
  - `run()` 内のフロー変更:
    - rerank有効時: 候補取得数 = max(limit, rerank_top) に調整
    - final_results 確定後に try_rerank() 呼び出し
    - .take(limit) で最終件数に切り詰め
- **テスト**:
  - MockRerankProviderを使った try_rerank テスト（正常系）
  - Graceful Degradation テスト（Provider作成失敗 → 元結果返却）

### Phase 3: CLI統合

#### Task 3.1: CLI引数追加（src/main.rs）
- **成果物**: `src/main.rs` 修正
- **依存**: Task 2.2
- **内容**:
  - `Commands::Search` に `rerank: bool` 追加（conflicts_with: symbol, related, semantic）
  - `Commands::Search` に `rerank_top: usize` 追加（requires: rerank, default: 20）
  - マッチアーム destructuring 更新
  - `run()` 呼び出しに rerank, rerank_top を渡す
- **テスト**:
  - `--rerank` フラグ受け入れテスト
  - `--rerank --rerank-top 30` 受け入れテスト
  - `--rerank --symbol` conflicts テスト
  - `--rerank --related` conflicts テスト
  - `--rerank --semantic` conflicts テスト
  - `--rerank-top 20` 単独（--rerankなし）エラーテスト

### Phase 4: 品質チェック・仕上げ

#### Task 4.1: 品質チェック
- **内容**:
  - `cargo build` エラー0件
  - `cargo clippy --all-targets -- -D warnings` 警告0件
  - `cargo test --all` 全テストパス
  - `cargo fmt --all -- --check` 差分なし
  - 既存テスト全パス確認

## 実装順序

```
Task 1.1 (型定義)
  ├→ Task 1.2 (Config拡張)
  ├→ Task 1.3 (lib.rs)
  └→ Task 2.1 (OllamaProvider)
       └→ Task 2.2 (try_rerank統合)
            └→ Task 3.1 (CLI引数)
                 └→ Task 4.1 (品質チェック)
```

## TDD実装ガイドライン

各タスクでTDD（Red-Green-Refactor）を適用:

1. **Red**: テストを先に書く（コンパイルエラーも含む）
2. **Green**: テストを通す最小限の実装
3. **Refactor**: コードの整理

### MockRerankProvider（テスト用）

```rust
#[cfg(test)]
pub struct MockRerankProvider {
    pub scores: Vec<f32>,
    pub should_fail: bool,
}

#[cfg(test)]
impl RerankProvider for MockRerankProvider {
    fn rerank(&self, _query: &str, documents: &[RerankCandidate])
        -> Result<Vec<RerankResult>, RerankError> {
        if self.should_fail {
            return Err(RerankError::NetworkError("mock error".to_string()));
        }
        Ok(documents.iter().enumerate().map(|(i, _)| {
            RerankResult {
                index: i,
                score: self.scores.get(i).copied().unwrap_or(0.0),
            }
        }).collect())
    }
}
```

## 品質チェック項目

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |

## Definition of Done

- [x] 設計方針書が作成・レビュー済み
- [ ] すべてのタスク（Task 1.1〜4.1）が完了
- [ ] `--rerank` でCross-Encoderによる再順位付けが実行される
- [ ] `--rerank` なしでは従来の検索結果
- [ ] `--rerank-top` でリランク候補数を指定できる
- [ ] Graceful Degradation が動作する
- [ ] テストが全パス
- [ ] clippy警告ゼロ
- [ ] cargo fmt 差分なし
