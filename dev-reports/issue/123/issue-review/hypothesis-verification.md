# 仮説検証レポート - Issue #123

## 検証対象
`--with-snippet` が空文字列を返すバグ

## 仮説
snippet取得時のパス不一致により、tantivyインデックスから該当ドキュメントが見つからない。

## 検証結果: **Confirmed**

### 根本原因

`score_import_deps()` (related.rs:181) が `imp.target_module.clone()` をscores HashMapのキーとして使用する。
`target_module` はTypeScriptパーサから直接取得したインポートパス（例: `@/components/worktree/TerminalSearchBar`, `react`）であり、
tantivyインデックスに保存された実ファイルパス（例: `src/components/worktree/TerminalSearchBar.tsx`）と一致しない。

### 影響フロー

1. **インデックス作成時**: TypeScriptパーサが `import { X } from "@/components/Foo"` を抽出
2. **SQLite保存**: `dependencies.target_module = "@/components/Foo"` (パスエイリアス未解決)
3. **related検索**: `score_import_deps()` が `@/components/Foo` をscoresに追加
4. **snippet取得**: `fetch_snippet(reader, "@/components/Foo")` → tantivyで完全一致検索 → 不一致 → 空文字列

### 問題箇所

| ファイル | 行 | 問題 |
|---|---|---|
| `src/search/related.rs` | 181 | `imp.target_module` がそのままscoresのキーに使われる |
| `src/cli/snippet_helper.rs` | 18 | パス正規化なしで `search_by_exact_path` を呼ぶ |
| `src/parser/typescript.rs` | ~191 | インポートパスのエイリアス解決がない |

### 補足

- macOS/Linux環境でも再現する（Windows固有ではない）
- `react` のような外部モジュールも同様にスニペットが空になる（これは期待動作に近い）
- `source_file` 側は `to_relative_path_string()` で正規化されているため、逆方向（誰がこのファイルをimportしているか）は正常
