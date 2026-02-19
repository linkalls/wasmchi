#!/usr/bin/env bash
set -euo pipefail

# Build wasmchi-rs and compile a vsignal sample to vitrio/src/vsignal.wasm
# Usage:
#   ./scripts/build-vitrio-vsignal.sh [sample_path]
# Default:
#   samples/vsignal-vopt-uc.wm

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
SAMPLE_REL=${1:-samples/vsignal-vopt-uc.wm}
SAMPLE="$ROOT_DIR/$SAMPLE_REL"
VITRIO_DIR=${VITRIO_DIR:-/home/poteto/clawd/vitrio}
OUT_WASM="$VITRIO_DIR/src/vsignal.wasm"

if [[ ! -f "$SAMPLE" ]]; then
  echo "sample not found: $SAMPLE" >&2
  exit 1
fi

echo "[1/3] build wasmchi-rs (release)"
cd "$ROOT_DIR/wasmchi-rs"
source "$HOME/.cargo/env" >/dev/null 2>&1 || true
cargo build --release

echo "[2/3] compile $SAMPLE_REL -> vitrio/src/vsignal.wasm"
"$ROOT_DIR/wasmchi-rs/target/release/wasmchi" \
  --input "$SAMPLE" \
  --out "$OUT_WASM" \
  --target web \
  --export-start=true

echo "[3/3] sanity check exports"
WASM_PATH="$OUT_WASM" node - <<'NODE'
const fs = require('fs');
const wasmPath = process.env.WASM_PATH;
const bytes = fs.readFileSync(wasmPath);
const mod = new WebAssembly.Module(bytes);
const ex = WebAssembly.Module.exports(mod).map(e=>e.name);
console.log('exports:', ex.join(', '));
NODE

BYTES=$(wc -c < "$OUT_WASM" | tr -d ' ')
echo "done: $OUT_WASM ($BYTES bytes)"
