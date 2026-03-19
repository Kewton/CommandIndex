# リポジトリレイアウト

## ディレクトリ構成（v0.0.0 時点）

```
CommandIndex/
├── Cargo.toml                # パッケージ定義
├── Cargo.lock                # 依存ロック（Git 管理対象）
├── README.md                 # プロジェクト概要
├── CLAUDE.md                 # AI アシスタント向けガイドライン
├── COMMANDINDEX.md           # プロジェクトガードレール
├── CHANGELOG.md              # 変更履歴
├── LICENSE                   # MIT ライセンス
├── .gitignore                # Git 除外設定
│
├── src/
│   ├── main.rs               # CLI エントリポイント（clap）
│   └── lib.rs                # ライブラリルート
│
├── tests/
│   ├── common/
│   │   └── mod.rs            # テスト共有ユーティリティ
│   └── cli_args.rs           # CLI パーステスト
│
├── .github/
│   ├── workflows/
│   │   ├── ci.yml            # CI パイプライン
│   │   └── release.yml       # リリースパイプライン
│   ├── PULL_REQUEST_TEMPLATE.md
│   └── ISSUE_TEMPLATE/
│       ├── bug_report.md
│       └── feature_request.md
│
├── .claude/
│   ├── commands/             # カスタムコマンド
│   ├── agents/               # サブエージェント定義
│   ├── prompts/              # 共有プロンプト
│   └── lib/                  # 共有ユーティリティ
│
└── workspace/
    ├── plan/
    │   └── plan_v0.0.1.md    # 企画書
    └── v0.0.0/
        ├── README.md
        ├── dev-environment-plan.md
        ├── repository-layout.md
        ├── ci-cd-plan.md
        ├── claude-code-setup.md
        └── initial-implementation-slice.md
```

## ディレクトリ構成（Phase 1 実装後の想定）

Phase 1 以降でモジュールが増えていく。初期構成では以下を想定する。

```
src/
├── main.rs                   # CLI エントリポイント
├── lib.rs                    # ライブラリルート
├── cli/
│   ├── mod.rs                # CLI モジュール
│   ├── index.rs              # index サブコマンド
│   ├── search.rs             # search サブコマンド
│   ├── update.rs             # update サブコマンド
│   ├── status.rs             # status サブコマンド
│   └── clean.rs              # clean サブコマンド
├── parser/
│   ├── mod.rs                # パーサーモジュール
│   ├── markdown.rs           # Markdown 解析
│   ├── frontmatter.rs        # frontmatter 抽出
│   └── link.rs               # リンク解析
├── indexer/
│   ├── mod.rs                # インデクサーモジュール
│   ├── tantivy.rs            # tantivy インデックス操作
│   ├── manifest.rs           # manifest.json 管理
│   └── state.rs              # state.json 管理
├── search/
│   ├── mod.rs                # 検索モジュール
│   ├── fulltext.rs           # 全文検索
│   ├── path.rs               # パス検索
│   └── tag.rs                # タグ検索
└── output/
    ├── mod.rs                # 出力モジュール
    ├── human.rs              # human 形式出力
    ├── json.rs               # JSON 形式出力
    └── path.rs               # path 形式出力
```

## モジュール責務

| モジュール | 責務 | Phase |
|---|---|---|
| `cli` | コマンドライン引数のパース、サブコマンドのディスパッチ | 0 |
| `parser` | Markdown / ソースコードの解析、構造化データの生成 | 1 |
| `indexer` | tantivy / SQLite へのインデックス書き込み、manifest / state 管理 | 1 |
| `search` | インデックスからの検索、結果のランキング | 1 |
| `output` | 検索結果のフォーマット（human / json / path） | 1 |

## モジュール間ルール

- 各モジュールは他モジュールの内部構造に直接アクセスしない
- モジュール間の依存は `pub` インターフェースを通じて行う
- `parser` → `indexer` → `search` → `output` の一方向の依存を基本とする
- 循環依存を許容しない

## テスト配置方針

| テスト種別 | 配置場所 | 対象 |
|---|---|---|
| ユニットテスト | 各 `*.rs` ファイル内の `#[cfg(test)] mod tests` | パース処理、変換ロジック等の局所的な動作 |
| 統合テスト | `tests/` ディレクトリ | モジュール横断の動作（CLI → index → search 等） |

## .gitignore

```
# Build artifacts
/target/

# CommandIndex index (generated, not tracked)
.commandindex/

# Editor
*.swp
*.swo
*~
.idea/
.vscode/

# OS
.DS_Store
Thumbs.db
```
