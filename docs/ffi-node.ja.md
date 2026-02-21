# Node host FFI（ドラフト）

## 目的
Node側（ホスト）で npm パッケージを使って、その関数を wasmchi から `env.*` 経由で呼べるようにする。

Wasm本体はピュア/ミニマルに保ちつつ、JSエコシステムを使い倒すための仕組み。

## 現状（実装済み）

### 手動host
`wasmchi-cli run --target node` は `--host <path>` をサポート。

- hostモジュールは dynamic `import()` で読み込む
- `export default { ... }` で関数オブジェクトを返す
- それらの関数は `env` import オブジェクトにマージされる（built-inの `print` も同居）

#### CLI
```bash
wasmchi-cli run --target node <file.wm> --host host.mjs
```

#### host.mjs 例
```js
import { nanoid } from 'nanoid'

export default {
  nanoid: () => nanoid(),
}
```

関連: `samples/npm-host/`

### 自動host（host.mjs無し）
ソースに TSっぽい npm import がある場合：

```wasmchi
import { nanoid } from "npm:nanoid"
```

`wasmchi-cli run --target node <file.wm>` が host を自動生成して依存も入れる（bun優先）。

関連: `samples/npm-auto/` と `docs/npm-auto.ja.md`

> npm解決について：node実行時の `cwd` は `<file.wm>` のあるディレクトリになる。
> そこに `package.json` / `node_modules` があれば普通に解決できる。

## ABIメモ（v0）
- `string` param は `(i32 ptr, i32 len)` に lowering（UTF-8）
- `string` return も `(i32 ptr, i32 len)`（multi-value）
  - hostは `__alloc(len)` で確保して書き込む

## 次の予定
- import/exportの型チェックを強化
- wasmchi内部で非リテラル文字列（変数）を扱えるようにする
- アロケータ/メモリ管理をまともにする
