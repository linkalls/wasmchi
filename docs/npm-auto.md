# npm auto imports (Node runner)

## Goal
Allow TS-like imports from npm **without writing a host module**.

```wasmchi
import { nanoid } from "npm:nanoid"

export fn main(): i32 {
  print(nanoid(10))
  return 0
}
```

## How it works (v0)
When you run:

```bash
wasmchi-cli run --target node main.wm
```

and the source contains `import { ... } from "npm:<pkg>"`, the runner will:

1) Ensure a local npm project exists in the `.wm` directory (`package.json`)
2) Run **`bun install`** (preferred) to materialize `node_modules`
3) Read the package `.d.ts` (heuristic: `node_modules/<pkg>/index.d.ts`)
4) Infer a tiny FFI signature subset (currently: `string | number | void`, where `number` => `f64`)
5) Rewrite the source by prepending `import fn ...` declarations
6) Generate a temporary host module that imports from npm and exposes functions via `env.*`

## Limits (current)
- Only a small set of TS types is supported (`string`, `number`, `void`).
- `number` is lowered to `i32` for now.
- Optional params (e.g. `size?: number`) are supported in a simple way:
  - if you call it with 0 args, runner picks the 0-arg signature
  - if you call it with 1 arg, runner picks the 1-arg signature
  (call-site arity is inferred with a naive scan)

## Samples
- `samples/npm-auto/` (no host.mjs)
- `samples/npm-host/` (manual --host)
