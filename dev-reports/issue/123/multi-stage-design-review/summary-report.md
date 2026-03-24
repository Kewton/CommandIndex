# マルチステージ設計レビュー サマリーレポート - Issue #123

## 概要
- **Issue**: [BUG] --with-snippet が空文字列を返す（related/impact）
- **レビュー日**: 2026-03-24
- **実施ステージ**: Stage 1-4（1回目の4段階レビュー）
- **スキップ**: Stage 5-8（Must Fix全対応済みのため）

## Stage 1: 設計原則レビュー
### Must Fix対応
- M1(SRP): resolve_import_pathを独立関数として切り出し → **反映済み**
- M2(DRY): add_relation共通ヘルパーでスコア加算パターン統一 → **反映済み**

## Stage 2: 整合性レビュー
### Must Fix対応
- M1: 外部パッケージ解決失敗時の挙動明確化（スキップ） → **反映済み**
- M2: all_indexed_pathsの戻り値をHashSetに → **反映済み**
- M3: 逆方向ルックアップにfind_all_imports追加 → **反映済み**（symbol_store.rs変更対象に追加）

## Stage 3: 影響分析レビュー
### Must Fix対応
- M1: OnceCell<HashSet>によるキャッシュ設計 → **反映済み**
- M2: マッチング優先順位の明確化 → **反映済み**
- M3: 逆方向ルックアップの設計明記 → **反映済み**

## Stage 4: セキュリティレビュー
### Must Fix対応
- M1: パスコンポーネント境界チェック追加（auth vs oauth問題解消） → **反映済み**
- M2: 入力バリデーション（長さ・空文字チェック） → **反映済み**

## 設計方針書の主な改善点
1. resolve_import_path を独立関数化（SRP）
2. path_component_suffix_matches でコンポーネント境界チェック（セキュリティ）
3. OnceCell<HashSet<String>> でキャッシュ（パフォーマンス）
4. add_relation 共通ヘルパー（DRY）
5. find_all_imports() 追加で逆方向ルックアップ対応
6. 入力バリデーション追加
7. テスト方針（ユニット+E2E）明記
