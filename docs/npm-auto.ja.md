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

を実行して、ソースに `import ... from "npm:<pkg>"` が含まれていると、runnerが裏で次をやる：

1) `.wm` ファイルのディレクトリにローカルnpmプロジェクトを用意（`package.json`）
2) **`bun install`** を実行（bun優先）して `node_modules` を作る
3) パッケージの `.d.ts` を読む（ヒューリスティック: `node_modules/<pkg>/index.d.ts`）
4) 使える範囲だけFFIシグネチャを推論（現状: `string | number | void`。`number` は `f64` に落とす）
5) ソースを書き換えて、先頭に `import fn ...` を自動生成して足す
6) 一時的なhostモジュールを生成して、npmからimportした関数を `env.*` としてWasmに渡す

## 制約（現状）
- TSの型はまだ全部は無理：`string`, `number`, `void` だけ対応。
- `number` は `f64` に落とす。
- optional引数（例: `size?: number`）は簡易対応：
  - 呼び出しが0引数なら0引数シグネチャを選ぶ
  - 呼び出しが1引数なら1引数シグネチャを選ぶ
  （呼び出し側の引数個数はかなり雑なスキャンで推定してる）

## default import
default import は **ベストエフォート**。

```wasmchi
import nanoid from "npm:nanoid"
```

`nanoid` みたいに default export を提供しないパッケージが多いので、今は namespace import して `mod.default ?? 最初のexport` を拾う方式。
基本は named import 推奨。

## 上書き（ディレクティブ）
コメントで推論を誘導できる：

- このファイルでは `number` を `i32` 扱いにしたい：
  ```
  // wasmchi:number=i32
  ```

- 特定シンボルのシグネチャを固定：
  ```
  // wasmchi:sig nanoid(): string
  // wasmchi:sig foo(a0: i32, a1: string): void
  ```

## サンプル
- `samples/npm-auto/`（host.mjs無し）
- `samples/npm-host/`（手動 --host）
