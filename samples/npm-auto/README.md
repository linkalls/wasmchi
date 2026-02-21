# npm-auto sample (no host.mjs)

This sample demonstrates TS-like npm imports without providing a `host.mjs`.

## main.wm
```wasmchi
import { nanoid } from "npm:nanoid"

export fn main(): i32 {
  print(nanoid())
  return 0
}
```

## Run

```bash
cargo build -p wasmchi-cli
./target/debug/wasmchi-cli run --target node samples/npm-auto/main.wm
```

Notes:
- The runner will create/update `samples/npm-auto/package.json` and run `bun install`.
- It auto-generates a host module under the temp dir.
