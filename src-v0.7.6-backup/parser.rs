use crate::lexer::{Token, TokenKind};

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Int, Float, Bool, String,
    Date, Time, DateTime,
    Connection, Row,
    List(Box<Type>),
    Any,
    Nullable(Box<Type>),
}

#[derive(Debug, Clone)]
pub enum Expr {
    IntLit(i64), FloatLit(f64), BoolLit(bool), StringLit(String), NullLit,
    Ident(String),
    ArrayLit(Vec<Expr>),
    Binary { left: Box<Expr>, op: BinOp, right: Box<Expr> },
    Unary { op: UnOp, operand: Box<Expr> },
    Call { callee: Box<Expr>, args: Vec<Expr> },
    Cast { expr: Box<Expr>, ty: Type },
    Index { obj: Box<Expr>, index: Box<Expr> },
}

#[derive(Debug, Clone, Copy)]
pub enum BinOp { Add, Sub, Mul, Div, Mod, Eq, Neq, Lt, Lte, Gt, Gte, And, Or }

#[derive(Debug, Clone, Copy)]
pub enum UnOp { Neg, Not }

#[derive(Debug, Clone)]
pub struct Param { pub name: String, pub ty: Type }

#[derive(Debug, Clone)]
pub struct FnDecl {
    pub name: String,
    pub params: Vec<Param>,
    pub ret: Option<Type>,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Let { name: String, ty: Option<Type>, value: Expr },
    Assign { name: String, value: Expr },
    Expr(Expr),
    If { cond: Expr, then: Vec<Stmt>, else_: Option<Vec<Stmt>> },
    While { cond: Expr, body: Vec<Stmt> },
    For { var: String, iter: Expr, body: Vec<Stmt> },
    Break, Continue,
    Try { body: Vec<Stmt>, err_var: String, catch: Vec<Stmt>, finally: Option<Vec<Stmt>> },
    Throw(Expr),
    Block(Vec<Stmt>),
    Fn(FnDecl),
    Return(Option<Expr>),
}

#[derive(Debug)]
pub struct ParseError { pub line: usize, pub column: usize, pub msg: String }

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "[{}:{}] {}", self.line, self.column, self.msg)
    }
}

pub struct Parser { tokens: Vec<Token>, pos: usize }

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self { Self { tokens, pos: 0 } }
    fn peek(&self) -> &TokenKind { &self.tokens[self.pos].kind }
    fn peek_at(&self, offset: usize) -> &TokenKind {
        self.tokens.get(self.pos + offset).map(|t| &t.kind).unwrap_or(&TokenKind::Eof)
    }
    fn advance(&mut self) -> TokenKind {
        let k = self.tokens[self.pos].kind.clone();
        if !matches!(k, TokenKind::Eof) { self.pos += 1; }
        k
    }
    fn here(&self) -> (usize, usize) { let t = &self.tokens[self.pos]; (t.line, t.column) }
    fn error<T>(&self, msg: impl Into<String>) -> Result<T, ParseError> {
        let (line, column) = self.here();
        Err(ParseError { line, column, msg: msg.into() })
    }
    fn expect(&mut self, expected: TokenKind) -> Result<(), ParseError> {
        if self.peek() == &expected { self.advance(); Ok(()) }
        else { self.error(format!("esperado {:?}, encontrado {:?}", expected, self.peek())) }
    }

    pub fn parse_program(&mut self) -> Result<Vec<Stmt>, ParseError> {
        let mut stmts = Vec::new();
        while !matches!(self.peek(), TokenKind::Eof) { stmts.push(self.parse_stmt()?); }
        Ok(stmts)
    }

    fn parse_stmt(&mut self) -> Result<Stmt, ParseError> {
        match self.peek() {
            TokenKind::Let => self.parse_let(),
            TokenKind::If => self.parse_if(),
            TokenKind::While => self.parse_while(),
            TokenKind::For => self.parse_for(),
            TokenKind::Fn => self.parse_fn(),
            TokenKind::Try => self.parse_try(),
            TokenKind::Throw => {
                self.advance();
                let e = self.parse_expr()?;
                self.expect(TokenKind::Semicolon)?;
                Ok(Stmt::Throw(e))
            }
            TokenKind::Break => { self.advance(); self.expect(TokenKind::Semicolon)?; Ok(Stmt::Break) }
            TokenKind::Continue => { self.advance(); self.expect(TokenKind::Semicolon)?; Ok(Stmt::Continue) }
            TokenKind::Return => {
                self.advance();
                if matches!(self.peek(), TokenKind::Semicolon) {
                    self.advance();
                    Ok(Stmt::Return(None))
                } else {
                    let e = self.parse_expr()?;
                    self.expect(TokenKind::Semicolon)?;
                    Ok(Stmt::Return(Some(e)))
                }
            }
            TokenKind::LBrace => { let body = self.parse_block()?; Ok(Stmt::Block(body)) }
            _ => {
                let expr = self.parse_expr()?;
                if matches!(self.peek(), TokenKind::Assign) {
                    self.advance();
                    if let Expr::Ident(name) = expr {
                        let value = self.parse_expr()?;
                        self.expect(TokenKind::Semicolon)?;
                        Ok(Stmt::Assign { name, value })
                    } else { self.error("alvo de atribuição inválido") }
                } else {
                    self.expect(TokenKind::Semicolon)?;
                    Ok(Stmt::Expr(expr))
                }
            }
        }
    }

    fn parse_let(&mut self) -> Result<Stmt, ParseError> {
        self.expect(TokenKind::Let)?;
        let name = match self.advance() {
            TokenKind::Ident(s) => s,
            _ => return self.error("esperado nome de variável"),
        };
        let ty = if matches!(self.peek(), TokenKind::Colon) {
            self.advance();
            Some(self.parse_type()?)
        } else { None };
        self.expect(TokenKind::Assign)?;
        let value = self.parse_expr()?;
        self.expect(TokenKind::Semicolon)?;
        Ok(Stmt::Let { name, ty, value })
    }

    fn parse_type(&mut self) -> Result<Type, ParseError> {
        let base = match self.advance() {
            TokenKind::IntType => Type::Int,
            TokenKind::FloatType => Type::Float,
            TokenKind::BoolType => Type::Bool,
            TokenKind::StringType => Type::String,
            TokenKind::DateType => Type::Date,
            TokenKind::TimeType => Type::Time,
            TokenKind::DateTimeType => Type::DateTime,
            _ => return self.error("esperado tipo"),
        };
        if matches!(self.peek(), TokenKind::Question) {
            self.advance();
            Ok(Type::Nullable(Box::new(base)))
        } else { Ok(base) }
    }

    fn parse_if(&mut self) -> Result<Stmt, ParseError> {
        self.expect(TokenKind::If)?;
        let cond = self.parse_expr()?;
        let then = self.parse_block()?;
        let else_ = if matches!(self.peek(), TokenKind::Else) {
            self.advance();
            if matches!(self.peek(), TokenKind::If) {
                let inner = self.parse_if()?;
                Some(vec![inner])
            } else { Some(self.parse_block()?) }
        } else { None };
        Ok(Stmt::If { cond, then, else_ })
    }

    fn parse_while(&mut self) -> Result<Stmt, ParseError> {
        self.expect(TokenKind::While)?;
        let cond = self.parse_expr()?;
        let body = self.parse_block()?;
        Ok(Stmt::While { cond, body })
    }

    fn parse_for(&mut self) -> Result<Stmt, ParseError> {
        self.expect(TokenKind::For)?;
        let var = match self.advance() {
            TokenKind::Ident(s) => s,
            _ => return self.error("esperado nome de variável após 'for'"),
        };
        self.expect(TokenKind::In)?;
        let iter = self.parse_expr()?;
        let body = self.parse_block()?;
        Ok(Stmt::For { var, iter, body })
    }

    /// `try { } catch e { }` (clássico)
    /// `try { } catch e { } finally { }` (com finally)
    /// `try { } finally { }` (sem catch — usa sentinela "_")
    fn parse_try(&mut self) -> Result<Stmt, ParseError> {
        self.expect(TokenKind::Try)?;
        let body = self.parse_block()?;

        // catch é opcional SE houver finally depois
        let (err_var, catch) = if matches!(self.peek(), TokenKind::Catch) {
            self.advance();
            let var = match self.advance() {
                TokenKind::Ident(s) => s,
                _ => return self.error("esperado nome de variável após 'catch'"),
            };
            let catch_block = self.parse_block()?;
            (var, catch_block)
        } else {
            // sem catch: usa sentinela "_" (vai propagar erro após o finally)
            ("_".to_string(), Vec::new())
        };

        let finally = if matches!(self.peek(), TokenKind::Finally) {
            self.advance();
            Some(self.parse_block()?)
        } else {
            None
        };

        // Precisa de pelo menos um dos dois: catch ou finally
        if finally.is_none() && err_var == "_" {
            return self.error("'try' precisa de pelo menos 'catch' ou 'finally'");
        }

        Ok(Stmt::Try { body, err_var, catch, finally })
    }

    fn parse_fn(&mut self) -> Result<Stmt, ParseError> {
        self.expect(TokenKind::Fn)?;
        let name = match self.advance() {
            TokenKind::Ident(s) => s,
            _ => return self.error("esperado nome de função"),
        };
        self.expect(TokenKind::LParen)?;
        let mut params = Vec::new();
        while !matches!(self.peek(), TokenKind::RParen) {
            let pname = match self.advance() {
                TokenKind::Ident(s) => s,
                _ => return self.error("esperado nome de parâmetro"),
            };
            self.expect(TokenKind::Colon)?;
            let pty = self.parse_type()?;
            params.push(Param { name: pname, ty: pty });
            if matches!(self.peek(), TokenKind::Comma) { self.advance(); }
        }
        self.expect(TokenKind::RParen)?;
        let ret = if matches!(self.peek(), TokenKind::Arrow) {
            self.advance();
            Some(self.parse_type()?)
        } else { None };
        let body = self.parse_block()?;
        Ok(Stmt::Fn(FnDecl { name, params, ret, body }))
    }

    fn parse_block(&mut self) -> Result<Vec<Stmt>, ParseError> {
        self.expect(TokenKind::LBrace)?;
        let mut stmts = Vec::new();
        while !matches!(self.peek(), TokenKind::RBrace) {
            if matches!(self.peek(), TokenKind::Eof) {
                return self.error("bloco não fechado com '}'");
            }
            stmts.push(self.parse_stmt()?);
        }
        self.expect(TokenKind::RBrace)?;
        Ok(stmts)
    }

    fn parse_expr(&mut self) -> Result<Expr, ParseError> { self.parse_or() }

    fn parse_or(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_and()?;
        while matches!(self.peek(), TokenKind::Or) {
            self.advance();
            let right = self.parse_and()?;
            left = Expr::Binary { left: Box::new(left), op: BinOp::Or, right: Box::new(right) };
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_eq()?;
        while matches!(self.peek(), TokenKind::And) {
            self.advance();
            let right = self.parse_eq()?;
            left = Expr::Binary { left: Box::new(left), op: BinOp::And, right: Box::new(right) };
        }
        Ok(left)
    }

    fn parse_eq(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_cmp()?;
        loop {
            let op = match self.peek() {
                TokenKind::Eq => BinOp::Eq,
                TokenKind::Neq => BinOp::Neq,
                _ => break,
            };
            self.advance();
            let right = self.parse_cmp()?;
            left = Expr::Binary { left: Box::new(left), op, right: Box::new(right) };
        }
        Ok(left)
    }

    fn parse_cmp(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_term()?;
        loop {
            let op = match self.peek() {
                TokenKind::Lt => BinOp::Lt,
                TokenKind::Lte => BinOp::Lte,
                TokenKind::Gt => BinOp::Gt,
                TokenKind::Gte => BinOp::Gte,
                _ => break,
            };
            self.advance();
            let right = self.parse_term()?;
            left = Expr::Binary { left: Box::new(left), op, right: Box::new(right) };
        }
        Ok(left)
    }

    fn parse_term(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_factor()?;
        loop {
            let op = match self.peek() {
                TokenKind::Plus => BinOp::Add,
                TokenKind::Minus => BinOp::Sub,
                _ => break,
            };
            self.advance();
            let right = self.parse_factor()?;
            left = Expr::Binary { left: Box::new(left), op, right: Box::new(right) };
        }
        Ok(left)
    }

    fn parse_factor(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_unary()?;
        loop {
            let op = match self.peek() {
                TokenKind::Star => BinOp::Mul,
                TokenKind::Slash => BinOp::Div,
                TokenKind::Percent => BinOp::Mod,
                _ => break,
            };
            self.advance();
            let right = self.parse_unary()?;
            left = Expr::Binary { left: Box::new(left), op, right: Box::new(right) };
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr, ParseError> {
        match self.peek() {
            TokenKind::Minus => {
                self.advance();
                let e = self.parse_unary()?;
                Ok(Expr::Unary { op: UnOp::Neg, operand: Box::new(e) })
            }
            TokenKind::Not => {
                self.advance();
                let e = self.parse_unary()?;
                Ok(Expr::Unary { op: UnOp::Not, operand: Box::new(e) })
            }
            _ => self.parse_postfix(),
        }
    }

    fn parse_postfix(&mut self) -> Result<Expr, ParseError> {
        let mut e = self.parse_primary()?;
        loop {
            match self.peek() {
                TokenKind::LParen => {
                    self.advance();
                    let mut args = Vec::new();
                    while !matches!(self.peek(), TokenKind::RParen) {
                        args.push(self.parse_expr()?);
                        if matches!(self.peek(), TokenKind::Comma) { self.advance(); }
                    }
                    self.expect(TokenKind::RParen)?;
                    e = Expr::Call { callee: Box::new(e), args };
                }
                TokenKind::As => {
                    self.advance();
                    let ty = self.parse_type()?;
                    e = Expr::Cast { expr: Box::new(e), ty };
                }
                TokenKind::LBracket => {
                    self.advance();
                    let idx = self.parse_expr()?;
                    self.expect(TokenKind::RBracket)?;
                    e = Expr::Index { obj: Box::new(e), index: Box::new(idx) };
                }
                _ => break,
            }
        }
        Ok(e)
    }

    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        let _ = self.peek_at(0);
        match self.advance() {
            TokenKind::Int(i) => Ok(Expr::IntLit(i)),
            TokenKind::Float(f) => Ok(Expr::FloatLit(f)),
            TokenKind::Bool(b) => Ok(Expr::BoolLit(b)),
            TokenKind::String(s) => Ok(Expr::StringLit(s)),
            TokenKind::Null => Ok(Expr::NullLit),
            TokenKind::Ident(name) => Ok(Expr::Ident(name)),
            TokenKind::LBracket => {
                let mut items = Vec::new();
                if !matches!(self.peek(), TokenKind::RBracket) {
                    loop {
                        items.push(self.parse_expr()?);
                        if matches!(self.peek(), TokenKind::Comma) {
                            self.advance();
                            if matches!(self.peek(), TokenKind::RBracket) { break; }
                        } else { break; }
                    }
                }
                self.expect(TokenKind::RBracket)?;
                Ok(Expr::ArrayLit(items))
            }
            TokenKind::LParen => {
                let e = self.parse_expr()?;
                self.expect(TokenKind::RParen)?;
                Ok(e)
            }
            other => self.error(format!("expressão inesperada: {:?}", other)),
        }
    }
}