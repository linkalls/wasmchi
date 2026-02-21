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
    F64,
    Bool,
    String,
    Void,
    JsObj,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stmt {
    Let { name: String, expr: Expr },
    Return(Expr),
    Print(Expr),
    Expr(Expr),
    If { cond: Expr, then_body: Vec<Stmt>, else_body: Option<Vec<Stmt>> },
    While { cond: Expr, body: Vec<Stmt> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    Int(i32),
    Float(u64), // f64::to_bits()
    Bool(bool),
    Str(String),
    Var(String),

    // Calls
    Call { callee: String, args: Vec<Expr> },
    CallExpr { callee: Box<Expr>, args: Vec<Expr> },

    // Property access
    Dot { base: Box<Expr>, prop: String },

    Binary { op: BinOp, left: Box<Expr>, right: Box<Expr> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}
