# Testing strategy (t-wada style)

## 原則
- 仕様はテストで固定する（特にエラーメッセージと位置情報）
- 小さく赤→緑→リファクタ（1コミット1価値）

## レベル分け
### 1) Lexer unit tests
- 入力 → Token列
- 改行がseparatorであることを固定

### 2) Parser unit tests
- 入力 → AST
- 期待ASTはstructで比較（最初はこれが一番強い）

### 3) Typechecker unit tests (予定)
- AST → 型付きAST or エラー
- エラーメッセージは `insta` でスナップショット固定

### 4) Codegen tests (予定)
- 型付きAST → wasm bytes
- `wasmparser` でvalidate

### 5) E2E (Node)
- ソース → wasm → Node実行 → stdout検証
- `cargo test` から `node` を `Command` で起動して黒箱テスト

### 6) E2E (Browser)
- Playwright等でconsole出力/DOM出力を検証（後）

## 生成物の自動更新
- スナップショット更新は `cargo insta accept` を使う
- 将来的に `wasmchi test --update` でgolden更新も提供
