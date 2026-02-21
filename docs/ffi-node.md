# Node host FFI (draft)

## Goal
Use npm packages from the **Node host** and expose selected functions to wasmchi via `env.*` imports.

This keeps the wasm module pure/minimal, while letting you reuse the entire JS ecosystem.

## Current state (implemented)
`wasmchi-cli run --target node` supports an optional `--host <path>`.

- The host module is loaded with dynamic `import()`.
- It should default-export an object of functions.
- Those functions are merged into the `env` import object alongside the built-in `print`.

### CLI
```bash
wasmchi run --target node <file.wm> --host host.mjs
```

### host.mjs example
```js
import { ulid } from 'ulid'

export default {
  // numeric only for now (wasmchi v0 has i32)
  now_i32: () => (Date.now() | 0),
  ulid_len: () => ulid().length | 0,
}
```

> npm resolution: `node` is executed with `cwd` set to the directory of `<file.wm>`.
> Put your `package.json` + `node_modules` there (or run from a project dir).

## Next steps
- (done) Add `import fn ...` syntax in wasmchi source (v0: module is fixed to `env`)
- Typecheck imports/exports
- (partial) string args for imports: `string` lowers to `(i32 ptr, i32 len)` (currently literal-only)
- Next: string returns, non-literal strings, and safe allocation strategy
- Node host: `--host` のJS関数は、wasmchiが `string` import を検出した場合 **自動で ptr/len → JS string にデコードして渡す**（つまり host 側は `js_log(msg)` でOK）
