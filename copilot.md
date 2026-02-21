# Copilot Notes (wasmchi)

このファイルは「これから完全に実用的な言語にする」ための **仕様の中核** と **TODOの母艦**。
（実装しながら随時更新する前提）

## Vision / 目標

- **TS×Vの融合**：読みやすい + 書きやすい
- **Wasmを第一級ターゲット**：pure wasm を出す（ホストは薄く）
- **JS/TSエコシステムに寄り添う**：npmの資産を型付きで呼べる
- **Bunっぽい all-in-one 体験**：`run/build/bundle/test` を一つに

## Current State (ざっくり現状)

### 言語
- 改行が文区切り（セミコロン不要）
- `export fn` / `fn` / `import fn` / `let` / `:=` / `return` / `print(...)`
- `i32`, `f64`, `string`, `void`（ユーザー定義fnの戻り値は今は `i32`）
- npm auto import（Node）: `import { x } from "npm:pkg"` を runner が変換

### FFI / ホスト
- `string` param: `(ptr,len)` lowering
- `string` return: `(ptr,len)` lowering（multi-value）
- `__alloc(len)` を wasm export（ホストが文字列返却時に確保して書き込む）
- Node host glue:
  - `--host host.mjs` で `export default { ... }`
  - import fn の型を見て、stringの encode/decode + alloc/write を自動ラップ

### npm auto (Node)
- `bun install` 優先
- `.d.ts` を雑に読んで `import fn` を生成
- optional params は「呼び出し側の引数個数」を雑スキャンして選ぶ
- directives:
  - `// wasmchi:number=i32`
  - `// wasmchi:sig name(...): ret`

## Specs (v0/v1 方向性)

### 1) Syntax
- TS風
  - `fn add(a: i32, b: i32): i32 { ... }`
  - `export fn main(): i32 { ... }`
- V風
  - `fn add(a i32, b i32) i32 { ... }`
  - `x := expr`
- 将来（v1+）:
  - `if/else`, `while`, 比較演算, `bool`
  - `struct`, `enum`（どこまでやるか要設計）

### 2) Types
- v0: `i32`, `f64`, `string`, `void`
- v1候補:
  - `bool`
  - `i64`
  - `f32`
  - `array/slice`（hot-loop の核）

### 3) ABI (FFI)
- `string` => UTF-8 `(i32 ptr, i32 len)`
- `string` return => `(i32 ptr, i32 len)`
  - hostは `__alloc(len)` で確保して `memory` に書き込み
- 今後の必須:
  - `Result`/例外相当のエラー伝搬
  - `nullable`/`option` の表現

## TODO (実用レベル=v1.0 まで)

### A. 言語コア
- [ ] `bool` + 比較演算（`== != < <= > >=`）
- [ ] `if/else`
- [ ] `while`
- [ ] ユーザー定義関数の戻り値 `void` / `f64` 解禁（今はi32固定）
- [ ] 変数の型（いまは型検査ほぼ無し）
- [ ] エラーメッセージ改善（line/col、原因が一意）

### B. Codegen / Wasm
- [ ] `f64` リテラル（`1.23`）と演算（`f64.add`等）
- [ ] `f64` の print（まずは host に `print_f64` を追加してもいい）
- [ ] `__alloc` をまともに（境界/アライメント/データセグと衝突しない）
- [ ] data segment の配置管理（4096固定を廃止）

### C. npm auto import（Node）
- [ ] `.d.ts` 解析の精度アップ
  - `export default function` / `export default const` / `export =` 系
  - overload 選択ロジック（args型/optional まで）
  - `Promise<T>` をどう扱うか（まず弾く）
- [ ] `import { x as y }` のalias対応
- [ ] `import * as ns`（要るか？）
- [ ] bun lock/依存の扱い方針（sampleはlockコミットしない）

### D. Browser
- [ ] browser bundle の自動E2E（Playwright等）
- [ ] npm auto import の browser 対応（バンドル必須）

### E. Tooling / UX
- [ ] `wasmchi fmt`
- [ ] `wasmchi test`（golden更新含む）
- [ ] `wasmchi init`（npm auto用の雛形生成）

## Repo Cleanup Policy

- `target/` や `node_modules/` は **絶対にコミットしない**
- サンプルは `.gitignore` を置いて依存を追跡しない
- 旧実装/実験コードを残す場合は `legacy/` に隔離して、READMEに明記

---

## 実用言語として使える (v1.0) ための要件（ガチ版）

### 0) “実用” の定義（最低ライン）
- **落ちない**: panic/unwrap で落ちない（入力が壊れてても「エラーで返す」）
- **遅くない**: コンパイルが体感速い（小さなファイルは瞬時、npm autoでも待たせすぎない）
- **困らない**: エラーが読める（line/col + 期待してたトークン/型 + 直し方のヒント）
- **再現できる**: tests が仕様をロックしてる（壊したら即赤）

### 1) 言語コア v1.0 必須（「日常コード書ける」）
- 文法
  - [ ] `if / else if / else`
  - [ ] `while`
  - [ ] 比較演算 `== != < <= > >=`（i32/f64/bool）
  - [ ] 論理演算 `&& || !`（短絡評価は必須）
  - [ ] ブロックスコープ（`let` の shadowing ルールを決めて固定）
  - [ ] 関数呼び出し（`foo(a,b)`）
  - [ ] （JS interop 用）`expr.prop` と `expr.prop()` の最小サポート
- 型
  - [ ] `i32, f64, bool, string, void, jsobj`
  - [ ] 関数の引数/戻り値の型チェック（少なくとも「間違いは落とす」）
  - [ ] `number` の方針: デフォ `f64`、必要なら directive で i32
- 実行
  - [ ] `main()` の戻り値: `i32|f64|void|bool`（runner側の表示ルールも固定）

### 2) Wasm/ABI v1.0 必須（「動く・壊れない」）
- [ ] data segment と heap の衝突を防ぐ（固定 4096 廃止）
- [ ] `__alloc` の仕様固定（アライメント/上限/エラー時の挙動）
- [ ] `string` ABI: UTF-8 `(ptr,len)`
- [ ] `string` return ABI: multi-value `(ptr,len)` + `__alloc` で host が書き込む
- [ ] host imports の index/順序依存を排除（今の conditional import を維持）

### 3) JS Interop v1.0 必須（「npmで物を作れる」）
#### 3.1 目標
- wasmchi 側から **npm/JS の “関数” と “オブジェクト”** を扱える
- Node first（Browser は v1.1 以降で良い）

#### 3.2 ランタイムの基本（handle table 方式）
- `jsobj` は **opaque i32 handle**
- Node host 側で `Map<i32, any>` を持つ
- 必須プリミティブ（最小）
  - [ ] `env.js_get(obj: jsobj, prop: string) -> jsobj`
  - [ ] `env.js_call0(fn: jsobj, this: jsobj) -> jsobj`（まずは 0 引数だけ）
  - [ ] `env.js_to_bool(x: jsobj) -> bool`（最初は truthy 判定でもOK）
  - [ ] `env.js_to_string(x: jsobj) -> string`
  - [ ] `env.js_drop(x: jsobj) -> void`（リーク許容でも v1.0 で入れたい）
- スコープ
  - v1.0 は **同期のみ**（Promise/async は明確にエラーで弾く）

#### 3.3 言語側の表現（最小）
- `import fn`（既存）
- **オブジェクト import**
  - [ ] source は TS import で書く: `import { z } from "npm:zod"`
  - [ ] runner が `import val z: jsobj` に落とす（言語コアに `import val` を入れるならここ）
  - もしくは v1.0 は「runner内部で `z` を特別扱いして `jsobj` handle を渡す」でも良い

### 4) npm auto import v1.0 必須（d.ts “全対応” の定義と方針）

#### 4.1 “全対応” の定義
- **TypeScript の型システムを全部 wasmchi に持ち込む**のは無理なので、ここでの「全対応」はこう定義する：
  - `.d.ts` の構文を **落とさず解析**できる（TS AST を使う/自前パーサでもOK）
  - ただし wasmchi が理解できない型は **安全に縮退**して扱う（`unknown`/`jsobj`/拒否）
  - そして「何が縮退されたか」を **エラー/警告で可視化**できる

#### 4.2 実装方針（現実）
- v1.0 では **TypeScript compiler API** を使うのが最短（自前 `.d.ts` 解析は地獄）
  - Node/Bun で `typescript` を呼ぶ（CLI が Rust でも、解析だけは node subprocess で良い）
  - 解析結果は JSON で Rust に返す

#### 4.3 `.d.ts` で拾うべき “エクスポート形” 全部
- [ ] `export function foo(...)` / `declare function foo(...)`
- [ ] `export const foo: (...) => ...`
- [ ] `export default function` / `export default const` / `export default class`
- [ ] `export =`（CommonJS） + `import x = require('...')`
- [ ] `export { a as b }` alias
- [ ] `export * from` 再エクスポート（追跡して最終シンボルへ）
- [ ] `namespace` / `declare namespace`（`zod` みたいな静的メンバ塊に出る）

#### 4.4 型のマッピング（wasmchi へ落とすルール）
- 1st-class で扱う
  - `string` -> `string`
  - `number` -> `f64`（directive で i32 可）
  - `boolean` -> `bool`
  - `void` -> `void`
- それ以外は原則 `jsobj` に縮退
  - `object`, `Record<...>`, `T`, `unknown`, `any`, union, intersection, literal types など
- **拒否する型**（v1.0）
  - `Promise<T>` / `AsyncIterable` / callback で非同期確定
  - `bigint`（i64 を入れるまで）
  - `symbol`

#### 4.5 overload 解決（最重要）
- [ ] `n` 個 overload がある場合に、wasmchi の **呼び出し側**から最適1つに決める
- v1.0 の最小アルゴリズム
  - 1) 引数個数で絞る（optional/rest を考慮）
  - 2) wasmchi 側の型（i32/f64/string/bool/jsobj）で一致度スコア
  - 3) 同点は「より具体」優先（string literal > string > jsobj みたいな）
  - 4) 決まらない場合は警告して “最も安全” を選ぶ or ビルド失敗（方針を固定）

#### 4.6 失敗時の挙動（ここがUX）
- 解析できない/型が決められない場合
  - [ ] `--strict-ffi` ならビルド失敗
  - [ ] デフォルトは `jsobj` へ縮退 + 警告
- 「関数が取れない」場合（例: `z` は object）
  - [ ] `import val` を生成して LinkError を回避

### 5) CLI/Tooling v1.0 必須
- [ ] `wasmchi run`（Node）: npm auto と host injection を統合して壊れない
- [ ] `wasmchi bundle --target browser`: 最低限動く（npm auto は v1.1 でもOK）
- [ ] `wasmchi test`: Rust の tests + サンプル E2E を 1 コマンドで
- [ ] `wasmchi doctor`: bun/node/wasm validator などの依存チェック

### 6) 品質ゲート（リリース条件）
- [ ] `cargo test` 常時 green
- [ ] npm auto E2E: `nanoid`（関数）と `zod`（オブジェクト）両方のサンプルが動く
- [ ] README / docs（EN+JA）に「できること/できないこと/縮退ルール」を明記

### 7) v1.0 で “やらない” と明記して切るもの
- async/await（Promise）
- class/struct の完全対応
- GC/参照カウントの完全実装（`js_drop` が最低限）
- Browser npm auto（まず Node で勝つ）
