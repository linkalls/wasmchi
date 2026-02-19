use logos::Logos;

#[derive(Logos, Debug, Clone, PartialEq)]
pub enum Tok {
    #[token("fn")] Fn,
    #[token("let")] Let,
    #[token("if")] If,
    #[token("else")] Else,
    #[token("while")] While,
    #[token("return")] Return,
    #[token("export")] Export,
    #[token("const")] Const,

    // symbols
    #[token("(")] LParen,
    #[token(")")] RParen,
    #[token("{")] LBrace,
    #[token("}")] RBrace,
    #[token(":")] Colon,
    #[token(";")] Semi,
    #[token(",")] Comma,
    #[token("=")] Eq,

    #[token("==")] EqEq,
    #[token("!=")] Ne,
    #[token("<=")] Le,
    #[token(">=")] Ge,
    #[token("<<")] Shl,
    #[token(">>")] Shr,

    #[token("+")] Plus,
    #[token("-")] Minus,
    #[token("*")] Star,
    #[token("&")] Amp,
    #[token("|")] Pipe,
    #[token("^")] Caret,
    #[token("<")] Lt,
    #[token(">")] Gt,

    #[token("@")] At,

    #[regex(r"[A-Za-z_][A-Za-z0-9_]*", |lex| lex.slice().to_string())]
    Ident(String),

    #[regex(r"[0-9]+", |lex| lex.slice().parse::<i32>().unwrap())]
    Num(i32),

    #[regex(r"//[^\n]*", logos::skip)]
    #[regex(r"[ \t\n\r]+", logos::skip)]
    Error,
}

#[derive(Debug, Clone)]
pub struct Spanned {
    pub tok: Tok,
    pub start: usize,
    pub end: usize,
}

pub fn lex(src: &str) -> Vec<Spanned> {
    let mut l = Tok::lexer(src);
    let mut out = Vec::new();
    while let Some(tok) = l.next() {
        let span = l.span();
        let tok = match tok {
            Ok(t) => t,
            Err(()) => Tok::Error,
        };
        out.push(Spanned {
            tok,
            start: span.start,
            end: span.end,
        });
    }
    out
}
