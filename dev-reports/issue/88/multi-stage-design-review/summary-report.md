# マルチステージ設計レビュー サマリーレポート

## Issue: #88 [Feature] --index-path オプション（インデックスパス指定）

## レビュー結果サマリー

| Stage | 種別 | エージェント | Must Fix | Should Fix | Nice to Have |
|-------|------|-------------|----------|------------|-------------|
| 1 | 設計原則レビュー | Claude (opus) | 3 | 5 | 4 |
| 2 | 整合性レビュー | Claude (opus) | 6 | 6 | 3 |
| 3 | 影響分析レビュー | Claude (opus) | 8 | 8 | 5 |
| 4 | セキュリティレビュー | Claude (opus) | 3 | 4 | 3 |
| 5 | 設計原則レビュー(2回目) | Codex (gpt-5.4) | 3 | 3 | 1 |
| 7 | 整合性・影響分析(2回目) | Codex (gpt-5.4) | 4 | 4 | 2 |
| **合計** | | | **27** | **30** | **18** |

## 主要な設計改善

### 1回目レビュー（Claude opus × 4段階）
- resolve_index_path を Result 型に変更（unwrap_or_default 排除）
- load_config と resolve_index_path の循環依存を2段階解決で回避
- IgnoreFilter にパターンリストを保持する方式に変更（カスタムパターン喪失防止）
- コードスニペットと現コードベースの不整合を全面修正
- 影響箇所数の過小評価を修正（28箇所以上のヘルパー関数修正）
- canonicalize の実装、symlink チェックの共通化
- clean のホワイトリスト方式 + インデックスマーカー検証
- config.local.toml の機密情報保護方針

### 2回目レビュー（Codex × 2段階）
- config.local.toml の読み込み方針を一本化（常にリポジトリローカルから）
- AppConfig の raw 値 / effective path の責務分離を明確化
- パストラバーサル対策を明示的エラー（PathTraversal）に変更
- IndexConfig.path を Option<String>（raw 値）に統一
- clean を既存挙動ベースに修正（ディレクトリ全削除 + keep_embeddings 時のみ部分削除）
- symlink ポリシーを read-only（許可）vs destructive/write（拒否）に分離

## 設計方針書の最終状態

- 判断 1-10 の設計判断が文書化済み
- 影響範囲表: 15ファイル、影響度「高」8ファイル
- テスト計画: 既存テスト期待値更新 + 新規テスト15項目
- セキュリティ設計: パストラバーサル、symlink、機密情報、同時書き込みの4脅威に対策
- 実装順序: 11ステップに詳細化

## 完了日時: 2026-03-23
