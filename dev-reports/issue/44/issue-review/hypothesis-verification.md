# 仮説検証レポート: Issue #44

## 仮説
「commandindex searchのhuman形式出力において、スニペット表示の行数・文字数がハードコード（2行・120文字）されている」

## 判定: Confirmed

## 検証結果

### ハードコード箇所
- **ファイル**: `src/output/human.rs:28`
- **コード**: `let snippet = truncate_body(&strip_control_chars(&result.body), 2, 120);`
- **行数**: 2行固定
- **文字数**: 120文字固定

### truncate_body 関数
- **ファイル**: `src/output/mod.rs:136-153`
- 複数行: 最初のN行を取得、超過時に"..."追加
- 単一行: 最初のM文字を取得、超過時に"..."追加
- マルチバイト文字安全（`chars()`使用）

### 現在のデータフロー
```
main.rs (CLIパーサ) → cli/search.rs (run) → output/mod.rs (format_results) → output/human.rs (format_human) → truncate_body(body, 2, 120)
```

### CLIオプション
- `--snippet-lines` / `--snippet-chars` は未定義
- SearchOptions にスニペット関連フィールドなし

## 結論
Issue記載の仮説は完全に正確。変更対象ファイルの特定も正しい。
