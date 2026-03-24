# 仮説検証レポート - Issue #134

## 検証結果サマリー

| 仮説 | 状態 | 詳細 |
|------|------|------|
| T1: `known_dimension`に1行追加でBGE-M3対応可能 | ✅ Confirmed | `ollama.rs` 行71-78に `"qllama/bge-m3:q8_0" => Some(1024)` を追加するだけ |
| T2: commandindex.tomlでモデル変更可能 | ✅ Confirmed | `RawEmbeddingConfig.model` が `Option<String>` で定義済み |
| T3: 次元数変更時にDB再構築が必要 | ⚠️ Partially Confirmed | スキーマ上共存可能だが、実務的には `clean → index → embed` 推奨 |
| T4: embedding再構築手順が整備済み | ✅ Confirmed | `clean → index → embed` の3ステップで対応可能 |

## 検証詳細

### T1: known_dimension 1行追加

- **ファイル**: `src/embedding/ollama.rs` (行71-78)
- **現在のマッピング**: nomic-embed-text(768), all-minilm(384), mxbai-embed-large(1024)
- **追加行**: `"qllama/bge-m3:q8_0" => Some(1024),`
- **既存設計**: スケーラブルなパターンマッチで拡張容易

### T2: TOML設定でのモデル変更

- **ファイル**: `src/config/mod.rs`
- **設定**: `[embedding] model = "qllama/bge-m3:q8_0"` で指定可能
- **デフォルト**: `"nomic-embed-text"`
- **プロバイダー**: Ollama/OpenAI選択可能

### T3: embeddings.db の互換性

- **スキーマ**: `dimension` と `model` がレコード単位で管理
- **UNIQUEインデックス**: `(section_path, section_heading, model)`
- **検索時フィルタ**: 次元数不一致レコードは自動スキップ (`search_similar()`)
- **結論**: 異なるモデルのembeddingは共存可能だが、統一推奨

### T4: 再構築手順

- `embed.rs` でファイルハッシュベースのキャッシング機構あり
- モデル変更時: `clean → index → embed` で全更新

## 実装影響

- **変更ファイル**: `src/embedding/ollama.rs` のみ（1行追加）
- **テスト追加**: `test_dimension_known_models()` にBGE-M3のアサーション
- **スキーマ変更**: なし
- **検索ロジック変更**: なし
