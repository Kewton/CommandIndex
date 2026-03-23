# CommandMate × CommandIndex 連携シナリオ

> 作成日: 2026-03-23
> CommandMate: 複数エージェントの並列実行・監視・制御（control plane）
> CommandIndex: ローカルナレッジの検索・文脈取得（knowledge base）

---

## シナリオ1: Issue 補完の精度向上

**現状の課題:**
`/issue-enhance` でAIにIssueを補完させるとき、AIはコードベースを「その場で読む」。ファイルが多いとコンテキストに入りきらず、関連ファイルの見落としが起きる。

**連携後:**
```bash
# Issue に書かれたキーワードで CommandIndex を検索
commandindex search "認証 JWT ログイン" --format json

# → 関連ファイル、関数、Markdown ドキュメントが構造化された形で返る
# → この結果をAIのコンテキストに注入してから /issue-enhance を実行
```

→ AIが「コードベースを読む」のではなく、**CommandIndex が「関連する文脈だけを渡す」**。コンテキストウィンドウを無駄遣いせず、影響ファイルの特定精度が上がる。

**具体的な実装イメージ:**
```bash
# /issue-enhance の内部で CommandIndex を呼ぶ
CONTEXT=$(commandindex search "$ISSUE_KEYWORDS" --format json --limit 10)
commandmate send "$WT" "以下の文脈を踏まえて Issue #101 を補完して: $CONTEXT" --auto-yes
```

---

## シナリオ2: レビュー観点の自動生成

**現状の課題:**
マルチステージレビューの4観点（設計原則・整合性・影響分析・セキュリティ）は汎用的。Issue ごとに「この Issue 特有のリスク」を検出できていない。

**連携後:**
```bash
# Issue の影響ファイルに関連するコードのシンボル・依存関係を取得
commandindex search --symbol "getDbInstance" --format json
commandindex search --related "src/lib/db-instance.ts" --format json

# → 「この関数を呼んでいるファイルが30個ある」
# → 「この型を変更すると56箇所に影響する」
# → これをレビュー観点に自動追加
```

→ 汎用4観点 + **Issue 固有の影響グラフに基づくカスタム観点**。レビューの網羅性が上がる。

---

## シナリオ3: 並列エージェントの「共有知識」

**現状の課題:**
並列で動く複数のエージェントは、それぞれ独立したworktreeで動いている。エージェントAが発見した設計判断を、エージェントBは知らない。

**連携後:**
```bash
# エージェントAが設計判断をMarkdownに書き出す
# → CommandIndex が自動でインデックス

# エージェントBが関連する作業を始めるとき
commandindex search "認証ミドルウェア 設計判断" --format json
# → エージェントAの設計判断が検索結果に出る
```

→ 並列エージェント間の**非同期的な知識共有**。直接通信しなくても、CommandIndex を介して文脈を共有できる。

---

## シナリオ4: マルチリポジトリ横断の文脈取得

**現状の課題:**
3リポジトリ（CommandMate / Anvil / CommandIndex）を並行運営しているが、リポジトリをまたいだ知識検索ができない。例えば「CommandMate のAPI設計」を Anvil の開発時に参照したいとき、手動でファイルを探す必要がある。

**連携後:**
```bash
# workspace 設定で複数リポジトリを横断検索
commandindex search "REST API 設計" --workspace multi-repo.toml --format json

# multi-repo.toml:
# [[sources]]
# path = "../CommandMate"
# [[sources]]
# path = "../Anvil"
# [[sources]]
# path = "../CommandIndex"
```

→ CommandMate のオーケストレーションで並列開発しつつ、CommandIndex で**リポジトリ横断の知識検索**。ミッション「複数リポジトリの同時運営」の基盤になる。

---

## シナリオ5: 「朝のブリーフィング」自動生成

**現状の課題:**
朝PCを開いたとき、昨夜のcron実行結果を1つずつ確認する必要がある。3リポジトリ × 複数worktree の状態を把握するのに時間がかかる。

**連携後:**
```bash
# CommandIndex で昨夜の変更をまとめて検索
commandindex search --related "$(git log --since='12 hours ago' --name-only --format='')" --format json

# CommandMate で各エージェントの最終状態を取得
commandmate ls --json
commandmate capture "$WT1" --json
commandmate capture "$WT2" --json

# 両方の結果をAIに渡してブリーフィング生成
# → 「昨夜の進捗: Issue #101 完了、#102 テスト失敗（auth_test.rs:45）、#103 は未着手」
# → 「#102 の失敗は、#101 で変更した getDbInstance の影響。関連ファイル: ...」
```

→ CommandMate の「何が起きたか」+ CommandIndex の「なぜ起きたか」を組み合わせた**自動ブリーフィング**。スマホで朝5分見るだけで全体像がわかる。

---

## シナリオ6: セマンティック検索によるIssue重複検出

**現状の課題:**
Issue を大量に登録していると、似たような Issue を重複して作ってしまうことがある。

**連携後:**
```bash
# 新しいIssueを作る前に、類似Issueを検索
commandindex search --semantic "JWT認証のリフレッシュトークン実装" --format json

# → 既存Issue（Markdownファイル化されたもの）やdev-reportsの設計書から
#   類似のものがヒットする
# → 「Issue #135 で似た設計判断をしている」と気づける
```

→ Issue の品質向上 + 作業の重複排除。

---

## シナリオ7: エージェントの迷走を文脈で早期検出する

**現状の課題:**
並列エージェントが迷走しているかどうかは、`commandmate capture` でターミナル出力を人間が読んで判断するしかない。出力が長いと見落とす。スマホからだと特に厳しい。

**連携後:**
```bash
# エージェントの最新出力を取得
OUTPUT=$(commandmate capture "$WT" --json)

# 出力中に登場するファイルパス・シンボルを抽出し、
# Issue の影響ファイルと突合
commandindex search --related "$CHANGED_FILE" --format json

# → エージェントが触っているファイルが Issue の影響範囲外なら「迷走」と判定
# → 自動で「Issue #101 の範囲外のファイルを変更しています。戻ってください」と送信
commandmate send "$WT" "Issue #101 の影響ファイル外を変更しています。影響ファイルは: $EXPECTED_FILES" --auto-yes
```

**ポイント:**
- 人間が出力を読まなくても、CommandIndex の「影響範囲」と照合すれば迷走を機械的に検出できる
- スマホでの監視負荷が大幅に下がる
- `/orchestrate` コマンドの Phase 3（進捗監視）に組み込める

---

## シナリオ8: 設計突合の自動化（並列Issue間のコンフリクト予測）

**現状の課題:**
`/orchestrate` の Phase 4（設計突合）では、並列 Issue の設計書を人間がクロスチェックしている。影響ファイルの重複確認は目視。

**連携後:**
```bash
# Issue #101 の影響範囲を取得
FILES_101=$(commandindex search --related "src/auth/jwt.rs" --format path)

# Issue #102 の影響範囲を取得
FILES_102=$(commandindex search --related "src/auth/middleware.rs" --format path)

# 共通ファイルを検出
CONFLICT=$(comm -12 <(echo "$FILES_101" | sort) <(echo "$FILES_102" | sort))

if [ -n "$CONFLICT" ]; then
  echo "コンフリクトリスク: $CONFLICT"
  # 該当エージェントに警告を送信
  commandmate send "$WT1" "Issue #102 と共通ファイル ($CONFLICT) があります。変更箇所を確認してください" --auto-yes
fi
```

**ポイント:**
- CommandIndex の `--related` 検索は直接の依存だけでなく、間接的に影響を受けるファイルも返す
- `git diff` ベースの重複チェックより「まだ変更していないが影響を受けるファイル」を先に検出できる
- Composio の agent-orchestrator が内蔵でやっているコンフリクト解決を、CommandMate は CommandIndex との連携で柔軟に実現する

---

## シナリオ9: 過去の設計判断を踏まえた Issue 作成

**現状の課題:**
週次のロードマップ策定時、「前に同じようなことを考えて却下した」判断を忘れてしまう。dev-reports に設計書が蓄積されているが、数が多すぎて探せない。

**連携後:**
```bash
# AIとのブレスト中に、過去の設計判断を検索
commandindex search --semantic "認証のリフレッシュトークン" --path "dev-reports/" --format json

# → dev-reports/design/issue-135-jwt-auth-design-policy.md がヒット
# → 「Issue #135 の設計レビューで『リフレッシュトークンは v2 でやる』と判断済み」
# → AIがブレスト中にこの文脈を踏まえて提案してくれる

# さらに、過去の UAT 結果も検索
commandindex search "認証 テスト失敗" --path "sandbox/" --format json
# → 過去に認証周りで失敗したテストケースが見つかる
# → 新しい Issue の受け入れ条件に反映
```

**ポイント:**
- dev-reports/ に蓄積された設計書・レビュー結果・UAT レポートが「組織の記憶」になる
- AIとのブレストにこの記憶を注入することで、**同じ議論を繰り返さない**
- 人間の記憶に依存しない意思決定が可能になる

---

## シナリオ10: リリース前のリグレッション影響分析

**現状の課題:**
develop → main へのマージ前に、変更の影響範囲を確認したい。`git diff` でファイル一覧は見えるが、「このファイルを変えると何が壊れうるか」は見えない。

**連携後:**
```bash
# develop ブランチの変更ファイル一覧を取得
CHANGED=$(git diff main...develop --name-only)

# 各ファイルの影響範囲を CommandIndex で展開
for FILE in $CHANGED; do
  echo "=== $FILE ==="
  commandindex search --related "$FILE" --format json --limit 5
done > /tmp/impact-report.json

# 影響レポートを CommandMate 経由でAIに分析させる
commandmate send "$DEVELOP_WT" "以下の影響分析レポートを読んで、リグレッションリスクを評価してください: $(cat /tmp/impact-report.json)" --auto-yes
commandmate wait "$DEVELOP_WT" --timeout 600
commandmate capture "$DEVELOP_WT" --json
```

**ポイント:**
- `git diff` = 「何が変わったか」、CommandIndex の `--related` = 「何に影響するか」
- CI のテスト通過だけでなく、**テストでカバーされていない間接的な影響**を事前に検出
- リリース判断の材料としてスマホから確認できる（CommandMate の UI 経由）
- `/release` コマンドのプレフライトチェックとして組み込める

---

## まとめ: 全10シナリオの位置づけ

| # | シナリオ | フェーズ | 価値 |
|---|---------|---------|------|
| 1 | Issue 補完の精度向上 | Issue 作成時 | 影響ファイル特定の精度向上 |
| 2 | レビュー観点の自動生成 | レビュー時 | Issue 固有のリスク検出 |
| 3 | 並列エージェントの共有知識 | 開発中 | エージェント間の文脈共有 |
| 4 | マルチリポジトリ横断検索 | 全フェーズ | 複数事業の知識統合 |
| 5 | 朝のブリーフィング | 監視・確認 | 状態把握の効率化 |
| 6 | Issue 重複検出 | Issue 作成時 | 作業の重複排除 |
| 7 | 迷走の早期検出 | 開発中 | 監視負荷の削減 |
| 8 | 設計突合の自動化 | 設計レビュー | コンフリクト予測 |
| 9 | 過去の設計判断の活用 | ロードマップ策定 | 同じ議論を繰り返さない |
| 10 | リリース前のリグレッション分析 | リリース | テスト外の影響検出 |

### 本質

```
CommandMate = 手と足（実行・制御）
CommandIndex = 目と記憶（知識・文脈）

組み合わせ = 実行ラインに知識を注入する
```

CommandMate 単体では「動かす」ことしかできない。CommandIndex が加わることで、エージェントが**文脈を持って動く**ようになる。これは競合（Composio, Superset, claude-squad）のどれも持っていないレイヤー。

---
---

# 開発以外への活用シナリオ

> CommandMate の本質は「複数のAIエージェントを並列で動かし、人間が監視・制御する」こと。
> CommandIndex の本質は「ローカルのドキュメント・ファイルを横断検索し、文脈を引き出す」こと。
> この2つの組み合わせは、コーディングに限定されない。

---

## シナリオ11: マーケティング施策の並列運用

**ユースケース:**
個人開発者がプロダクト開発と同時にマーケティングも回す場合。ブログ記事、SNS投稿、競合分析、リリースノートなどを並列でAIに書かせる。

**連携の流れ:**
```bash
# マーケティング用のリポジトリ（CommandMate-Marketing）にも CommandIndex でインデックス構築
commandindex index --path ../CommandMate-Marketing

# Issue として施策を登録
# Issue #10: Qiita記事「3日で97 Issue」の執筆
# Issue #11: X投稿文の作成（日英）
# Issue #12: 競合分析レポート更新

# 各施策を並列でエージェントに投入
commandmate send "$WT_BLOG" "/write-article 10" --auto-yes
commandmate send "$WT_SNS" "/draft-x-posts 11" --auto-yes
commandmate send "$WT_ANALYSIS" "/competitor-analysis 12" --auto-yes

# ブログ記事のエージェントが過去の戦略ドキュメントを参照
commandindex search "ターゲットユーザー ポジショニング" --path ../CommandMate-Marketing/strategy/ --format json
# → strategy/positioning.md の内容が注入され、一貫したメッセージングになる
```

**価値:**
- プロダクト開発とマーケティングを同じツールチェーンで回せる
- 過去の戦略ドキュメント・分析結果を CommandIndex で検索し、施策間の一貫性を保つ
- スマホから「ブログ記事のドラフト確認→修正指示」「X投稿文の承認」ができる

---

## シナリオ12: 技術ドキュメント・社内Wiki の一括更新

**ユースケース:**
大きなリファクタリングや機能追加のあと、README、API ドキュメント、CHANGELOG、内部設計書、チュートリアルなど複数のドキュメントを同時に更新する必要がある。

**連携の流れ:**
```bash
# コード変更の影響を受けるドキュメントを CommandIndex で特定
commandindex search --related "src/auth/jwt.rs" --type markdown --format json
# → README.md, docs/api-reference.md, docs/tutorial/auth.md, CHANGELOG.md がヒット

# 各ドキュメントの更新を並列で実行
commandmate send "$WT1" "src/auth/jwt.rs の変更に合わせて README.md の認証セクションを更新して" --auto-yes
commandmate send "$WT2" "API リファレンスの /api/auth/login のレスポンス仕様を更新して" --auto-yes
commandmate send "$WT3" "チュートリアルの認証フローの手順を新しいAPIに合わせて修正して" --auto-yes

# 更新後、ドキュメント間の整合性をクロスチェック
commandindex search "JWT トークン 有効期限" --format json
# → 全ドキュメントで「24時間」と統一されているか確認
```

**価値:**
- 「コードは更新したがドキュメントが古いまま」を構造的に防ぐ
- CommandIndex の `--related` でドキュメントの更新漏れを検出
- ドキュメント間の記述の不整合を横断検索で発見

---

## シナリオ13: 学習ノート・リサーチの並列深掘り

**ユースケース:**
新しい技術（例: Rust の非同期処理）を学ぶとき、複数の切り口で同時にリサーチし、Markdown ノートとして蓄積する。

**連携の流れ:**
```bash
# リサーチテーマを Issue として登録
# Issue #1: tokio のランタイムモデルを調査
# Issue #2: async/await のエラーハンドリングパターン
# Issue #3: 実プロジェクトへの適用設計

# 並列でリサーチ実行
commandmate send "$WT1" "tokio のランタイムモデルについて調査し、notes/tokio-runtime.md にまとめて" --auto-yes
commandmate send "$WT2" "Rust の async/await のエラーハンドリングパターンを調査し、notes/async-error.md にまとめて" --auto-yes
commandmate send "$WT3" "CommandIndex への async 導入設計を notes/async-design.md にまとめて" --auto-yes

commandmate wait "$WT1" "$WT2" "$WT3" --timeout 3600

# 蓄積されたノートを CommandIndex でインデックス化
commandindex index

# 後日、関連知識を横断検索
commandindex search --semantic "非同期処理でのエラー伝播" --format json
# → 過去のリサーチノート、設計書、実装コードが横断でヒット
```

**価値:**
- リサーチの並列化で学習速度が上がる
- Markdown ノートが CommandIndex にインデックスされ、「個人のナレッジベース」になる
- 数ヶ月後に「あのとき調べたやつ」をセマンティック検索で引き出せる

---

## シナリオ14: 複数事業のKPI管理・レポート生成

**ユースケース:**
複数のOSSプロダクトを運営している場合、各プロダクトの GitHub Insights、npm DL 数、競合動向などを定期的に収集・分析・レポート化する。

**連携の流れ:**
```bash
# cron で毎朝自動実行（CMATE.md に定義）
# 3つのリポジトリの KPI を並列で収集

commandmate send "$WT_CM" "CommandMate の GitHub Insights を取得して analytics/reports/ に出力して" --auto-yes
commandmate send "$WT_ANVIL" "Anvil の GitHub Insights を取得して analytics/reports/ に出力して" --auto-yes
commandmate send "$WT_CI" "CommandIndex の GitHub Insights を取得して analytics/reports/ に出力して" --auto-yes

commandmate wait "$WT_CM" "$WT_ANVIL" "$WT_CI" --timeout 600

# 過去のレポートと比較して傾向分析
commandindex search "Stars 推移 週次" --path "analytics/reports/" --format json
# → 過去4週分のレポートがヒット
# → AIに渡して「先週比での変化」「異常値の検出」を分析させる

# 結果をサマリーレポートとして出力
commandmate send "$WT_SUMMARY" "3プロダクトのKPIサマリーを作成して: $(commandindex search 'GitHub Insights 2026-03' --path analytics/ --format json)" --auto-yes
```

**価値:**
- 「複数事業の同時運営」をデータ駆動で回す
- CommandIndex で過去レポートを横断検索し、時系列の傾向を自動分析
- 朝スマホで3プロダクト分のKPIサマリーを確認できる
- ミッション「複数の実行ラインを持つ execution OS」の非開発領域での体現

---

## シナリオ15: 法務・規約対応の並列調査

**ユースケース:**
OSSのライセンス変更、利用規約の改定（例: Anthropic の ToS 変更）など、複数のプロダクトに横断的に影響する法務イベントが発生した場合。

**連携の流れ:**
```bash
# 影響調査を並列で実行
commandmate send "$WT_CM" "Anthropic の利用規約変更が CommandMate に与える影響を調査して。LICENSE、README、依存ライブラリを確認" --auto-yes
commandmate send "$WT_ANVIL" "同規約変更が Anvil に与える影響を調査して" --auto-yes
commandmate send "$WT_CI" "同規約変更が CommandIndex に与える影響を調査して" --auto-yes

commandmate wait "$WT_CM" "$WT_ANVIL" "$WT_CI" --timeout 1800

# 過去の規約対応履歴を検索
commandindex search "ライセンス 規約 変更 対応" --workspace multi-repo.toml --format json
# → 過去に行った規約対応（例: strategy/anthropic-tos-impact-analysis.md）がヒット
# → 前回の対応方針を踏まえて、今回の対応を判断

# 3プロダクト分の影響をまとめて確認
commandmate capture "$WT_CM" --json
commandmate capture "$WT_ANVIL" --json
commandmate capture "$WT_CI" --json
# → スマホから「3プロダクトとも影響なし」or「CommandMate のみ対応必要」を判断
```

**価値:**
- 複数プロダクトへの横断的な影響調査を1人で並列に回せる
- 過去の対応履歴を CommandIndex で検索し、一貫した判断基準を保つ
- 法務対応のような「全プロダクトに波及するイベント」にスピーディに対応

---

## 開発以外シナリオのまとめ

| # | シナリオ | 領域 | 核心 |
|---|---------|------|------|
| 11 | マーケティング施策の並列運用 | マーケティング | 施策の並列実行 + 戦略の一貫性担保 |
| 12 | ドキュメントの一括更新 | テクニカルライティング | コード変更→ドキュメント更新の連動 |
| 13 | 学習・リサーチの並列深掘り | 学習・調査 | 並列リサーチ + 個人ナレッジベース構築 |
| 14 | 複数事業のKPI管理 | 事業運営 | 定期レポート自動生成 + 時系列分析 |
| 15 | 法務・規約対応の並列調査 | 法務・ガバナンス | 横断的影響調査 + 過去対応の参照 |

### 共通点

開発以外のシナリオに共通するのは:

- **「調べて、書いて、まとめる」作業の並列化** — これはコーディングに限らない
- **蓄積されたドキュメントが「組織の記憶」になる** — CommandIndex がそれを引き出す
- **複数の事業ラインを1人で回す** — ミッションそのもの

→ CommandMate + CommandIndex は「コーディングツール」ではなく、**「知識を持ったAIを複数動かして、複数の事業を運営するための基盤」**。
