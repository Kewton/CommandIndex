# Issue #168 マルチステージレビュー サマリーレポート

## Issue概要
**issue/before-changeの出力に判断理由のスニペットを付与する**

## レビュー実施状況

| Stage | 種別 | 実行者 | Must Fix | Should Fix | Nice to Have |
|-------|------|--------|----------|------------|--------------|
| 0.5 | 仮説検証 | Claude | - | - | - |
| 1 | 通常レビュー | Claude (opus) | 4 | 4 | 3 |
| 2 | 指摘反映 | Claude (sonnet) | - | - | - |
| 3 | 影響範囲レビュー | Claude (opus) | 5 | 6 | 5 |
| 4 | 指摘反映 | Claude (sonnet) | - | - | - |
| 5 | 通常レビュー(2回目) | Codex (gpt-5.4) | 3 | 4 | 2 |
| 6 | 指摘反映 | Claude (sonnet) | - | - | - |
| 7 | 影響範囲レビュー(2回目) | Codex (gpt-5.4) | 2 | 5 | 2 |
| 8 | 指摘反映 | Claude (sonnet) | - | - | - |

## 主要な改善点（レビューを通じて追加・明確化された事項）

### 1. snippet 未取得時の契約統一
- `Option<String>` に統一: `Some(non-empty)` / `None` のみ
- `Some("")` は禁止
- JSON: `--with-snippet` 指定時のみ snippet フィールド出力

### 2. 後方互換性の確保
- `--with-snippet` フラグをデフォルトオフで追加
- issue JSON: 未指定時は現行 `string[]` 維持、指定時のみオブジェクト配列
- before-change JSON: 既存ポリシーに準拠

### 3. 既存パターンとの一貫性
- `--snippet-lines` / `--snippet-chars` の追加（impact/search と同じパターン）
- `enrich_*_with_snippets()` パターンの踏襲
- tantivy 未存在時の非fatal フォールバック

### 4. テスト方針の明確化
- CLI引数テスト（tests/cli_args.rs）
- フォーマッタ単体テスト（tests/output_format.rs）
- E2E: tantivy有り/無しの2系統

### 5. 影響範囲の明確化
- 変更ファイル15件を網羅的にリスト
- パフォーマンス: before-change 最大20回、issue 最大100回の fetch_snippet()
- 依存関係: tantivy reader 依存の拡大（非fatal フォールバック）

## Issue の最終状態

Issue #168 は全8ステージのレビューを経て、以下が明確に定義された状態:
- 実装方針（Phase 1: 基本スニペット付与）
- snippet 未取得時の契約
- CLIオプション仕様
- JSON スキーマ（条件付き）
- 表示順序仕様
- 影響範囲テーブル
- テスト方針
- 受け入れ基準14項目
