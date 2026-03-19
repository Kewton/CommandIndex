# 初期実装スライス — CLI スケルトン

## スライス名

**CLI Skeleton & Dev Pipeline E2E**

## このスライスが証明すること

1. Rust プロジェクトが正しくビルド・テストできること
2. CLIのサブコマンド構造が動作すること
3. CI/CD パイプラインが正常に機能すること
4. feature → PR → CI → マージの開発フロー全体が動作すること

## 含まれるもの

- `commandindex --help` でサブコマンド一覧が表示される
- `commandindex index` で「未実装」メッセージが表示される（`todo!()` ではなくユーザーフレンドリーなメッセージ）
- `commandindex search "test"` で「未実装」メッセージが表示される
- `commandindex update` で「未実装」メッセージが表示される
- `commandindex status` で「未実装」メッセージが表示される
- `commandindex clean` で「未実装」メッセージが表示される
- `commandindex --version` でバージョンが表示される
- CLI 引数パースの統合テスト

## 含まれないもの

- Markdown 解析
- tantivy インデックス操作
- ファイル走査
- 検索ロジック
- 出力フォーマット（human / json / path）の実装

## デモシナリオ

### フロー 1: CLI ヘルプ表示

```bash
$ commandindex --help
CommandIndex — Git-native knowledge CLI

Usage: commandindex <COMMAND>

Commands:
  index   Build search index from repository
  search  Search the index
  update  Incrementally update the index
  status  Show index status
  clean   Remove index and prepare for rebuild
  help    Print this message or the help of the given subcommand(s)

Options:
  -V, --version  Print version
  -h, --help     Print help
```

### フロー 2: 未実装コマンドの実行

```bash
$ commandindex index
Error: `index` command is not yet implemented. Coming in Phase 1.

$ commandindex search "認証"
Error: `search` command is not yet implemented. Coming in Phase 1.
```

### フロー 3: CI パイプライン確認

```
feature/setup-skeleton ブランチを作成
  → PR を作成
  → CI が自動実行（fmt ✓, clippy ✓, test ✓, build ✓）
  → develop にマージ
  → develop → main の PR を作成・マージ
```

## このスライスが重要な理由

- 開発基盤が正しく機能することを、機能実装の前に確認できる
- CI/CD の問題を早期に発見・修正できる
- Phase 1 の実装開始時に、インフラ起因のブロッカーがない状態を保証する

## 次のスライス

Phase 1 の最初のスライス: **Markdown 走査 & heading 単位分割**
- 指定ディレクトリの Markdown ファイルを走査する
- heading 単位でチャンクに分割する
- 分割結果を stdout に出力する（インデックス格納はその次のスライス）
