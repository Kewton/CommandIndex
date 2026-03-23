# CommandIndex v0.1.0 必要機能一覧

> 作成日: 2026-03-23
> scenario.md の全15シナリオを実現するために、CommandIndex 側で対応が必要な機能を整理する

---

## 概要

全15シナリオのうち、CommandIndex の**既存機能だけで実現可能**なものが7つ、**新機能が必要**なものが8つある。

| 区分 | シナリオ数 | 該当シナリオ |
|------|-----------|------------|
| 既存機能で実現可能 | 7 | 1, 2, 4, 6, 9, 11-15 |
| 新機能が必要 | 8 | 3, 5, 7, 8, 10 |

※ シナリオ 1, 2, 6, 7, 8 は CommandIndex の既存機能を CommandMate のスラッシュコマンドから呼び出すだけで実現可能だが、CommandIndex 側の UX 改善（CI-2, CI-6）があるとより効果的。

---

## 新機能一覧

### CI-1: `impact` サブコマンド（git diff ベースの影響分析）

**対応シナリオ:** 5（朝のブリーフィング）, 10（リグレッション影響分析）

**概要:**
Git の変更ファイル一覧を入力として受け取り、各ファイルの `--related` 結果を一括で返す。リリース前の影響分析や、ブリーフィング生成の基盤となる。

**CLI インターフェース:**
```bash
# git diff の出力をパイプで受け取る
git diff main...develop --name-only | commandindex impact --format json

# 直接ファイル指定
commandindex impact src/auth/jwt.rs src/auth/middleware.rs --format json

# 出力制限
commandindex impact --limit 5 --format json < changed_files.txt
```

**出力フォーマット（JSON）:**
```json
{
  "changed_files": ["src/auth/jwt.rs", "src/auth/middleware.rs"],
  "impact": [
    {
      "file": "src/auth/jwt.rs",
      "related": [
        {"path": "docs/auth/design.md", "score": 0.95, "relations": ["markdown_link"]},
        {"path": "tests/auth_test.rs", "score": 0.8, "relations": ["import_dependency"]}
      ]
    }
  ],
  "overlap": ["tests/auth_test.rs"],
  "summary": {
    "changed": 2,
    "total_impacted": 8,
    "overlap_count": 1
  }
}
```

**ポイント:**
- `overlap` フィールドで、複数の変更ファイルから共通して影響を受けるファイルを検出（コンフリクトリスクの高い箇所）
- stdin 対応（CI-6）と組み合わせると `git diff | commandindex impact` のパイプラインが成立

**優先度:** 高 | **実装規模:** 中

---

### CI-2: `--related` の複数ファイル対応

**対応シナリオ:** 8（設計突合の自動化）, 10（リグレッション影響分析）

**概要:**
現在 `--related` は1ファイルしか指定できない。複数ファイルを指定して、それぞれの影響範囲を結合して返す機能。

**CLI インターフェース:**
```bash
# 複数ファイル指定（カンマ区切り or 複数オプション）
commandindex search --related src/auth/jwt.rs --related src/auth/middleware.rs --format json

# 結果はスコア最大値でマージ（context コマンドと同じ方式）
```

**ポイント:**
- `context` コマンドの複数ファイル指定と同じマージ方式（union + スコア最大値）
- 既存の `related` 検索ロジックを再利用できるため実装は軽量

**優先度:** 高 | **実装規模:** 小

---

### CI-3: `--changed-since` オプション（Git 履歴ベースの変更検索）

**対応シナリオ:** 5（朝のブリーフィング）

**概要:**
指定期間内に変更されたファイルを Git 履歴から取得し、それらの関連情報を返す。`git log` + `--related` の統合コマンド。

**CLI インターフェース:**
```bash
# 12時間以内の変更ファイルの関連情報
commandindex search --changed-since "12 hours ago" --format json

# 昨日以降
commandindex search --changed-since "yesterday" --format json

# 特定コミット以降
commandindex search --changed-since "abc1234" --format json
```

**実装方針:**
- 内部で `git log --since=<期間> --name-only --format=''` を実行
- 変更ファイル一覧を取得し、各ファイルに対して `--related` 検索を実行
- `impact` サブコマンド（CI-1）の内部ロジックを共用可能

**優先度:** 高 | **実装規模:** 中

---

### CI-4: `--index-path` オプション（インデックスパス指定）

**対応シナリオ:** 3（並列エージェントの共有知識）

**概要:**
`.commandindex/` のデフォルトパスではなく、任意のパスにあるインデックスを参照する。複数 worktree が共通のインデックスを共有できるようにする。

**CLI インターフェース:**
```bash
# 共有インデックスを参照して検索
commandindex search "認証" --index-path /shared/.commandindex/

# 共有インデックスに対してupdate
commandindex update --index-path /shared/.commandindex/

# 設定ファイルでも指定可能
# commandindex.toml:
# [index]
# path = "/shared/.commandindex/"
```

**ポイント:**
- 並列エージェント（worktree A, B, C）が同一のインデックスを参照することで、エージェント A の設計判断をエージェント B が検索できる
- `commandindex.toml` の `[index]` セクションでも指定可能にする（チーム設定としての共有）
- ロック機構が必要（複数プロセスからの同時書き込み防止）→ SQLite の WAL モードで対応可能

**優先度:** 高 | **実装規模:** 小

---

### CI-5: `diff` サブコマンド（影響範囲の比較・共通ファイル検出）

**対応シナリオ:** 8（設計突合の自動化）

**概要:**
2つ以上のファイル/Issue の影響範囲を比較し、共通ファイル（コンフリクトリスク）を検出する専用コマンド。

**CLI インターフェース:**
```bash
# 2ファイルの影響範囲を比較
commandindex diff src/auth/jwt.rs src/auth/middleware.rs --format json

# 出力
# {
#   "file_a": "src/auth/jwt.rs",
#   "file_b": "src/auth/middleware.rs",
#   "only_a": ["docs/jwt-design.md"],
#   "only_b": ["docs/middleware-design.md"],
#   "overlap": ["src/auth/types.rs", "tests/auth_test.rs"],
#   "overlap_count": 2
# }
```

**ポイント:**
- `/orchestrate` Phase 4（設計突合）での自動コンフリクト予測に使用
- `impact` サブコマンド（CI-1）の `overlap` フィールドと機能的に重複するため、CI-1 で十分な場合はスキップ可能

**優先度:** 中 | **実装規模:** 中

---

### CI-6: stdin パイプ入力対応

**対応シナリオ:** 10（リグレッション影響分析）

**概要:**
stdin からファイルパスリストやテキストを受け取り、検索やimpact分析の入力として使用する。

**CLI インターフェース:**
```bash
# git diff の出力を直接パイプ
git diff main...develop --name-only | commandindex impact --format json

# ファイルリストをパイプ
cat file_list.txt | commandindex search --related-stdin --format json

# grep 結果をパイプ
grep -rl "getDbInstance" src/ | commandindex impact --format json
```

**実装方針:**
- `impact` サブコマンドは stdin をデフォルト入力とする（引数がなければ stdin を読む）
- `search --related-stdin` で stdin からファイルリストを読み取る
- 1行1ファイルパスの形式

**優先度:** 中 | **実装規模:** 小

---

### CI-7: ファイル変更監視（watch モード）

**対応シナリオ:** 3（並列エージェントの共有知識）

**概要:**
ファイルシステムの変更を監視し、変更があれば自動で `update` を実行する常駐プロセス。

**CLI インターフェース:**
```bash
# 変更を監視して自動 update
commandindex watch

# バックグラウンドで実行
commandindex watch --daemon

# 特定パスのみ監視
commandindex watch --path src/ --path docs/
```

**実装方針:**
- `notify` crate でファイルシステムイベントを監視
- デバウンス（1秒以内の連続変更をまとめる）
- `.cmindexignore` に従ってフィルタリング

**ポイント:**
- エージェントがファイルを書き出すたびにインデックスが更新され、他のエージェントがすぐに検索できる
- 実装規模が大きいため、まずは CI-4（`--index-path`）+ CommandMate のフック（`commandindex update` 自動実行）で代替可能

**優先度:** 低 | **実装規模:** 大

---

## 推奨実装順序

```
Phase A（即効性・基盤）:
  CI-2  --related 複数ファイル対応        [小] → シナリオ 8, 10 の基盤
  CI-4  --index-path オプション           [小] → シナリオ 3 の基盤
  CI-6  stdin パイプ入力対応              [小] → シナリオ 10、CI-1 の前提

Phase B（コア機能）:
  CI-1  impact サブコマンド               [中] → シナリオ 5, 10 を実現
  CI-3  --changed-since オプション        [中] → シナリオ 5 を実現

Phase C（最適化）:
  CI-5  diff サブコマンド                 [中] → シナリオ 8 の UX 向上
  CI-7  watch モード                      [大] → シナリオ 3 の最適化
```

**Phase A の3機能（CI-2, CI-4, CI-6）はいずれも実装規模「小」**で、合計しても1〜2日程度の工数。これだけで全シナリオの基盤が整う。

---

## シナリオ × 機能 対応表

| シナリオ | CI-1 | CI-2 | CI-3 | CI-4 | CI-5 | CI-6 | CI-7 | 既存機能 |
|---------|------|------|------|------|------|------|------|---------|
| 1. Issue補完精度向上 | | | | | | | | ✅ search, context |
| 2. レビュー観点自動生成 | | | | | | | | ✅ --related, --symbol |
| 3. 並列エージェント共有知識 | | | | **◎** | | | △ | update |
| 4. マルチリポジトリ横断検索 | | | | | | | | ✅ --workspace |
| 5. 朝のブリーフィング | **◎** | | **◎** | | | ○ | | |
| 6. Issue重複検出 | | | | | | | | ✅ --semantic |
| 7. 迷走の早期検出 | | | | | | | | ✅ --related |
| 8. 設計突合の自動化 | ○ | **◎** | | | ○ | | | --related |
| 9. 過去の設計判断活用 | | | | | | | | ✅ --semantic, --path |
| 10. リグレッション影響分析 | **◎** | **◎** | | | | **◎** | | |
| 11-15. 開発以外 | | | | | | | | ✅ 既存機能 |

◎ = 必須, ○ = あると良い, △ = 代替手段あり
