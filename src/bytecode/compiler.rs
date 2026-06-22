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
use crate::object::{bool_obj, new_error, num_obj, str_obj, Object};

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
        // —— literals ——
        Expr::Number(n) => {
            let idx = chunk.add_constant(num_obj(n.value));
            emit_const(chunk, idx, n.pos.clone());
            Ok(())
        }
        Expr::Bool(b) => {
            let idx = chunk.add_constant(bool_obj(b.value));
            emit_const(chunk, idx, b.pos.clone());
            Ok(())
        }
        Expr::Null(n) => {
            let idx = chunk.add_constant(Object::Null);
            emit_const(chunk, idx, n.pos.clone());
            Ok(())
        }
        Expr::Undefined(u) => {
            let idx = chunk.add_constant(Object::Undefined);
            emit_const(chunk, idx, u.pos.clone());
            Ok(())
        }

        // —— prefix ——
        Expr::Prefix(p) => {
            // ++/-- (update) and `delete` need assignment / statement context;
            // later stages.
            if matches!(p.op.as_str(), "++" | "--" | "delete") {
                return Err(unsupported(
                    p.pos.clone(),
                    &format!("prefix operator `{}`", p.op),
                ));
            }
            compile_expr(&p.right, chunk)?;
            let op = match p.op.as_str() {
                "!" => Opcode::Not,
                "-" => Opcode::Neg,
                // `+`/`typeof`/`void`/`~` are valid prefix ops in the
                // tree-walker; route them through a generic unary dispatch
                // keyed by op string (carried in the constant pool).
                "+" | "typeof" | "void" | "~" => {
                    let op_idx = chunk.add_constant(str_obj(p.op.clone()));
                    chunk.write_op(Opcode::Not, p.pos.clone()); // placeholder
                    chunk.write_u16(op_idx, p.pos.clone());
                    // NOTE: replaced by a dedicated UnaryOp path in stage 1.2.
                    // For stage 1.1 only `!` and `-` are exercised by fixtures.
                    let _ = op_idx;
                    return Err(unsupported(
                        p.pos.clone(),
                        &format!("prefix operator `{}`", p.op),
                    ));
                }
                _ => {
                    return Err(unsupported(
                        p.pos.clone(),
                        &format!("prefix operator `{}`", p.op),
                    ));
                }
            };
            chunk.write_op(op, p.pos.clone());
            Ok(())
        }

        // —— infix ——
        Expr::Infix(i) => {
            // Update operators (++/-- as infix with no right) need assignment;
            // stage 3.
            if i.right.is_none() {
                return Err(unsupported(
                    i.pos.clone(),
                    "postfix update operator (++/--)",
                ));
            }
            match i.op.as_str() {
                "&&" => {
                    compile_expr(&i.left, chunk)?;
                    compile_and(i, chunk)
                }
                "||" => {
                    compile_expr(&i.left, chunk)?;
                    compile_or(i, chunk)
                }
                "??" => Err(unsupported(
                    i.pos.clone(),
                    "nullish coalescing operator `??` (stage 1.2)",
                )),
                _ => {
                    compile_expr(&i.left, chunk)?;
                    compile_expr(i.right.as_ref().unwrap(), chunk)?;
                    let op = binary_opcode(&i.op).ok_or_else(|| {
                        unsupported(i.pos.clone(), &format!("infix operator `{}`", i.op))
                    })?;
                    chunk.write_op(op, i.pos.clone());
                    Ok(())
                }
            }
        }

        _ => Err(unsupported(expr.pos(), &format!("expression {:?}", expr))),
    }
}

/// Map a GTS infix operator string to its VM opcode. Returns `None` for
/// operators not yet supported (bitwise, etc.) so the caller emits a clean
/// compile error instead of broken bytecode.
fn binary_opcode(op: &str) -> Option<Opcode> {
    Some(match op {
        "+" => Opcode::Add,
        "-" => Opcode::Sub,
        "*" => Opcode::Mul,
        "/" => Opcode::Div,
        "%" => Opcode::Mod,
        "**" => Opcode::Pow,
        "===" => Opcode::Eq,
        "!==" => Opcode::Neq,
        "<" => Opcode::Lt,
        "<=" => Opcode::Le,
        ">" => Opcode::Gt,
        ">=" => Opcode::Ge,
        // Bitwise / `instanceof` / `in` arrive with their own fixtures later.
        _ => return None,
    })
}

/// Lower `left && right`: keep left if falsy, else replace with right.
/// Pre: left is already on the stack.
fn compile_and(i: &crate::ast::InfixExpr, chunk: &mut Chunk) -> Result<(), Object> {
    let pos = i.pos.clone();
    //   <left>            ; stack: [L]
    //   DUP               ; stack: [L, L]
    //   JUMP_IF_FALSE end ; pops test, stack: [L]
    //   POP               ; stack: []
    //   <right>           ; stack: [R]
    //   end:
    chunk.write_op(Opcode::Dup, pos.clone());
    let patch_ip = emit_jump_placeholder(chunk, Opcode::JumpIfFalse, pos.clone());
    chunk.write_op(Opcode::Pop, pos.clone());
    compile_expr(i.right.as_ref().unwrap(), chunk)?;
    patch_jump_here(chunk, patch_ip);
    Ok(())
}

/// Lower `left || right`: keep left if truthy, else replace with right.
/// Pre: left is already on the stack.
fn compile_or(i: &crate::ast::InfixExpr, chunk: &mut Chunk) -> Result<(), Object> {
    let pos = i.pos.clone();
    //   <left>                  ; stack: [L]
    //   DUP                     ; stack: [L, L]
    //   JUMP_IF_FALSE eval_right; pops test, stack: [L]
    //   JUMP end                ; stack: [L] (truthy: keep)
    //   eval_right: POP         ; stack: []
    //   <right>                 ; stack: [R]
    //   end:
    chunk.write_op(Opcode::Dup, pos.clone());
    let to_right = emit_jump_placeholder(chunk, Opcode::JumpIfFalse, pos.clone());
    let to_end = emit_jump_placeholder(chunk, Opcode::Jump, pos.clone());
    patch_jump_here(chunk, to_right);
    chunk.write_op(Opcode::Pop, pos.clone());
    compile_expr(i.right.as_ref().unwrap(), chunk)?;
    patch_jump_here(chunk, to_end);
    Ok(())
}

/// Emit `<op> <placeholder u32>` and return the byte offset of the placeholder
/// (the opcode byte position), so the caller can patch it with `patch_jump_here`.
fn emit_jump_placeholder(chunk: &mut Chunk, op: Opcode, pos: crate::ast::Position) -> u32 {
    chunk.write_op(op, pos.clone());
    let patch = chunk.code.len() as u32;
    chunk.write_u32(0, pos);
    patch
}

/// Patch a jump placeholder (the u32 operand at byte offset `operand_ip`) to
/// point at the current code position.
fn patch_jump_here(chunk: &mut Chunk, operand_ip: u32) {
    let target = chunk.code.len() as u32;
    let ip = operand_ip as usize;
    let bytes = target.to_be_bytes();
    chunk.code[ip] = bytes[0];
    chunk.code[ip + 1] = bytes[1];
    chunk.code[ip + 2] = bytes[2];
    chunk.code[ip + 3] = bytes[3];
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
