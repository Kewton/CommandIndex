# Issue #135 マルチステージ設計レビュー サマリーレポート

## 実施日: 2026-03-24

## レビューステージ完了状況

| Stage | 種別 | エージェント | Must Fix | Should Fix | Nice to Have |
|-------|------|-------------|----------|------------|-------------|
| 1 | 設計原則 | Claude opus | 0 | 3 | 3 |
| 2 | 整合性 | Claude opus | 3 | 4 | 2 |
| 3 | 影響分析 | Claude opus | 2 | 4 | 3 |
| 4 | セキュリティ | Claude opus | 0 | 3 | 4 |
| 5 | 設計原則2回目 | Codex (gpt-5.4) | 2 | 2 | 1 |
| 6 | 指摘反映 | Claude sonnet | - | - | - |
| 7 | 整合性・影響2回目 | Codex (gpt-5.4) | 0 | 4 | 1 |
| 8 | 指摘反映 | Claude sonnet | - | - | - |

## 主な改善事項

### Must Fix（全7件、全て反映済み）
1. **beforeコードの正確性** — embed.rsのgenerated += 1を追加
2. **failedカウンタ粒度変更の明文化** — per-section → per-file
3. **rusqlite制約説明の正確化** — Connection::transaction(&mut self) vs execute_batch(&self)
4. **orphan sections対策** — DELETE+INSERTのアトミック置換方式を追加
5. **zip件数不一致の検出** — sections.len() != embeddings.len() の事前検証を追加
6. **ROLLBACK失敗のストア層ログ除去** — SRP遵守、エラーはCLI層に委譲
7. **failedカウンタの振る舞い変更文書化** — Stage 2, 3で重複指摘

### 重要なShould Fix（採用分）
- index.rs固有のbefore/afterコード例追加
- tests/e2e_semantic_hybrid.rsを影響範囲に追加
- 影響範囲の記述精度向上（回帰確認対象として明記）
- テスト計画にorphan/件数不一致テストを追加
- セキュリティ設計に可用性リスクを追加

### スコープ外とした指摘
- DRY共通化（embed.rs/index.rsのループ統合）→ 別Issue
- BATCH_SIZEのconfig化 → 別Issue
- WAL mode最適化 → 別Issue
- replace_file_embeddings()への用途特化API → KISS優先で現状維持

## 結論
設計方針書は8段階のレビューを経て、コード整合性・エラーハンドリング・影響範囲の精度が大幅に向上。特にzipの件数不一致問題とorphan sections対策は実装時のバグを事前に防ぐ重要な改善。
