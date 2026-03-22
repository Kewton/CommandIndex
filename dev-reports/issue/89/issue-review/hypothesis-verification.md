# 仮説検証レポート - Issue #89

## 検証対象

Issue #89「[Feature] stdin パイプ入力対応」に記載された実装方針・仮説を検証。

## 検証結果

| 仮説 | 判定 | 詳細 |
|------|------|------|
| `impact` サブコマンドで stdin からファイルリスト読み取り | **Confirmed (実装必要)** | サブコマンド自体が存在しない。新規追加が必要 |
| `search --related-stdin` で stdin からファイルリスト読み取り | **Confirmed (実装必要)** | オプション自体が存在しない。新規追加が必要 |
| 既存の related search エンジンを活用可能 | **Confirmed** | `src/search/related.rs` に RelatedSearchEngine が完全実装済。`find_related()` を複数ファイルに対してループ呼び出し可能 |
| CLI フレームワーク（clap）で拡張可能 | **Confirmed** | `conflicts_with_all` による排他制御が既に設計されており、拡張容易 |
| stdin 読み取り既存実装なし | **Confirmed** | コードベース全体に stdin/BufRead 関連の実装なし |

## 主要な発見

1. **`impact` サブコマンドは完全新規**: Commands enum に定義なし。impact の具体的なロジック（影響分析）をどう実装するかが Issue に明記されていない
2. **`--related-stdin` は既存 `--related` の拡張**: 単一ファイル → 複数ファイル（stdin経由）への拡張が自然
3. **impact サブコマンドの機能定義が曖昧**: 「impact」が具体的に何を返すのか（関連ファイル？依存グラフ？変更影響スコア？）の定義が不足

## リスク

- `impact` サブコマンドの機能仕様が曖昧（関連検索の再利用？独自ロジック？）
- stdin が空の場合のエラーハンドリング未定義
