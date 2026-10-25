#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
    Ident(String),
    Null,

    Let, Fn, If, Else, While, Return, As, For, In, Break, Continue,
    Try, Catch, Finally, Throw,

    IntType, FloatType, BoolType, StringType,
    DateType, TimeType, DateTimeType,

    LParen, RParen, LBrace, RBrace,
    LBracket, RBracket,
    Comma, Colon, Semicolon, Arrow, Question,
    Assign, Eq, Neq, Lt, Lte, Gt, Gte,
    Plus, Minus, Star, Slash, Percent,
    And, Or, Not,

    Eof,
}

#[derive(Debug, Clone)]
pub struct Token { pub kind: TokenKind, pub line: usize, pub column: usize }

#[derive(Debug)]
pub struct LexError { pub line: usize, pub column: usize, pub msg: String }

impl std::fmt::Display for LexError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "[{}:{}] {}", self.line, self.column, self.msg)
    }
}

pub struct Lexer { chars: Vec<char>, pos: usize, line: usize, column: usize }

impl Lexer {
    pub fn new(source: &str) -> Self {
        Self { chars: source.chars().collect(), pos: 0, line: 1, column: 1 }
    }

    fn peek(&self) -> Option<char> { self.chars.get(self.pos).copied() }
    fn peek_next(&self) -> Option<char> { self.chars.get(self.pos + 1).copied() }

    fn advance(&mut self) -> Option<char> {
        let c = self.chars.get(self.pos).copied();
        if let Some(ch) = c {
            self.pos += 1;
            if ch == '\n' { self.line += 1; self.column = 1; } else { self.column += 1; }
        }
        c
    }

    pub fn tokenize(mut self) -> Result<Vec<Token>, LexError> {
        let mut tokens = Vec::new();
        loop {
            self.skip_ws_and_comments();
            let (line, column) = (self.line, self.column);
            let Some(c) = self.advance() else {
                tokens.push(Token { kind: TokenKind::Eof, line, column });
                break;
            };
            let kind = match c {
                '(' => TokenKind::LParen,
                ')' => TokenKind::RParen,
                '{' => TokenKind::LBrace,
                '}' => TokenKind::RBrace,
                '[' => TokenKind::LBracket,
                ']' => TokenKind::RBracket,
                ',' => TokenKind::Comma,
                ':' => TokenKind::Colon,
                ';' => TokenKind::Semicolon,
                '?' => TokenKind::Question,
                '+' => TokenKind::Plus,
                '*' => TokenKind::Star,
                '/' => TokenKind::Slash,
                '%' => TokenKind::Percent,
                '-' if self.peek() == Some('>') => { self.advance(); TokenKind::Arrow }
                '-' => TokenKind::Minus,
                '=' if self.peek() == Some('=') => { self.advance(); TokenKind::Eq }
                '=' => TokenKind::Assign,
                '!' if self.peek() == Some('=') => { self.advance(); TokenKind::Neq }
                '!' => TokenKind::Not,
                '<' if self.peek() == Some('=') => { self.advance(); TokenKind::Lte }
                '<' => TokenKind::Lt,
                '>' if self.peek() == Some('=') => { self.advance(); TokenKind::Gte }
                '>' => TokenKind::Gt,
                '&' if self.peek() == Some('&') => { self.advance(); TokenKind::And }
                '|' if self.peek() == Some('|') => { self.advance(); TokenKind::Or }
                '"' => self.read_string(line, column)?,
                d if d.is_ascii_digit() => self.read_number(d, line, column)?,
                c if c.is_alphabetic() || c == '_' => self.read_ident(c),
                other => return Err(LexError { line, column, msg: format!("caractere inesperado '{}'", other) }),
            };
            tokens.push(Token { kind, line, column });
        }
        Ok(tokens)
    }

    fn skip_ws_and_comments(&mut self) {
        loop {
            match self.peek() {
                Some(c) if c.is_whitespace() => { self.advance(); }
                Some('/') if self.peek_next() == Some('/') => {
                    while let Some(c) = self.peek() { if c == '\n' { break; } self.advance(); }
                }
                _ => break,
            }
        }
    }

    fn read_string(&mut self, line: usize, column: usize) -> Result<TokenKind, LexError> {
        let mut s = String::new();
        loop {
            match self.advance() {
                Some('"') => return Ok(TokenKind::String(s)),
                Some('\\') => match self.advance() {
                    Some('n') => s.push('\n'),
                    Some('t') => s.push('\t'),
                    Some('"') => s.push('"'),
                    Some('\\') => s.push('\\'),
                    _ => return Err(LexError { line, column, msg: "escape inválido".into() }),
                },
                Some(c) => s.push(c),
                None => return Err(LexError { line, column, msg: "string não terminada".into() }),
            }
        }
    }

    fn read_number(&mut self, first: char, line: usize, column: usize) -> Result<TokenKind, LexError> {
        let mut buf = String::from(first);
        let mut is_float = false;
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() { buf.push(c); self.advance(); }
            else if c == '.' && !is_float && self.peek_next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                is_float = true; buf.push(c); self.advance();
            } else { break; }
        }
        if is_float {
            buf.parse::<f64>().map(TokenKind::Float).map_err(|_| LexError { line, column, msg: "float inválido".into() })
        } else {
            buf.parse::<i64>().map(TokenKind::Int).map_err(|_| LexError { line, column, msg: "int inválido".into() })
        }
    }

    fn read_ident(&mut self, first: char) -> TokenKind {
        let mut s = String::from(first);
        loop {
            while let Some(c) = self.peek() {
                if c.is_alphanumeric() || c == '_' { s.push(c); self.advance(); } else { break; }
            }
            if self.peek() == Some('.') && self.peek_next().map(|c| c.is_alphabetic() || c == '_').unwrap_or(false) {
                s.push('.'); self.advance();
                continue;
            }
            break;
        }
        match s.as_str() {
            "let" => TokenKind::Let,
            "fn" => TokenKind::Fn,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "while" => TokenKind::While,
            "for" => TokenKind::For,
            "in" => TokenKind::In,
            "break" => TokenKind::Break,
            "continue" => TokenKind::Continue,
            "try" => TokenKind::Try,
            "catch" => TokenKind::Catch,
            "finally" => TokenKind::Finally,
            "throw" => TokenKind::Throw,
            "return" => TokenKind::Return,
            "as" => TokenKind::As,
            "true" => TokenKind::Bool(true),
            "false" => TokenKind::Bool(false),
            "null" => TokenKind::Null,
            "int" => TokenKind::IntType,
            "float" => TokenKind::FloatType,
            "bool" => TokenKind::BoolType,
            "string" => TokenKind::StringType,
            "date" => TokenKind::DateType,
            "time" => TokenKind::TimeType,
            "datetime" => TokenKind::DateTimeType,
            _ => TokenKind::Ident(s),
        }
    }
}