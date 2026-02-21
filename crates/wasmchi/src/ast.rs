#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    ExportFn(Function),
    Fn(Function),
    ImportFn(ImportFn),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportFn {
    pub name: String,
    pub params: Vec<Param>,
    pub ret_ty: Type,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    pub name: String,
    pub params: Vec<Param>,
    pub ret_ty: Type,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    pub name: String,
    pub ty: Type,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    I32,
    String,
    Void,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stmt {
    Let { name: String, expr: Expr },
    Return(Expr),
    Print(Expr),
    Expr(Expr),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    Int(i32),
    Str(String),
    Var(String),
    Call { callee: String, args: Vec<Expr> },
    Binary { op: BinOp, left: Box<Expr>, right: Box<Expr> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
}
