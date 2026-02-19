import fs from 'node:fs/promises';

// This runs our WASI-targeted wasm *without* full WASI.
// We only need env.host_print_i32 + a dummy memory if required.
// For real WASI, use wasmtime: wasmtime out/wasi.wasm

const wasmBytes = await fs.readFile('out/wasi.wasm');

const imports = {
  env: {
    host_print_i32(n) {
      console.log('[wasi/wasm]', n | 0);
    },
  },
};

const { instance } = await WebAssembly.instantiate(wasmBytes, imports);
// WASI convention expects _start
instance.exports._start();
