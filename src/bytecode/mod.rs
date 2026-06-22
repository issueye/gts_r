//! Bytecode VM: AST → Chunk → interpretation.
//!
//! This module is the new execution pipeline described in
//! `docs/bytecode-vm-development-plan.md`. It coexists with the tree-walking
//! evaluator until full feature parity is reached; the tree-walker remains the
//! default until stage 10.
//!
//! Stage 0 scope: `1 + 2` → `3.0`. Every other AST node is a deliberate
//! compile error; see the stage plan for the coverage roadmap.

pub mod call;
pub mod chunk;
pub mod closure;
pub mod compiler;
pub mod frame;
pub mod interp;
pub mod opcode;
pub mod resolve;
pub mod upvalue;

pub use chunk::{Chunk, ProtectedRegion};
pub use compiler::compile;
pub use interp::interpret;
pub use opcode::Opcode;
