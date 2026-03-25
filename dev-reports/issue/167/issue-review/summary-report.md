# マルチステージIssueレビュー サマリーレポート

## Issue #167: suggestコマンドのナレッジグラフ展開が過剰（80件提案）

### レビュー概要

| ステージ | 種別 | 実行エージェント | Must Fix | Should Fix | Nice to Have |
|---|---|---|---|---|---|
| 0.5 | 仮説検証 | Claude | - | - | - |
| 1 | 通常レビュー（1回目） | Claude Opus | 2 | 4 | 2 |
| 2 | 指摘反映（1回目） | Claude Sonnet | - | - | - |
| 3 | 影響範囲レビュー（1回目） | Claude Opus | 2 | 3 | 4 |
| 4 | 指摘反映（1回目） | Claude Sonnet | - | - | - |
| 5 | 通常レビュー（2回目） | Codex (gpt-5.4) | 2 | 3 | 1 |
| 6 | 指摘反映（2回目） | Claude Sonnet | - | - | - |
| 7 | 影響範囲レビュー（2回目） | Codex (gpt-5.4) | 2 | 3 | 2 |
| 8 | 指摘反映（2回目） | Claude Sonnet | - | - | - |

### 仮説検証結果

**Confirmed**: suggestコマンドの`prepend_knowledge_steps()`がフィルタリングなしで全ドキュメントを展開している。

### 主要な指摘と反映内容

#### 1回目レビュー（Claude Opus）
- **受け入れ基準の追加**: Issue本文に具体的な受け入れ基準セクションを新設
- **modifies除外の明示**: retain()によるフィルタリング除外を明記
- **フィルタリングレイヤー設計**: prepend_knowledge_steps()の前段で実施する方針を確定
- **MAX_KG_DOCS_PER_ISSUE定数の導入**: Issue単位の上限制御を明確化
- **テスト要件の追加**: filter_and_limit_kg_docs()のユニットテスト項目を追加

#### 2回目レビュー（Codex）
- **MAX_KG_DOCS_PER_ISSUE矛盾の解消**: 2→4に変更し、期待結果の4件と整合
- **doc_subtype判定の修正**: "summary"ではなくIssueReview/DesignReview(保持) vs StageReview(除外)に修正
- **KG文書取得APIの変更**: find_knowledge_by_issue()→find_documents_by_issue()に変更（doc_subtype取得可能）
- **複数Issue集約フローの具体化**: 各Issue個別呼び出し→DTO変換→結合→フィルタ
- **E2Eテスト要件の追加**: 提案数制御、modifies除外、Issue単位上限の3項目

### 最終Issue状態

Issue #167 は全8ステージのレビューを経て更新済み。以下が確定:
- フィルタリング戦略: modifies/has_progress/StageReview除外、IssueReview/DesignReview保持
- 上限制御: MAX_KG_DOCS_PER_ISSUE = 4、relation_priorityによるソート
- KG文書取得: find_documents_by_issue() API使用（doc_subtype対応）
- テスト: ユニットテスト5項目 + E2Eテスト3項目 + DocSubtype整合確認
