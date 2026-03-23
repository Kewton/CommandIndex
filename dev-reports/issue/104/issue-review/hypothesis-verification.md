# 仮説検証レポート: Issue #104

## 検証対象の仮説

1. `--format json` の出力はLLMのプロンプトとして冗長
2. search結果が82KB（デフォルト20件）に達し、コンテキストウィンドウを圧迫
3. LLMが必要とするのは「ファイルパス + 関連コードスニペット」のみ

## 検証結果

### 仮説1: JSON出力の冗長性 → **Partially Confirmed**

**現在のJSON出力フィールド（search）:**
- `path`, `heading`, `heading_level`, `body`, `tags`, `line_start`, `score`

LLMにとって不要なフィールド:
- `score` (BM25スコア) - LLMには不要
- `heading_level` - LLMには不要
- `line_start` - 必ずしも不要ではない

ただし、「メタデータ、統計情報等が不要」という記述について:
- 現在のJSON出力にはファイル更新日時や検索実行時間などのメタデータは**含まれていない**
- 統計情報も**含まれていない**
- つまりIssueの記述は実際のJSON出力よりやや誇張されている

### 仮説2: 82KBのサイズ → **Unverifiable**

実際のデータサイズはインデックス対象のコンテンツに依存するため、コードベースからは検証不可。
ただし、`body`フィールドにMarkdownセクション全体が含まれるため、大きなドキュメントでは容易に肥大化し得る。

### 仮説3: LLMに必要な情報 → **Confirmed**

既存の `context` コマンドが参考になる:
- `ContextEntry` に `path`, `snippet`, `symbols` が含まれる
- `ContextSummary` に `estimated_tokens` が含まれる
- これはまさにIssueで提案されている「ファイルパス + コードスニペット」の構造

## 拡張ポイント

1. `OutputFormat` enum に `Llm` バリアントを追加
2. `src/output/llm.rs` を新規作成
3. 各CLIコマンド（search, impact, diff等）の出力分岐にllmケースを追加

## 判定: Partially Confirmed

Issueの方向性は正しいが、JSON出力の冗長性に関する記述は一部不正確（メタデータ・統計情報は現時点で含まれていない）。
