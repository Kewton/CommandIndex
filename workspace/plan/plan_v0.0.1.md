# CommandIndex 企画書

## 1. 企画名
**CommandIndex**

---

## 2. 概要
CommandIndex は、ローカルで動作するナレッジ検索・文脈取得システムです。  
Markdownファイル、ソースコード、Git履歴をもとに、個人および少人数チーム向けの知識検索基盤を提供します。

本ツールは、単なる全文検索ツールではなく、将来的に AI への文脈供給基盤としても活用できることを前提に設計します。  
ただし初期フェーズでは、重いRAG基盤を前提とせず、**軽量なローカル検索CLI** として立ち上げます。

---

## 3. 背景
個人開発者や少人数チームでは、知識が次のように分散しやすいです。

- Markdownメモ
- README
- 設計メモ
- ソースコード
- Git履歴
- AIとの作業ログ
- タスクメモ

この結果、知識は保存されていても、必要なときに素早く取り出せないという問題が発生します。

例えば以下のような問いにすぐ答えられないことがあります。

- 前に考えた認証設計はどこに書いたか
- このコードに関連する設計メモは何か
- 過去に似た変更をした箇所はどこか
- この関数に関連するノートやREADMEは何か

既存のノート系ツールはコードやGitとの統合が弱く、コード検索系ツールは個人知識管理に弱い傾向があります。  
また、汎用RAGツールはこの規模感では重すぎることがあり、ローカルCLI体験と相性が悪い場合があります。

---

## 4. 解決したい課題

### 4.1 知識の分散
Markdown、コード、Git履歴が別々に存在しており、横断的に検索しにくい。

### 4.2 文脈の再利用不足
既存の設計や過去の議論を再活用しにくく、毎回ゼロから考え直しやすい。

### 4.3 AI利用時の前提知識不足
AIへ渡すべき文脈を人手で集める必要があり、作業効率が下がる。

### 4.4 軽量・ローカル前提の選択肢不足
個人〜少人数チーム向けに、ローカル完結で軽量に使えるナレッジ基盤が少ない。

---

## 5. 目的
CommandIndex の目的は、以下を実現することです。

- ローカルの Markdown / Code / Git を知識として横断検索可能にする
- 個人でも軽く使えること
- 少人数チームでも活用できること
- 将来的にAI向け文脈供給基盤へ拡張できること
- CommandMate / Anvil のようなローカルAIワークフローとも CLI 経由で接続可能にすること

---

## 6. コンセプト
CommandIndex は、以下のコンセプトで設計します。

> **ローカルファイルを原本とし、検索用インデックスを派生物として生成する Git-native な知識検索CLI**

重要な思想は次の通りです。

- 真実のソースはファイルそのもの
- インデックスは再生成可能な派生物
- Gitと自然に共存する
- 最初は全文検索と構造検索を主軸にする
- 将来的にRAG的取得層を追加する

---

## 7. 対象ユーザ

### 7.1 一次ターゲット
- 個人開発者
- ローカル環境を重視する開発者
- Markdownでメモを取る技術者
- コードと設計ノートを横断して扱いたい人

### 7.2 二次ターゲット
- 少人数の技術チーム
- GitとMarkdownを併用している開発チーム
- AIコーディングを行うチーム
- 暗黙知を整理したいチーム

### 7.3 将来的な対象
- 開発者以外の知識労働者
- ドキュメント中心の個人ナレッジ運用ユーザ
- 小規模な業務チーム

---

## 8. 提供価値

### 8.1 個人向け価値
- 自分のノート・コード・過去変更を横断検索できる
- 設計や判断の再利用がしやすくなる
- AIに渡す文脈を集めやすくなる
- ローカル完結で軽く使える

### 8.2 少人数チーム向け価値
- リポジトリ内知識の探索コストを下げる
- README / 設計 / 実装の横断がしやすくなる
- チーム知識の散逸を抑えやすくなる
- 共有リポジトリ上の暗黙知を引き出しやすくなる

### 8.3 AIワークフロー向け価値
- AIセッション前の関連文脈取得に使える
- ローカル知識の取得基盤として活用できる
- 将来的なRAG / context pack の土台になる

---

## 9. プロダクト方針

### 9.1 最初から重いRAG製品にしない
初期フェーズでは、重いベクトルDBや大規模RAG基盤を前提にしません。  
まずは **ローカル知識検索CLI** として成立させます。

### 9.2 全文検索と構造検索を主軸にする
最初に価値が出るのは以下です。

- 見出し検索
- パス検索
- タグ検索
- シンボル検索
- 関連ノート
- 関連コード

### 9.3 後からRAG化できる設計にする
初期実装は軽くしつつ、将来的には以下を追加可能にします。

- Embedding生成
- Semantic Search
- Hybrid Search
- Context Pack生成
- Reranking
- AIセッション連携

---

## 10. 基本アーキテクチャ

### 10.1 原本
- Markdown files
- Source code files
- Git repository

### 10.2 解析層
- Markdown parser
- frontmatter parser
- link parser
- source code parser
- symbol extractor
- git diff parser

### 10.3 インデックス層（tantivy ベース）
- full-text index
- metadata index
- path index
- symbol index（関数・クラス・コールグラフ・依存関係を含む）
- link index
- state / manifest

保存形式には **tantivy**（Rust製全文検索エンジンライブラリ）を採用する。
- Rustとの親和性が高く、高速な全文検索が可能
- スキーマ定義による構造化インデックスが可能
- 将来的な hybrid search（BM25 + semantic）への拡張と相性がよい

### 10.4 検索層
- keyword search
- path search
- tag search
- symbol search（Phase 3〜）
- related document lookup
- related code lookup

### 10.5 関連検索の判定ロジック

「関連ドキュメント」「関連コード」の判定は、フェーズに応じて以下のロジックを段階的に導入する。

**Phase 1〜2（構造ベース）:**
- Markdownリンク（`[[]]` / `[]()` ）による明示的な参照関係
- frontmatter のタグ一致
- 同一ディレクトリ内のファイル近接性
- ファイルパスの部分一致（例: `docs/auth/` と `src/auth/`）

**Phase 3（コード連携）:**
- シンボル名とMarkdown内のキーワード一致
- import / require による依存関係
- Git履歴上の同時変更（co-change: 同一コミットで変更されたファイル群）

**Phase 5（意味ベース）:**
- embedding による意味的類似度

**Phase 7（グラフRAG）:**
- LLM抽出エンティティ間の関係性によるグラフ探索（2〜3ホップ）
- LLM生成要約の類似度による関連判定
- 暗黙的リンク推定（明示的参照がないファイル間の関連性）

### 10.6 外部連携層（CLI パイプライン）

CommandMate / Anvil 等の外部ツールとは **CLI のパイプライン** で連携する。

```bash
# 関連文脈を取得して AI ツールに渡す例
commandindex search --related src/auth/handler.ts --format json | commandmate context --stdin

# 検索結果を fzf で絞り込み、選択したファイルを開く
commandindex search "認証" --format path | fzf | xargs $EDITOR

# JSON出力をjqで加工して他ツールに渡す
commandindex search "設計" --format json | jq '.[] | .path' | xargs cat
```

連携の基本方針:
- 外部ツールとの接続は CLI の標準入出力（stdin / stdout）を介する
- `--format json` (JSONL) を連携の標準フォーマットとする
- 専用のサーバプロセスやAPIエンドポイントは初期では提供しない
- 将来的に必要であれば、Phase 4 以降でローカルAPIサーバを検討する

### 10.7 将来の拡張層
- embedding index（Phase 5）
- semantic retrieval（Phase 5）
- hybrid retrieval（Phase 5）
- context pack builder（Phase 4）
- LLMによるエンティティ・関係性抽出（Phase 7）
- LLMによる要約生成（Phase 7）
- グラフ探索による関連知識発見（Phase 7）

---

## 11. CLIコンセプト

### 11.1 基本思想
- まず解析してインデックスを作る
- そのインデックスを使って高速検索する

### 11.2 想定コマンド
```bash
commandindex index
commandindex search "認証の流れ"
commandindex update
commandindex status
commandindex clean
```

### 11.3 コマンド役割

| コマンド | 役割 |
|---|---|
| `index` | リポジトリを解析し、検索高速化用インデックスを初回フルビルドする |
| `search` | インデックスを使って高速検索する |
| `update` | Git差分をもとにインデックスを差分更新する |
| `status` | インデックス状態を確認する |
| `clean` | インデックス削除・再構築準備を行う |

> **`index` と `update` の使い分け:**
> `index` は初回フルビルド、`update` は差分更新を行う。
> 挙動が安定するまでは `clean` → `index` によるリビルドパスを残す。
> 将来的に `index` に差分判定を統合する可能性はあるが、初期は明示的に分離する。

### 11.4 CLI出力形式

検索結果の出力は以下の3形式をサポートする。

| 形式 | 用途 | フラグ |
|---|---|---|
| **human** | ターミナルでの目視確認（デフォルト） | `--format human` |
| **json** | プログラムからの利用、AI文脈供給 | `--format json` |
| **path** | パイプ連携（fzf, xargs 等） | `--format path` |

- デフォルトは `human` 形式（色付き、見出し・スニペット表示）
- `--format json` はJSONL（1行1レコード）で出力し、`jq` との連携を容易にする
- `--format path` はファイルパスのみを出力し、`fzf` や `xargs` にパイプしやすくする
- 将来的にAI文脈供給で使う場合は `--format json` を基本とする

### 11.5 検索クエリ構文

検索は `search` サブコマンドに統一し、フラグで検索種別を制御する。

```bash
# 全文検索（デフォルト）
commandindex search "認証の流れ"

# タグ検索
commandindex search --tag auth

# パス絞り込み
commandindex search "認証" --path docs/

# シンボル検索（Phase 3〜）
commandindex search --symbol handleAuth

# ファイル種別絞り込み
commandindex search "認証" --type markdown
commandindex search "認証" --type typescript

# 関連ドキュメント検索
commandindex search --related src/auth/handler.ts

# 組み合わせ
commandindex search "認証" --tag auth --path src/ --format json
```

| フラグ | 説明 | Phase |
|---|---|---|
| （なし） | 全文検索 | 1 |
| `--tag <tag>` | frontmatter タグで絞り込み | 1 |
| `--path <path>` | パスプレフィックスで絞り込み | 1 |
| `--type <type>` | ファイル種別で絞り込み（markdown, typescript, python） | 1 |
| `--heading <text>` | 見出しテキストで検索 | 1 |
| `--symbol <name>` | シンボル名で検索 | 3 |
| `--related <file>` | 指定ファイルの関連ドキュメントを検索 | 2 |
| `--format <fmt>` | 出力形式（human / json / path） | 1 |
| `--limit <n>` | 結果件数の上限（デフォルト: 20） | 1 |



---

## 12. 初期スコープ

### 12.1 対象

- Markdownファイル
- ソースコードファイル（TypeScript, Python を優先）
- Git情報

### 12.2 対象言語の優先度

| 優先度 | 言語 | 備考 |
|---|---|---|
| 1 | Markdown | ナレッジの主体 |
| 2 | TypeScript | フロントエンド・バックエンド双方で広く使われる |
| 3 | Python | データ処理・AI系で広く使われる |

- シンボル抽出には **tree-sitter** を使用し、多言語対応の土台を作る
- 上記以外の言語は Phase 3 以降で段階的に追加する

### 12.3 初期機能（Phase 1〜2 に対応）

- Markdown走査
- 見出し単位分割
- frontmatter抽出
- tag抽出
- path検索
- 全文検索（日本語・英語対応）
- Git差分による更新判定
- 高速検索

> **注:** シンボル抽出（関数名・クラス名・コールグラフ・依存関係）は Phase 3 で対応する。
> 初期フェーズでは Markdown 中心の知識検索 MVP に集中する。

### 12.4 初期スコープ外

- シンボル抽出・コールグラフ解析（Phase 3）
- 高度な意味検索（Phase 5）
- 自動要約
- 複雑なナレッジグラフ
- 分散構成
- SaaS前提の共有機能
- 重い常駐型サーバ構成



---

## 13. 想定ユースケース

### 13.1 個人利用

- 過去に書いた設計メモを検索する
- 実装コードに関係するノートを探す
- 関数名・概念名・タグで知識を探す
- 過去の変更と関連知識を確認する

### 13.2 開発利用

- README / 設計メモ / 実装コードを横断する
- 今触っているファイルに関係する情報を探す
- 過去の設計判断の痕跡を再利用する
- AIに渡す前提知識を収集する

### 13.3 少人数チーム利用

- チーム共有リポジトリ上の知識を横断検索する
- 暗黙知をノート・コードから回収する
- 設計と実装の対応関係を追いやすくする



---

## 14. 競合・類似カテゴリ

近いカテゴリには以下があります。

- Obsidian系のローカル知識検索
- コード文脈検索ツール
- AI coding memory ツール
- チーム向けナレッジ検索サービス

しかし、Markdown + Code + Git + Local CLI + 少人数チーム を軽量に一体化したものは相対的に少なく、そこに差別化余地があります。


---

## 15. 差別化ポイント

CommandIndex の差別化ポイントは以下です。

- ローカルファースト
- Git-native
- Markdown と Code を横断
- CLI中心で軽量
- 個人利用から少人数チームまで対応可能
- 将来的にAI文脈基盤へ拡張可能
- 原本をDBに閉じ込めない



---

## 16. 非機能要件

### 16.1 想定規模

- Markdownファイル（数百行）: 1000ファイル
- ソースコード（数百行）: 500ファイル

### 16.2 性能目標

| 操作 | 目標 | 備考 |
|---|---|---|
| 初回 full index | 1〜3分 | 1500ファイル規模 |
| 差分更新（数十ファイル） | 1分以内 | |
| 検索レスポンス | 500ms以内 | 1500ファイル規模、結果20件取得 |
| `status` コマンド | 100ms以内 | state.json 読み取りのみ |

- ローカルCLIとしてストレスなく動くこと

### 16.3 エラー時の挙動方針

| 状況 | 挙動 |
|---|---|
| インデックス未作成で `search` を実行 | エラーメッセージを表示し `commandindex index` の実行を案内する |
| インデックス未作成で `update` を実行 | エラーメッセージを表示し `commandindex index` の実行を案内する |
| インデックス破損を検知 | エラーメッセージを表示し `commandindex clean && commandindex index` を案内する |
| `update` 中にファイルが変更された | 変更前の状態でインデックスを更新する（次回 `update` で反映） |
| 対象ファイルの読み取り権限がない | 警告を出力しスキップする（他のファイルは継続処理） |
| `.cmindexignore` が不正な記法 | 該当行を警告付きでスキップし、残りのルールは適用する |

- インデックスの整合性は `state.json` のバージョンとスキーマバージョンの照合で簡易チェックする
- 破損の自動検知は最小限とし、明示的な `clean` → `index` によるリビルドを基本復旧手段とする

### 16.4 成立条件

- 初期は全文検索中心であること
- 重い embedding 生成を同期必須にしないこと
- 差分更新設計を前提とすること
- インデックスを再生成可能な派生物とすること



---

## 17. 技術方針

### 17.1 言語

- Rust

### 17.2 理由

- 高速
- 軽量
- ローカルCLIに向く
- メモリ効率を詰めやすい
- 並列処理・差分更新との相性がよい
- 単一バイナリ配布に向く

### 17.3 主要ライブラリ

| ライブラリ | 用途 | Phase |
|---|---|---|
| tantivy | 全文検索インデックス | 1 |
| lindera | 日本語トークナイザー（tantivy に組み込み） | 1 |
| tree-sitter | ソースコード解析・シンボル抽出 | 3 |
| rusqlite | SQLite による補助ストア（シンボル・グラフ情報） | 3 |

### 17.4 多言語テキスト検索

対応言語は **日本語** と **英語** とする。

- tantivy のカスタムトークナイザーとして **lindera**（MeCab辞書ベース）を組み込む
- 日本語テキストは lindera でトークナイズし、英語テキストは tantivy 標準のトークナイザーを使用する
- フィールドごとに言語を判定し、適切なトークナイザーを適用する
- 初期はファイル単位の言語判定とし、段落単位の混在対応は将来課題とする

### 17.5 インデックス思想

- 単一巨大ファイルではなく、インデックスディレクトリを前提にする
- state / manifest / index data を分離する
- 壊れた場合に `clean` → `index` できるようにする

### 17.6 インデックスディレクトリ `.commandindex/`

インデックスは `.commandindex/` ディレクトリに保存する。

```
.commandindex/
├── tantivy/          # tantivy 全文検索インデックス
├── symbols.db        # SQLite: シンボル・コールグラフ・依存関係（Phase 3〜）
├── manifest.json     # ファイル一覧・ハッシュ・最終更新情報
└── state.json        # インデックスのメタ情報（バージョン、最終ビルド日時等）
```

**tantivy と SQLite の役割分担:**

| ストア | 用途 | 得意なクエリ |
|---|---|---|
| tantivy | 全文検索、キーワード検索、タグ検索 | テキスト一致、BM25ランキング |
| SQLite (`symbols.db`) | シンボル、コールグラフ、依存関係、ファイル間リンク | グラフ探索、JOIN、集計 |

- tantivy は「テキストからドキュメントを見つける」検索に使う
- SQLite は「構造や関係性を辿る」検索に使う（Phase 3〜）
- 両方とも `.commandindex/` 内に格納し、`clean` で一括削除・再構築できる
- `.commandindex/` は `.gitignore` に追加する（派生物であり、Git管理対象外）
- インデックスは常に再生成可能な派生物として扱う

### 17.7 除外設定 `.cmindexignore`

インデックス対象外のファイル・ディレクトリは `.cmindexignore` で指定する。

```
# .cmindexignore の例
node_modules/
target/
dist/
.git/
*.min.js
*.lock
```

- `.gitignore` と同様の記法（glob パターン）をサポートする
- `.cmindexignore` が存在しない場合は、デフォルトの除外ルール（`node_modules/`, `target/`, `.git/` 等）を適用する
- `.cmindexignore` はリポジトリルートに配置し、Git管理対象とする（チーム共有可能）



---

## 18. 今後のロードマップ

### Phase 1: Markdown Knowledge MVP

- Markdown解析
- heading単位インデックス
- path / tag / text検索
- `index` / `search` / `status` コマンド提供

### Phase 2: Git-aware Update

- 差分更新
- last commit 反映
- 削除検知
- 高速な incremental update

### Phase 3: Code Knowledge

- tree-sitter によるソースコード解析（TypeScript, Python）
- シンボル抽出（関数名・クラス名・コールグラフ・依存関係）
- note / code 横断検索
- 関連コード検索

### Phase 4: Context Retrieval

- related context
- context pack 生成
- AI向け文脈取り出しAPI
- 関連ノート / 関連コード統合

### Phase 5: Semantic Extension

- embedding追加
- hybrid retrieval
- semantic search
- rerank

### Phase 6: Team Extension

- チーム向け設定
- index共有方針の整理
- 少人数チーム利用最適化

### Phase 7: Graph RAG（LLM活用によるナレッジグラフ拡張）

LLMをインデックス構築時に活用し、ルールベースでは抽出できない意味的なエンティティ・関係性をグラフとして格納する。

**目的:**
- Markdownやコードから概念レベルのエンティティを抽出する（例: 「認証」「セッション管理」「ユーザ権限」）
- エンティティ間の意味的関係を抽出する（例: 「認証モジュールはセッション管理に依存する」）
- ドキュメント・関数単位の要約を生成しインデックスに格納する
- 明示的なリンクがないファイル間の暗黙的な関連性を推定する

**機能:**
- LLMによるエンティティ抽出・関係性抽出
- LLMによる要約生成（ドキュメント単位・関数単位）
- 暗黙的リンク推定（明示的参照がないファイル間の関連性）
- グラフ探索による関連知識の発見（2〜3ホップの関係辿り）

**データストア方針:**
- グラフデータは既存の SQLite (`symbols.db`) に追加テーブルとして格納する
- Neo4j 等の専用グラフDBは導入しない（CommandIndex の想定規模ではオーバースペック）

```sql
-- symbols.db への追加テーブル（Phase 7）
-- エンティティ（LLM抽出）
CREATE TABLE entities (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,           -- エンティティ名（例: "認証"）
    entity_type TEXT,             -- 種別（concept / module / pattern 等）
    source_file TEXT NOT NULL,    -- 抽出元ファイル
    source_section TEXT,          -- 抽出元セクション（見出し等）
    summary TEXT,                 -- LLM生成の要約
    file_hash TEXT NOT NULL       -- キャッシュ用: 抽出元ファイルのハッシュ
);

-- 関係性（LLM抽出）
CREATE TABLE semantic_relations (
    id INTEGER PRIMARY KEY,
    source_id INTEGER NOT NULL REFERENCES entities(id),
    target_id INTEGER NOT NULL REFERENCES entities(id),
    relation_type TEXT NOT NULL,  -- 関係種別（depends_on / related_to / implements 等）
    confidence REAL,              -- LLMの確信度（0.0〜1.0）
    evidence TEXT                 -- 根拠となるテキスト断片
);

-- ドキュメント要約（LLM生成）
CREATE TABLE summaries (
    id INTEGER PRIMARY KEY,
    file_path TEXT NOT NULL,
    section TEXT,                 -- 見出し or 関数名（NULL = ファイル全体）
    summary TEXT NOT NULL,
    file_hash TEXT NOT NULL       -- キャッシュ用
);
```

SQLite の再帰CTEでグラフ探索が十分可能:

```sql
-- 「認証」に関連するエンティティを2ホップで探索
WITH RECURSIVE related AS (
    SELECT target_id, 1 AS depth
    FROM semantic_relations
    WHERE source_id = (SELECT id FROM entities WHERE name = '認証')
    UNION ALL
    SELECT r.target_id, related.depth + 1
    FROM semantic_relations r
    JOIN related ON r.source_id = related.target_id
    WHERE related.depth < 2
)
SELECT DISTINCT e.name, e.entity_type, e.source_file
FROM related JOIN entities e ON e.id = related.target_id;
```

**想定LLM:**

| LLM | コスト | 速度 | 適性 |
|---|---|---|---|
| ローカルLLM（Ollama等） | 無料 | 遅い（数十分〜数時間 / 1500ファイル） | コスト重視・オフライン環境 |
| gpt-4.1-mini 等の軽量API | 数百円〜数千円 / 1500ファイル | 速い（数分〜十数分） | 速度重視・精度重視 |

**キャッシュ戦略（必須）:**

LLMによるインデックス構築はコスト・時間がかかるため、再生成可能な派生物の思想と両立させるためにキャッシュ戦略を設ける。

- LLM抽出結果はファイルハッシュと紐づけて保持する
- ファイルが変更されていなければ、LLM再実行をスキップする
- `clean` コマンドに `--keep-llm-cache` オプションを追加し、LLMキャッシュのみ保持可能にする
- LLM抽出は非同期・バックグラウンドで実行し、通常の検索をブロックしない
- LLMキャッシュが存在しない場合でも、Phase 1〜5 の機能は正常に動作する（LLM層はオプショナル）



---

## 19. リスクと対策

| リスク | 対策 |
|---|---|
| 機能を盛り込みすぎて重くなる | 初期は全文検索・構造検索に絞る。Embedding は後から追加する |
| 差分更新が実装上フル再構築に近づく | manifest と file state を明確に持ち、変更ファイル単位で更新する |
| 単なる grep の延長に見えてしまう | Markdown / Code / Git 横断、関連検索、AI文脈取得という方向で価値を示す |
| 競合との差別化不足 | ローカル、Git-native、CLI、少人数チーム対応、CommandMate / Anvil との接続可能性を明確にする |
| LLM活用でインデックス再構築コストが跳ね上がる | ファイルハッシュベースのキャッシュ戦略を必須とし、LLM抽出結果を保持可能にする。`clean --keep-llm-cache` で通常インデックスのみ再構築できるようにする |
| LLM層への依存でローカル軽量の原則が崩れる | LLM層は完全にオプショナルとし、Phase 1〜5 の機能はLLMなしで正常動作することを保証する |


---

## 20. 成功条件

CommandIndex が成功したと判断できる状態は以下です。

- 個人開発者が日常的に使いたくなる
- Markdown / Code / Git の横断検索が価値として伝わる
- 初回 index と差分更新が目標時間内に収まる
- AI前提でなくても十分便利である
- 後からRAG的取得機能を自然に追加できる



---

## 21. 最終方針

CommandIndex は、最初から「フルRAGツール」として作るのではなく、まずは ローカル知識検索CLI として立ち上げる。
その上で、Markdown・Code・Git を横断する土台を作り、将来的に semantic retrieval や AI向け context pack に拡張していく。

つまり、方針は以下です。

1. まず軽く作る
2. まず日常的に使える検索体験を作る
3. その上でRAG的取得層を足す

この順序で進めることで、個人利用でも価値があり、少人数チームにも展開可能なナレッジ基盤を目指す。

---

## 22. 一言でいうと

> CommandIndex は、Markdown・Code・Git を横断し、ローカルで高速に知識を引き出すための Git-native knowledge CLI である。