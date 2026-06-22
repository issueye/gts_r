//! The compiler: walks an AST once and emits a `Chunk`.
//!
//! Stage 0 coverage (kept deliberately minimal — see
//! `docs/bytecode-vm-development-plan.md` §3.5):
//!   - `Stmt::Expr` wrapping an expression statement
//!   - `Expr::Number`           → CONST
//!   - `Expr::Infix` with op `+` → post-order: left, right, ADD
//!   - trailing RETURN for the top-level program
//!
//! Every other AST node returns a compile error rather than emitting broken
//! bytecode. This is by design: a stage-N PR must extend coverage and remove
//! the corresponding error path; nothing compiles to "do nothing".

use crate::ast::{Expr, Program, Stmt};
use crate::object::{new_error, num_obj, Object};

use super::chunk::Chunk;
use super::opcode::Opcode;

/// Compile a whole program. Emits each statement in order followed by a
/// terminal RETURN, so the interpreter leaves the last value on the stack.
pub fn compile(program: &Program) -> Result<Chunk, Object> {
    let mut chunk = Chunk::new();
    for stmt in &program.body {
        compile_stmt(stmt, &mut chunk)?;
    }
    // Top-level RETURN: the program's result is whatever sits on the stack.
    chunk.write_op(Opcode::Return, program.pos.clone());
    Ok(chunk)
}

fn compile_stmt(stmt: &Stmt, chunk: &mut Chunk) -> Result<(), Object> {
    match stmt {
        Stmt::Expr(e) => {
            compile_expr(&e.expr, chunk)?;
            Ok(())
        }
        _ => Err(unsupported(stmt.pos(), &format!("statement {:?}", stmt))),
    }
}

fn compile_expr(expr: &Expr, chunk: &mut Chunk) -> Result<(), Object> {
    match expr {
        Expr::Number(n) => {
            let idx = chunk.add_constant(num_obj(n.value));
            emit_const(chunk, idx, n.pos.clone());
            Ok(())
        }
        Expr::Infix(i) => {
            // Post-order: evaluate both operands, then apply the operator.
            // Stage 0 only handles binary `+`; `++`/`--` (postfix-as-infix,
            // marked by `right == None`) is a stage-3 concern.
            if i.right.is_none() {
                return Err(unsupported(
                    i.pos.clone(),
                    "postfix update operator (++/--)",
                ));
            }
            compile_expr(&i.left, chunk)?;
            compile_expr(i.right.as_ref().unwrap(), chunk)?;
            match i.op.as_str() {
                "+" => {
                    chunk.write_op(Opcode::Add, i.pos.clone());
                    Ok(())
                }
                other => Err(unsupported(
                    i.pos.clone(),
                    &format!("infix operator `{}`", other),
                )),
            }
        }
        _ => Err(unsupported(expr.pos(), &format!("expression {:?}", expr))),
    }
}

/// Emit a CONST opcode with its u16 operand.
fn emit_const(chunk: &mut Chunk, idx: u16, pos: crate::ast::Position) {
    chunk.write_op(Opcode::Const, pos.clone());
    chunk.write_u16(idx, pos);
}

fn unsupported(pos: crate::ast::Position, what: &str) -> Object {
    new_error(
        pos,
        format!("CompileError: bytecode VM does not yet support {}", what),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn compile_src(src: &str) -> Chunk {
        let lexer = Lexer::new(src);
        let mut parser = Parser::new(lexer, "t.gs");
        let program = parser.parse_program();
        assert!(
            program.errors.is_empty(),
            "parse errors: {:?}",
            program.errors
        );
        compile(&program).expect("compile should succeed for stage-0 inputs")
    }

    #[test]
    fn compiles_literal_number() {
        let chunk = compile_src("42");
        assert_eq!(chunk.code[0], Opcode::Const as u8);
        assert!(matches!(chunk.constants[0], Object::Number(n) if n == 42.0));
        assert_eq!(*chunk.code.last().unwrap(), Opcode::Return as u8);
    }

    #[test]
    fn compiles_add_post_order() {
        // 1 + 2 + 3  ⇒  CONST 1, CONST 2, ADD, CONST 3, ADD, RETURN
        let chunk = compile_src("1 + 2 + 3");
        // Walk the instruction stream properly (don't flat-filter bytes: a
        // CONST operand byte could collide with an opcode value).
        let spine = decode_opcode_spine(&chunk);
        let expected = vec![
            Opcode::Const,
            Opcode::Const,
            Opcode::Add,
            Opcode::Const,
            Opcode::Add,
            Opcode::Return,
        ];
        assert_eq!(spine, expected);
    }

    /// Decode just the opcode bytes, skipping each instruction's operands.
    /// Stage 0 only emits CONST(u16), ADD(0), RETURN(0), so operand widths are
    /// known; this helper will grow as later stages add instructions.
    fn decode_opcode_spine(chunk: &Chunk) -> Vec<Opcode> {
        let mut out = Vec::new();
        let mut ip = 0;
        while ip < chunk.code.len() {
            let op = Opcode::from_byte(chunk.code[ip]).expect("valid opcode");
            out.push(op);
            ip += 1;
            // Skip operands: CONST reads a u16, the rest read nothing.
            if op == Opcode::Const {
                ip += 2;
            }
        }
        out
    }

    #[test]
    fn rejects_unsupported_node() {
        // `let` is stage 1; stage 0 must refuse rather than miscompile.
        let lexer = Lexer::new("let x = 1");
        let mut parser = Parser::new(lexer, "t.gs");
        let program = parser.parse_program();
        let result = compile(&program);
        assert!(result.is_err(), "let should not compile at stage 0");
    }
}
