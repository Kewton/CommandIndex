# Issue #169 マルチステージIssueレビュー サマリーレポート

## Issue概要
- **タイトル**: issue listサブコマンドの追加
- **目的**: インデックス内のIssue一覧を表示するCLIコマンドの追加

## レビュー実施状況

| Stage | 種別 | 実行者 | Must Fix | Should Fix | Nice to Have |
|-------|------|--------|----------|------------|--------------|
| 0.5 | 仮説検証 | Claude | - | - | - |
| 1 | 通常レビュー（1回目） | Claude opus | 2 | 3 | 2 |
| 2 | 指摘反映（1回目） | Claude sonnet | - | - | - |
| 3 | 影響範囲レビュー（1回目） | Claude opus | 3 | 3 | 1 |
| 4 | 指摘反映（1回目） | Claude sonnet | - | - | - |
| 5 | 通常レビュー（2回目） | Codex (gpt-5.4) | 3 | 4 | 1 |
| 6 | 指摘反映（2回目） | Claude sonnet | - | - | - |
| 7 | 影響範囲レビュー（2回目） | Codex (gpt-5.4) | 3 | 4 | 1 |
| 8 | 指摘反映（2回目） | Claude sonnet | - | - | - |

## 仮説検証結果

| 仮説 | 判定 |
|------|------|
| `issue` コマンド存在 | Confirmed |
| `knowledge_edges` に `issue_number` カラム | Rejected（knowledge_nodesでJOIN） |
| `--format` オプション実装 | Confirmed |
| 設計書ファイルからラベル抽出 | Confirmed（DocSubtypeメタデータ経由） |

## 主要な改善点（レビューで追加・修正された項目）

### CLI設計（Stage 1で確定）
- サブコマンド構造化: `issue list` + `issue show <number>`
- IssueCommands enum パターン（Configコマンド踏襲）

### データ取得（Stage 5で修正）
- SQL条件にrelation/typeフィルタ追加（modifies除外）
- label fallback を knowledge_nodes.title から空文字固定に変更

### 影響範囲（Stage 3, 7で拡充）
- suggest.rs が breaking change の影響を受けることを発見・追加
- help_llm.rs の全参照箇所更新を明記
- SymbolStore 単体テスト追加の必要性を特定

### 受け入れ基準（段階的に17項目まで拡充）
- 0件時の挙動、symbols.db未作成時の挙動を具体化
- JSON出力に has_review, has_progress を追加
- 旧構文残存チェック、clap部分一致テスト等を追加

## 最終Issue状態
- 受け入れ基準: 17項目
- 影響ファイル: 7ファイル
- Breaking Change: `issue <number>` → `issue show <number>`（完全廃止）
