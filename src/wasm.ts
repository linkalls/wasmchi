import { Bytes, encodeString } from './bytes.js';

// Minimal WebAssembly binary emitter for a tiny subset.

const WASM_MAGIC = [0x00, 0x61, 0x73, 0x6d];
const WASM_VERSION = [0x01, 0x00, 0x00, 0x00];

enum Section {
  Type = 1,
  Import = 2,
  Function = 3,
  Memory = 5,
  Export = 7,
  Code = 10,
}

enum ValType {
  i32 = 0x7f,
}

enum ExternalKind {
  Func = 0x00,
  Memory = 0x02,
}

enum Op {
  // control
  unreachable = 0x00,
  block = 0x02,
  loop = 0x03,
  if_ = 0x04,
  else_ = 0x05,
  br = 0x0c,
  br_if = 0x0d,
  return_ = 0x0f,
  end = 0x0b,

  // calls
  call = 0x10,

  // locals
  local_get = 0x20,
  local_set = 0x21,
  local_tee = 0x22,

  // memory
  i32_load = 0x28,
  i32_store = 0x36,

  // const
  i32_const = 0x41,

  // ops
  i32_eqz = 0x45,
  i32_eq = 0x46,
  i32_ne = 0x47,
  i32_lt_s = 0x48,
  i32_gt_s = 0x4a,
  i32_le_s = 0x4c,
  i32_ge_s = 0x4e,

  i32_add = 0x6a,
  i32_sub = 0x6b,
  i32_mul = 0x6c,
  i32_and = 0x71,
  i32_or = 0x72,
  i32_xor = 0x73,
  i32_shl = 0x74,
  i32_shr_s = 0x75,
}

export type FuncType = { params: ValType[]; results: ValType[] };
export type ImportFunc = { module: string; name: string; typeIndex: number };
export type DefinedFunc = { typeIndex: number; locals: ValType[]; body: Uint8Array };
export type ExportFunc = { name: string; funcIndex: number };
export type ExportMemory = { name: string; memoryIndex: number };


function emitSection(out: Bytes, id: Section, payload: Uint8Array): void {
  out.u8(id);
  out.uleb(payload.length);
  out.bytes(payload);
}

function emitVec<T>(items: T[], emitItem: (b: Bytes, item: T) => void): Uint8Array {
  const b = new Bytes();
  b.uleb(items.length);
  for (const it of items) emitItem(b, it);
  return b.toU8();
}

function emitName(b: Bytes, s: string): void {
  const u8 = encodeString(s);
  b.uleb(u8.length);
  b.bytes(u8);
}

export function buildModule(args: {
  types: FuncType[];
  imports: ImportFunc[];
  funcs: DefinedFunc[];
  exports: ExportFunc[];
  memory?: { initialPages: number; maxPages?: number; exportName?: string };
}): Uint8Array {
  const out = new Bytes();
  out.bytes(WASM_MAGIC).bytes(WASM_VERSION);

  // type section
  const typePayload = emitVec(args.types, (b, t) => {
    b.u8(0x60); // func
    b.uleb(t.params.length);
    for (const p of t.params) b.u8(p);
    b.uleb(t.results.length);
    for (const r of t.results) b.u8(r);
  });
  emitSection(out, Section.Type, typePayload);

  // import section
  const importPayload = emitVec(args.imports, (b, im) => {
    emitName(b, im.module);
    emitName(b, im.name);
    b.u8(ExternalKind.Func);
    b.uleb(im.typeIndex);
  });
  if (args.imports.length > 0) emitSection(out, Section.Import, importPayload);

  // function section
  const funcPayload = emitVec(args.funcs, (b, f) => {
    b.uleb(f.typeIndex);
  });
  emitSection(out, Section.Function, funcPayload);

  // memory section (optional)
  if (args.memory) {
    const memPayload = emitVec([args.memory], (b, m) => {
      const hasMax = typeof m.maxPages === 'number';
      b.u8(hasMax ? 0x01 : 0x00); // limits: 0x00 min, 0x01 min/max
      b.uleb(m.initialPages);
      if (hasMax) b.uleb(m.maxPages!);
    });
    emitSection(out, Section.Memory, memPayload);
  }

  // export section
  const entries: Array<{ kind: 'func' | 'mem'; name: string; index: number }> = [];
  for (const ex of args.exports) entries.push({ kind: 'func', name: ex.name, index: ex.funcIndex });
  if (args.memory?.exportName) entries.push({ kind: 'mem', name: args.memory.exportName, index: 0 });

  const exportPayload = emitVec(entries, (b, ex) => {
    emitName(b, ex.name);
    if (ex.kind === 'func') {
      b.u8(ExternalKind.Func);
    } else {
      b.u8(ExternalKind.Memory);
    }
    b.uleb(ex.index);
  });
  emitSection(out, Section.Export, exportPayload);

  // code section
  const codePayload = emitVec(args.funcs, (b, f) => {
    const fb = new Bytes();
    // locals are grouped by type; we only use i32 so group as one entry
    if (f.locals.length === 0) {
      fb.uleb(0);
    } else {
      fb.uleb(1);
      fb.uleb(f.locals.length);
      fb.u8(f.locals[0]!);
    }
    fb.bytes(f.body);
    // body must end with end
    // (we enforce in codegen but keep it safe)
    if (f.body.length === 0 || f.body[f.body.length - 1] !== Op.end) {
      fb.u8(Op.end);
    }

    const fbU8 = fb.toU8();
    b.uleb(fbU8.length);
    b.bytes(fbU8);
  });
  emitSection(out, Section.Code, codePayload);

  return out.toU8();
}

export const wasm = {
  ValType,
  Op,
  ExternalKind,
};
