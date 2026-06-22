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
use crate::object::{new_error, EnvRef, Object};

use super::chunk::Chunk;
use super::opcode::Opcode;
use crate::evaluator::expressions::{apply_binary_op, apply_unary_op};

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
            match self.step() {
                Ok(Flow::Continue) => {}
                Ok(Flow::Return(v)) => return v,
                Err(e) => return e,
            }
        }
    }

    /// Decode and execute one instruction. Returning `Result` lets opcode
    /// handlers use `?` for error propagation; `run` translates the outcomes.
    fn step(&mut self) -> Result<Flow, Object> {
        let byte = self.chunk.code[self.ip];
        let op = match Opcode::from_byte(byte) {
            Some(op) => op,
            None => {
                return Err(new_error(
                    self.chunk.position_at(self.ip),
                    format!("VMError: unknown opcode byte 0x{:02x}", byte),
                ));
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
            Opcode::Pop => {
                self.stack.pop();
            }
            Opcode::Dup => {
                let v = self.stack.last().cloned().ok_or_else(|| {
                    self.stack_underflow(self.chunk.position_at(self.ip - 1))
                })?;
                self.stack.push(v);
            }

            // —— binary operators: delegate to the shared evaluator core ——
            Opcode::Add => self.bin_op("+")?,
            Opcode::Sub => self.bin_op("-")?,
            Opcode::Mul => self.bin_op("*")?,
            Opcode::Div => self.bin_op("/")?,
            Opcode::Mod => self.bin_op("%")?,
            Opcode::Pow => self.bin_op("**")?,
            Opcode::Eq => self.bin_op("===")?,
            Opcode::Neq => self.bin_op("!==")?,
            Opcode::Lt => self.bin_op("<")?,
            Opcode::Le => self.bin_op("<=")?,
            Opcode::Gt => self.bin_op(">")?,
            Opcode::Ge => self.bin_op(">=")?,
            // Concat is a specialised `+` for the string-only fast path; route
            // through the same core so semantics stay identical.
            Opcode::Concat => self.bin_op("+")?,

            // —— unary operators ——
            Opcode::Not => self.un_op("!")?,
            Opcode::Neg => self.un_op("-")?,

            // —— control flow ——
            Opcode::Jump => {
                let target = self.chunk.read_u32(self.ip) as usize;
                self.ip = target;
            }
            Opcode::JumpIfFalse => {
                let target = self.chunk.read_u32(self.ip) as usize;
                self.ip += 4;
                let pos = self.chunk.position_at(self.ip - 5);
                let cond = self.stack.pop().ok_or_else(|| self.stack_underflow(pos))?;
                if !cond.is_truthy() {
                    self.ip = target;
                }
            }
            Opcode::JumpIfTrue => {
                let target = self.chunk.read_u32(self.ip) as usize;
                self.ip += 4;
                let pos = self.chunk.position_at(self.ip - 5);
                let cond = self.stack.pop().ok_or_else(|| self.stack_underflow(pos))?;
                if cond.is_truthy() {
                    self.ip = target;
                }
            }

            Opcode::Return => {
                let v = self.stack.pop().unwrap_or(Object::Undefined);
                return Ok(Flow::Return(v));
            }

            other => {
                return Err(new_error(
                    self.chunk.position_at(self.ip - 1),
                    format!("VMError: opcode {:?} not implemented yet", other),
                ));
            }
        }
        Ok(Flow::Continue)
    }

    /// Pop two operands, apply a binary op via the shared evaluator core, push
    /// the result. The op string matches the GTS source operator so semantics
    /// are byte-identical to the tree-walker.
    fn bin_op(&mut self, op: &'static str) -> Result<(), Object> {
        let pos = self.chunk.position_at(self.ip - 1);
        let right = self
            .stack
            .pop()
            .ok_or_else(|| self.stack_underflow(pos.clone()))?;
        let left = self
            .stack
            .pop()
            .ok_or_else(|| self.stack_underflow(pos.clone()))?;
        let result = apply_binary_op(op, &left, &right, pos);
        if result.is_runtime_error() {
            return Err(result);
        }
        self.stack.push(result);
        Ok(())
    }

    /// Pop one operand, apply a unary op, push the result.
    fn un_op(&mut self, op: &'static str) -> Result<(), Object> {
        let pos = self.chunk.position_at(self.ip - 1);
        let right = self
            .stack
            .pop()
            .ok_or_else(|| self.stack_underflow(pos.clone()))?;
        let result = apply_unary_op(op, &right, pos);
        if result.is_runtime_error() {
            return Err(result);
        }
        self.stack.push(result);
        Ok(())
    }

    fn stack_underflow(&self, pos: Position) -> Object {
        new_error(pos, "VMError: stack underflow")
    }
}

/// One-step control-flow outcome.
enum Flow {
    Continue,
    Return(Object),
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

    // —— arithmetic operators (each covered by its own case) ——
    #[test]
    fn arithmetic_sub() {
        assert!(matches!(run_src("10 - 3"), Object::Number(n) if n == 7.0));
    }
    #[test]
    fn arithmetic_mul() {
        assert!(matches!(run_src("4 * 5"), Object::Number(n) if n == 20.0));
    }
    #[test]
    fn arithmetic_div() {
        assert!(matches!(run_src("20 / 4"), Object::Number(n) if n == 5.0));
    }
    #[test]
    fn arithmetic_mod() {
        // number_op uses rem_euclid; 10 % 3 == 1
        assert!(matches!(run_src("10 % 3"), Object::Number(n) if n == 1.0));
    }
    #[test]
    fn arithmetic_pow() {
        assert!(matches!(run_src("2 ** 10"), Object::Number(n) if n == 1024.0));
    }
    #[test]
    fn precedence_mul_before_add() {
        assert!(matches!(run_src("2 + 3 * 4"), Object::Number(n) if n == 14.0));
    }

    // —— comparison operators ——
    #[test]
    fn compare_eq_true() {
        assert!(matches!(run_src("3 === 3"), Object::Boolean(true)));
    }
    #[test]
    fn compare_eq_false() {
        assert!(matches!(run_src("3 === 4"), Object::Boolean(false)));
    }
    #[test]
    fn compare_neq() {
        assert!(matches!(run_src("3 !== 4"), Object::Boolean(true)));
    }
    #[test]
    fn compare_lt() {
        assert!(matches!(run_src("2 < 3"), Object::Boolean(true)));
        assert!(matches!(run_src("3 < 2"), Object::Boolean(false)));
    }
    #[test]
    fn compare_le() {
        assert!(matches!(run_src("3 <= 3"), Object::Boolean(true)));
        assert!(matches!(run_src("4 <= 3"), Object::Boolean(false)));
    }
    #[test]
    fn compare_gt() {
        assert!(matches!(run_src("5 > 3"), Object::Boolean(true)));
        assert!(matches!(run_src("3 > 5"), Object::Boolean(false)));
    }
    #[test]
    fn compare_ge() {
        assert!(matches!(run_src("3 >= 3"), Object::Boolean(true)));
        assert!(matches!(run_src("2 >= 3"), Object::Boolean(false)));
    }

    // —— unary ——
    #[test]
    fn unary_neg() {
        assert!(matches!(run_src("-5"), Object::Number(n) if n == -5.0));
        assert!(matches!(run_src("-(3 + 2)"), Object::Number(n) if n == -5.0));
    }
    #[test]
    fn unary_not_bool() {
        assert!(matches!(run_src("!false"), Object::Boolean(true)));
        assert!(matches!(run_src("!true"), Object::Boolean(false)));
    }
    #[test]
    fn unary_not_truthiness() {
        // numbers: 0 is falsy, non-zero truthy
        assert!(matches!(run_src("!0"), Object::Boolean(true)));
        assert!(matches!(run_src("!1"), Object::Boolean(false)));
    }

    // —— short-circuit && / || ——
    #[test]
    fn and_returns_left_when_falsy() {
        // 0 && 1 → 0 (left, short-circuits)
        assert!(matches!(run_src("0 && 1"), Object::Number(n) if n == 0.0));
    }
    #[test]
    fn and_returns_right_when_left_truthy() {
        // 1 && 2 → 2
        assert!(matches!(run_src("1 && 2"), Object::Number(n) if n == 2.0));
    }
    #[test]
    fn or_returns_left_when_truthy() {
        // 7 || 0 → 7
        assert!(matches!(run_src("7 || 0"), Object::Number(n) if n == 7.0));
    }
    #[test]
    fn or_returns_right_when_left_falsy() {
        // 0 || 9 → 9
        assert!(matches!(run_src("0 || 9"), Object::Number(n) if n == 9.0));
    }
    #[test]
    fn and_short_circuits_bool() {
        // false && true → false (right never semantically matters)
        assert!(matches!(run_src("false && true"), Object::Boolean(false)));
    }
    #[test]
    fn or_short_circuits_bool() {
        // true || false → true
        assert!(matches!(run_src("true || false"), Object::Boolean(true)));
    }

    // —— null / undefined literals (needed to exercise falsy paths) ——
    #[test]
    fn null_literal_is_falsy_in_and() {
        // null && 1 → null
        assert!(matches!(run_src("null && 1"), Object::Null));
    }
    #[test]
    fn undefined_literal_is_falsy_in_or() {
        // undefined || 42 → 42
        assert!(matches!(run_src("undefined || 42"), Object::Number(n) if n == 42.0));
    }

    // —— string literals + concatenation (stage 1.2) ——
    #[test]
    fn string_literal() {
        assert!(matches!(run_src("\"hello\""), Object::String(s) if &*s == "hello"));
    }
    #[test]
    fn string_literal_escape() {
        // \n is processed at compile time, mirroring eval_string_lit
        assert!(matches!(run_src("\"a\\nb\""), Object::String(s) if &*s == "a\nb"));
    }
    #[test]
    fn string_concat_now_supported() {
        // Previously deferred; String literals now compile so `+` routes
        // through apply_binary_op("+") which handles string+string.
        assert!(matches!(run_src("\"foo\" + \"bar\""), Object::String(s) if &*s == "foobar"));
    }
    #[test]
    fn string_strict_equal() {
        assert!(matches!(run_src("\"a\" === \"a\""), Object::Boolean(true)));
        assert!(matches!(run_src("\"a\" === \"b\""), Object::Boolean(false)));
    }
    #[test]
    fn static_template_literal() {
        // Backtick template with no interpolation reduces to a string.
        assert!(matches!(run_src("`hi there`"), Object::String(s) if &*s == "hi there"));
    }

    // Note: interpolated templates (`${...}`) land in stage 1.3 once variable
    // lookups are supported; the compile-time path for static templates is in
    // place and matches eval_template's output for the no-interpolation case.
}
