# 仮説検証レポート: Issue #127

## 検証対象Issue
[BUG] suggest の英語入力でキーワード部分一致により無関係ファイルを推薦

## 仮説一覧と検証結果

| # | 仮説 | 結果 |
|---|------|------|
| 1 | BM25のキーワードマッチングが「display」等の汎用語にマッチし無関係ファイルが上位に来る | **Confirmed** |
| 2 | テストファイルのスコアを下げる仕組みがない | **Confirmed** |
| 3 | コードファイルをドキュメント・テストより優先するスコアリングがない | **Confirmed** |
| 4 | suggestコマンドがBM25のみに依存している | **Confirmed** |

## 詳細検証

### 仮説1: BM25部分一致による汎用語マッチ

**Confirmed**

- `src/indexer/reader.rs:125-129`: `QueryParser::for_index()` で heading, body, tags の3フィールドを等価に検索
- Tantivy の標準 BM25 スコアリングが使用されており、"display" を含む全ドキュメントがマッチ
- `src/cli/suggest.rs:110-117`: `search_entry_files()` が `reader.search(query, BM25_SEARCH_LIMIT=20)` を呼び出し

### 仮説2: テストファイルのスコアペナルティなし

**Confirmed**

- `src/parser/ignore.rs:5-13`: デフォルトの ignore パターンにテストファイル除外なし
- `src/indexer/reader.rs:162-174`: ポストフィルターは `path_prefix` と `file_type` のみ対応
- テストファイルとソースファイルは完全に同等にスコアリングされる

### 仮説3: ファイル種別によるブースト機構なし

**Confirmed**

- `src/indexer/schema.rs:34-37`: heading, body, tags すべて同一のインデキシング設定（boost なし）
- `src/indexer/reader.rs:125-128`: QueryParser で全フィールド等価重み
- `.ts`, `.tsx`, `.md` による区別なし

### 仮説4: suggestコマンドの実装

**Confirmed** - BM25のみに依存

**処理フロー**:
1. 入力バリデーション（500文字以下、制御文字なし）
2. BM25検索（`reader.search(query, 20)`） - heading+body+tags を等価検索
3. ファイル単位の重複排除（最大スコアを採用、上位5件）
4. 各ファイルに対して context/related/impact コマンドの戦略を構築

**根本原因**:
- "add fullscreen feature to terminal display" が "display", "fullscreen", "terminal" 等にトークナイズされ、"display" を含む全文書が BM25 でスコアリングされる
- テストファイル・ドキュメントへのペナルティなし
- path フィールドは STRING（完全一致）でフルテキスト検索対象外
