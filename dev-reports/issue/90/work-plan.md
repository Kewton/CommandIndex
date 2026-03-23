# 作業計画書: Issue #90

## Issue: [Feature] impact サブコマンド（git diff ベースの影響分析）
**Issue番号**: #90
**サイズ**: M
**優先度**: High
**依存Issue**: #87（完了）, #89（完了）
**ブランチ**: feature/issue-90-impact（既存）

## 現状

既にプロトタイプ実装が存在するが、Issue仕様（レビュー反映後）との乖離が大きい:
- データモデル: フラット構造 → ネスト構造（per-file）
- overlap / summary: 未実装
- JSON フィールド名: 不一致
- E2E / 単体テスト: 旧仕様ベース

## 詳細タスク分解

### Phase 1: データモデル再設計

- [ ] **Task 1.1**: `src/output/mod.rs` の型定義変更
  - ImpactResult, ImpactFileResult を削除
  - ImpactResult, ImpactPerFile, ImpactRelatedFile, ImpactSummary を新規定義
  - 全型に `#[derive(Debug, Clone, Serialize)]` 付与
  - 成果物: `src/output/mod.rs`
  - 依存: なし

- [ ] **Task 1.2**: `format_impact_results` ディスパッチャー更新
  - 新 ImpactResult を受け取るシグネチャに変更
  - 成果物: `src/output/mod.rs`
  - 依存: Task 1.1

### Phase 2: コアロジック書き換え

- [ ] **Task 2.1**: `src/cli/impact.rs` の `aggregate_impact` 全面書き換え
  - INTERNAL_FETCH_LIMIT 定数定義
  - per-file 結果保持 + overlap 検出 + summary 計算
  - 入力ファイル除外（正規化済みパスで比較）
  - limit は per-file related に適用
  - FileNotFound/FileNotIndexed は warning 継続、その他は fail-fast
  - 成果物: `src/cli/impact.rs`
  - 依存: Task 1.1

- [ ] **Task 2.2**: `validate_and_normalize` に MAX_INPUT_FILES チェック追加
  - 成果物: `src/cli/impact.rs`
  - 依存: なし

### Phase 3: 出力フォーマッター更新

- [ ] **Task 3.1**: `src/output/json.rs` の `format_impact_json` 書き換え
  - `serde_json::to_writer_pretty(writer, &result)` に変更
  - 手動 json!() 構築を排除
  - 成果物: `src/output/json.rs`
  - 依存: Task 1.1

- [ ] **Task 3.2**: `src/output/human.rs` の `format_impact_human` 全面刷新
  - per-file グループ表示
  - Overlap セクション
  - Summary 行
  - strip_control_chars 適用
  - 成果物: `src/output/human.rs`
  - 依存: Task 1.1

- [ ] **Task 3.3**: `src/output/path.rs` の `format_impact_path` 更新
  - 全 impacted path の union（重複除去、max スコア降順）
  - strip_control_chars 適用
  - 成果物: `src/output/path.rs`
  - 依存: Task 1.1

### Phase 4: テスト更新

- [ ] **Task 4.1**: `tests/output_format.rs` の impact 単体テスト全面更新
  - make_impact_result() を新型で再構築
  - JSON/human/path 各フォーマットのアサーションを新フィールド名に
  - overlap / summary の出力検証を追加
  - 成果物: `tests/output_format.rs`
  - 依存: Task 3.1, 3.2, 3.3

- [ ] **Task 4.2**: `tests/e2e_impact.rs` 全面更新
  - 既存テストのフィールド名更新（input_files → changed_files 等）
  - overlap 検出テスト追加（a.md + b.md → c.md が overlap）
  - summary 統計値テスト追加
  - --limit per-file テスト追加
  - 入力ファイル除外テスト修正（正しいフィールド名 path）
  - 成果物: `tests/e2e_impact.rs`
  - 依存: Task 2.1

### Phase 5: 補助修正

- [ ] **Task 5.1**: `src/cli/stdin.rs` UTF-8 truncation バグ修正
  - `&path[..path.len().min(100)]` → char 境界安全な truncation
  - 成果物: `src/cli/stdin.rs`
  - 依存: なし

- [ ] **Task 5.2**: `src/main.rs` CLI help 更新（任意）
  - Impact コマンドの about にパイプ利用例を追加
  - 成果物: `src/main.rs`
  - 依存: なし

## 実行順序

```
Task 1.1 → Task 1.2 → Task 2.1 + Task 2.2（並列）
                     → Task 3.1 + Task 3.2 + Task 3.3（並列）
                     → Task 4.1 + Task 4.2（並列）
Task 5.1, Task 5.2: 任意のタイミングで実行可能
```

## 品質チェック項目

| チェック項目 | コマンド | 基準 |
|-------------|----------|------|
| ビルド | `cargo build` | エラー0件 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 警告0件 |
| テスト | `cargo test --all` | 全テストパス |
| フォーマット | `cargo fmt --all -- --check` | 差分なし |

## Definition of Done

- [ ] すべてのタスクが完了
- [ ] JSON 出力が Issue 仕様（changed_files, impact, overlap, summary）に準拠
- [ ] overlap 検出が正しく動作（E2Eテストで検証）
- [ ] summary 統計値が正確（limit 前基準）
- [ ] human / json / path 全出力形式が新仕様に対応
- [ ] cargo test / clippy / fmt 全パス
