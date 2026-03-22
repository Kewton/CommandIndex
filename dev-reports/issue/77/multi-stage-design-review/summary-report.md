# Issue #77 マルチステージ設計レビュー サマリーレポート

## 実施ステージ

| Stage | 種別 | エージェント | Must Fix | Should Fix | Nice to Have |
|-------|------|-------------|----------|-----------|-------------|
| 1 | 設計原則 (SOLID/KISS/YAGNI/DRY) | Claude opus | 2 | 3 | 5 |
| 2 | 整合性 | Claude opus | 2 | 4 | 3 |
| 3 | 影響分析 | Claude opus | 4 | 5 | 4 |
| 4 | セキュリティ | Claude opus | 3 | 5 | 4 |
| 5-8 | 2回目 | Codex | スキップ（サーバーエラー） | - | - |

## Must Fix 対応サマリー (11件)

### 設計原則 (Stage 1)
1. **YAGNI**: StatusOptions 構造体 → verify: bool 引数に簡素化
2. **SRP**: snapshot.rs の責務を ExportMeta 定義+読み書きに限定

### 整合性 (Stage 2)
3. **定数統一**: CLI層は commandindex_dir() ヘルパー、Indexer層は既存定数
4. **パターン準拠**: status::run() は format 別引数のまま verify: bool 追加（CleanOptions パターン）

### 影響分析 (Stage 3)
5. **破壊的変更最小化**: テスト修正は verify: false 追加の4箇所のみ
6. **import後整合性**: update コマンドとの整合性を統合テストで検証
7. **相対パス検証**: export 前に tantivy ドキュメント内パスが相対パスであることを確認
8. **相対パス検証**: export 前に symbols.db 内パスが相対パスであることを確認

### セキュリティ (Stage 4)
9. **シンボリックリンク**: validate_entry_type() でSymlink/Link エントリを即座に拒否
10. **パス検証統一**: canonicalize() は使わず、文字列レベル+components()検証に一本化
11. **圧縮爆弾対策**: 展開サイズ上限(1GB), エントリ数上限(10000) を追加

## 設計方針書の主要改善点

- ExportMeta から index_root を削除（情報漏洩防止）
- state.json のパック時サニタイズ
- ExportResult/ImportResult 構造体導入（既存パターン準拠）
- エラー型に std::error::Error + From 実装を明記
- doc comment を英語に統一
- テストファイル命名を既存パターンに統一
- git hash 取得関数の分離（DIP, テスタビリティ）
- #[serde(deny_unknown_fields)] 追加
