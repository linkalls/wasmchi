# wasmchi

A tiny, V-ish toy language that compiles directly to **pure WebAssembly**.

Current goals:
- **Small** wasm output
- **Fast** runtime (especially for hot-loop code)
- **Simple** compiler (TypeScript)

## Status

This is an experimental project.

## Language snapshot

- Types: `i32`, `void`
- Top-level: `const`, `export fn`
- Control flow: `if/else`, `while`, `return`
- Low-level memory builtins:
  - `load_i32(ptr)`
  - `store_i32(ptr, val)`

The compiler always exports a WebAssembly memory as `memory`.

## Build

```bash
npm i
npm run build
```

## Compile

```bash
node dist/cli.js build samples/vsignal-const.wm --target web --out out/vsignal.wasm
```

## Bench (Rust / wasmtime)

Bench harness lives in the `vitrio` repo right now.

## License

MIT
