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
