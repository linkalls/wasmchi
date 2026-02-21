use thiserror::Error;

use crate::{ast::*, lexer};

#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("parse error at {pos}: {message}")]
pub struct ParseError {
    pub pos: usize,
    pub message: String,
}

pub fn parse_program(source: &str) -> Result<Program, ParseError> {
    let tokens = lexer::lex(source);
    let mut p = Parser { tokens, i: 0 };
    p.parse_program()
}

struct Parser {
    tokens: Vec<lexer::Token>,
    i: usize,
}

impl Parser {
    fn parse_program(&mut self) -> Result<Program, ParseError> {
        self.skip_newlines();
        let mut items = Vec::new();
        while !self.at_eof() {
            items.push(self.parse_item()?);
            self.skip_newlines();
        }
        Ok(Program { items })
    }

    fn parse_item(&mut self) -> Result<Item, ParseError> {
        if self.eat_export() {
            let f = self.parse_fn_after_export()?;
            Ok(Item::ExportFn(f))
        } else if matches!(self.peek().kind, lexer::TokenKind::Import) {
            self.i += 1; // import
            self.expect_fn()?;
            let import = self.parse_import_fn_rest()?;
            Ok(Item::ImportFn(import))
        } else if matches!(self.peek().kind, lexer::TokenKind::Fn) {
            self.expect_fn()?;
            let f = self.parse_fn_rest()?;
            Ok(Item::Fn(f))
        } else {
            Err(self.err("expected 'export' or 'import' or 'fn'"))
        }
    }

    fn parse_import_fn_rest(&mut self) -> Result<ImportFn, ParseError> {
        let name = self.expect_ident()?;
        self.expect(lexer::TokenKind::LParen, "(")?;
        let params = self.parse_params_ts()?;
        self.expect(lexer::TokenKind::RParen, ")")?;

        let ret_ty = if self.eat(lexer::TokenKind::Colon) {
            self.parse_type()?
        } else {
            self.parse_type()?
        };

        Ok(ImportFn { name, params, ret_ty })
    }

    fn parse_fn_after_export(&mut self) -> Result<Function, ParseError> {
        self.expect_fn()?;
        self.parse_fn_rest()
    }

    fn parse_fn_rest(&mut self) -> Result<Function, ParseError> {
        let name = self.expect_ident()?;
        self.expect(lexer::TokenKind::LParen, "(")?;
        let params = self.parse_params_ts()?;
        self.expect(lexer::TokenKind::RParen, ")")?;

        // Return type: TS style `: i32` or V style `i32`
        let ret_ty = if self.eat(lexer::TokenKind::Colon) {
            self.parse_type()?
        } else {
            self.parse_type()?
        };

        self.expect(lexer::TokenKind::LBrace, "{")?;
        self.skip_newlines();
        let body = self.parse_block_stmts()?;
        self.expect(lexer::TokenKind::RBrace, "}")?;

        Ok(Function { name, params, ret_ty, body })
    }

    fn skip_newlines(&mut self) {
        while matches!(self.peek().kind, lexer::TokenKind::Newline) {
            self.i += 1;
        }
    }

    fn parse_type(&mut self) -> Result<Type, ParseError> {
        match &self.peek().kind {
            lexer::TokenKind::Ident(s) if s == "i32" => {
                self.i += 1;
                Ok(Type::I32)
            }
            lexer::TokenKind::Ident(s) if s == "f64" => {
                self.i += 1;
                Ok(Type::F64)
            }
            lexer::TokenKind::Ident(s) if s == "string" => {
                self.i += 1;
                Ok(Type::String)
            }
            lexer::TokenKind::Ident(s) if s == "void" => {
                self.i += 1;
                Ok(Type::Void)
            }
            lexer::TokenKind::Ident(s) if s == "bool" => {
                self.i += 1;
                Ok(Type::Bool)
            }
            _ => Err(self.err("expected type")),
        }
    }

    fn parse_block_stmts(&mut self) -> Result<Vec<Stmt>, ParseError> {
        let mut stmts = Vec::new();
        while !matches!(self.peek().kind, lexer::TokenKind::RBrace | lexer::TokenKind::Eof) {
            if matches!(self.peek().kind, lexer::TokenKind::Newline) {
                self.skip_newlines();
                continue;
            }
            let stmt = self.parse_stmt()?;
            // if/while end with '}', no newline required after
            let is_block_stmt = matches!(stmt, Stmt::If { .. } | Stmt::While { .. });
            stmts.push(stmt);
            // statement separator is newline or '}'
            if matches!(self.peek().kind, lexer::TokenKind::Newline) {
                self.skip_newlines();
            } else if matches!(self.peek().kind, lexer::TokenKind::RBrace) {
                // ok
            } else if is_block_stmt {
                // block statements don't need a trailing newline
            } else {
                return Err(self.err("expected newline"));
            }
        }
        Ok(stmts)
    }

    fn parse_stmt(&mut self) -> Result<Stmt, ParseError> {
        match &self.peek().kind {
            lexer::TokenKind::Let => {
                self.i += 1;
                let name = self.expect_ident()?;
                self.expect(lexer::TokenKind::Equal, "=")?;
                let expr = self.parse_expr()?;
                Ok(Stmt::Let { name, expr })
            }
            lexer::TokenKind::Return => {
                self.i += 1;
                let expr = self.parse_expr()?;
                Ok(Stmt::Return(expr))
            }
            lexer::TokenKind::If => {
                self.i += 1;
                let cond = self.parse_expr()?;
                self.expect(lexer::TokenKind::LBrace, "{")?;
                self.skip_newlines();
                let then_body = self.parse_block_stmts()?;
                self.expect(lexer::TokenKind::RBrace, "}")?;
                let else_body = if self.eat(lexer::TokenKind::Else) {
                    self.expect(lexer::TokenKind::LBrace, "{")?;
                    self.skip_newlines();
                    let body = self.parse_block_stmts()?;
                    self.expect(lexer::TokenKind::RBrace, "}")?;
                    Some(body)
                } else {
                    None
                };
                Ok(Stmt::If { cond, then_body, else_body })
            }
            lexer::TokenKind::While => {
                self.i += 1;
                let cond = self.parse_expr()?;
                self.expect(lexer::TokenKind::LBrace, "{")?;
                self.skip_newlines();
                let body = self.parse_block_stmts()?;
                self.expect(lexer::TokenKind::RBrace, "}")?;
                Ok(Stmt::While { cond, body })
            }
            lexer::TokenKind::Ident(s) if s == "print" => {
                // print(<expr>) as statement
                self.i += 1;
                self.expect(lexer::TokenKind::LParen, "(")?;
                let expr = self.parse_expr()?;
                self.expect(lexer::TokenKind::RParen, ")")?;
                Ok(Stmt::Print(expr))
            }
            lexer::TokenKind::Ident(_) => {
                // V-ish short declaration: `x := expr`
                // Only when pattern matches ident followed by :=
                if let lexer::TokenKind::Ident(name) = &self.peek().kind {
                    let name = name.clone();
                    if matches!(self.tokens.get(self.i + 1).map(|t| &t.kind), Some(lexer::TokenKind::ColonEqual)) {
                        self.i += 1; // ident
                        self.i += 1; // :=
                        let expr = self.parse_expr()?;
                        return Ok(Stmt::Let { name, expr });
                    }
                }
                let expr = self.parse_expr()?;
                Ok(Stmt::Expr(expr))
            }
            _ => {
                let expr = self.parse_expr()?;
                Ok(Stmt::Expr(expr))
            }
        }
    }

    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_compare()
    }

    fn parse_compare(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_add_sub()?;
        loop {
            let op = match self.peek().kind {
                lexer::TokenKind::EqEq => BinOp::Eq,
                lexer::TokenKind::BangEq => BinOp::Ne,
                lexer::TokenKind::Lt => BinOp::Lt,
                lexer::TokenKind::LtEq => BinOp::Le,
                lexer::TokenKind::Gt => BinOp::Gt,
                lexer::TokenKind::GtEq => BinOp::Ge,
                _ => break,
            };
            self.i += 1;
            let right = self.parse_add_sub()?;
            expr = Expr::Binary { op, left: Box::new(expr), right: Box::new(right) };
        }
        Ok(expr)
    }

    fn parse_add_sub(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_mul_div()?;
        loop {
            let op = match self.peek().kind {
                lexer::TokenKind::Plus => BinOp::Add,
                lexer::TokenKind::Minus => BinOp::Sub,
                _ => break,
            };
            self.i += 1;
            let right = self.parse_mul_div()?;

            // v0 convenience: constant-fold string literal concatenation
            expr = match (op, &expr, &right) {
                (BinOp::Add, Expr::Str(a), Expr::Str(b)) => Expr::Str(format!("{a}{b}")),
                _ => Expr::Binary { op, left: Box::new(expr), right: Box::new(right) },
            };
        }
        Ok(expr)
    }

    fn parse_mul_div(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_primary()?;
        loop {
            let op = match self.peek().kind {
                lexer::TokenKind::Star => BinOp::Mul,
                lexer::TokenKind::Slash => BinOp::Div,
                _ => break,
            };
            self.i += 1;
            let right = self.parse_primary()?;
            expr = Expr::Binary { op, left: Box::new(expr), right: Box::new(right) };
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        match &self.peek().kind {
            lexer::TokenKind::Int(n) => {
                let n = *n;
                self.i += 1;
                Ok(Expr::Int(n))
            }
            lexer::TokenKind::Float(bits) => {
                let bits = *bits;
                self.i += 1;
                Ok(Expr::Float(bits))
            }
            lexer::TokenKind::True => {
                self.i += 1;
                Ok(Expr::Bool(true))
            }
            lexer::TokenKind::False => {
                self.i += 1;
                Ok(Expr::Bool(false))
            }
            lexer::TokenKind::Str(s) => {
                let s = s.clone();
                self.i += 1;
                Ok(Expr::Str(s))
            }
            lexer::TokenKind::Ident(s) => {
                let name = s.clone();
                self.i += 1;
                if self.eat(lexer::TokenKind::LParen) {
                    // call expr
                    let mut args = Vec::new();
                    self.skip_newlines();
                    if !matches!(self.peek().kind, lexer::TokenKind::RParen) {
                        loop {
                            let arg = self.parse_expr()?;
                            args.push(arg);
                            if self.eat(lexer::TokenKind::Comma) {
                                self.skip_newlines();
                                continue;
                            }
                            break;
                        }
                    }
                    self.expect(lexer::TokenKind::RParen, ")")?;
                    Ok(Expr::Call { callee: name, args })
                } else {
                    Ok(Expr::Var(name))
                }
            }
            lexer::TokenKind::LParen => {
                self.i += 1;
                let expr = self.parse_expr()?;
                self.expect(lexer::TokenKind::RParen, ")")?;
                Ok(expr)
            }
            _ => Err(self.err("expected expression")),
        }
    }

    fn at_eof(&self) -> bool {
        matches!(self.peek().kind, lexer::TokenKind::Eof)
    }

    fn eat_export(&mut self) -> bool {
        if matches!(self.peek().kind, lexer::TokenKind::Export) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn expect_fn(&mut self) -> Result<(), ParseError> {
        self.expect(lexer::TokenKind::Fn, "fn")?;
        Ok(())
    }

    fn parse_params_ts(&mut self) -> Result<Vec<Param>, ParseError> {
        let mut params = Vec::new();
        self.skip_newlines();
        if matches!(self.peek().kind, lexer::TokenKind::RParen) {
            return Ok(params);
        }

        loop {
            // allow either TS style: name: ty  OR V style: name ty
            let name = self.expect_ident()?;
            let ty = if self.eat(lexer::TokenKind::Colon) {
                self.parse_type()?
            } else {
                self.parse_type()?
            };
            params.push(Param { name, ty });

            if self.eat(lexer::TokenKind::Comma) {
                self.skip_newlines();
                continue;
            }
            break;
        }

        Ok(params)
    }

    fn expect_ident(&mut self) -> Result<String, ParseError> {
        match &self.peek().kind {
            lexer::TokenKind::Ident(s) => {
                let s = s.clone();
                self.i += 1;
                Ok(s)
            }
            _ => Err(self.err("expected identifier")),
        }
    }

    fn expect(&mut self, kind: lexer::TokenKind, expected: &str) -> Result<(), ParseError> {
        if std::mem::discriminant(&self.peek().kind) == std::mem::discriminant(&kind) {
            self.i += 1;
            Ok(())
        } else {
            Err(self.err(format!("expected '{expected}'")))
        }
    }

    fn eat(&mut self, kind: lexer::TokenKind) -> bool {
        if std::mem::discriminant(&self.peek().kind) == std::mem::discriminant(&kind) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn peek(&self) -> &lexer::Token {
        &self.tokens[self.i]
    }

    fn err(&self, message: impl Into<String>) -> ParseError {
        ParseError { pos: self.peek().pos, message: message.into() }
    }
}
