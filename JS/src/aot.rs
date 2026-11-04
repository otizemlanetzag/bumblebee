//! Portable ahead-of-time bytecode compiler. Unsupported syntax is rejected.
use crate::{lexer, parser::{Expr, Parser, Stmt}, EngineError, Value};

#[derive(Clone, Debug)]
enum Op { Const(Value), Neg, Pos, Add, Sub, Mul, Div, Rem, Pow, And, Or, Xor, Shl, Shr, UShr, Eq, StrictEq, Ne, StrictNe, Lt, Le, Gt, Ge, Return }

/// Compiled bytecode, ready to run without reparsing JavaScript source.
#[derive(Clone, Debug)]
pub struct CompiledProgram { code: Vec<Op> }

impl CompiledProgram {
    pub fn instruction_count(&self) -> usize { self.code.len() }
    pub fn run(&self) -> Result<Value, EngineError> {
        let mut stack = Vec::<Value>::with_capacity(self.code.len());
        for op in &self.code {
            match op {
                Op::Const(v) => stack.push(v.clone()),
                Op::Neg | Op::Pos => {
                    let n = to_number(pop(&mut stack)?);
                    stack.push(Value::Number(if matches!(op, Op::Neg) { -n } else { n }));
                }
                Op::Return => return pop(&mut stack),
                _ => {
                    let b = pop(&mut stack)?;
                    let a = pop(&mut stack)?;
                    let an = to_number(a.clone());
                    let bn = to_number(b.clone());
                    let comparison = matches!(op, Op::Eq|Op::StrictEq|Op::Ne|Op::StrictNe|Op::Lt|Op::Le|Op::Gt|Op::Ge);
                    let value = match op {
                        Op::Add => Value::Number(an + bn), Op::Sub => Value::Number(an - bn),
                        Op::Mul => Value::Number(an * bn), Op::Div => Value::Number(an / bn),
                        Op::Rem => Value::Number(an % bn), Op::Pow => Value::Number(an.powf(bn)),
                        Op::And => Value::Number((i32v(an) & i32v(bn)) as f64),
                        Op::Or => Value::Number((i32v(an) | i32v(bn)) as f64),
                        Op::Xor => Value::Number((i32v(an) ^ i32v(bn)) as f64),
                        Op::Shl => Value::Number(i32v(an).wrapping_shl(u32v(bn) & 31) as f64),
                        Op::Shr => Value::Number(i32v(an).wrapping_shr(u32v(bn) & 31) as f64),
                        Op::UShr => Value::Number(((i32v(an) as u32) >> (u32v(bn) & 31)) as f64),
                        Op::Eq|Op::StrictEq => Value::Boolean(a == b),
                        Op::Ne|Op::StrictNe => Value::Boolean(a != b),
                        Op::Lt => Value::Boolean(an < bn), Op::Le => Value::Boolean(an <= bn),
                        Op::Gt => Value::Boolean(an > bn), Op::Ge => Value::Boolean(an >= bn),
                        _ => return Err(EngineError::Syntax("invalid AOT instruction".into())),
                    };
                    let _ = comparison;
                    stack.push(value);
                }
            }
        }
        Err(EngineError::Syntax("AOT bytecode ended without RETURN".into()))
    }
}

pub fn compile_aot(source: &str) -> Result<CompiledProgram, EngineError> {
    let statements = Parser::new(lexer::tokenize(source)?).parse_program()?;
    if statements.len() != 1 { return Err(EngineError::Syntax("AOT currently accepts one expression statement".into())); }
    let mut code = Vec::new();
    match &statements[0] {
        Stmt::Expression(expr) => lower(expr, &mut code)?,
        _ => return Err(EngineError::Syntax("AOT currently accepts expression statements only".into())),
    }
    code.push(Op::Return);
    Ok(CompiledProgram { code })
}

fn lower(expr: &Expr, out: &mut Vec<Op>) -> Result<(), EngineError> {
    match expr {
        Expr::Literal(v @ (Value::Number(_) | Value::String(_) | Value::Boolean(_) | Value::Null | Value::Undefined)) => out.push(Op::Const(v.clone())),
        Expr::Unary { op, value } if op == "-" || op == "+" => {
            lower(value, out)?; out.push(if op == "-" { Op::Neg } else { Op::Pos });
        }
        Expr::Binary { op, left, right } => {
            lower(left, out)?; lower(right, out)?;
            out.push(match op.as_str() {
                "+"=>Op::Add, "-"=>Op::Sub, "*"=>Op::Mul, "/"=>Op::Div, "%"=>Op::Rem, "**"=>Op::Pow,
                "&"=>Op::And, "|"=>Op::Or, "^"=>Op::Xor, "<<"=>Op::Shl, ">>"=>Op::Shr, ">>>"=>Op::UShr,
                "=="=>Op::Eq, "==="=>Op::StrictEq, "!="=>Op::Ne, "!=="=>Op::StrictNe,
                "<"=>Op::Lt, "<="=>Op::Le, ">"=>Op::Gt, ">="=>Op::Ge,
                _=>return Err(EngineError::Syntax(format!("AOT does not yet support operator '{op}'"))),
            });
        }
        _ => return Err(EngineError::Syntax("AOT does not yet support this expression".into())),
    }
    Ok(())
}
fn pop(s: &mut Vec<Value>) -> Result<Value, EngineError> { s.pop().ok_or_else(|| EngineError::Syntax("AOT stack underflow".into())) }
fn to_number(v: Value) -> f64 { match v { Value::Number(n)=>n, Value::Boolean(true)=>1.0, Value::Boolean(false)|Value::Null=>0.0, Value::String(s)=>if s.trim().is_empty(){0.0}else{s.trim().parse().unwrap_or(f64::NAN)}, Value::Undefined=>f64::NAN, _=>f64::NAN } }
fn i32v(n:f64)->i32 { if !n.is_finite()||n==0.0 {0} else {n.trunc().rem_euclid(4_294_967_296.0) as u32 as i32} }
fn u32v(n:f64)->u32 { i32v(n) as u32 }

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn compiles_before_execution() {
        let p=compile_aot("(2 + 3) * 4 - 5;").unwrap();
        assert_eq!(p.run().unwrap(),Value::Number(15.0));
        assert!(p.instruction_count() >= 6);
    }
    #[test] fn compiles_bitwise_and_comparison() {
        assert_eq!(compile_aot("(5 & 3) === 1;").unwrap().run().unwrap(),Value::Boolean(true));
        assert_eq!(compile_aot("8 << 2;").unwrap().run().unwrap(),Value::Number(32.0));
    }
    #[test] fn rejects_unimplemented_syntax() { assert!(compile_aot("let x=2; x+1;").is_err()); }
}
