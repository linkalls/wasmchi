#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    Export,
    Fn,
    Let,
    Return,

    Ident(String),
    Int(i32),
    Str(String),

    LParen,
    RParen,
    Colon,
    LBrace,
    RBrace,

    Plus,
    Minus,
    Star,
    Slash,
    Equal,
    ColonEqual,
    Comma,

    Newline,
    Eof,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub pos: usize,
}

pub fn lex(source: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let bytes = source.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        let b = bytes[i];
        match b {
            b' ' | b'\t' | b'\r' => {
                i += 1;
            }
            b'\n' => {
                tokens.push(Token { kind: TokenKind::Newline, pos: i });
                i += 1;
            }
            b'(' => { tokens.push(Token { kind: TokenKind::LParen, pos: i }); i += 1; }
            b')' => { tokens.push(Token { kind: TokenKind::RParen, pos: i }); i += 1; }
            b':' => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'=' {
                    tokens.push(Token { kind: TokenKind::ColonEqual, pos: i });
                    i += 2;
                } else {
                    tokens.push(Token { kind: TokenKind::Colon, pos: i });
                    i += 1;
                }
            }
            b'{' => { tokens.push(Token { kind: TokenKind::LBrace, pos: i }); i += 1; }
            b'}' => { tokens.push(Token { kind: TokenKind::RBrace, pos: i }); i += 1; }
            b'+' => { tokens.push(Token { kind: TokenKind::Plus, pos: i }); i += 1; }
            b'-' => { tokens.push(Token { kind: TokenKind::Minus, pos: i }); i += 1; }
            b'*' => { tokens.push(Token { kind: TokenKind::Star, pos: i }); i += 1; }
            b'/' => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'/' {
                    // line comment
                    i += 2;
                    while i < bytes.len() && bytes[i] != b'\n' {
                        i += 1;
                    }
                } else {
                    tokens.push(Token { kind: TokenKind::Slash, pos: i });
                    i += 1;
                }
            }
            b'=' => { tokens.push(Token { kind: TokenKind::Equal, pos: i }); i += 1; }
            b',' => { tokens.push(Token { kind: TokenKind::Comma, pos: i }); i += 1; }
            _ => {
                if b == b'"' {
                    let start = i;
                    i += 1;
                    let mut out = String::new();
                    while i < bytes.len() {
                        let c = bytes[i];
                        if c == b'"' {
                            i += 1;
                            break;
                        }
                        if c == b'\\' {
                            i += 1;
                            if i >= bytes.len() { break; }
                            let e = bytes[i];
                            match e {
                                b'n' => out.push('\n'),
                                b't' => out.push('\t'),
                                b'\\' => out.push('\\'),
                                b'"' => out.push('"'),
                                _ => out.push(e as char),
                            }
                            i += 1;
                            continue;
                        }
                        out.push(c as char);
                        i += 1;
                    }
                    tokens.push(Token { kind: TokenKind::Str(out), pos: start });
                } else if (b'0'..=b'9').contains(&b) {
                    let start = i;
                    i += 1;
                    while i < bytes.len() && (b'0'..=b'9').contains(&bytes[i]) {
                        i += 1;
                    }
                    let s = &source[start..i];
                    let n: i32 = s.parse().unwrap_or(0);
                    tokens.push(Token { kind: TokenKind::Int(n), pos: start });
                } else if is_ident_start(b) {
                    let start = i;
                    i += 1;
                    while i < bytes.len() && is_ident_continue(bytes[i]) {
                        i += 1;
                    }
                    let s = &source[start..i];
                    let kind = match s {
                        "export" => TokenKind::Export,
                        "fn" => TokenKind::Fn,
                        "let" => TokenKind::Let,
                        "return" => TokenKind::Return,
                        _ => TokenKind::Ident(s.to_string()),
                    };
                    tokens.push(Token { kind, pos: start });
                } else {
                    // Unknown char: skip for now.
                    i += 1;
                }
            }
        }
    }

    tokens.push(Token { kind: TokenKind::Eof, pos: source.len() });
    tokens
}

fn is_ident_start(b: u8) -> bool {
    (b'A'..=b'Z').contains(&b) || (b'a'..=b'z').contains(&b) || b == b'_'
}

fn is_ident_continue(b: u8) -> bool {
    is_ident_start(b) || (b'0'..=b'9').contains(&b)
}
