# wasmchi spec v0 (draft)

この仕様は「最初に実用に届く」こと優先で、あとから拡張しやすい最小セット。

## 1. Goals
- Semicolon不要（**改行区切り**）
- TS寄りの読みやすい構文
- 型は強い（`any` なし）
- Webターゲット最優先（Node + Browser）

## 2. Lexical
- トークン分割は基本 whitespace。
- **改行は意味を持つ**（statement separator）。
- v0は「1行=1文」を基本にして曖昧さを避ける。

## 3. Syntax (v0)
### Top-level
- Import:
  - `import fn <name>(<params>): <type>`
  - `import fn <name>(<params>) <type>`
  - module名は v0 では固定で `env`（Node/Browserホスト側の `env.<name>` に繋がる）
  - ABI (v0): `string` は wasm import 上は `(i32 ptr, i32 len)` にlowering
- Function:
  - TS風: `fn <name>(<params>): <type> { <stmts> }`
  - V風:  `fn <name>(<params>) <type> { <stmts> }`
- Export:
  - `export fn ...`（上のfunction構文に `export` を付ける）

paramsはTS風 `a: i32` / V風 `a i32` どっちも受理する（v0は混在も許すけど、後で締めるかも）。

例:
```wasmchi
export fn main(): i32 {
  let x = 40
  return x + 2
}
```

### Statements
- `let <name> = <expr>`
- `<name> := <expr>`（V風の短縮宣言。v0は `let` と同じ扱い）
- `return <expr>`（v0は同一行のみ許可）
- `print(<expr>)`（v0: まずは **文字列リテラルのみ** 対応）
- `<expr>`（式文。副作用用。いまは値を捨てる）

### Expressions
- 整数リテラル（10進）
- 文字列リテラル（`"..."`。最小エスケープ: `\n \t \\ \"`）
- 変数参照
- 関数呼び出し: `<name>(a, b, ...)`
- `+ - * /`（i32）
- `+`（string: v0は **リテラル + リテラルのみ** コンパイル時結合）
- 括弧 `(<expr>)`
- 優先順位: `* /` > `+ -`

## 4. Types (v0)
- `i32`
- `void`（import用に導入。ユーザー定義fnの戻り値は今はi32のみ）
- `string`（現状: print/importでリテラルのみ）

拡張予定:
- `bool, i64, f32, f64`

## 5. Execution targets
- `wasmchi run --target node <file>`: Nodeホスト生成して実行
- `wasmchi bundle --target browser <file> [--out-dir dist]`: `index.html` + `app.mjs` + `out.wasm` を生成（httpでserveして開く）

## 6. Safety model (予定)
- 低レベルメモリアクセスは `unsafe { ... }` に閉じ込める
- 通常のコードは境界チェック/型安全寄り
