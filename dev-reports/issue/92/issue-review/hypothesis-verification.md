# 仮説検証レポート - Issue #92

## 検証日: 2026-03-23

## 仮説一覧

### 仮説1: 「内部的には各ファイルに対して `--related` を実行し、結果の集合演算を返す」

**判定: Confirmed**

- `--related` オプションは `search` サブコマンドに実装済み（`src/cli/search.rs` L269-310）
- `run_related_search(file_path, limit, format)` → `Vec<RelatedSearchResult>` を返す
- 内部では `RelatedSearchEngine::find_related()` が6種の関係タイプでスコアリング
- diff サブコマンドは2つのファイルに対してそれぞれ `find_related()` を呼び、結果のファイルパス集合で only_a / only_b / overlap を算出可能

### 仮説2: 「`impact` サブコマンド（#90）の `overlap` フィールドと機能的に近い」

**判定: Partially Confirmed**

- `impact` サブコマンド（Issue #90）は **未実装**（Commands enum, src/cli/ に該当なし）
- 機能的な重複の懸念はあるが、実装順序として diff が先行しても問題ない
- ただし impact 実装時に diff のロジックを共有できる設計が望ましい

### 仮説3: 「human / json 出力形式に対応」

**判定: Confirmed**

- 出力フォーマットは3種類（human, json, path）が既存パターンとして確立
- `OutputFormat` enum（`src/output/mod.rs` L30-36）に Human, Json, Path が定義済み
- diff 出力は既存パターンに新しい出力型を追加する形で対応可能

## コードベース状態サマリー

| 項目 | 状態 |
|------|------|
| `--related` オプション | 実装済み（search サブコマンド） |
| `RelatedSearchEngine` | 実装済み（search/related.rs） |
| `impact` サブコマンド | 未実装 |
| 出力フォーマット基盤 | 3種類対応済み |
| CLIサブコマンド数 | 10個（Index, Search, Update, Status, Clean, Context, Embed, Config, Export, Import） |
