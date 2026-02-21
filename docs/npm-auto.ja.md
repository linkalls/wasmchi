# npm 自動 import（Node runner）

## 目的
TSっぽい import を書くだけで、**host.mjsを書かずに** npm パッケージを使えるようにする。

```wasmchi
import { nanoid } from "npm:nanoid"

export fn main(): i32 {
  print(nanoid(10))
  return 0
}
```

## 仕組み（v0）

```bash
wasmchi-cli run --target node main.wm
```

を実行して、ソースに `import { ... } from "npm:<pkg>"` が含まれていると、runnerが裏で次をやる：

1) `.wm` ファイルのディレクトリにローカルnpmプロジェクトを用意（`package.json`）
2) **`bun install`** を実行（bun優先）して `node_modules` を作る
3) パッケージの `.d.ts` を読む（ヒューリスティック: `node_modules/<pkg>/index.d.ts`）
4) 使える範囲だけFFIシグネチャを推論（現状: `string | number | void`）
5) ソースを書き換えて、先頭に `import fn ...` を自動生成して足す
6) 一時的なhostモジュールを生成して、npmからimportした関数を `env.*` としてWasmに渡す

## 制約（現状）
- TSの型はまだ全部は無理：`string`, `number`, `void` だけ対応。
- `number` は今は `i32` に落としてる（将来 `f64` も欲しい）。
- optional引数（例: `size?: number`）は簡易対応：
  - 呼び出しが0引数なら0引数シグネチャを選ぶ
  - 呼び出しが1引数なら1引数シグネチャを選ぶ
  （呼び出し側の引数個数はかなり雑なスキャンで推定してる）

## サンプル
- `samples/npm-auto/`（host.mjs無し）
- `samples/npm-host/`（手動 --host）
