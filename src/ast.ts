export type TypeName = 'i32' | 'void';

export type Program = {
  consts: ConstDecl[];
  fns: FnDecl[];
};

export type ConstDecl = {
  name: string;
  type: TypeName;
  expr: Expr;
};

export type FnDecl = {
  exported: boolean;
  inlineHint: boolean;
  name: string;
  params: { name: string; type: TypeName }[];
  ret: TypeName;
  body: Stmt[];
};

export type Stmt =
  | { kind: 'let'; name: string; type: TypeName; expr: Expr }
  | { kind: 'assign'; name: string; expr: Expr }
  | { kind: 'if'; cond: Expr; then: Stmt[]; elseBranch: Stmt[] | null }
  | { kind: 'while'; cond: Expr; body: Stmt[] }
  | { kind: 'return'; expr: Expr | null }
  | { kind: 'expr'; expr: Expr };

export type BinOp =
  | 'add'
  | 'sub'
  | 'mul'
  | 'and'
  | 'or'
  | 'xor'
  | 'shl'
  | 'shr'
  | 'eq'
  | 'ne'
  | 'lt'
  | 'le'
  | 'gt'
  | 'ge';

export type Expr =
  | { kind: 'num'; value: number }
  | { kind: 'var'; name: string }
  | { kind: 'bin'; op: BinOp; left: Expr; right: Expr }
  | { kind: 'call'; name: string; args: Expr[] };
