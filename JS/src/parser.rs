use crate::{lexer::{Token, TokenKind}, Value};
use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub struct ParseError { pub offset: usize, pub message: String }
impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{} at byte {}", self.message, self.offset) }
}
impl std::error::Error for ParseError {}

#[derive(Debug, PartialEq)]
pub(crate) enum Stmt { Expression(Expr), Declaration { name: String, value: Expr, constant: bool } }
#[derive(Debug, PartialEq)]
pub(crate) enum Expr {
    Literal(Value), Identifier(String),
    Unary { op: String, value: Box<Expr> },
    Binary { op: String, left: Box<Expr>, right: Box<Expr> },
}

pub(crate) struct Parser { tokens: Vec<Token>, cursor: usize }
impl Parser {
    pub(crate) fn new(tokens: Vec<Token>) -> Self { Self { tokens, cursor: 0 } }

    pub(crate) fn parse_program(&mut self) -> Result<Vec<Stmt>, ParseError> {
        let mut out = Vec::new();
        while !matches!(self.peek(), TokenKind::Eof) {
            if matches!(self.peek(), TokenKind::Semicolon) { self.advance(); continue; }
            out.push(self.statement()?);
            if matches!(self.peek(), TokenKind::Semicolon) { self.advance(); }
            else if !matches!(self.peek(), TokenKind::Eof) { return Err(self.error("expected ';' or end of input")); }
        }
        Ok(out)
    }

    fn statement(&mut self) -> Result<Stmt, ParseError> {
        let constant = match self.peek() {
            TokenKind::Let => { self.advance(); Some(false) },
            TokenKind::Const => { self.advance(); Some(true) },
            _ => None,
        };
        if let Some(constant) = constant {
            let name = match self.advance().kind.clone() {
                TokenKind::Identifier(n) => n,
                _ => return Err(self.error("expected binding name")),
            };
            if !matches!(self.peek(), TokenKind::Equal) {
                return Err(self.error("expected '=' after binding name"));
            }
            self.advance();
            let value = self.expression(0)?;
            return Ok(Stmt::Declaration { name, value, constant });
        }
        Ok(Stmt::Expression(self.expression(0)?))
    }

    fn expression(&mut self, min_bp: u8) -> Result<Expr, ParseError> {
        let token = self.advance().clone();
        let mut lhs = match token.kind {
            TokenKind::Number(n) => Expr::Literal(Value::Number(n)),
            TokenKind::String(s) => Expr::Literal(Value::String(s)),
            TokenKind::True => Expr::Literal(Value::Boolean(true)),
            TokenKind::False => Expr::Literal(Value::Boolean(false)),
            TokenKind::Null => Expr::Literal(Value::Null),
            TokenKind::Undefined => Expr::Literal(Value::Undefined),
            TokenKind::Identifier(n) => Expr::Identifier(n),
            TokenKind::Bang => Expr::Unary { op: "!".into(), value: Box::new(self.expression(13)?) },
            TokenKind::Minus => Expr::Unary { op: "-".into(), value: Box::new(self.expression(13)?) },
            TokenKind::Plus => Expr::Unary { op: "+".into(), value: Box::new(self.expression(13)?) },
            TokenKind::LeftParen => {
                let e = self.expression(0)?;
                if !matches!(self.peek(), TokenKind::RightParen) { return Err(self.error("expected ')'")); }
                self.advance(); e
            }
            _ => return Err(ParseError { offset: token.offset, message: "expected expression".into() }),
        };
        loop {
            let (op, left_bp, right_bp) = match self.peek() {
                TokenKind::OrOr => ("||", 1, 2), TokenKind::AndAnd => ("&&", 3, 4),
                TokenKind::EqualEqual => ("==", 5, 6), TokenKind::StrictEqual => ("===", 5, 6),
                TokenKind::BangEqual => ("!=", 5, 6), TokenKind::StrictBangEqual => ("!==", 5, 6),
                TokenKind::Less => ("<", 7, 8), TokenKind::LessEqual => ("<=", 7, 8),
                TokenKind::Greater => (">", 7, 8), TokenKind::GreaterEqual => (">=", 7, 8),
                TokenKind::Plus => ("+", 9, 10), TokenKind::Minus => ("-", 9, 10),
                TokenKind::Star => ("*", 11, 12), TokenKind::Slash => ("/", 11, 12),
                TokenKind::Percent => ("%", 11, 12), TokenKind::Power => ("**", 14, 13),
                _ => break,
            };
            if left_bp < min_bp { break; }
            self.advance();
            let rhs = self.expression(right_bp)?;
            lhs = Expr::Binary { op: op.into(), left: Box::new(lhs), right: Box::new(rhs) };
        }
        Ok(lhs)
    }

    fn peek(&self) -> &TokenKind { &self.tokens[self.cursor].kind }
    fn advance(&mut self) -> &Token { let i = self.cursor; if self.cursor + 1 < self.tokens.len() { self.cursor += 1; } &self.tokens[i] }
    fn error(&self, message: &str) -> ParseError { ParseError { offset: self.tokens[self.cursor].offset, message: message.into() } }
}