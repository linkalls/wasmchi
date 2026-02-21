# wasmchi

**pure WebAssembly** に直接コンパイルする、小さくて速いミニ言語。

このプロジェクトは、**TS × V のハイブリッド構文**（TypeScriptの読みやすさ + Vっぽい手触り）を目指しつつ、次を重視して育ててる：

- **小さい** wasm出力
- **速い** 実行（特に hot-loop）
- **最小** のランタイム前提（Wasm + 小さなホスト）

## ステータス

実験段階。Rust版リライトがアクティブ開発中で、まだ小さいサブセットだけど「動く」ところまで来てる。

## 実装

このリポジトリには今これが入ってる：

- **Rust（メイン）**: コンパイラ + CLI（`crates/`）
  - `crates/wasmchi`（ライブラリ）
  - `crates/wasmchi-cli`（CLI）
- **Legacy Rust（旧実装）**: `wasmchi-rs/`（参照/ツール用に残してる）

## 言語スナップショット（Rust v0 サブセット）

- セミコロン無し：**改行が文区切り**
- トップレベル：
  - `export fn main(): i32 { ... }`（TS風）
  - `export fn main() i32 { ... }`（V風）
- 文：
  - `let x = <expr>`
  - `x := <expr>`（V風短縮宣言）
  - `return <expr>`
  - `print("...")`（現状: 文字列リテラル中心）
- 式：
  - 整数リテラル
  - 変数
  - `+ - * /`（i32）
  - 文字列リテラル
  - 文字列リテラルの連結：`"h" + "i"`（コンパイル時結合）

## Quickstart（Rust）

### ビルド

```bash
cargo build -p wasmchi-cli
```

### 実行（Node）

```bash
./target/debug/wasmchi-cli run --target node <file.wm>
```

### 実行（Node / npm自動import：host.mjs無し）

```wasmchi
import { nanoid } from "npm:nanoid"

export fn main(): i32 {
  print(nanoid(10))
  return 0
}
```

```bash
./target/debug/wasmchi-cli run --target node <file.wm>
# runnerが: package.json作成/更新 + bun install + 自動host生成 をやる
```

### バンドル（Browser）

```bash
./target/debug/wasmchi-cli bundle --target browser <file.wm> --out-dir dist
cd dist
python3 -m http.server 8000
# open http://localhost:8000/
```

## 例

```wasmchi
export fn main() i32 {
  x := 40
  print("h" + "i")
  return x + 2
}
```

## Docs

`docs/` に仕様とロードマップがある：

- `docs/spec-v0.md`
- `docs/roadmap.md`
- `docs/testing.md`
- `docs/ffi-node.md`
- `docs/npm-auto.md`

日本語版：
- `docs/ffi-node.ja.md`
- `docs/npm-auto.ja.md`

## License

MIT
