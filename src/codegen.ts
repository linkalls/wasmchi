import type { ConstDecl, Expr, FnDecl, Program, Stmt, TypeName } from './ast.js';
import { Bytes } from './bytes.js';
import { buildModule, wasm } from './wasm.js';

export type Target = 'web' | 'wasi';

type LocalInfo = { index: number };

type FuncSig = { params: TypeName[]; ret: TypeName };

type ExprKey = string;
function keyOfExpr(e: Expr): ExprKey {
  // Only safe for our pure subset (no side effects besides store_i32 which is a call).
  return JSON.stringify(e);
}

function countExprs(e: Expr, counts: Map<ExprKey, number>): void {
  const k = keyOfExpr(e);
  counts.set(k, (counts.get(k) ?? 0) + 1);
  if (e.kind === 'bin') {
    countExprs(e.left, counts);
    countExprs(e.right, counts);
  } else if (e.kind === 'call') {
    for (const a of e.args) countExprs(a, counts);
  }
}

function countExprsInStmt(s: Stmt, counts: Map<ExprKey, number>): void {
  switch (s.kind) {
    case 'let':
      countExprs(s.expr, counts);
      return;
    case 'assign':
      countExprs(s.expr, counts);
      return;
    case 'expr':
      countExprs(s.expr, counts);
      return;
    case 'return':
      if (s.expr) countExprs(s.expr, counts);
      return;
    case 'if':
      countExprs(s.cond, counts);
      for (const x of s.then) countExprsInStmt(x, counts);
      if (s.elseBranch) for (const x of s.elseBranch) countExprsInStmt(x, counts);
      return;
    case 'while':
      countExprs(s.cond, counts);
      for (const x of s.body) countExprsInStmt(x, counts);
      return;
  }
}

function sigKey(sig: FuncSig): string {
  return `${sig.params.join(',')}->${sig.ret}`;
}

function valTypeOf(t: TypeName): number {
  if (t === 'i32') return wasm.ValType.i32;
  throw new Error(`Unsupported val type: ${t}`);
}

function substituteExpr(expr: Expr, subst: Map<string, Expr>): Expr {
  switch (expr.kind) {
    case 'num':
      return expr;
    case 'var':
      return subst.get(expr.name) ?? expr;
    case 'bin':
      return { kind: 'bin', op: expr.op, left: substituteExpr(expr.left, subst), right: substituteExpr(expr.right, subst) };
    case 'call':
      return { kind: 'call', name: expr.name, args: expr.args.map((a) => substituteExpr(a, subst)) };
  }
}

function patchStmtConsts(st: Stmt, consts: Map<string, Expr>): Stmt {
  const sub = (e: Expr) => foldConstsInExpr(substituteExpr(e, consts));
  switch (st.kind) {
    case 'let':
      return { ...st, expr: sub(st.expr) };
    case 'assign':
      return { ...st, expr: sub(st.expr) };
    case 'expr':
      return { ...st, expr: sub(st.expr) };
    case 'return':
      return { ...st, expr: st.expr ? sub(st.expr) : null };
    case 'if':
      return {
        ...st,
        cond: sub(st.cond),
        then: st.then.map((x) => patchStmtConsts(x, consts)),
        elseBranch: st.elseBranch ? st.elseBranch.map((x) => patchStmtConsts(x, consts)) : null,
      };
    case 'while':
      return { ...st, cond: sub(st.cond), body: st.body.map((x) => patchStmtConsts(x, consts)) };
  }
}

function compileExpr(
  b: Bytes,
  e: Expr,
  locals: Map<string, LocalInfo>,
  funcIndexByName: Map<string, number>,
  builtins: { printImportIndex: number | null },
  cse?: {
    counts: Map<ExprKey, number>;
    temps: Map<ExprKey, number>; // key -> local index
    nextTempLocal: () => number;
  },
): void {
  // CSE experiment is currently disabled (can break semantics across mutations)
  const cseEnabled = false;

  // very small CSE: cache pure expressions used >=2 times
  const pureForCse =
    e.kind === 'num' ||
    e.kind === 'var' ||
    e.kind === 'bin' ||
    (e.kind === 'call' && e.name === 'load_i32');

  const cseKey = cseEnabled && cse && pureForCse ? keyOfExpr(e) : null;
  let teeTmp: number | null = null;
  if (cse && cseKey && (cse.counts.get(cseKey) ?? 0) >= 2) {
    const existing = cse.temps.get(cseKey);
    if (existing != null) {
      b.u8(wasm.Op.local_get).uleb(existing);
      return;
    }
    // compute and tee into temp
    teeTmp = cse.nextTempLocal();
    cse.temps.set(cseKey, teeTmp);
  }

  const maybeTee = () => {
    if (teeTmp != null) b.u8(wasm.Op.local_tee).uleb(teeTmp);
  };

  switch (e.kind) {
    case 'num':
      b.u8(wasm.Op.i32_const).sleb32(e.value);
      maybeTee();
      return;
    case 'var': {
      const li = locals.get(e.name);
      if (!li) throw new Error(`Undefined variable: ${e.name}`);
      b.u8(wasm.Op.local_get).uleb(li.index);
      maybeTee();
      return;
    }
    case 'bin': {
      compileExpr(b, e.left, locals, funcIndexByName, builtins);
      compileExpr(b, e.right, locals, funcIndexByName, builtins);
      switch (e.op) {
        case 'add':
          b.u8(wasm.Op.i32_add);
          maybeTee();
          return;
        case 'sub':
          b.u8(wasm.Op.i32_sub);
          maybeTee();
          return;
        case 'mul':
          b.u8(wasm.Op.i32_mul);
          maybeTee();
          return;
        case 'and':
          b.u8(wasm.Op.i32_and);
          maybeTee();
          return;
        case 'or':
          b.u8(wasm.Op.i32_or);
          maybeTee();
          return;
        case 'xor':
          b.u8(wasm.Op.i32_xor);
          maybeTee();
          return;
        case 'shl':
          b.u8(wasm.Op.i32_shl);
          maybeTee();
          return;
        case 'shr':
          b.u8(wasm.Op.i32_shr_s);
          maybeTee();
          return;
        case 'eq':
          b.u8(wasm.Op.i32_eq);
          maybeTee();
          return;
        case 'ne':
          b.u8(wasm.Op.i32_ne);
          maybeTee();
          return;
        case 'lt':
          b.u8(wasm.Op.i32_lt_s);
          maybeTee();
          return;
        case 'le':
          b.u8(wasm.Op.i32_le_s);
          maybeTee();
          return;
        case 'gt':
          b.u8(wasm.Op.i32_gt_s);
          maybeTee();
          return;
        case 'ge':
          b.u8(wasm.Op.i32_ge_s);
          maybeTee();
          return;
      }
    }
    case 'call': {
      // builtins
      if (e.name === 'println_i32') {
        throw new Error('println_i32 removed from default compiler output (no imports). Use host-specific glue or add a print import flag later.');
      }
      if (e.name === 'load_i32') {
        if (e.args.length !== 1) throw new Error('load_i32 expects 1 arg');
        compileExpr(b, e.args[0]!, locals, funcIndexByName, builtins, cse);
        b.u8(wasm.Op.i32_load);
        b.uleb(2).uleb(0); // align=4, offset=0
        maybeTee();
        return;
      }
      if (e.name === 'store_i32') {
        if (e.args.length !== 2) throw new Error('store_i32 expects 2 args');
        compileExpr(b, e.args[0]!, locals, funcIndexByName, builtins);
        compileExpr(b, e.args[1]!, locals, funcIndexByName, builtins);
        b.u8(wasm.Op.i32_store);
        b.uleb(2).uleb(0);
        return;
      }

      // user function call (with tiny inliner)
      const inlineFn = (funcIndexByName as any).__inlineFns?.get(e.name) as
        | { params: { name: string }[]; expr: Expr }
        | undefined;
      if (inlineFn) {
        if (inlineFn.params.length !== e.args.length) throw new Error(`Arity mismatch for ${e.name}`);
        const subst = new Map<string, Expr>();
        for (let i = 0; i < inlineFn.params.length; i++) subst.set(inlineFn.params[i]!.name, e.args[i]!);
        const inlined = substituteExpr(inlineFn.expr, subst);
        compileExpr(b, inlined, locals, funcIndexByName, builtins);
        return;
      }

      const idx = funcIndexByName.get(e.name);
      if (idx == null) throw new Error(`Unknown function: ${e.name}`);
      for (const arg of e.args) compileExpr(b, arg, locals, funcIndexByName, builtins);
      b.u8(wasm.Op.call).uleb(idx);
      return;
    }
  }
}

function compileStmts(
  b: Bytes,
  stmts: Stmt[],
  locals: Map<string, LocalInfo>,
  localTypes: number[],
  funcIndexByName: Map<string, number>,
  builtins: { printImportIndex: number | null },
): void {
  for (const s of stmts) {
    compileStmt(b, s, locals, localTypes, funcIndexByName, builtins);
  }
}

function compileStmt(
  b: Bytes,
  s: Stmt,
  locals: Map<string, LocalInfo>,
  localTypes: number[],
  funcIndexByName: Map<string, number>,
  builtins: { printImportIndex: number | null },
): void {
  switch (s.kind) {
    case 'let': {
      if (s.type !== 'i32') throw new Error('Only i32 locals supported');
      const index = localTypes.length;
      localTypes.push(wasm.ValType.i32);
      locals.set(s.name, { index });
      compileExpr(b, s.expr, locals, funcIndexByName, builtins);
      b.u8(wasm.Op.local_set).uleb(index);
      return;
    }
    case 'assign': {
      const li = locals.get(s.name);
      if (!li) throw new Error(`Undefined variable: ${s.name}`);
      compileExpr(b, s.expr, locals, funcIndexByName, builtins);
      b.u8(wasm.Op.local_set).uleb(li.index);
      return;
    }
    case 'expr': {
      compileExpr(b, s.expr, locals, funcIndexByName, builtins);
      // For now we only use expr statements for void-returning calls.
      return;
    }
    case 'return': {
      if (s.expr) {
        compileExpr(b, s.expr, locals, funcIndexByName, builtins);
        b.u8(wasm.Op.return_);
      } else {
        b.u8(wasm.Op.return_);
      }
      return;
    }
    case 'if': {
      compileExpr(b, s.cond, locals, funcIndexByName, builtins);
      b.u8(wasm.Op.if_);
      b.u8(0x40); // blocktype empty
      compileStmts(b, s.then, locals, localTypes, funcIndexByName, builtins);
      if (s.elseBranch) {
        b.u8(wasm.Op.else_);
        compileStmts(b, s.elseBranch, locals, localTypes, funcIndexByName, builtins);
      }
      b.u8(wasm.Op.end);
      return;
    }
    case 'while': {
      // block $break
      b.u8(wasm.Op.block).u8(0x40);
      // loop $continue
      b.u8(wasm.Op.loop).u8(0x40);

      compileExpr(b, s.cond, locals, funcIndexByName, builtins);
      b.u8(wasm.Op.i32_eqz);
      b.u8(wasm.Op.br_if).uleb(1); // break out of block

      compileStmts(b, s.body, locals, localTypes, funcIndexByName, builtins);
      b.u8(wasm.Op.br).uleb(0); // continue

      b.u8(wasm.Op.end); // end loop
      b.u8(wasm.Op.end); // end block
      return;
    }
  }
}

function ensureStart(program: Program): void {
  if (program.fns.some((f) => f.name === '_start')) return;
  program.fns.unshift({
    exported: true,
    inlineHint: false,
    name: '_start',
    params: [],
    ret: 'void',
    body: [
      // heap ptr at memory[0] = 1024
      { kind: 'expr', expr: { kind: 'call', name: 'store_i32', args: [{ kind: 'num', value: 0 }, { kind: 'num', value: 1024 }] } },
      { kind: 'return', expr: null },
    ],
  });
}

function substituteConsts(expr: Expr, constMap: Map<string, Expr>): Expr {
  // Reuse substituteExpr
  return substituteExpr(expr, constMap);
}

function tryEvalConstExpr(expr: Expr): number | null {
  switch (expr.kind) {
    case 'num':
      return expr.value | 0;
    case 'var':
      return null;
    case 'call':
      return null;
    case 'bin': {
      const a = tryEvalConstExpr(expr.left);
      const b = tryEvalConstExpr(expr.right);
      if (a == null || b == null) return null;
      switch (expr.op) {
        case 'add':
          return (a + b) | 0;
        case 'sub':
          return (a - b) | 0;
        case 'mul':
          return Math.imul(a, b) | 0;
        case 'and':
          return (a & b) | 0;
        case 'or':
          return (a | b) | 0;
        case 'xor':
          return (a ^ b) | 0;
        case 'shl':
          return (a << (b & 31)) | 0;
        case 'shr':
          return (a >> (b & 31)) | 0;
        case 'eq':
          return a === b ? 1 : 0;
        case 'ne':
          return a !== b ? 1 : 0;
        case 'lt':
          return a < b ? 1 : 0;
        case 'le':
          return a <= b ? 1 : 0;
        case 'gt':
          return a > b ? 1 : 0;
        case 'ge':
          return a >= b ? 1 : 0;
      }
    }
  }
}

function foldConstsInExpr(expr: Expr): Expr {
  if (expr.kind === 'bin') {
    const l = foldConstsInExpr(expr.left);
    const r = foldConstsInExpr(expr.right);
    const folded: Expr = { kind: 'bin', op: expr.op, left: l, right: r };
    const v = tryEvalConstExpr(folded);
    if (v != null) return { kind: 'num', value: v };
    return folded;
  }
  if (expr.kind === 'call') {
    return { kind: 'call', name: expr.name, args: expr.args.map(foldConstsInExpr) };
  }
  return expr;
}

function buildConstMap(consts: ConstDecl[]): Map<string, Expr> {
  const m = new Map<string, Expr>();

  // resolve consts in order with substitution + folding (allows const-to-const references)
  for (const c of consts) {
    const substituted = substituteExpr(c.expr, m);
    const folded = foldConstsInExpr(substituted);
    m.set(c.name, folded);
  }

  return m;
}

export function compileToWasm(program: Program, opts: { target: Target }): Uint8Array {
  // Always provide a tiny heap init _start if user didn't define one.
  ensureStart(program);

  // Inline top-level consts by substitution (no evaluation pass yet)
  const constMap = buildConstMap(program.consts);
  const patchedFns: FnDecl[] = program.fns.map((f) => ({
    ...f,
    body: f.body.map((st) => patchStmtConsts(st, constMap)),
  }));
  program = { ...program, fns: patchedFns };

  // Collect function signatures
  const sigMap = new Map<string, number>();
  const types: Array<{ params: number[]; results: number[] }> = [];
  const typeIndexOf = (sig: FuncSig): number => {
    const key = sigKey(sig);
    const existing = sigMap.get(key);
    if (existing != null) return existing;
    const idx = types.length;
    sigMap.set(key, idx);
    types.push({
      params: sig.params.filter((p) => p !== 'void').map(valTypeOf),
      results: sig.ret === 'void' ? [] : [valTypeOf(sig.ret)],
    });
    return idx;
  };

  // imports
  const imports: Array<{ module: string; name: string; typeIndex: number }> = [];
  // For now, keep the compiler pure: no imports by default.
  // println_i32 is available only when the host provides it via ABI; we omit it here.
  let printImportIndex: number | null = null;

  // function indices: imports first
  const funcIndexByName = new Map<string, number>();

  // collect tiny inlineable fns
  // 1) value inline: non-exported, ret i32, body exactly `return <expr>;`
  const inlineFns = new Map<string, { params: { name: string }[]; expr: Expr }>();
  // 2) void inline: non-exported, ret void, body only expr-statements (no lets/assign/if/while/return)
  const inlineVoidFns = new Map<string, { params: { name: string }[]; exprs: Expr[] }>();

  for (const f of program.fns) {
    if (f.exported) continue;

    const mustInline = f.inlineHint;

    // i32-return expression inline
    if (f.ret === 'i32' && f.body.length === 1) {
      const st = f.body[0]!;
      if (st.kind === 'return' && st.expr) {
        inlineFns.set(f.name, { params: f.params, expr: st.expr });
        continue;
      }
    }

    // void inline: only expr-statements
    if (f.ret === 'void') {
      let ok = true;
      const exprs: Expr[] = [];
      for (const st of f.body) {
        if (st.kind !== 'expr') {
          ok = false;
          break;
        }
        exprs.push(st.expr);
      }
      if (ok && exprs.length > 0) {
        inlineVoidFns.set(f.name, { params: f.params, exprs });
        continue;
      }
    }

    if (mustInline) {
      throw new Error(
        `@inline fn ${f.name} is not inlineable yet. Allowed shapes: (1) i32 fn { return <expr>; } or (2) void fn { <expr>; ... }`,
      );
    }
  }

  (funcIndexByName as any).__inlineFns = inlineFns;
  (funcIndexByName as any).__inlineVoidFns = inlineVoidFns;

  // Emit all functions except inline-only helpers (unless exported)
  const emittedFns = program.fns.filter((f) => f.exported || (!inlineFns.has(f.name) && !inlineVoidFns.has(f.name)));

  let nextFuncIndex = imports.length;
  for (const f of emittedFns) {
    funcIndexByName.set(f.name, nextFuncIndex++);
  }

  // compile funcs
  const funcs: Array<{ typeIndex: number; locals: number[]; body: Uint8Array }> = [];

  for (const f of emittedFns) {
    const ti = typeIndexOf({ params: f.params.map((p) => p.type), ret: f.ret });

    // locals map: params are locals 0..n-1
    const locals = new Map<string, LocalInfo>();
    for (let i = 0; i < f.params.length; i++) {
      locals.set(f.params[i]!.name, { index: i });
    }

    // extra locals are declared after params
    const localTypes: number[] = [];
    const localIndexBase = f.params.length;

    const bodyB = new Bytes();

    const stmtToLocals = new Map<string, LocalInfo>();
    // let/assign use a single locals map; for lets we must offset index by base

    // wrap locals map to incorporate base
    const localsWithBase = new Map<string, LocalInfo>();
    for (const [k, v] of locals) localsWithBase.set(k, v);

    const builtins = { printImportIndex };

    // CSE context (per function)
    const exprCounts = new Map<ExprKey, number>();
    for (const st of f.body) countExprsInStmt(st, exprCounts);
    const cseCtx = {
      counts: exprCounts,
      temps: new Map<ExprKey, number>(),
      nextTempLocal: () => {
        const idx = localIndexBase + localTypes.length;
        localTypes.push(wasm.ValType.i32);
        return idx;
      },
    };

    const compileStmtWithBase = (
      bb: Bytes,
      st: Stmt,
    ) => {
      // patched let handling to add base
      if (st.kind === 'let') {
        if (st.type !== 'i32') throw new Error('Only i32 locals supported');
        const index = localIndexBase + localTypes.length;
        localTypes.push(wasm.ValType.i32);
        localsWithBase.set(st.name, { index });
        compileExpr(bb, st.expr, localsWithBase, funcIndexByName, builtins, cseCtx);
        bb.u8(wasm.Op.local_set).uleb(index);
        return;
      }
      if (st.kind === 'assign') {
        const li = localsWithBase.get(st.name);
        if (!li) throw new Error(`Undefined variable: ${st.name}`);
        compileExpr(bb, st.expr, localsWithBase, funcIndexByName, builtins, cseCtx);
        bb.u8(wasm.Op.local_set).uleb(li.index);
        return;
      }
      if (st.kind === 'expr') {
        // inline void helper if possible (only when called as statement)
        if (st.expr.kind === 'call') {
          const iv = (funcIndexByName as any).__inlineVoidFns?.get(st.expr.name) as
            | { params: { name: string }[]; exprs: Expr[] }
            | undefined;
          if (iv) {
            if (iv.params.length !== st.expr.args.length) throw new Error(`Arity mismatch for ${st.expr.name}`);
            const subst = new Map<string, Expr>();
            for (let i = 0; i < iv.params.length; i++) subst.set(iv.params[i]!.name, st.expr.args[i]!);
            for (const ex of iv.exprs) {
              compileExpr(bb, substituteExpr(ex, subst), localsWithBase, funcIndexByName, builtins, cseCtx);
            }
            return;
          }
        }

        compileExpr(bb, st.expr, localsWithBase, funcIndexByName, builtins, cseCtx);
        return;
      }
      if (st.kind === 'return') {
        if (st.expr) {
          compileExpr(bb, st.expr, localsWithBase, funcIndexByName, builtins, cseCtx);
        }
        bb.u8(wasm.Op.return_);
        return;
      }
      if (st.kind === 'if') {
        compileExpr(bb, st.cond, localsWithBase, funcIndexByName, builtins);
        bb.u8(wasm.Op.if_).u8(0x40);
        for (const x of st.then) compileStmtWithBase(bb, x);
        if (st.elseBranch) {
          bb.u8(wasm.Op.else_);
          for (const x of st.elseBranch) compileStmtWithBase(bb, x);
        }
        bb.u8(wasm.Op.end);
        return;
      }
      if (st.kind === 'while') {
        bb.u8(wasm.Op.block).u8(0x40);
        bb.u8(wasm.Op.loop).u8(0x40);
        compileExpr(bb, st.cond, localsWithBase, funcIndexByName, builtins);
        bb.u8(wasm.Op.i32_eqz);
        bb.u8(wasm.Op.br_if).uleb(1);
        for (const x of st.body) compileStmtWithBase(bb, x);
        bb.u8(wasm.Op.br).uleb(0);
        bb.u8(wasm.Op.end);
        bb.u8(wasm.Op.end);
        return;
      }
    };

    for (const st of f.body) compileStmtWithBase(bodyB, st);

    // implicit return for void
    bodyB.u8(wasm.Op.end);

    funcs.push({
      typeIndex: ti,
      locals: localTypes.map(() => wasm.ValType.i32),
      body: bodyB.toU8(),
    });
  }

  // exports
  const exports: Array<{ name: string; funcIndex: number }> = [];
  // Always export everything marked export in source.
  for (const f of program.fns) {
    if (f.exported) exports.push({ name: f.name, funcIndex: funcIndexByName.get(f.name)! });
  }

  // Convenience: if source doesn't export anything, keep old ergonomics for demos
  if (exports.length === 0) {
    if (opts.target === 'web') {
      const mainIdx = funcIndexByName.get('main');
      if (mainIdx == null) throw new Error('Missing fn main (and no exported functions)');
      exports.push({ name: 'main', funcIndex: mainIdx });
    } else {
      const startIdx = funcIndexByName.get('_start');
      if (startIdx == null) throw new Error('Missing fn _start (and no exported functions)');
      exports.push({ name: '_start', funcIndex: startIdx });
    }
  }

  return buildModule({
    types,
    imports,
    funcs,
    exports,
    memory: { initialPages: 32, exportName: 'memory' },
  });
}
