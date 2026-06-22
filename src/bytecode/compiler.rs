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
use crate::evaluator::string_lit::{eval_regexp_lit, eval_string_lit};
use crate::lexer::Lexer;
use crate::object::{bool_obj, new_error, num_obj, str_obj, Object};
use crate::parser::Parser;
use std::rc::Rc;

use super::chunk::Chunk;
use super::closure::FunctionProto;
use super::opcode::Opcode;

/// Compile a whole program. Emits each statement in order followed by a
/// terminal RETURN, so the interpreter leaves the last value on the stack.
pub fn compile(program: &Program) -> Result<Chunk, Object> {
    let mut chunk = Chunk::new();
    let mut loops: Vec<LoopFrame> = Vec::new();
    let n = program.body.len();
    for (i, stmt) in program.body.iter().enumerate() {
        compile_stmt(stmt, &mut chunk, &mut loops, i + 1 == n)?;
    }
    // Top-level RETURN: the program's result is whatever sits on the stack.
    chunk.write_op(Opcode::Return, program.pos.clone());
    Ok(chunk)
}

/// A loop being compiled: holds patch sites for `break` (jump to end) and
/// `continue` (jump to the post-expression / condition re-test).
#[derive(Default)]
struct LoopFrame {
    /// Optional label attached to this loop.
    label: Option<String>,
    /// Byte offsets of pending `break` jumps (each is a JUMP placeholder).
    breaks: Vec<u32>,
    /// Byte offsets of pending `continue` jumps.
    continues: Vec<u32>,
}

fn compile_stmt(
    stmt: &Stmt,
    chunk: &mut Chunk,
    loops: &mut Vec<LoopFrame>,
    keep_value: bool,
) -> Result<(), Object> {
    match stmt {
        Stmt::Expr(e) => {
            compile_expr(&e.expr, chunk)?;
            if !keep_value {
                // Discard the expression value so it doesn't accumulate on the
                // stack across iterations / statements. The top-level last
                // statement keeps its value as the program result.
                chunk.write_op(Opcode::Pop, e.pos.clone());
            }
            Ok(())
        }
        Stmt::Let(s) => compile_decl(&s.name, s.value.as_ref(), false, s.pos.clone(), chunk),
        Stmt::Var(s) => compile_decl(&s.name, s.value.as_ref(), false, s.pos.clone(), chunk),
        Stmt::Const(s) => compile_decl(&s.name, s.value.as_ref(), true, s.pos.clone(), chunk),
        Stmt::Block(b) => {
            for s in &b.statements {
                compile_stmt(s, chunk, loops, false)?;
            }
            Ok(())
        }
        Stmt::If(s) => compile_if(s, chunk, loops, keep_value),
        Stmt::While(s) => compile_while(s, None, chunk, loops, keep_value),
        Stmt::For(s) => compile_for(s, None, chunk, loops, keep_value),
        Stmt::ForIn(s) => compile_for_iter(
            &s.name,
            &s.iterable,
            &s.body,
            Opcode::IterKeys,
            s.pos.clone(),
            None,
            chunk,
            loops,
        ),
        Stmt::ForOf(s) => compile_for_iter(
            &s.name,
            &s.iterable,
            &s.body,
            Opcode::IterValues,
            s.pos.clone(),
            None,
            chunk,
            loops,
        ),
        Stmt::Break(s) => compile_break_continue(true, &s.label, s.pos.clone(), chunk, loops),
        Stmt::Continue(s) => compile_break_continue(false, &s.label, s.pos.clone(), chunk, loops),
        Stmt::Labeled(s) => compile_labeled(s, chunk, loops, keep_value),
        Stmt::FuncDecl(f) => {
            // Compile the body to a proto (which lives in this chunk's proto
            // table), emit OP_CLOSURE to construct the closure value, then
            // store it under the function's name.
            let proto_idx = compile_function_proto(
                &f.name,
                f.params.clone(),
                f.body.clone(),
                f.is_async,
                false,
                f.return_t.clone(),
                f.pos.clone(),
                chunk,
            )?;
            chunk.write_op(Opcode::Closure, f.pos.clone());
            chunk.write_u16(proto_idx, f.pos.clone());
            let name_idx = chunk.add_constant(str_obj(f.name.clone()));
            chunk.write_op(Opcode::StoreName, f.pos.clone());
            chunk.write_u16(name_idx, f.pos.clone());
            Ok(())
        }
        Stmt::Return(r) => {
            if let Some(v) = &r.value {
                compile_expr(v, chunk)?;
            } else {
                let idx = chunk.add_constant(Object::Undefined);
                emit_const(chunk, idx, r.pos.clone());
            }
            chunk.write_op(Opcode::Return, r.pos.clone());
            Ok(())
        }
        _ => Err(unsupported(stmt.pos(), &format!("statement {:?}", stmt))),
    }
}

/// Compile `if (cond) { ... } else { ... }`.
fn compile_if(
    s: &crate::ast::IfStmt,
    chunk: &mut Chunk,
    loops: &mut Vec<LoopFrame>,
    _keep_value: bool,
) -> Result<(), Object> {
    // cond ; JUMP_IF_FALSE else ; <then> ; JUMP end ; else: <else> ; end:
    compile_expr(&s.cond, chunk)?;
    let to_else = emit_jump_placeholder(chunk, Opcode::JumpIfFalse, s.pos.clone());
    for stmt in &s.consequence.statements {
        compile_stmt(stmt, chunk, loops, false)?;
    }
    let to_end = if s.alternative.is_some() {
        Some(emit_jump_placeholder(chunk, Opcode::Jump, s.pos.clone()))
    } else {
        None
    };
    patch_jump_here(chunk, to_else);
    if let Some(alt) = &s.alternative {
        compile_stmt(alt, chunk, loops, false)?;
    }
    if let Some(end) = to_end {
        patch_jump_here(chunk, end);
    }
    Ok(())
}

/// Compile `while (cond) { body }`.
fn compile_while(
    s: &crate::ast::WhileStmt,
    label: Option<String>,
    chunk: &mut Chunk,
    loops: &mut Vec<LoopFrame>,
    _keep_value: bool,
) -> Result<(), Object> {
    // start: cond ; JUMP_IF_FALSE end ; <body> ; LOOP start ; end:
    let start = chunk.code.len() as u32;
    compile_expr(&s.cond, chunk)?;
    let to_end = emit_jump_placeholder(chunk, Opcode::JumpIfFalse, s.pos.clone());
    loops.push(LoopFrame {
        label,
        ..LoopFrame::default()
    });
    for stmt in &s.body.statements {
        compile_stmt(stmt, chunk, loops, false)?;
    }
    let frame = loops.pop().unwrap();
    // Back-edge: LOOP to the condition test.
    chunk.write_op(Opcode::Loop, s.pos.clone());
    chunk.write_u32(start, s.pos.clone());
    let end = chunk.code.len() as u32;
    patch_jump_here(chunk, to_end);
    // Patch break/continue jumps collected in the frame.
    for b in &frame.breaks {
        patch_jump_to(chunk, *b, end);
    }
    for c in &frame.continues {
        patch_jump_to(chunk, *c, start);
    }
    Ok(())
}

/// Compile `for (init; cond; post) { body }`.
fn compile_for(
    s: &crate::ast::ForStmt,
    label: Option<String>,
    chunk: &mut Chunk,
    loops: &mut Vec<LoopFrame>,
    _keep_value: bool,
) -> Result<(), Object> {
    // <init> ; start: <cond> ; JUMP_IF_FALSE end ; <body> ; post_start: <post> ; LOOP start ; end:
    if let Some(init) = &s.init {
        compile_stmt(init, chunk, loops, false)?;
    }
    let start = chunk.code.len() as u32;
    let mut to_end: Option<u32> = None;
    if let Some(cond) = &s.cond {
        compile_expr(cond, chunk)?;
        to_end = Some(emit_jump_placeholder(
            chunk,
            Opcode::JumpIfFalse,
            s.pos.clone(),
        ));
    }
    loops.push(LoopFrame {
        label,
        ..LoopFrame::default()
    });
    for stmt in &s.body.statements {
        compile_stmt(stmt, chunk, loops, false)?;
    }
    let frame = loops.pop().unwrap();
    // post expression (continue targets here) — recorded AFTER the body so its
    // offset is correct.
    let post_start = chunk.code.len() as u32;
    if let Some(post) = &s.post {
        compile_expr(post, chunk)?;
        chunk.write_op(Opcode::Pop, s.pos.clone()); // discard post value
    }
    chunk.write_op(Opcode::Loop, s.pos.clone());
    chunk.write_u32(start, s.pos.clone());
    let end = chunk.code.len() as u32;
    if let Some(end_patch) = to_end {
        patch_jump_here(chunk, end_patch);
    }
    for b in &frame.breaks {
        patch_jump_to(chunk, *b, end);
    }
    for c in &frame.continues {
        patch_jump_to(chunk, *c, post_start);
    }
    Ok(())
}

fn compile_for_iter(
    name: &str,
    iterable: &Expr,
    body: &crate::ast::BlockStmt,
    iter_op: Opcode,
    pos: crate::ast::Position,
    label: Option<String>,
    chunk: &mut Chunk,
    loops: &mut Vec<LoopFrame>,
) -> Result<(), Object> {
    let suffix = format!("{}_{}", pos.line, pos.col);
    let items_name = format!("__gts_bc_iter_items_{}", suffix);
    let idx_name = format!("__gts_bc_iter_idx_{}", suffix);

    // items = ITER_KEYS/ITER_VALUES(iterable)
    compile_expr(iterable, chunk)?;
    chunk.write_op(iter_op, pos.clone());
    let items_idx = chunk.add_constant(str_obj(items_name.clone()));
    chunk.write_op(Opcode::StoreName, pos.clone());
    chunk.write_u16(items_idx, pos.clone());

    // idx = 0
    let zero = chunk.add_constant(num_obj(0.0));
    emit_const(chunk, zero, pos.clone());
    let idx_idx = chunk.add_constant(str_obj(idx_name.clone()));
    chunk.write_op(Opcode::StoreName, pos.clone());
    chunk.write_u16(idx_idx, pos.clone());

    // start: idx < len(items)
    let start = chunk.code.len() as u32;
    emit_load_name(chunk, &idx_name, pos.clone());
    emit_load_name(chunk, &items_name, pos.clone());
    chunk.write_op(Opcode::Len, pos.clone());
    chunk.write_op(Opcode::Lt, pos.clone());
    let to_end = emit_jump_placeholder(chunk, Opcode::JumpIfFalse, pos.clone());

    // loop variable = items[idx]
    emit_load_name(chunk, &items_name, pos.clone());
    emit_load_name(chunk, &idx_name, pos.clone());
    chunk.write_op(Opcode::GetIndex, pos.clone());
    let name_idx = chunk.add_constant(str_obj(name.to_string()));
    chunk.write_op(Opcode::StoreName, pos.clone());
    chunk.write_u16(name_idx, pos.clone());

    loops.push(LoopFrame {
        label,
        ..LoopFrame::default()
    });
    for stmt in &body.statements {
        compile_stmt(stmt, chunk, loops, false)?;
    }
    let frame = loops.pop().unwrap();

    // continue target: idx = idx + 1
    let increment = chunk.code.len() as u32;
    emit_load_name(chunk, &idx_name, pos.clone());
    let one = chunk.add_constant(num_obj(1.0));
    emit_const(chunk, one, pos.clone());
    chunk.write_op(Opcode::Add, pos.clone());
    chunk.write_op(Opcode::Dup, pos.clone());
    let idx_idx = chunk.add_constant(str_obj(idx_name));
    chunk.write_op(Opcode::AssignName, pos.clone());
    chunk.write_u16(idx_idx, pos.clone());
    chunk.write_op(Opcode::Pop, pos.clone());
    chunk.write_op(Opcode::Loop, pos.clone());
    chunk.write_u32(start, pos.clone());

    let end = chunk.code.len() as u32;
    patch_jump_here(chunk, to_end);
    for b in &frame.breaks {
        patch_jump_to(chunk, *b, end);
    }
    for c in &frame.continues {
        patch_jump_to(chunk, *c, increment);
    }
    Ok(())
}

fn compile_labeled(
    s: &crate::ast::LabeledStmt,
    chunk: &mut Chunk,
    loops: &mut Vec<LoopFrame>,
    keep_value: bool,
) -> Result<(), Object> {
    match s.stmt.as_ref() {
        Stmt::While(w) => compile_while(w, Some(s.label.clone()), chunk, loops, keep_value),
        Stmt::For(f) => compile_for(f, Some(s.label.clone()), chunk, loops, keep_value),
        Stmt::ForIn(f) => compile_for_iter(
            &f.name,
            &f.iterable,
            &f.body,
            Opcode::IterKeys,
            f.pos.clone(),
            Some(s.label.clone()),
            chunk,
            loops,
        ),
        Stmt::ForOf(f) => compile_for_iter(
            &f.name,
            &f.iterable,
            &f.body,
            Opcode::IterValues,
            f.pos.clone(),
            Some(s.label.clone()),
            chunk,
            loops,
        ),
        other => compile_stmt(other, chunk, loops, keep_value),
    }
}

/// Compile `break` (is_break=true) or `continue`. Records a pending JUMP in
/// the current loop frame to be patched when the loop's end / continue-target
/// is known. Labeled break/continue is stage 2 polish (defers to plain).
fn compile_break_continue(
    is_break: bool,
    label: &str,
    pos: crate::ast::Position,
    chunk: &mut Chunk,
    loops: &mut Vec<LoopFrame>,
) -> Result<(), Object> {
    let frame = match loops.iter_mut().rev().find(|f| {
        label.is_empty()
            || f.label
                .as_ref()
                .map(|frame_label| frame_label == label)
                .unwrap_or(false)
    }) {
        Some(f) => f,
        None => {
            return Err(unsupported(
                pos,
                if label.is_empty() {
                    if is_break {
                        "break outside loop"
                    } else {
                        "continue outside loop"
                    }
                } else if is_break {
                    "labeled break target"
                } else {
                    "labeled continue target"
                },
            ));
        }
    };
    let patch = emit_jump_placeholder(chunk, Opcode::Jump, pos);
    if is_break {
        frame.breaks.push(patch);
    } else {
        frame.continues.push(patch);
    }
    Ok(())
}

/// Compile an interpolated template literal into a string concatenation.
///
/// Each `${expr}` segment is re-parsed as a sub-expression (matching the
/// tree-walker's `eval_template_expression`), evaluated, and converted to its
/// string form via TO_STRING. Literal text segments are CONST strings. All
/// parts are joined left-to-right with `+` (string concat).
fn compile_template_interpolated(
    t: &crate::ast::TemplateLit,
    chunk: &mut Chunk,
) -> Result<(), Object> {
    let lit = &t.literal;
    if lit.len() < 2 || !lit.starts_with('`') {
        let value = crate::evaluator::string_lit::eval_template_static(t);
        let idx = chunk.add_constant(value);
        emit_const(chunk, idx, t.pos.clone());
        return Ok(());
    }
    let mut inner = &lit[1..];
    if inner.ends_with('`') {
        inner = &inner[..inner.len() - 1];
    }
    let bytes = inner.as_bytes();
    let mut segments_emitted = 0;
    let mut i = 0;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'$' && bytes[i + 1] == b'{' {
            let end = match find_template_expr_end(inner, i + 2) {
                Some(end) => end,
                None => {
                    return Err(unsupported(
                        t.pos.clone(),
                        "unterminated template expression",
                    ));
                }
            };
            let expr_str = inner[i + 2..end].trim();
            if !expr_str.is_empty() {
                // Re-parse the sub-expression at compile time so the emitted
                // bytecode reflects its structure (not a runtime re-parse).
                let sub_expr = parse_template_expr(expr_str, t.pos.clone())?;
                compile_expr(&sub_expr, chunk)?;
                chunk.write_op(Opcode::ToString, t.pos.clone());
                if segments_emitted > 0 {
                    chunk.write_op(Opcode::Concat, t.pos.clone());
                }
                segments_emitted += 1;
            }
            i = end + 1;
            continue;
        }
        // Collect a run of literal chars up to the next `${`.
        let start = i;
        while i < bytes.len() && !(i + 1 < bytes.len() && bytes[i] == b'$' && bytes[i + 1] == b'{')
        {
            i += 1;
        }
        let text = crate::evaluator::string_lit::unescape_string(&inner[start..i]);
        let idx = chunk.add_constant(str_obj(text));
        emit_const(chunk, idx, t.pos.clone());
        if segments_emitted > 0 {
            chunk.write_op(Opcode::Concat, t.pos.clone());
        }
        segments_emitted += 1;
    }
    // Empty template → empty string.
    if segments_emitted == 0 {
        let idx = chunk.add_constant(str_obj(""));
        emit_const(chunk, idx, t.pos.clone());
    }
    Ok(())
}

/// Re-parse a template `${...}` sub-expression string into an AST Expr, so the
/// compiler can emit bytecode for it (rather than deferring to a runtime
/// re-parse). Mirrors the tree-walker's `eval_template_expression` parse step.
fn parse_template_expr(src: &str, pos: crate::ast::Position) -> Result<Expr, Object> {
    let wrap = format!("let __gts_tpl = {};", src);
    let lex = Lexer::new(&wrap);
    let mut parser = Parser::new(lex, pos.file.as_ref());
    let prog = parser.parse_program();
    if !parser.errors().is_empty() || !prog.errors.is_empty() {
        return Err(unsupported(pos, "template expression parse error"));
    }
    // Extract the initializer expression from `let __gts_tpl = <expr>;`.
    for stmt in &prog.body {
        if let Stmt::Let(l) = stmt {
            if let Some(v) = &l.value {
                return Ok(v.clone());
            }
        }
    }
    Err(unsupported(pos, "template expression parse error"))
}

/// Find the matching `}` for a `${...}` template expression, accounting for
/// nested braces and quoted strings. Mirrors the tree-walker's helper.
fn find_template_expr_end(s: &str, start: usize) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut depth = 0i32;
    let mut quote: u8 = 0;
    let mut escape = false;
    let mut i = start;
    while i < bytes.len() {
        let ch = bytes[i];
        if quote != 0 {
            if escape {
                escape = false;
            } else if ch == b'\\' {
                escape = true;
            } else if ch == quote {
                quote = 0;
            }
            i += 1;
            continue;
        }
        match ch {
            b'"' | b'\'' => quote = ch,
            b'{' => depth += 1,
            b'}' => {
                if depth == 0 {
                    return Some(i);
                }
                depth -= 1;
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Patch a jump placeholder to an explicit target offset (not necessarily
/// "here").
fn patch_jump_to(chunk: &mut Chunk, operand_ip: u32, target: u32) {
    let ip = operand_ip as usize;
    let bytes = target.to_be_bytes();
    chunk.code[ip] = bytes[0];
    chunk.code[ip + 1] = bytes[1];
    chunk.code[ip + 2] = bytes[2];
    chunk.code[ip + 3] = bytes[3];
}

/// Compile a function body into a sub-Chunk, register a `FunctionProto` on
/// the *parent* chunk's proto table, and return the proto index.
///
/// The body is compiled with its own statement stream and a trailing RETURN
/// (returning the last statement's value, or Undefined). Parameters are bound
/// by the interpreter at call time into the call environment, so the body's
/// parameter references resolve as plain `LoadName`.
fn compile_function_proto(
    name: &str,
    params: Vec<crate::ast::Param>,
    body: crate::ast::BlockStmt,
    is_async: bool,
    lexical_this: bool,
    return_t: Option<crate::ast::TypeAnnotation>,
    pos: crate::ast::Position,
    parent: &mut Chunk,
) -> Result<u16, Object> {
    let mut sub = Chunk::new();
    let mut loops: Vec<LoopFrame> = Vec::new();
    let n = body.statements.len();
    for (i, stmt) in body.statements.iter().enumerate() {
        compile_stmt(stmt, &mut sub, &mut loops, i + 1 == n)?;
    }
    // If the body didn't end in an explicit RETURN, emit one so the call
    // always returns (the last value, or Undefined).
    if !matches_last_opcode(&sub, Opcode::Return) {
        sub.write_op(Opcode::Return, pos.clone());
    }
    let proto = FunctionProto::new(name, params, body, is_async, lexical_this, return_t, pos);
    let idx = parent.protos.len() as u16;
    // Fill the chunk before moving proto into the table (borrow then push).
    *proto.chunk.borrow_mut() = Some(Rc::new(sub));
    parent.protos.push(proto);
    Ok(idx)
}

/// True if the last instruction in the chunk is `op`.
fn matches_last_opcode(chunk: &Chunk, op: Opcode) -> bool {
    // Walk backwards skipping operand bytes is hard; instead scan forward with
    // known operand widths. For stage 3 the only opcodes with operands in a
    // function body are Const/LoadName/StoreName/AssignName/Call/Closure (u16)
    // and Jump/JumpIfFalse/JumpIfTrue/Loop (u32). Simpler: track the opcode
    // positions by scanning.
    let mut ip = 0;
    let mut last_op = None;
    while ip < chunk.code.len() {
        let b = chunk.code[ip];
        last_op = Opcode::from_byte(b);
        ip += 1;
        // skip operands based on the opcode
        if let Some(o) = last_op {
            ip += operand_width(o) as usize;
        }
    }
    last_op == Some(op)
}

/// Byte width of the operand for an opcode (0 if none).
fn operand_width(op: Opcode) -> u8 {
    match op {
        Opcode::Const
        | Opcode::LoadName
        | Opcode::StoreName
        | Opcode::AssignName
        | Opcode::GetProperty
        | Opcode::SetProperty
        | Opcode::DefineMethod
        | Opcode::NewClass
        | Opcode::SuperMethod
        | Opcode::NewArray
        | Opcode::New
        | Opcode::Call
        | Opcode::Closure => 2,
        Opcode::Jump | Opcode::JumpIfFalse | Opcode::JumpIfTrue | Opcode::Loop => 4,
        _ => 0,
    }
}
///
/// Stage 1 keeps all variables in the (root) environment's name table, so a
/// declaration evaluates its initializer (if any) and emits a STORE_NAME.
/// `const` is recorded so a later assignment raises the matching TypeError;
/// the const-ness is tracked by the environment binding, not the opcode.
fn compile_decl(
    name: &str,
    value: Option<&Expr>,
    is_const: bool,
    pos: crate::ast::Position,
    chunk: &mut Chunk,
) -> Result<(), Object> {
    if let Some(v) = value {
        compile_expr(v, chunk)?;
    } else {
        // Declaration without initializer → undefined.
        let idx = chunk.add_constant(Object::Undefined);
        emit_const(chunk, idx, pos.clone());
    }
    let name_idx = chunk.add_constant(str_obj(name));
    // Encode const-ness in the high bit of the name index operand so the
    // interpreter knows which binding flavor to create. (Name pools stay
    // small; a u16 with a flag bit is plenty.)
    let operand = if is_const {
        name_idx | 0x8000
    } else {
        name_idx
    };
    chunk.write_op(Opcode::StoreName, pos.clone());
    chunk.write_u16(operand, pos);
    Ok(())
}

fn compile_expr(expr: &Expr, chunk: &mut Chunk) -> Result<(), Object> {
    match expr {
        // —— identifier read ——
        Expr::Ident(i) => {
            let name_idx = chunk.add_constant(str_obj(i.name.clone()));
            chunk.write_op(Opcode::LoadName, i.pos.clone());
            chunk.write_u16(name_idx, i.pos.clone());
            Ok(())
        }
        // —— assignment `name = expr` (and compound `+=` etc.) ——
        Expr::Assign(a) => compile_assign(a, chunk),

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
        Expr::String(s) => {
            // String literals are pure (escape processing only, no env), so
            // evaluate them at compile time and intern the result.
            let value = eval_string_lit(s);
            if value.is_runtime_error() {
                return Err(value);
            }
            let idx = chunk.add_constant(value);
            emit_const(chunk, idx, s.pos.clone());
            Ok(())
        }
        Expr::Regexp(r) => {
            // Regexp literals compile to a RegexpData value (pure).
            let value = eval_regexp_lit(r);
            if value.is_runtime_error() {
                return Err(value);
            }
            let idx = chunk.add_constant(value);
            emit_const(chunk, idx, r.pos.clone());
            Ok(())
        }
        Expr::Template(t) => {
            // Templates with `${...}` interpolation are lowered to a series of
            // string concatenations: each literal text segment is a CONST
            // string, each `${expr}` segment is the expression followed by
            // TO_STRING. All parts are joined with `+` (string concat).
            if !t.literal.contains("${") {
                // Static template (no interpolation): reduce at compile time.
                let value = crate::evaluator::string_lit::eval_template_static(t);
                let idx = chunk.add_constant(value);
                emit_const(chunk, idx, t.pos.clone());
                return Ok(());
            }
            compile_template_interpolated(t, chunk)
        }
        Expr::Array(a) => {
            for element in &a.elements {
                if matches!(element, Expr::Spread(_)) {
                    return Err(unsupported(element.pos(), "array spread literal element"));
                }
                compile_expr(element, chunk)?;
            }
            chunk.write_op(Opcode::NewArray, a.pos.clone());
            chunk.write_u16(a.elements.len() as u16, a.pos.clone());
            Ok(())
        }
        Expr::Object(o) => {
            chunk.write_op(Opcode::NewObject, o.pos.clone());
            for prop in &o.properties {
                if prop.spread || prop.computed || prop.is_accessor {
                    return Err(unsupported(
                        prop.pos.clone(),
                        "object spread/computed/accessor property",
                    ));
                }
                compile_expr(&prop.value, chunk)?;
                let key = object_property_key(prop)?;
                let key_idx = chunk.add_constant(str_obj(key));
                chunk.write_op(Opcode::SetProperty, prop.pos.clone());
                chunk.write_u16(key_idx, prop.pos.clone());
            }
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

        // —— function call (callee + args, then CALL) ——
        Expr::Call(c) => {
            compile_expr(&c.callee, chunk)?;
            if c.args.iter().any(|arg| matches!(arg, Expr::Spread(_))) {
                chunk.write_op(Opcode::NewArray, c.pos.clone());
                chunk.write_u16(0, c.pos.clone());
                for arg in &c.args {
                    match arg {
                        Expr::Spread(sp) => {
                            compile_expr(&sp.value, chunk)?;
                            chunk.write_op(Opcode::Spread, sp.pos.clone());
                        }
                        _ => {
                            compile_expr(arg, chunk)?;
                            chunk.write_op(Opcode::PushArg, arg.pos());
                        }
                    }
                }
                chunk.write_op(Opcode::CallSpread, c.pos.clone());
                return Ok(());
            }
            for arg in &c.args {
                compile_expr(arg, chunk)?;
            }
            let arg_count = c.args.len() as u16;
            chunk.write_op(Opcode::Call, c.pos.clone());
            chunk.write_u16(arg_count, c.pos.clone());
            Ok(())
        }
        Expr::Member(m) => {
            compile_expr(&m.object, chunk)?;
            if m.computed {
                compile_expr(&m.property, chunk)?;
                chunk.write_op(Opcode::GetIndex, m.pos.clone());
            } else {
                let name = object_property_key_expr(&m.property);
                if name.is_empty() {
                    return Err(unsupported(m.pos.clone(), "member property key"));
                }
                let name_idx = chunk.add_constant(str_obj(name));
                chunk.write_op(Opcode::GetProperty, m.pos.clone());
                chunk.write_u16(name_idx, m.pos.clone());
            }
            Ok(())
        }
        Expr::Index(i) => {
            compile_expr(&i.left, chunk)?;
            compile_expr(&i.index, chunk)?;
            chunk.write_op(Opcode::GetIndex, i.pos.clone());
            Ok(())
        }
        Expr::New(n) => {
            compile_expr(&n.callee, chunk)?;
            for arg in &n.args {
                compile_expr(arg, chunk)?;
            }
            chunk.write_op(Opcode::New, n.pos.clone());
            chunk.write_u16(n.args.len() as u16, n.pos.clone());
            Ok(())
        }
        // —— function expression ——
        Expr::Func(f) => {
            let idx = compile_function_proto(
                &f.name,
                f.params.clone(),
                f.body.clone(),
                f.is_async,
                false,
                f.return_t.clone(),
                f.pos.clone(),
                chunk,
            )?;
            chunk.write_op(Opcode::Closure, f.pos.clone());
            chunk.write_u16(idx, f.pos.clone());
            Ok(())
        }
        // —— arrow function ——
        Expr::Arrow(a) => {
            // Arrow body: either an expression (implicit return) or a block.
            let body = match &a.body {
                crate::ast::ArrowBody::Expr(e) => {
                    // Wrap the expression in a single return statement.
                    crate::ast::BlockStmt {
                        pos: a.pos.clone(),
                        statements: vec![Stmt::Return(crate::ast::ReturnStmt {
                            pos: a.pos.clone(),
                            value: Some(e.clone()),
                        })],
                    }
                }
                crate::ast::ArrowBody::Block(b) => b.clone(),
            };
            let idx = compile_function_proto(
                "",
                a.params.clone(),
                body,
                a.is_async,
                true, // arrow functions capture `this` lexically
                a.return_t.clone(),
                a.pos.clone(),
                chunk,
            )?;
            chunk.write_op(Opcode::Closure, a.pos.clone());
            chunk.write_u16(idx, a.pos.clone());
            Ok(())
        }
        _ => Err(unsupported(expr.pos(), &format!("expression {:?}", expr))),
    }
}

fn emit_load_name(chunk: &mut Chunk, name: &str, pos: crate::ast::Position) {
    let idx = chunk.add_constant(str_obj(name.to_string()));
    chunk.write_op(Opcode::LoadName, pos.clone());
    chunk.write_u16(idx, pos);
}

fn object_property_key(prop: &crate::ast::Property) -> Result<String, Object> {
    if prop.shorthand {
        if let Expr::Ident(i) = &prop.key {
            return Ok(i.name.clone());
        }
    }
    let key = object_property_key_expr(&prop.key);
    if key.is_empty() {
        Err(unsupported(prop.pos.clone(), "object property key"))
    } else {
        Ok(key)
    }
}

fn object_property_key_expr(expr: &Expr) -> String {
    match expr {
        Expr::Ident(i) => i.name.clone(),
        Expr::String(s) => crate::evaluator::eval_core::strip_quotes(&s.literal),
        Expr::Number(n) => crate::object::format_number(n.value),
        _ => String::new(),
    }
}

/// Compile an assignment expression.
///
/// Stage 1 supports `name = expr` and compound `name <op>= expr` for an
/// identifier target. Member/index assignment is stage 5.
fn compile_assign(a: &crate::ast::AssignExpr, chunk: &mut Chunk) -> Result<(), Object> {
    let name = match &a.left {
        Expr::Ident(i) => i.name.clone(),
        _ => {
            return Err(unsupported(
                a.pos.clone(),
                "assignment to non-identifier target",
            ));
        }
    };
    if a.op == "=" {
        compile_expr(&a.right, chunk)?;
        // DUP so the assigned value is both stored and left on the stack as
        // the expression result (assignment evaluates to the value).
        chunk.write_op(Opcode::Dup, a.pos.clone());
        let name_idx = chunk.add_constant(str_obj(name));
        chunk.write_op(Opcode::AssignName, a.pos.clone());
        chunk.write_u16(name_idx, a.pos.clone());
        Ok(())
    } else {
        // Compound: read current, combine with right, store.
        // LOAD_NAME name ; <right> ; <op> ; DUP ; ASSIGN_NAME name
        let name_idx_load = chunk.add_constant(str_obj(name.clone()));
        chunk.write_op(Opcode::LoadName, a.pos.clone());
        chunk.write_u16(name_idx_load, a.pos.clone());
        compile_expr(&a.right, chunk)?;
        // Strip the `=` suffix to get the binary op (`+=` → `+`).
        let bin_op: String = a.op[..a.op.len() - 1].to_string();
        let op = binary_opcode(&bin_op).ok_or_else(|| {
            unsupported(a.pos.clone(), &format!("compound assignment `{}`", a.op))
        })?;
        chunk.write_op(op, a.pos.clone());
        chunk.write_op(Opcode::Dup, a.pos.clone());
        let name_idx_store = chunk.add_constant(str_obj(name));
        chunk.write_op(Opcode::AssignName, a.pos.clone());
        chunk.write_u16(name_idx_store, a.pos.clone());
        Ok(())
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
        // try/catch is stage 6; the compiler must refuse rather than silently
        // miscompile. (Earlier this tested `let`, then `function` — both now
        // supported.)
        let lexer = Lexer::new("try { 1 } catch (e) { 2 }");
        let mut parser = Parser::new(lexer, "t.gs");
        let program = parser.parse_program();
        let result = compile(&program);
        assert!(
            result.is_err(),
            "try/catch should not compile before stage 6"
        );
    }
}
