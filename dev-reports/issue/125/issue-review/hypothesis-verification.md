# Issue #125 仮説検証レポート

## 検証対象
`--rerank` がモデル未検出時にサイレントフォールバックし結果が変わらない

## 仮説検証結果

### 仮説1: モデル未検出時にstderrにエラー出力されるがexitコードは0
**状態: Confirmed ✓**

- `src/rerank/ollama.rs:103-104` で `RerankError::ModelNotFound` を返す
- `src/cli/search.rs:924-929` で `eprintln!` でエラー出力し、元の結果を返す
- `try_rerank()` は `Vec<SearchResult>` を直接返すため、`main.rs` のエラーハンドリングに到達しない → exitコード0

### 仮説2: rerankが失敗した場合、結果は--rerankなしと完全に同一
**状態: Confirmed ✓**

- `try_rerank()` のエラーハンドリング（L904-909, L924-929）は全て `return results;` で元の結果をそのまま返す
- デフォルトモデル `llama3`（`src/rerank/mod.rs:12`）が未インストールの場合に発生

### 仮説3: ユーザーからはrerankが成功したように見える
**状態: Confirmed ✓**

- stdoutには通常の検索結果のみ出力
- stderr の `[rerank] Reranking failed:` メッセージは出力リダイレクト時に隠れる
- JSON出力等にrerankスキップ情報は含まれない

## 根本原因
- `try_rerank()` は `Result` 型を返さず `Vec<SearchResult>` を直接返す設計
- エラー情報は `eprintln!()` でのみ出力され、呼び出し側で捕捉不可能
- 出力フォーマット（human/json/llm/path）のいずれにもrerankステータス情報がない

## 関連コード
| ファイル | 行 | 内容 |
|---|---|---|
| `src/cli/search.rs` | L893-964 | `try_rerank()` フォールバック処理 |
| `src/cli/search.rs` | L260-268 | rerank呼び出し箇所 |
| `src/rerank/mod.rs` | L11-13 | デフォルトモデル定義 |
| `src/rerank/ollama.rs` | L103-104 | ModelNotFoundエラー生成 |
| `src/main.rs` | L541-546 | exitコード制御 |
