//! Original Rust JavaScript-runtime foundation. This remains an incomplete ECMAScript subset.
mod lexer;
mod parser;

use std::collections::HashMap;
pub use lexer::{LexError, Token, TokenKind};
use parser::{Expr, ParseError, Parser, Stmt};

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Number(f64), String(String), Boolean(bool), Null, Undefined,
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
    Lex(LexError), Parse(ParseError), Type(String), Reference(String), Syntax(String),
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

#[derive(Clone, Debug)]
struct Binding { value: Value, constant: bool }

/// Runtime with persistent global bindings and lexical block scopes.
pub struct Runtime {
    scopes: Vec<HashMap<String, Binding>>,
    output: Vec<String>,
}
impl Default for Runtime {
    fn default() -> Self {
        Self { scopes: vec![HashMap::new()], output: Vec::new() }
    }
}
impl Runtime {
    pub fn new() -> Self { Self::default() }

    pub fn execute(&mut self, source: &str) -> Result<Value, EngineError> {
        let program = Parser::new(lexer::tokenize(source)?).parse_program()?;
        self.execute_statements(program)
    }
    pub fn take_output(&mut self) -> Vec<String> { std::mem::take(&mut self.output) }

    /// Returns a reference to a global binding, if present.
    pub fn get_global(&self, name: &str) -> Option<&Value> {
        self.scopes.first()?.get(name).map(|binding| &binding.value)
    }

    fn execute_statements(&mut self, statements: Vec<Stmt>) -> Result<Value, EngineError> {
        let mut last = Value::Undefined;
        for statement in statements { last = self.execute_statement(statement)?; }
        Ok(last)
    }
    fn execute_scoped(&mut self, statements: Vec<Stmt>) -> Result<Value, EngineError> {
        self.scopes.push(HashMap::new());
        let result = self.execute_statements(statements);
        self.scopes.pop();
        result
    }
    fn execute_statement(&mut self, statement: Stmt) -> Result<Value, EngineError> {
        match statement {
            Stmt::Expression(expression) => self.evaluate(expression),
            Stmt::Declaration { name, value, constant } => {
                if self.scopes.last().is_some_and(|scope| scope.contains_key(&name)) {
                    return Err(EngineError::Syntax(format!("'{name}' has already been declared in this scope")));
                }
                let value = self.evaluate(value)?;
                self.scopes.last_mut().expect("global scope always exists")
                    .insert(name, Binding { value: value.clone(), constant });
                Ok(value)
            }
            Stmt::Block(body) => self.execute_scoped(body),
            Stmt::If { condition, then_branch, else_branch } => {
                if self.evaluate(condition)?.truthy() {
                    self.execute_scoped(then_branch)
                } else if let Some(branch) = else_branch {
                    self.execute_scoped(branch)
                } else { Ok(Value::Undefined) }
            }
            Stmt::While { condition, body } => {
                let mut last = Value::Undefined;
                while self.evaluate(condition.clone())?.truthy() {
                    last = self.execute_scoped(body.clone())?;
                }
                Ok(last)
            }
        }
    }
    fn evaluate(&mut self, expression: Expr) -> Result<Value, EngineError> {
        match expression {
            Expr::Literal(value) => Ok(value),
            Expr::Identifier(name) => self.lookup(&name)
                .map(|binding| binding.value.clone())
                .ok_or_else(|| EngineError::Reference(format!("{name} is not defined"))),
            Expr::Assign { name, value } => {
                let index = self.scopes.iter().rposition(|scope| scope.contains_key(&name))
                    .ok_or_else(|| EngineError::Reference(format!("{name} is not defined")))?;
                if self.scopes[index][&name].constant {
                    return Err(EngineError::Type(format!("assignment to constant variable '{name}'")));
                }
                let value = self.evaluate(*value)?;
                self.scopes[index].get_mut(&name).expect("binding was found").value = value.clone();
                Ok(value)
            }
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
                self.binary(&op, left, right)
            }
        }
    }
    fn lookup(&self, name: &str) -> Option<&Binding> {
        self.scopes.iter().rev().find_map(|scope| scope.get(name))
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
        if op == "+" && (matches!(left, Value::String(_)) || matches!(right, Value::String(_))) {
            return Ok(Value::String(format!("{}{}", left.display(), right.display())));
        }
        if op == "===" || op == "!==" {
            let equal = strict_equal(&left, &right);
            return Ok(Value::Boolean(if op == "===" { equal } else { !equal }));
        }
        if op == "==" || op == "!=" {
            let equal = loose_equal(&left, &right, self)?;
            return Ok(Value::Boolean(if op == "==" { equal } else { !equal }));
        }
        if let (Value::String(a), Value::String(b)) = (&left, &right) {
            if matches!(op, "<" | "<=" | ">" | ">=") {
                return Ok(Value::Boolean(match op { "<" => a < b, "<=" => a <= b, ">" => a > b, ">=" => a >= b, _ => false }));
            }
        }
        if op == "&&" { return Ok(if left.truthy() { right } else { left }); }
        if op == "||" { return Ok(if left.truthy() { left } else { right }); }
        let a = self.to_number(left)?;
        let b = self.to_number(right)?;
        Ok(match op {
            "+" => Value::Number(a + b), "-" => Value::Number(a - b),
            "*" => Value::Number(a * b), "/" => Value::Number(a / b),
            "%" => Value::Number(a % b), "**" => Value::Number(a.powf(b)),
            "<" => Value::Boolean(a < b), "<=" => Value::Boolean(a <= b),
            ">" => Value::Boolean(a > b), ">=" => Value::Boolean(a >= b),
            _ => return Err(EngineError::Syntax(format!("unsupported binary operator {op}"))),
        })
    }
}
fn strict_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x == y,
        (Value::String(x), Value::String(y)) => x == y,
        (Value::Boolean(x), Value::Boolean(y)) => x == y,
        (Value::Null, Value::Null) | (Value::Undefined, Value::Undefined) => true,
        _ => false,
    }
}
fn loose_equal(a: &Value, b: &Value, runtime: &Runtime) -> Result<bool, EngineError> {
    if strict_equal(a, b) { return Ok(true); }
    match (a, b) {
        (Value::Null, Value::Undefined) | (Value::Undefined, Value::Null) => Ok(true),
        (Value::Number(_), Value::String(_)) | (Value::String(_), Value::Number(_))
        | (Value::Boolean(_), _) | (_, Value::Boolean(_)) =>
            Ok(runtime.to_number(a.clone())? == runtime.to_number(b.clone())?),
        _ => Ok(false),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn precedence() { assert_eq!(Runtime::new().execute("1+2*(3+4)").unwrap(), Value::Number(15.0)); }
    #[test] fn assignment() { let mut r=Runtime::new(); r.execute("let x=4; x=x+1;").unwrap(); assert_eq!(r.execute("x").unwrap(),Value::Number(5.0)); }
    #[test] fn const_guard() { let mut r=Runtime::new(); r.execute("const x=4").unwrap(); assert!(matches!(r.execute("x=5"),Err(EngineError::Type(_)))); }
    #[test] fn if_else() { let mut r=Runtime::new(); r.execute("let x=1; if(x<2){x=8;}else{x=9;}").unwrap(); assert_eq!(r.execute("x").unwrap(),Value::Number(8.0)); }
    #[test] fn while_loop() { let mut r=Runtime::new(); r.execute("let i=0; while(i<4){i=i+1;}").unwrap(); assert_eq!(r.execute("i").unwrap(),Value::Number(4.0)); }
    #[test] fn equality() { let mut r=Runtime::new(); assert_eq!(r.execute("1==='1'").unwrap(),Value::Boolean(false)); assert_eq!(r.execute("1=='1'").unwrap(),Value::Boolean(true)); }
    #[test] fn block_scope_does_not_leak() {
        let mut r=Runtime::new();
        r.execute("let x=1; { let x=2; x=3; }").unwrap();
        assert_eq!(r.execute("x").unwrap(),Value::Number(1.0));
        assert!(matches!(r.execute("y"),Err(EngineError::Reference(_))));
    }
    #[test] fn inner_scope_can_update_outer_binding() {
        let mut r=Runtime::new();
        r.execute("let x=1; { x=7; }").unwrap();
        assert_eq!(r.execute("x").unwrap(),Value::Number(7.0));
    }
    #[test] fn const_is_block_scoped() {
        let mut r=Runtime::new();
        r.execute("{ const x=2; }").unwrap();
        r.execute("const x=3;").unwrap();
        assert_eq!(r.execute("x").unwrap(),Value::Number(3.0));
    }
}