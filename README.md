# wasmchi

A tiny language that compiles directly to **pure WebAssembly**.

The project is evolving toward a **TS × V hybrid syntax** (TypeScript readability + V-ish ergonomics), while keeping:

- **Small** wasm output
- **Fast** runtime (especially for hot-loop code)
- **Minimal** runtime assumptions (Wasm + a tiny host)

## Status

Experimental. The Rust rewrite is under active development and currently supports a small but working subset.

## Implementations

This repo currently contains:

- **Rust (new, active):** compiler + CLI in `crates/`
  - `crates/wasmchi` (library)
  - `crates/wasmchi-cli` (CLI)
- **TypeScript (legacy/prototype):** older compiler code still present (will be deprecated once Rust reaches feature parity)

## Language snapshot (Rust v0 subset)

- Semicolon-less: **newline separates statements**
- Top-level:
  - `export fn main(): i32 { ... }` (TS style)
  - `export fn main() i32 { ... }` (V style)
- Statements:
  - `let x = <expr>`
  - `x := <expr>` (V-style short declaration)
  - `return <expr>`
  - `print("...")` (currently: string literal only)
- Expressions:
  - integer literals
  - variables
  - `+ - * /` (i32)
  - string literals
  - string literal concat: `"h" + "i"` (compile-time folding)

## Quickstart (Rust)

### Build

```bash
cargo build -p wasmchi-cli
```

### Run (Node)

```bash
./target/debug/wasmchi-cli run --target node <file.wm>
```

### Bundle (Browser)

```bash
./target/debug/wasmchi-cli bundle --target browser <file.wm> --out-dir dist
cd dist
python3 -m http.server 8000
# open http://localhost:8000/
```

## Example

```wasmchi
export fn main() i32 {
  x := 40
  print("h" + "i")
  return x + 2
}
```

## Docs

See `docs/` for the living spec + roadmap:

- `docs/spec-v0.md`
- `docs/roadmap.md`
- `docs/testing.md`

## License

MIT
