use std::cell::RefCell;
use std::rc::Rc;

use crate::object::{Awaitable, EnvRef, PollResult, Waker};

use super::chunk::Chunk;
use super::interp::interpret;

enum FrameState {
    Pending,
    Done(PollResult),
}

/// Awaitable wrapper for a bytecode chunk.
///
/// The current async runtime is single-threaded and the tree-walker runs async
/// bodies inline, so polling a bytecode frame executes it to completion on the
/// first poll and memoizes the result for later polls.
pub struct BytecodeFrameAwaitable {
    chunk: Chunk,
    env: EnvRef,
    state: RefCell<FrameState>,
}

impl BytecodeFrameAwaitable {
    pub fn new(chunk: Chunk, env: EnvRef) -> Self {
        BytecodeFrameAwaitable {
            chunk,
            env,
            state: RefCell::new(FrameState::Pending),
        }
    }
}

impl Awaitable for BytecodeFrameAwaitable {
    fn poll(&self, _waker: Waker) -> PollResult {
        let mut state = self.state.borrow_mut();
        match &*state {
            FrameState::Done(result) => return result.clone(),
            FrameState::Pending => {}
        }

        let result = interpret(&self.chunk, &self.env);
        let poll = if result.is_runtime_error() {
            PollResult::Rejected(result)
        } else {
            PollResult::Ready(result)
        };
        *state = FrameState::Done(poll.clone());
        poll
    }
}

impl From<BytecodeFrameAwaitable> for Rc<dyn Awaitable> {
    fn from(value: BytecodeFrameAwaitable) -> Self {
        Rc::new(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bytecode::compile;
    use crate::lexer::Lexer;
    use crate::object::{Environment, Object, VirtualMachine};
    use crate::parser::Parser;

    fn compile_src(src: &str) -> Chunk {
        let lexer = Lexer::new(src);
        let mut parser = Parser::new(lexer, "awaitable.gs");
        let program = parser.parse_program();
        assert!(
            program.errors.is_empty(),
            "parse errors: {:?}",
            program.errors
        );
        compile(&program).expect("compile")
    }

    #[test]
    fn bytecode_frame_awaitable_polls_chunk_to_ready() {
        let chunk = compile_src("1 + 2");
        let env = Environment::new_root(VirtualMachine::new());
        let awaitable = BytecodeFrameAwaitable::new(chunk, env);

        let result = awaitable.poll(Rc::new(|| {}));
        assert!(matches!(result, PollResult::Ready(Object::Number(n)) if n == 3.0));

        let second = awaitable.poll(Rc::new(|| {}));
        assert!(matches!(second, PollResult::Ready(Object::Number(n)) if n == 3.0));
    }
}
