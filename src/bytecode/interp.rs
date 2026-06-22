//! The bytecode interpreter: a stack machine that executes a `Chunk`.
//!
//! Stage 0 implements only `Const` / `Add` / `Pop` / `Return`. The dispatch
//! loop is structured so later stages add arms without reshaping control flow.
//!
//! `Add` semantics mirror `evaluator::expressions::eval_add` byte-for-byte:
//! number+number → numeric add, string+string → concatenation, mixed →
//! TypeError. The full operator family is wired up in stage 1; for now only
//! the two happy paths and the error are implemented so that the stage-0
//! contract (`1 + 2` → `3.0`) holds while still rejecting bad types.

use crate::ast::Position;
use crate::object::{new_error, str_obj, EnvRef, Object};

use super::chunk::Chunk;
use super::opcode::Opcode;

/// Execute a compiled chunk under the given (root) environment. The
/// environment is only used to reach the VM and globals; stage-0 code has no
/// variable lookups.
pub fn interpret(chunk: &Chunk, _env: &EnvRef) -> Object {
    let mut vm = VmState::new(chunk);
    vm.run()
}

struct VmState<'a> {
    chunk: &'a Chunk,
    ip: usize,
    stack: Vec<Object>,
}

impl<'a> VmState<'a> {
    fn new(chunk: &'a Chunk) -> Self {
        VmState {
            chunk,
            ip: 0,
            stack: Vec::new(),
        }
    }

    fn run(&mut self) -> Object {
        loop {
            // Defensive: bail on truncated bytecode rather than panicking.
            if self.ip >= self.chunk.code.len() {
                return new_error(
                    Position::default(),
                    "VMError: ran off the end of bytecode without RETURN",
                );
            }
            let byte = self.chunk.code[self.ip];
            let op = match Opcode::from_byte(byte) {
                Some(op) => op,
                None => {
                    return new_error(
                        self.chunk.position_at(self.ip),
                        format!("VMError: unknown opcode byte 0x{:02x}", byte),
                    );
                }
            };
            self.ip += 1;
            match op {
                Opcode::Const => {
                    let idx = self.chunk.read_u16(self.ip) as usize;
                    self.ip += 2;
                    let value = self.chunk.constants[idx].clone();
                    self.stack.push(value);
                }
                Opcode::Add => {
                    let pos = self.chunk.position_at(self.ip - 1);
                    if let Err(e) = self.do_add(pos) {
                        return e;
                    }
                }
                Opcode::Pop => {
                    self.stack.pop();
                }
                Opcode::Return => {
                    // The top-level program returns whatever sits on the stack
                    // (or Undefined if empty), matching the tree-walker's
                    // "last expression statement" result.
                    return self.stack.pop().unwrap_or(Object::Undefined);
                }
                // Every other opcode is a later stage's responsibility. Hitting
                // one here means the compiler emitted bytecode the interpreter
                // can't run yet — surface it loudly rather than silently
                // misbehaving.
                other => {
                    return new_error(
                        self.chunk.position_at(self.ip - 1),
                        format!("VMError: opcode {:?} not implemented in stage 0", other),
                    );
                }
            }
        }
    }

    /// Pop two operands and push their sum. Mirrors `eval_add`:
    /// number+number → number, string+string → string, otherwise TypeError.
    fn do_add(&mut self, pos: Position) -> Result<(), Object> {
        let right = match self.stack.pop() {
            Some(v) => v,
            None => return Err(self.stack_underflow(pos)),
        };
        let left = match self.stack.pop() {
            Some(v) => v,
            None => return Err(self.stack_underflow(pos)),
        };
        if let (Object::Number(a), Object::Number(b)) = (&left, &right) {
            self.stack.push(Object::Number(a + b));
            return Ok(());
        }
        if let (Object::String(a), Object::String(b)) = (&left, &right) {
            self.stack.push(str_obj(format!("{}{}", a, b)));
            return Ok(());
        }
        // Stage 1 will promote this to a shared evaluator helper so the error
        // wording stays identical to the tree-walker. For stage 0 the only
        // contract inputs are numeric, so this branch is unreachable in tests.
        Err(new_error(
            pos,
            format!(
                "TypeError: cannot add {} and {} — types must match",
                left.type_tag(),
                right.type_tag()
            ),
        ))
    }

    fn stack_underflow(&self, pos: Position) -> Object {
        new_error(pos, "VMError: stack underflow")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bytecode::compile;
    use crate::lexer::Lexer;
    use crate::object::Environment;
    use crate::object::VirtualMachine;
    use crate::parser::Parser;

    fn run_src(src: &str) -> Object {
        let lexer = Lexer::new(src);
        let mut parser = Parser::new(lexer, "t.gs");
        let program = parser.parse_program();
        assert!(program.errors.is_empty(), "parse errors: {:?}", program.errors);
        let chunk = compile(&program).expect("compile");
        let vm = VirtualMachine::new();
        let env = Environment::new_root(vm);
        interpret(&chunk, &env)
    }

    #[test]
    fn stage0_contract_one_plus_two() {
        // The single non-negotiable stage-0 contract: 1 + 2 → 3.0
        let result = run_src("1 + 2");
        assert!(matches!(result, Object::Number(n) if n == 3.0));
    }

    #[test]
    fn chain_add_left_associative() {
        let result = run_src("1 + 2 + 3");
        assert!(matches!(result, Object::Number(n) if n == 6.0));
    }

    // Note: string concatenation (`"foo" + "bar"`) is exercised in stage 1
    // once String literals compile. The `do_add` string path is already in
    // place; it just has no driver test until literals are supported.
}
