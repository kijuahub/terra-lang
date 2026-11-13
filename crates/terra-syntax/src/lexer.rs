use crate::token::{Token, TokenKind};
use terra_diagnostics::{DiagKind, Diagnostic, Span};

pub fn lex(file: u32, source: &str) -> Result<Vec<Token>, Diagnostic> {
    Lexer {
        file,
        src: source,
        i: 0,
    }
    .run()
}

struct Lexer<'a> {
    file: u32,
    src: &'a str,
    i: usize,
}

impl<'a> Lexer<'a> {
    fn run(&mut self) -> Result<Vec<Token>, Diagnostic> {
        if self.peek() == Some('\u{feff}') {
            self.bump();
        }
        let mut tokens = Vec::new();
        loop {
            self.skip_ws_and_comments()?;
            let start = self.i as u32;
            let Some(c) = self.peek() else {
                tokens.push(self.tok(TokenKind::Eof, start));
                break;
            };
            let kind = match c {
                '(' => {
                    self.bump();
                    TokenKind::LParen
                }
                ')' => {
                    self.bump();
                    TokenKind::RParen
                }
                '{' => {
                    self.bump();
                    TokenKind::LBrace
                }
                '}' => {
                    self.bump();
                    TokenKind::RBrace
                }
                '[' => {
                    self.bump();
                    TokenKind::LBracket
                }
                ']' => {
                    self.bump();
                    TokenKind::RBracket
                }
                ',' => {
                    self.bump();
                    TokenKind::Comma
                }
                ';' => {
                    self.bump();
                    TokenKind::Semi
                }
                ':' => {
                    self.bump();
                    TokenKind::Colon
                }
                '+' => {
                    self.bump();
                    TokenKind::Plus
                }
                '*' => {
                    self.bump();
                    TokenKind::Star
                }
                '%' => {
                    self.bump();
                    TokenKind::Percent
                }
                '.' => {
                    self.bump();
                    if self.peek() == Some('.') {
                        self.bump();
                        TokenKind::DotDot
                    } else {
                        TokenKind::Dot
                    }
                }
                '-' => {
                    self.bump();
                    if self.peek() == Some('>') {
                        self.bump();
                        TokenKind::Arrow
                    } else {
                        TokenKind::Minus
                    }
                }
                '=' => {
                    self.bump();
                    if self.peek() == Some('=') {
                        self.bump();
                        TokenKind::EqEq
                    } else if self.peek() == Some('>') {
                        self.bump();
                        TokenKind::FatArrow
                    } else {
                        TokenKind::Eq
                    }
                }
                '!' => {
                    self.bump();
                    if self.peek() == Some('=') {
                        self.bump();
                        TokenKind::NotEq
                    } else {
                        TokenKind::Not
                    }
                }
                '<' => {
                    self.bump();
                    if self.peek() == Some('=') {
                        self.bump();
                        TokenKind::Le
                    } else {
                        TokenKind::Lt
                    }
                }
                '>' => {
                    self.bump();
                    if self.peek() == Some('=') {
                        self.bump();
                        TokenKind::Ge
                    } else {
                        TokenKind::Gt
                    }
                }
                '&' => {
                    self.bump();
                    if self.peek() == Some('&') {
                        self.bump();
                        TokenKind::AndAnd
                    } else {
                        return Err(self.err(start, DiagKind::UnexpectedToken {
                            found: "&".into(),
                            expected: "&&".into(),
                        }));
                    }
                }
                '|' => {
                    self.bump();
                    if self.peek() == Some('|') {
                        self.bump();
                        TokenKind::OrOr
                    } else {
                        return Err(self.err(start, DiagKind::UnexpectedToken {
                            found: "|".into(),
                            expected: "||".into(),
                        }));
                    }
                }
                '/' => {
                    self.bump();
                    TokenKind::Slash
                }
                '"' => self.string(start)?,
                c if c.is_ascii_digit() => self.number(start)?,
                c if is_ident_start(c) => self.ident(start),
                other => {
                    return Err(self.err(
                        start,
                        DiagKind::UnexpectedToken {
                            found: other.to_string(),
                            expected: "token".into(),
                        },
                    ));
                }
            };
            tokens.push(self.tok(kind, start));
        }
        Ok(tokens)
    }

    fn string(&mut self, start: u32) -> Result<TokenKind, Diagnostic> {
        self.bump();
        let mut out = String::new();
        loop {
            match self.peek() {
                None => {
                    return Err(self.err(start, DiagKind::UnterminatedString));
                }
                Some('"') => {
                    self.bump();
                    break;
                }
                Some('\\') => {
                    self.bump();
                    let esc = self.peek().ok_or_else(|| self.err(start, DiagKind::UnterminatedString))?;
                    self.bump();
                    let ch = match esc {
                        'n' => '\n',
                        't' => '\t',
                        'r' => '\r',
                        '\\' => '\\',
                        '"' => '"',
                        '0' => '\0',
                        _ => {
                            return Err(self.err(self.i as u32, DiagKind::InvalidEscape));
                        }
                    };
                    out.push(ch);
                }
                Some(c) => {
                    self.bump();
                    out.push(c);
                }
            }
        }
        Ok(TokenKind::StringLit(out))
    }

    fn number(&mut self, start: u32) -> Result<TokenKind, Diagnostic> {
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.bump();
        }
        let mut is_float = false;
        if self.peek() == Some('.') && self.peek_at(1).is_some_and(|c| c.is_ascii_digit()) {
            is_float = true;
            self.bump();
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                self.bump();
            }
        }
        if matches!(self.peek(), Some('e') | Some('E')) {
            is_float = true;
            self.bump();
            if matches!(self.peek(), Some('+') | Some('-')) {
                self.bump();
            }
            if !self.peek().is_some_and(|c| c.is_ascii_digit()) {
                let literal = self.src[start as usize..self.i].to_string();
                return Err(self.err(start, DiagKind::InvalidNumber { literal }));
            }
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                self.bump();
            }
        }
        let mut force_float = false;
        if matches!(self.peek(), Some('f') | Some('F')) {
            force_float = true;
            is_float = true;
            self.bump();
        }
        let end = self.i;
        let literal = &self.src[start as usize..end];
        let parsed = literal.trim_end_matches(['f', 'F']);
        if force_float {
            let n: f32 = parsed.parse().map_err(|_| {
                self.err(start, DiagKind::InvalidNumber {
                    literal: literal.into(),
                })
            })?;
            Ok(TokenKind::Float(n))
        } else if is_float {
            let n: f64 = parsed.parse().map_err(|_| {
                self.err(start, DiagKind::InvalidNumber {
                    literal: literal.into(),
                })
            })?;
            Ok(TokenKind::Double(n))
        } else {
            let n: i64 = parsed.parse().map_err(|_| {
                self.err(start, DiagKind::InvalidNumber {
                    literal: literal.into(),
                })
            })?;
            Ok(TokenKind::Int(n))
        }
    }

    fn ident(&mut self, start: u32) -> TokenKind {
        self.bump();
        while self.peek().is_some_and(is_ident_cont) {
            self.bump();
        }
        let text = &self.src[start as usize..self.i];
        keyword(text).unwrap_or_else(|| {
            if text == "_" {
                TokenKind::Underscore
            } else {
                TokenKind::Ident(text.to_string())
            }
        })
    }

    fn skip_ws_and_comments(&mut self) -> Result<(), Diagnostic> {
        loop {
            match self.peek() {
                Some(c) if c.is_whitespace() => {
                    self.bump();
                }
                Some('/') if self.peek_at(1) == Some('/') => {
                    self.bump();
                    self.bump();
                    while self.peek().is_some_and(|c| c != '\n') {
                        self.bump();
                    }
                }
                Some('/') if self.peek_at(1) == Some('*') => {
                    let start = self.i as u32;
                    self.bump();
                    self.bump();
                    let mut closed = false;
                    while let Some(c) = self.peek() {
                        if c == '*' && self.peek_at(1) == Some('/') {
                            self.bump();
                            self.bump();
                            closed = true;
                            break;
                        }
                        self.bump();
                    }
                    if !closed {
                        return Err(self.err(start, DiagKind::UnterminatedComment));
                    }
                }
                _ => return Ok(()),
            }
        }
    }

    fn tok(&self, kind: TokenKind, start: u32) -> Token {
        Token {
            kind,
            span: Span::new(self.file, start, self.i as u32),
        }
    }

    fn err(&self, start: u32, kind: DiagKind) -> Diagnostic {
        Diagnostic::error(Span::new(self.file, start, self.i.max(start as usize) as u32), kind)
    }

    fn peek(&self) -> Option<char> {
        self.src[self.i..].chars().next()
    }

    fn peek_at(&self, n: usize) -> Option<char> {
        self.src[self.i..].chars().nth(n)
    }

    fn bump(&mut self) {
        if let Some(c) = self.peek() {
            self.i += c.len_utf8();
        }
    }
}

fn is_ident_start(c: char) -> bool {
    c == '_' || c.is_alphabetic()
}

fn is_ident_cont(c: char) -> bool {
    c == '_' || c.is_alphanumeric()
}

fn keyword(s: &str) -> Option<TokenKind> {
    Some(match s {
        "class" => TokenKind::Class,
        "interface" => TokenKind::Interface,
        "extends" => TokenKind::Extends,
        "implements" => TokenKind::Implements,
        "super" => TokenKind::Super,
        "self" => TokenKind::SelfKw,
        "override" => TokenKind::Override,
        "pub" => TokenKind::Pub,
        "priv" => TokenKind::Priv,
        "static" => TokenKind::Static,
        "final" => TokenKind::Final,
        "const" => TokenKind::Const,
        "var" => TokenKind::Var,
        "mut" => TokenKind::Mut,
        "enum" => TokenKind::Enum,
        "try" => TokenKind::Try,
        "exception" => TokenKind::Exception,
        "throw" => TokenKind::Throw,
        "if" => TokenKind::If,
        "else" => TokenKind::Else,
        "match" => TokenKind::Match,
        "loop" => TokenKind::Loop,
        "while" => TokenKind::While,
        "for" => TokenKind::For,
        "in" => TokenKind::In,
        "break" => TokenKind::Break,
        "continue" => TokenKind::Continue,
        "func" => TokenKind::Func,
        "return" => TokenKind::Return,
        "async" => TokenKind::Async,
        "await" => TokenKind::Await,
        "true" => TokenKind::True,
        "false" => TokenKind::False,
        "do" => TokenKind::Do,
        "package" => TokenKind::Package,
        "use" => TokenKind::Use,
        "new" => TokenKind::New,
        "delete" => TokenKind::Delete,
        "null" => TokenKind::Null,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexes_function_sample() {
        let src = "func main(name: string, age: int) {\n    println(name);\n}\n";
        let tokens = lex(0, src).unwrap();
        assert!(matches!(tokens[0].kind, TokenKind::Func));
        assert!(tokens.iter().any(|t| matches!(t.kind, TokenKind::Semi)));
        assert!(matches!(tokens.last().unwrap().kind, TokenKind::Eof));
    }

    #[test]
    fn numbers_and_range() {
        let tokens = lex(0, "1 3.14 2.5f 0..10").unwrap();
        assert!(matches!(tokens[0].kind, TokenKind::Int(1)));
        assert!(matches!(tokens[1].kind, TokenKind::Double(n) if (n - 3.14).abs() < 1e-9));
        assert!(matches!(tokens[2].kind, TokenKind::Float(_)));
        assert!(matches!(tokens[3].kind, TokenKind::Int(0)));
        assert!(matches!(tokens[4].kind, TokenKind::DotDot));
        assert!(matches!(tokens[5].kind, TokenKind::Int(10)));
    }

    #[test]
    fn string_escape_and_comment() {
        let tokens = lex(0, "\"a\\nb\" // c\n/* d */ true").unwrap();
        match &tokens[0].kind {
            TokenKind::StringLit(s) => assert_eq!(s, "a\nb"),
            _ => panic!("string"),
        }
        assert!(matches!(tokens[1].kind, TokenKind::True));
    }
}
