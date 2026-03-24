# マルチステージIssueレビュー サマリーレポート - Issue #134

## Issue概要
**タイトル:** 多言語embeddingモデル対応 (BGE-M3)

## レビュー実施結果

| Stage | 種別 | 実行者 | Must Fix | Should Fix | Nice to Have |
|-------|------|--------|----------|------------|--------------|
| 0.5 | 仮説検証 | Claude | - | - | - |
| 1 | 通常レビュー | Claude (opus) | 1 | 3 | 3 |
| 2 | 指摘反映 | Claude (sonnet) | - | - | - |
| 3 | 影響範囲レビュー | Claude (opus) | 2 | 3 | 3 |
| 4 | 指摘反映 | Claude (sonnet) | - | - | - |
| 5 | 通常レビュー（2回目） | Codex (gpt-5.4) | 1 | 2 | 1 |
| 6 | 指摘反映 | Claude (sonnet) | - | - | - |
| 7 | 影響範囲レビュー（2回目） | Codex (gpt-5.4) | 1 | 3 | 1 |
| 8 | 指摘反映 | Claude (sonnet) | - | - | - |

## 主要な改善点

### 新規タスク追加
1. **T1.5: モデル変更時のキャッシュ無効化対応** - has_current_embedding()にmodel条件追加、旧モデルレコード自動削除
2. **T2.5: 次元不一致時の警告メッセージ** - search_similar()にメタ情報追加、全CLI経路で一貫した警告

### Issue内容の充実化
- T1.5を必須要件として明確化（3つの必須要件 + 受け入れ基準）
- T1.5の影響範囲をembed以外（index --with-embedding, update --with-embedding）にも拡大
- T2.5の既存テスト影響を具体化
- T3を手動評価と明記（CI再現性の問題を明示）
- T4にREADME.md Embeddingセクション新設を明記
- 「依存関係」セクション新設
- 「今後の検討事項」セクション新設

## 仮説検証結果

| 仮説 | 状態 |
|------|------|
| T1: known_dimensionに1行追加で対応可能 | ✅ Confirmed |
| T2: commandindex.tomlでモデル変更可能 | ✅ Confirmed |
| T3: 次元数変更時にDB再構築が必要 | ⚠️ Partially Confirmed |
| T4: 再構築手順は clean → index → embed | ✅ Confirmed |

## 最終Issue構成

- T1: known_dimension追加（1行）
- T1.5: キャッシュ無効化対応（必須・全経路）
- T2: 動作確認
- T2.5: 次元不一致警告（全CLI経路）
- T3: 精度比較テスト（手動評価）
- T4: ドキュメント整備（README Embeddingセクション新設）
