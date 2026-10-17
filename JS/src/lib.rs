//! Bumblebee's original JavaScript-engine foundation.
//! This crate is deliberately dependency-free. It currently implements a small,
//! tested expression-and-binding core; it is not yet an ECMAScript-complete engine.

mod lexer;
mod parser;

use std::collections::HashMap;

pub use lexer::{LexError, Token, TokenKind};
use parser::{Expr, ParseError, Parser, Stmt};

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Number(f64),
    String(String),
    Boolean(bool),
    Null,
    Undefined,
}

impl Value {
    fn truthy(&self) -> bool {
        match self {
            Self::Boolean(v) => *v,
            Self::Null | Self::Undefined => false,
            Self::Number(n) => *n != 0.0 && !n.is_nan(),
            Self::String(s) => !s.is_empty(),
        }
    }

    fn display(&self) -> String {
        match self {
            Self::Number(n) if n.fract() == 0.0 => format!("{n:.0}"),
            Self::Number(n) => n.to_string(),
            Self::String(s) => s.clone(),
            Self::Boolean(v) => v.to_string(),
            Self::Null => "null".into(),
            Self::Undefined => "undefined".into(),
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum EngineError {
    Lex(LexError),
    Parse(ParseError),
    Type(String),
    Reference(String),
    Syntax(String),
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Lex(e) => write!(f, "lex error: {e}"),
            Self::Parse(e) => write!(f, "parse error: {e}"),
            Self::Type(s) => write!(f, "type error: {s}"),
            Self::Reference(s) => write!(f, "reference error: {s}"),
            Self::Syntax(s) => write!(f, "syntax error: {s}"),
        }
    }
}
impl std::error::Error for EngineError {}
impl From<LexError> for EngineError { fn from(e: LexError) -> Self { Self::Lex(e) } }
impl From<ParseError> for EngineError { fn from(e: ParseError) -> Self { Self::Parse(e) } }

#[derive(Default)]
pub struct Runtime {
    globals: HashMap<String, Value>,
    output: Vec<String>,
}

impl Runtime {
    pub fn new() -> Self { Self::default() }

    /// Execute the supported JavaScript subset and return the completion value.
    pub fn execute(&mut self, source: &str) -> Result<Value, EngineError> {
        let tokens = lexer::tokenize(source)?;
        let program = Parser::new(tokens).parse_program()?;
        let mut last = Value::Undefined;
        for statement in program {
            last = self.execute_statement(statement)?;
        }
        Ok(last)
    }

    /// Output produced by the engine's deliberately small built-in console.log.
    pub fn take_output(&mut self) -> Vec<String> { std::mem::take(&mut self.output) }

    pub fn get_global(&self, name: &str) -> Option<&Value> { self.globals.get(name) }

    fn execute_statement(&mut self, stmt: Stmt) -> Result<Value, EngineError> {
        match stmt {
            Stmt::Expression(expr) => self.evaluate(expr),
            Stmt::Declaration { name, value, constant: _ } => {
                if self.globals.contains_key(&name) {
                    return Err(EngineError::Syntax(format!("'{}' has already been declared", name)));
                }
                let value = self.evaluate(value)?;
                self.globals.insert(name, value.clone());
                Ok(value)
            }
        }
    }

    fn evaluate(&mut self, expr: Expr) -> Result<Value, EngineError> {
        match expr {
            Expr::Literal(v) => Ok(v),
            Expr::Identifier(name) => self.globals.get(&name).cloned()
                .ok_or_else(|| EngineError::Reference(format!("{name} is not defined"))),
            Expr::Unary { op, value } => {
                let value = self.evaluate(*value)?;
                match op.as_str() {
                    "!" => Ok(Value::Boolean(!value.truthy())),
                    "+" => self.to_number(value).map(Value::Number),
                    "-" => self.to_number(value).map(|n| Value::Number(-n)),
                    _ => Err(EngineError::Syntax(format!("unsupported unary operator {op}"))),
                }
            }
            Expr::Binary { op, left, right } => {
                let left = self.evaluate(*left)?;
                if op == "&&" && !left.truthy() { return Ok(left); }
                if op == "||" && left.truthy() { return Ok(left); }
                let right = self.evaluate(*right)?;
                self.binary(op.as_str(), left, right)
            }
        }
    }

    fn to_number(&self, value: Value) -> Result<f64, EngineError> {
        match value {
            Value::Number(n) => Ok(n),
            Value::Boolean(v) => Ok(if v { 1.0 } else { 0.0 }),
            Value::Null => Ok(0.0),
            Value::String(s) if s.trim().is_empty() => Ok(0.0),
            Value::String(s) => s.trim().parse().map_err(|_| EngineError::Type("cannot convert string to number".into())),
            Value::Undefined => Ok(f64::NAN),
        }
    }

    fn binary(&self, op: &str, left: Value, right: Value) -> Result<Value, EngineError> {
        if op == "+" {
            if matches!(left, Value::String(_)) || matches!(right, Value::String(_)) {
                return Ok(Value::String(format!("{}{}", left.display(), right.display())));
            }
        }
        if op == "===" || op == "==" { return Ok(Value::Boolean(left == right)); }
        if op == "!==" || op == "!=" { return Ok(Value::Boolean(left != right)); }
        if op == "&&" { return Ok(if left.truthy() { right } else { left }); }
        if op == "||" { return Ok(if left.truthy() { left } else { right }); }
        let a = self.to_number(left)?;
        let b = self.to_number(right)?;
        let result = match op {
            "+" => Value::Number(a + b),
            "-" => Value::Number(a - b),
            "*" => Value::Number(a * b),
            "/" => Value::Number(a / b),
            "%" => Value::Number(a % b),
            "**" => Value::Number(a.powf(b)),
            "<" => Value::Boolean(a < b),
            "<=" => Value::Boolean(a <= b),
            ">" => Value::Boolean(a > b),
            ">=" => Value::Boolean(a >= b),
            _ => return Err(EngineError::Syntax(format!("unsupported binary operator {op}"))),
        };
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operator_precedence_and_parentheses() {
        let mut rt = Runtime::new();
        assert_eq!(rt.execute("1 + 2 * (3 + 4);").unwrap(), Value::Number(15.0));
    }

    #[test]
    fn declarations_persist_between_scripts() {
        let mut rt = Runtime::new();
        rt.execute("let answer = 40 + 2;").unwrap();
        assert_eq!(rt.execute("answer").unwrap(), Value::Number(42.0));
    }

    #[test]
    fn strings_concatenate() {
        let mut rt = Runtime::new();
        assert_eq!(rt.execute("'bumble' + 'bee'").unwrap(), Value::String("bumblebee".into()));
    }

    #[test]
    fn logical_operators_short_circuit_result_values() {
        let mut rt = Runtime::new();
        assert_eq!(rt.execute("0 || 7").unwrap(), Value::Number(7.0));
        assert_eq!(rt.execute("5 && 9").unwrap(), Value::Number(9.0));
    }

    #[test]
    fn unknown_identifiers_are_reported() {
        let mut rt = Runtime::new();
        assert!(matches!(rt.execute("missingName"), Err(EngineError::Reference(_))));
    }
}
