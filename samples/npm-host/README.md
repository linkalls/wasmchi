# npm-host sample

This sample shows how to call a **regular npm package** function from wasmchi via the Node host.

## Files
- `main.wm`: wasmchi program (imports `nanoid()` and logs it)
- `host.mjs`: Node host module that imports `nanoid` from npm and exposes it to wasm as `env.nanoid`
- `package.json`: local npm project for the sample

## Run

```bash
cd samples/npm-host
npm i

# from repo root
cd ../..
cargo build -p wasmchi-cli

./target/debug/wasmchi-cli run --target node samples/npm-host/main.wm --host samples/npm-host/host.mjs
```

Expected output:
- A line like: `id=<random>`
- Then the return value `0`
