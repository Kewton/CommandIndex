---
model: sonnet
description: "GitHub Issueを作成（テンプレート準拠・品質チェック付き）"
---

# Issue作成スキル

## 概要
CommandIndexプロジェクトの規約に準拠したGitHub Issueを作成するスキルです。

## 使用方法
- `/issue-create [概要説明]`

## 前提条件
- GitHubリポジトリ（https://github.com/Kewton/CommandIndex）にアクセス可能
- `gh` CLIが認証済み

## 実行内容

### 1. Issue種別の判定

| 種別 | ラベル | 説明 |
|------|--------|------|
| 機能追加 | `feature` | 新しい機能の追加 |
| バグ修正 | `bug` | 既存機能の不具合修正 |
| リファクタリング | `refactor` | コード品質の改善 |
| ドキュメント | `documentation` | ドキュメントの追加・更新 |
| パフォーマンス | `performance` | パフォーマンス改善 |

### 2. コードベース調査

対象領域のコードを調査し、Issueに必要な情報を収集。

### 3. Issue本文の作成

#### 機能追加テンプレート

```markdown
## 概要
[機能の簡潔な説明]

## 背景・動機
[なぜこの機能が必要か]

## 技術スタック
- Rust (Edition 2024), Cargo
- 関連モジュール: src/xxx/

## 要件
### 機能要件
- [ ] [要件1]

### 非機能要件
- [ ] `cargo clippy --all-targets` 警告0件
- [ ] `cargo test --all` 全テストパス
- [ ] `cargo fmt --check` 差分なし

## 影響範囲
- 変更対象: `src/xxx/`
- テスト: `tests/xxx.rs`

## 受入条件
- [ ] [条件1]
- [ ] 品質チェック全パス
```

### 4. Issue作成

```bash
gh issue create \
  --repo Kewton/CommandIndex \
  --title "{type}: {description}" \
  --body "{issue_body}" \
  --label "{label}"
```

### 5. 品質チェック

- タイトルが `<type>: <description>` 形式
- 要件・受入条件が具体的
- 影響範囲が特定されている
- 品質基準が含まれている

## 完了条件

- GitHub Issueが作成されている
- テンプレートに準拠した本文
- 適切なラベルが付与されている

## 関連コマンド

- `/issue-enhance`: Issue内容の補完
- `/issue-split`: 大規模Issueの分割
- `/work-plan`: 作業計画立案
- `/pm-auto-issue2dev`: Issue補完から開発まで一括実行
