import { lex, type Token } from './lexer.js';
import type { ConstDecl, Expr, FnDecl, Program, Stmt, TypeName } from './ast.js';

class P {
  i = 0;
  constructor(public tokens: Token[]) {}

  peek(): Token {
    return this.tokens[this.i]!;
  }

  take(): Token {
    return this.tokens[this.i++]!;
  }

  expectSym(sym: string): void {
    const t = this.take();
    if (t.kind !== 'sym' || t.value !== sym) {
      throw new Error(`Expected symbol '${sym}', got ${tokToString(t)}`);
    }
  }

  expectIdent(): string {
    const t = this.take();
    if (t.kind !== 'ident') throw new Error(`Expected ident, got ${tokToString(t)}`);
    return t.value;
  }

  matchSym(sym: string): boolean {
    const t = this.peek();
    if (t.kind === 'sym' && t.value === sym) {
      this.take();
      return true;
    }
    return false;
  }

  matchKw(value: Token extends infer X ? any : never): boolean {
    const t = this.peek();
    if (t.kind === 'kw' && t.value === value) {
      this.take();
      return true;
    }
    return false;
  }

  matchKwAny(...values: Array<NonNullable<Extract<Token, { kind: 'kw' }>['value']>>): string | null {
    const t = this.peek();
    if (t.kind === 'kw' && values.includes(t.value as any)) {
      this.take();
      return t.value;
    }
    return null;
  }
}

function tokToString(t: Token): string {
  if (t.kind === 'eof') return 'EOF';
  if (t.kind === 'kw') return `kw(${t.value})`;
  if (t.kind === 'ident') return `ident(${t.value})`;
  if (t.kind === 'num') return `num(${t.value})`;
  return `sym(${t.value})`;
}

function parseType(p: P): TypeName {
  const id = p.expectIdent();
  if (id === 'i32') return 'i32';
  if (id === 'void') return 'void';
  throw new Error(`Only i32/void supported, got ${id}`);
}

function parsePrimary(p: P): Expr {
  const t = p.peek();
  if (t.kind === 'num') {
    p.take();
    return { kind: 'num', value: t.value };
  }
  if (t.kind === 'ident') {
    const name = t.value;
    p.take();
    if (p.matchSym('(')) {
      const args: Expr[] = [];
      if (!p.matchSym(')')) {
        while (true) {
          args.push(parseExpr(p));
          if (p.matchSym(')')) break;
          p.expectSym(',');
        }
      }
      return { kind: 'call', name, args };
    }
    return { kind: 'var', name };
  }
  if (p.matchSym('(')) {
    const e = parseExpr(p);
    p.expectSym(')');
    return e;
  }
  throw new Error(`Unexpected token in expression: ${tokToString(t)}`);
}

function precedence(op: string): number {
  switch (op) {
    case '*':
      return 70;
    case '+':
    case '-':
      return 60;
    case '<<':
    case '>>':
      return 55;
    case '&':
      return 50;
    case '^':
      return 45;
    case '|':
      return 40;
    case '==':
    case '!=':
    case '<':
    case '<=':
    case '>':
    case '>=':
      return 30;
    default:
      return 0;
  }
}

function toBinOp(sym: string) {
  switch (sym) {
    case '+':
      return 'add' as const;
    case '-':
      return 'sub' as const;
    case '*':
      return 'mul' as const;
    case '&':
      return 'and' as const;
    case '|':
      return 'or' as const;
    case '^':
      return 'xor' as const;
    case '<<':
      return 'shl' as const;
    case '>>':
      return 'shr' as const;
    case '==':
      return 'eq' as const;
    case '!=':
      return 'ne' as const;
    case '<':
      return 'lt' as const;
    case '<=':
      return 'le' as const;
    case '>':
      return 'gt' as const;
    case '>=':
      return 'ge' as const;
    default:
      throw new Error(`Unsupported operator: ${sym}`);
  }
}

function parseExpr(p: P, minPrec = 0): Expr {
  let left = parsePrimary(p);

  while (true) {
    const tk = p.peek();
    if (tk.kind !== 'sym') break;
    const prec = precedence(tk.value);
    if (prec === 0 || prec < minPrec) break;

    const opSym = tk.value;
    p.take();
    const right = parseExpr(p, prec + 1);
    left = { kind: 'bin', op: toBinOp(opSym), left, right };
  }

  return left;
}

function parseBlock(p: P): Stmt[] {
  p.expectSym('{');
  const body: Stmt[] = [];
  while (true) {
    const tk = p.peek();
    if (tk.kind === 'sym' && tk.value === '}') break;
    body.push(parseStmt(p));
  }
  p.expectSym('}');
  return body;
}

function parseStmt(p: P): Stmt {
  const kw = p.matchKwAny('let', 'if', 'while', 'return');
  if (kw === 'let') {
    const name = p.expectIdent();
    p.expectSym(':');
    const type = parseType(p);
    p.expectSym('=');
    const expr = parseExpr(p);
    p.expectSym(';');
    return { kind: 'let', name, type, expr };
  }
  if (kw === 'if') {
    const cond = parseExpr(p);
    const then = parseBlock(p);
    let elseBranch: Stmt[] | null = null;
    if (p.matchKwAny('else')) {
      elseBranch = parseBlock(p);
    }
    return { kind: 'if', cond, then, elseBranch };
  }
  if (kw === 'while') {
    const cond = parseExpr(p);
    const body = parseBlock(p);
    return { kind: 'while', cond, body };
  }
  if (kw === 'return') {
    // return; or return expr;
    if (p.matchSym(';')) return { kind: 'return', expr: null };
    const expr = parseExpr(p);
    p.expectSym(';');
    return { kind: 'return', expr };
  }

  // assignment or expression
  const tk = p.peek();
  if (tk.kind === 'ident') {
    // lookahead: ident '='
    const tk2 = p.tokens[p.i + 1];
    if (tk2 && tk2.kind === 'sym' && tk2.value === '=') {
      const name = p.expectIdent();
      p.expectSym('=');
      const expr = parseExpr(p);
      p.expectSym(';');
      return { kind: 'assign', name, expr };
    }
  }

  const expr = parseExpr(p);
  p.expectSym(';');
  return { kind: 'expr', expr };
}

function parseFn(p: P): FnDecl {
  const exported = !!p.matchKwAny('export');

  let inlineHint = false;
  if (p.matchSym('@')) {
    const attr = p.expectIdent();
    if (attr !== 'inline') throw new Error(`Unknown attribute: @${attr}`);
    inlineHint = true;
  }

  if (!p.matchKwAny('fn')) throw new Error('Expected fn');
  const name = p.expectIdent();
  p.expectSym('(');
  const params: FnDecl['params'] = [];
  if (!p.matchSym(')')) {
    while (true) {
      const pn = p.expectIdent();
      p.expectSym(':');
      const pt = parseType(p);
      params.push({ name: pn, type: pt });
      if (p.matchSym(')')) break;
      p.expectSym(',');
    }
  }

  let ret: TypeName = 'void';
  if (p.matchSym(':')) {
    ret = parseType(p);
  }

  const body = parseBlock(p);
  return { exported, inlineHint, name, params, ret, body };
}

function parseConst(p: P): ConstDecl {
  if (!p.matchKwAny('const')) throw new Error('Expected const');
  const name = p.expectIdent();
  p.expectSym(':');
  const type = parseType(p);
  p.expectSym('=');
  const expr = parseExpr(p);
  p.expectSym(';');
  return { name, type, expr };
}

export function parse(src: string): Program {
  const tokens = lex(src);
  const p = new P(tokens);
  const consts: ConstDecl[] = [];
  const fns: FnDecl[] = [];
  while (p.peek().kind !== 'eof') {
    const tk = p.peek();
    if (tk.kind === 'kw' && tk.value === 'const') {
      consts.push(parseConst(p));
      continue;
    }
    fns.push(parseFn(p));
  }
  return { consts, fns };
}
