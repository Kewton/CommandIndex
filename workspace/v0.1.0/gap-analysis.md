# CommandMate × CommandIndex 連携: ギャップ分析・必要機能一覧

> 作成日: 2026-03-23
> scenario.md の全15シナリオを実現するために必要な機能・対応を洗い出す

---

## 1. 現状の実装状態

### CommandIndex（現在 v0.0.5）

| 機能 | 状態 | 備考 |
|------|------|------|
| 全文検索（BM25） | ✅ 実装済み | search コマンド |
| シンボル検索（--symbol） | ✅ 実装済み | tree-sitter ベース |
| 関連ファイル検索（--related） | ✅ 実装済み | リンク・タグ・パス・import |
| Semantic検索（--semantic） | ✅ 実装済み | Ollama / OpenAI embedding |
| Hybrid検索（BM25 + Semantic） | ✅ 実装済み | RRF統合 |
| Reranking（--rerank） | ✅ 実装済み | Ollama Cross-Encoder |
| Context Pack生成（context） | ✅ 実装済み | AI向けJSON出力 |
| マルチリポジトリ横断検索（--workspace） | ✅ 実装済み | ワークスペース設定ファイル |
| チーム共有設定（commandindex.toml） | ✅ 実装済み | 設定優先順位対応 |
| インデックス共有（export / import） | ✅ 実装済み | tar.gz形式 |
| status拡張（--detail / --format json） | ✅ 実装済み | カバレッジ・ストレージ |
| スニペット表示調整 | ✅ 実装済み | --snippet-lines / --snippet-chars |
| **Git履歴ベースの変更検索** | ❌ 未実装 | --since / --changed-since |
| **パイプ入力（stdin）対応** | ❌ 未実装 | 他コマンドの出力を受け取る |
| **自動インデックス更新（watchモード）** | ❌ 未実装 | ファイル変更時の自動update |
| **検索結果のマージ・比較ユーティリティ** | ❌ 未実装 | 複数検索結果の集合演算 |

### CommandMate（現在）

| 機能 | 状態 | 備考 |
|------|------|------|
| worktree管理・並列エージェント | ✅ 実装済み | send / wait / capture |
| 自動応答（auto-yes） | ✅ 実装済み | --auto-yes |
| プロンプト対応（respond） | ✅ 実装済み | wait --on-prompt |
| Web UI | ✅ 実装済み | デスクトップ・モバイル |
| スケジュール実行（CMATE.md） | ✅ 実装済み | cron定期実行 |
| スラッシュコマンド | ✅ 実装済み | /pm-auto-dev 等 |
| **CommandIndex連携の組み込み** | ❌ 未実装 | スラッシュコマンド内での呼び出し |
| **ブリーフィング生成** | ❌ 未実装 | 朝の要約自動生成 |
| **迷走検出** | ❌ 未実装 | 影響範囲外の変更検出 |

---

## 2. シナリオ別ギャップ分析

### シナリオ1: Issue補完の精度向上

| 必要機能 | 実装先 | 状態 | 優先度 |
|----------|--------|------|--------|
| `commandindex search` のJSON出力 | CommandIndex | ✅ 済 | - |
| スラッシュコマンド内でのCommandIndex呼び出し | CommandMate | ❌ 未 | **高** |
| Issue本文からのキーワード自動抽出 | CommandMate | ❌ 未 | 中 |

**対応:**
- CommandMateの `/issue-enhance` 等のスラッシュコマンド内で `commandindex search` を呼び出すフローを追加
- 具体的には `.claude/commands/` 内のコマンド定義にCommandIndex呼び出しステップを追加

---

### シナリオ2: レビュー観点の自動生成

| 必要機能 | 実装先 | 状態 | 優先度 |
|----------|--------|------|--------|
| `--related` でのimport依存展開 | CommandIndex | ✅ 済 | - |
| `--symbol` 検索 | CommandIndex | ✅ 済 | - |
| レビューコマンドへの影響グラフ注入 | CommandMate | ❌ 未 | **高** |

**対応:**
- `/multi-stage-design-review` の各Stageで、Issue影響ファイルに対して `commandindex search --related` を実行し、結果をレビュー観点に追加
- レビュープロンプトに「CommandIndex影響分析結果」セクションを追加

---

### シナリオ3: 並列エージェントの共有知識

| 必要機能 | 実装先 | 状態 | 優先度 |
|----------|--------|------|--------|
| `commandindex update`（差分更新） | CommandIndex | ✅ 済 | - |
| worktree間のインデックス共有 | CommandIndex | ❌ 未 | **高** |
| エージェント完了時の自動インデックス更新 | CommandMate | ❌ 未 | 中 |

**対応:**
- **新機能: 共有インデックスモード** — 複数worktreeが同一の `.commandindex/` を参照できる仕組み（シンボリックリンク or `--index-path` オプション）
- CommandMateの `/pm-auto-dev` 完了時に `commandindex update` を自動実行するフックを追加

---

### シナリオ4: マルチリポジトリ横断検索

| 必要機能 | 実装先 | 状態 | 優先度 |
|----------|--------|------|--------|
| `--workspace` オプション | CommandIndex | ✅ 済 | - |
| ワークスペース設定ファイル | CommandIndex | ✅ 済 | - |

**対応:** 既存機能で実現可能。ワークスペース設定ファイルの作成のみ必要。

---

### シナリオ5: 朝のブリーフィング自動生成

| 必要機能 | 実装先 | 状態 | 優先度 |
|----------|--------|------|--------|
| Git履歴ベースの変更ファイル検索 | CommandIndex | ❌ 未 | **高** |
| `commandmate ls --json` | CommandMate | ✅ 済 | - |
| ブリーフィング生成スクリプト / コマンド | CommandMate | ❌ 未 | **高** |
| CMATE.md でのcron定期実行 | CommandMate | ✅ 済 | - |

**対応:**
- **新機能: `--changed-since` オプション** — `commandindex search --changed-since "12 hours ago"` で最近変更されたファイルの関連情報を検索
- **新コマンド: `/briefing`** — CommandMateのスラッシュコマンドとして、CommandIndex + `commandmate ls` の結果を統合してブリーフィング生成

---

### シナリオ6: Issue重複検出

| 必要機能 | 実装先 | 状態 | 優先度 |
|----------|--------|------|--------|
| Semantic検索（--semantic） | CommandIndex | ✅ 済 | - |
| dev-reports/ のインデックス化 | CommandIndex | ✅ 済 | - |

**対応:** 既存機能で実現可能。`/issue-create` コマンド内で `commandindex search --semantic` を呼び出すフローを追加するのみ。

---

### シナリオ7: エージェントの迷走検出

| 必要機能 | 実装先 | 状態 | 優先度 |
|----------|--------|------|--------|
| `--related` 検索 | CommandIndex | ✅ 済 | - |
| `commandmate capture --json` | CommandMate | ✅ 済 | - |
| 変更ファイルと影響範囲の自動突合 | CommandMate | ❌ 未 | **高** |
| 迷走検出時の自動介入 | CommandMate | ❌ 未 | 中 |

**対応:**
- **新機能: `/orchestrate` Phase 3 への迷走検出ステップ追加** — `commandmate capture` でエージェントの変更ファイルを取得し、`commandindex search --related` で期待される影響範囲と突合
- `/orchestrate` コマンドの進捗監視ループ内に組み込み

---

### シナリオ8: 設計突合の自動化

| 必要機能 | 実装先 | 状態 | 優先度 |
|----------|--------|------|--------|
| `--related` の path 出力 | CommandIndex | ✅ 済 | - |
| 複数検索結果の集合演算（共通ファイル検出） | CommandIndex | ❌ 未 | 中 |

**対応:**
- **新機能: `commandindex diff` サブコマンド** — 2つのファイルの影響範囲を比較し、共通ファイルを検出
  ```bash
  commandindex diff --related file_a.rs --related file_b.rs --format json
  ```
- 代替: シェルスクリプトで `comm -12` を使う（既存機能で可能だが UX が悪い）

---

### シナリオ9: 過去の設計判断の活用

| 必要機能 | 実装先 | 状態 | 優先度 |
|----------|--------|------|--------|
| Semantic検索 + パスフィルタ | CommandIndex | ✅ 済 | - |
| dev-reports/ のインデックス化 | CommandIndex | ✅ 済 | - |

**対応:** 既存機能で実現可能。AIとのブレスト時に `commandindex search --semantic "..." --path dev-reports/` を呼び出すだけ。

---

### シナリオ10: リリース前のリグレッション影響分析

| 必要機能 | 実装先 | 状態 | 優先度 |
|----------|--------|------|--------|
| `--related` の複数ファイル一括検索 | CommandIndex | ❌ 未 | **高** |
| パイプ入力対応（stdin） | CommandIndex | ❌ 未 | 中 |
| `/release` コマンドへの影響分析ステップ追加 | CommandMate | ❌ 未 | 中 |

**対応:**
- **新機能: `--related` の複数ファイル対応** — `commandindex search --related file1.rs --related file2.rs` or stdin からファイルリストを受け取り
- **新機能: `commandindex impact` サブコマンド** — git diff の出力を受け取り、各変更ファイルの影響範囲を一括分析
  ```bash
  git diff main...develop --name-only | commandindex impact --format json
  ```

---

### シナリオ11-15: 開発以外の活用

| 必要機能 | 実装先 | 状態 | 優先度 |
|----------|--------|------|--------|
| マルチリポジトリ横断検索 | CommandIndex | ✅ 済 | - |
| Semantic検索 | CommandIndex | ✅ 済 | - |
| CMATE.md cron実行 | CommandMate | ✅ 済 | - |
| 非コードファイル（Markdown中心）のインデックス | CommandIndex | ✅ 済 | - |

**対応:** 既存機能の組み合わせで概ね実現可能。特別な新機能は不要。

---

## 3. 必要な新機能一覧（優先度順）

### CommandIndex 側

| # | 機能 | 対応シナリオ | 優先度 | 実装規模 |
|---|------|-------------|--------|---------|
| CI-1 | `impact` サブコマンド（git diffベースの影響分析） | 5, 10 | **高** | 中 |
| CI-2 | `--related` の複数ファイル対応 | 8, 10 | **高** | 小 |
| CI-3 | `--changed-since` オプション（Git履歴ベースの変更検索） | 5 | **高** | 中 |
| CI-4 | `--index-path` オプション（インデックスパス指定） | 3 | **高** | 小 |
| CI-5 | `diff` サブコマンド（影響範囲の比較・共通ファイル検出） | 8 | 中 | 中 |
| CI-6 | stdin パイプ入力対応 | 10 | 中 | 小 |
| CI-7 | ファイル変更監視（watchモード） | 3 | 低 | 大 |

### CommandMate 側（スラッシュコマンド改修）

| # | 機能 | 対応シナリオ | 優先度 | 実装規模 |
|---|------|-------------|--------|---------|
| CM-1 | スラッシュコマンド内での CommandIndex 呼び出し統合 | 1, 2, 6, 7, 8 | **高** | 中 |
| CM-2 | `/briefing` コマンド新規作成 | 5 | **高** | 中 |
| CM-3 | `/orchestrate` Phase 3 に迷走検出ステップ追加 | 7 | **高** | 中 |
| CM-4 | `/orchestrate` Phase 4 に CommandIndex ベースの自動突合 | 8 | **高** | 中 |
| CM-5 | `/release` にリグレッション影響分析ステップ追加 | 10 | 中 | 小 |
| CM-6 | `/issue-create` に類似Issue検出ステップ追加 | 6 | 中 | 小 |
| CM-7 | `/issue-enhance` に CommandIndex 文脈注入 | 1 | 中 | 小 |
| CM-8 | エージェント完了時の自動 `commandindex update` フック | 3 | 低 | 小 |

---

## 4. 実現可能性マトリクス

| シナリオ | 既存機能で実現 | 軽微な改修で実現 | 新機能が必要 |
|---------|--------------|----------------|------------|
| 1. Issue補完精度向上 | △ | ✅（CM-1, CM-7） | |
| 2. レビュー観点自動生成 | △ | ✅（CM-1） | |
| 3. 並列エージェント共有知識 | | △ | ✅（CI-4, CM-8） |
| 4. マルチリポジトリ横断検索 | ✅ | | |
| 5. 朝のブリーフィング | | | ✅（CI-1, CI-3, CM-2） |
| 6. Issue重複検出 | △ | ✅（CM-6） | |
| 7. 迷走の早期検出 | | | ✅（CM-3） |
| 8. 設計突合の自動化 | △ | ✅（CI-2, CM-4） | |
| 9. 過去の設計判断活用 | ✅ | | |
| 10. リグレッション影響分析 | | | ✅（CI-1, CI-2, CM-5） |
| 11-15. 開発以外の活用 | ✅ | | |

---

## 5. 推奨実装順序

### Phase A: スラッシュコマンド連携（即効性が高い）
1. **CM-1**: スラッシュコマンド内での CommandIndex 呼び出し統合
2. **CM-7**: `/issue-enhance` に文脈注入
3. **CM-6**: `/issue-create` に類似Issue検出
→ シナリオ 1, 2, 6, 9 が実現

### Phase B: CommandIndex 新機能（連携の基盤）
4. **CI-2**: `--related` の複数ファイル対応
5. **CI-3**: `--changed-since` オプション
6. **CI-1**: `impact` サブコマンド
7. **CI-4**: `--index-path` オプション
→ シナリオ 3, 5, 8, 10 の基盤が整う

### Phase C: 高度な連携（自動化・監視）
8. **CM-2**: `/briefing` コマンド
9. **CM-3**: `/orchestrate` 迷走検出
10. **CM-4**: `/orchestrate` 自動突合
11. **CM-5**: `/release` リグレッション分析
→ シナリオ 5, 7, 8, 10 が完全実現

### Phase D: 最適化（UX向上）
12. **CI-5**: `diff` サブコマンド
13. **CI-6**: stdin パイプ入力対応
14. **CM-8**: 自動 update フック
15. **CI-7**: watchモード
→ 全シナリオの操作性が向上
