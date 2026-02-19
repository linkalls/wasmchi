import fs from 'node:fs/promises';
import path from 'node:path';
import { parse } from './parser.js';
import { compileToWasm, type Target } from './codegen.js';

function usage(): never {
  console.error(
    `wasmchi\n\nUsage:\n  node dist/cli.js build <input.wm> --target <web|wasi> --out <output.wasm>\n\nNotes:\n  - This compiler always exports a WebAssembly memory as \"memory\".\n  - Functions are exported when you write: export fn name(...) ...\n  - For WASI-style startup, export fn _start() { ... } in your source.\n`,
  );
  throw new Error('Invalid arguments');
}

async function main() {
  const argv = process.argv.slice(2);
  const cmd = argv[0];
  if (cmd !== 'build') usage();

  const input = argv[1];
  if (!input) usage();

  let target: Target | null = null;
  let out: string | null = null;

  for (let i = 2; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--target') {
      target = argv[++i] as Target;
    } else if (a === '--out') {
      out = argv[++i] ?? null;
    } else {
      usage();
    }
  }

  if (target !== 'web' && target !== 'wasi') usage();
  if (!out) usage();

  const src = await fs.readFile(input, 'utf8');
  const program = parse(src);
  const wasm = compileToWasm(program, { target });

  await fs.mkdir(path.dirname(out), { recursive: true });
  await fs.writeFile(out, wasm);
  console.log(`Wrote ${out} (${wasm.length} bytes)`);
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
