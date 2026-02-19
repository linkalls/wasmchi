import fs from 'node:fs/promises';

const wasmBytes = await fs.readFile('out/web.wasm');

const imports = {
  env: {
    host_print_i32(n) {
      console.log('[wasm]', n | 0);
    },
  },
};

const { instance } = await WebAssembly.instantiate(wasmBytes, imports);
instance.exports.main();
