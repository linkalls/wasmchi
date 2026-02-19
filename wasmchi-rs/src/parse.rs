use anyhow::{bail, Result};

use crate::lex::{lex, Spanned, Tok};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TypeName {
    I32,
    Void,
}

#[derive(Debug, Clone)]
pub struct Program {
    pub consts: Vec<ConstDecl>,
    pub fns: Vec<FnDecl>,
}

#[derive(Debug, Clone)]
pub struct ConstDecl {
    pub name: String,
    pub ty: TypeName,
    pub expr: Expr,
}

#[derive(Debug, Clone)]
pub struct FnDecl {
    pub exported: bool,
    pub inline: bool,
    pub name: String,
    pub params: Vec<(String, TypeName)>,
    pub ret: TypeName,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Let { name: String, ty: TypeName, expr: Expr },
    Assign { name: String, expr: Expr },
    If { cond: Expr, then_body: Vec<Stmt>, else_body: Vec<Stmt> },
    While { cond: Expr, body: Vec<Stmt> },
    Return(Option<Expr>),
    Expr(Expr),
}

#[derive(Debug, Clone)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    And,
    Or,
    Xor,
    Shl,
    Shr,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Num(i32),
    Var(String),
    Bin { op: BinOp, l: Box<Expr>, r: Box<Expr> },
    Call { name: String, args: Vec<Expr> },
}

struct P {
    t: Vec<Spanned>,
    i: usize,
}

impl P {
    fn new(t: Vec<Spanned>) -> Self {
        Self { t, i: 0 }
    }

    fn peek(&self) -> Option<&Tok> {
        self.t.get(self.i).map(|s| &s.tok)
    }

    fn take(&mut self) -> Option<Tok> {
        let tok = self.t.get(self.i).map(|s| s.tok.clone());
        self.i += 1;
        tok
    }

    fn eat(&mut self, want: Tok) -> bool {
        if self.peek() == Some(&want) {
            self.take();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, want: Tok) -> Result<()> {
        if self.eat(want.clone()) {
            Ok(())
        } else {
            bail!("expected {:?}", want)
        }
    }

    fn expect_ident(&mut self) -> Result<String> {
        match self.take() {
            Some(Tok::Ident(s)) => Ok(s),
            other => bail!("expected ident, got {:?}", other),
        }
    }

    fn expect_type(&mut self) -> Result<TypeName> {
        let s = self.expect_ident()?;
        match s.as_str() {
            "i32" => Ok(TypeName::I32),
            "void" => Ok(TypeName::Void),
            _ => bail!("unknown type {s}"),
        }
    }
}

fn prec(t: &Tok) -> Option<u8> {
    Some(match t {
        Tok::Star => 70,
        Tok::Plus | Tok::Minus => 60,
        Tok::Shl | Tok::Shr => 55,
        Tok::Amp => 50,
        Tok::Caret => 45,
        Tok::Pipe => 40,
        Tok::EqEq | Tok::Ne | Tok::Lt | Tok::Le | Tok::Gt | Tok::Ge => 30,
        _ => return None,
    })
}

fn to_op(t: Tok) -> Result<BinOp> {
    Ok(match t {
        Tok::Plus => BinOp::Add,
        Tok::Minus => BinOp::Sub,
        Tok::Star => BinOp::Mul,
        Tok::Amp => BinOp::And,
        Tok::Pipe => BinOp::Or,
        Tok::Caret => BinOp::Xor,
        Tok::Shl => BinOp::Shl,
        Tok::Shr => BinOp::Shr,
        Tok::EqEq => BinOp::Eq,
        Tok::Ne => BinOp::Ne,
        Tok::Lt => BinOp::Lt,
        Tok::Le => BinOp::Le,
        Tok::Gt => BinOp::Gt,
        Tok::Ge => BinOp::Ge,
        other => bail!("not an op: {:?}", other),
    })
}

fn parse_primary(p: &mut P) -> Result<Expr> {
    match p.take() {
        Some(Tok::Num(n)) => Ok(Expr::Num(n)),
        Some(Tok::Ident(name)) => {
            if p.eat(Tok::LParen) {
                let mut args = Vec::new();
                if !p.eat(Tok::RParen) {
                    loop {
                        args.push(parse_expr(p, 0)?);
                        if p.eat(Tok::RParen) {
                            break;
                        }
                        p.expect(Tok::Comma)?;
                    }
                }
                Ok(Expr::Call { name, args })
            } else {
                Ok(Expr::Var(name))
            }
        }
        Some(Tok::LParen) => {
            let e = parse_expr(p, 0)?;
            p.expect(Tok::RParen)?;
            Ok(e)
        }
        other => bail!("unexpected primary: {:?}", other),
    }
}

fn parse_expr(p: &mut P, min_prec: u8) -> Result<Expr> {
    let mut left = parse_primary(p)?;
    loop {
        let Some(tk) = p.peek().cloned() else { break };
        let Some(pv) = prec(&tk) else { break };
        if pv < min_prec {
            break;
        }
        let op_tok = p.take().unwrap();
        let right = parse_expr(p, pv + 1)?;
        left = Expr::Bin {
            op: to_op(op_tok)?,
            l: Box::new(left),
            r: Box::new(right),
        };
    }
    Ok(left)
}

fn parse_block(p: &mut P) -> Result<Vec<Stmt>> {
    p.expect(Tok::LBrace)?;
    let mut out = Vec::new();
    while p.peek() != Some(&Tok::RBrace) {
        out.push(parse_stmt(p)?);
    }
    p.expect(Tok::RBrace)?;
    Ok(out)
}

fn parse_stmt(p: &mut P) -> Result<Stmt> {
    match p.peek() {
        Some(Tok::Let) => {
            p.take();
            let name = p.expect_ident()?;
            p.expect(Tok::Colon)?;
            let ty = p.expect_type()?;
            p.expect(Tok::Eq)?;
            let expr = parse_expr(p, 0)?;
            p.expect(Tok::Semi)?;
            Ok(Stmt::Let { name, ty, expr })
        }
        Some(Tok::If) => {
            p.take();
            let cond = parse_expr(p, 0)?;
            let then_body = parse_block(p)?;
            let else_body = if p.eat(Tok::Else) { parse_block(p)? } else { Vec::new() };
            Ok(Stmt::If {
                cond,
                then_body,
                else_body,
            })
        }
        Some(Tok::While) => {
            p.take();
            let cond = parse_expr(p, 0)?;
            let body = parse_block(p)?;
            Ok(Stmt::While { cond, body })
        }
        Some(Tok::Return) => {
            p.take();
            if p.eat(Tok::Semi) {
                Ok(Stmt::Return(None))
            } else {
                let e = parse_expr(p, 0)?;
                p.expect(Tok::Semi)?;
                Ok(Stmt::Return(Some(e)))
            }
        }
        Some(Tok::Ident(_)) => {
            // assignment or expr
            let save = p.i;
            let name = match p.take() {
                Some(Tok::Ident(s)) => s,
                _ => unreachable!(),
            };
            if p.eat(Tok::Eq) {
                let e = parse_expr(p, 0)?;
                p.expect(Tok::Semi)?;
                Ok(Stmt::Assign { name, expr: e })
            } else {
                // rewind and parse as expr
                p.i = save;
                let e = parse_expr(p, 0)?;
                p.expect(Tok::Semi)?;
                Ok(Stmt::Expr(e))
            }
        }
        _ => {
            let e = parse_expr(p, 0)?;
            p.expect(Tok::Semi)?;
            Ok(Stmt::Expr(e))
        }
    }
}

fn parse_fn(p: &mut P) -> Result<FnDecl> {
    let exported = p.eat(Tok::Export);

    // optional attribute: @inline
    let mut inline = false;
    if p.eat(Tok::At) {
        let a = p.expect_ident()?;
        if a != "inline" {
            bail!("unknown attribute @{a}");
        }
        inline = true;
    }

    p.expect(Tok::Fn)?;
    let name = p.expect_ident()?;
    p.expect(Tok::LParen)?;
    let mut params = Vec::new();
    if !p.eat(Tok::RParen) {
        loop {
            let pn = p.expect_ident()?;
            p.expect(Tok::Colon)?;
            let pt = p.expect_type()?;
            params.push((pn, pt));
            if p.eat(Tok::RParen) {
                break;
            }
            p.expect(Tok::Comma)?;
        }
    }
    let mut ret = TypeName::Void;
    if p.eat(Tok::Colon) {
        ret = p.expect_type()?;
    }
    let body = parse_block(p)?;
    Ok(FnDecl {
        exported,
        inline,
        name,
        params,
        ret,
        body,
    })
}

fn parse_const(p: &mut P) -> Result<ConstDecl> {
    p.expect(Tok::Const)?;
    let name = p.expect_ident()?;
    p.expect(Tok::Colon)?;
    let ty = p.expect_type()?;
    p.expect(Tok::Eq)?;
    let expr = parse_expr(p, 0)?;
    p.expect(Tok::Semi)?;
    Ok(ConstDecl { name, ty, expr })
}

pub fn parse(src: &str) -> Result<Program> {
    let toks = lex(src);
    let mut p = P::new(toks);
    let mut consts = Vec::new();
    let mut fns = Vec::new();

    while let Some(tk) = p.peek() {
        match tk {
            Tok::Const => consts.push(parse_const(&mut p)?),
            Tok::Export | Tok::Fn | Tok::At => fns.push(parse_fn(&mut p)?),
            Tok::Error => bail!("lex error"),
            _ => bail!("unexpected token at top-level: {:?}", tk),
        }
    }

    Ok(Program { consts, fns })
}
