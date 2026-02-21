# Roadmap (実用レベルまで)

優先度: **Web(3) → hot loop(4) → CLI(1) → server/edge(2)**

> NOTE: 「無限に実装」はできないので、実用の定義をマイルストーンで切る。

## Milestone 0: 開発基盤
- [x] Rust workspace
- [x] parser smoke test
- [x] ルール固定: **改行が文区切り**（改行無しは `expected newline`）
- [ ] エラーに行・列（line/col）を付ける

## Milestone 1: i32で計算できる（最小言語）
- [x] `let` statement
- [x] integer literal
- [x] binary `+ - * /`
- [x] `return <expr>`
- [x] `export fn main(): i32 { return 0 }` を codegen
- [x] wasm validate（wasmparser）

## Milestone 2: Node E2E（Webの入口）
- [x] `wasmchi build <file> [--out ...]`（wasm生成）
- [x] `wasmchi run --target node <file>`（main()呼んでstdoutに返り値）
- [x] Nodeホスト自動生成（instantiate + call main）
- [x] E2E: return value を stdout に出してテスト（Rustテストからnode起動）

## Milestone 3: string + print（書きやすさ）
- [x] string literal（"..."、最小エスケープ対応）
- [x] `print("literal")` builtin（v0: リテラルのみ対応）
- [x] string concat (`+`)（v0: リテラル同士はコンパイル時に結合）
- [x] E2E: `print("hi")` が `hi\n`

## Milestone 4: Browser E2E
- [x] `wasmchi bundle --target browser` でESMホスト生成（index.html + app.mjs + out.wasm）
- [x] Browserで `print` が console.log へ（ホスト側print実装）
- [ ] 自動E2E（Playwright等）で固定

## Milestone 5: hot loop 基礎
- [ ] `while` / `for`(desugar)
- [ ] `array` or `slice` の最小
- [ ] bounds-check方針（safe/unsafe境界）

## Milestone 6: WASI/CLI
- [ ] `--target wasi`
- [ ] args/stdout
- [ ] ファイルI/O（後）

## Doneの定義（最低限の実用）
- Node/Browserで同じソースが動く
- 文字列と配列で普通の処理が書ける
- エラーメッセージが読める（位置付き）
