//! Bytecode function-call helpers.
//!
//! The interpreter dispatches `CALL`, but closure invocation lives here so
//! call-frame semantics can grow without making `interp.rs` carry every stage.

use crate::ast::Position;
use crate::object::{new_error, EnvRef, Object};

/// Call a bytecode closure: bind params into a child scope of the closure's
/// home environment, then run the body chunk.
pub fn call_closure(
    c: &crate::bytecode::closure::ClosureData,
    args: &[Object],
    caller_env: &EnvRef,
    pos: Position,
) -> Result<Object, Object> {
    call_closure_impl(c, caller_env, args, pos)
}

/// Public entry for native -> VM callback (used by `apply_function` when it
/// encounters an `Object::Closure`, e.g. an array `.map(fn)` callback).
pub fn call_closure_object(
    c: std::rc::Rc<crate::bytecode::closure::ClosureData>,
    caller_env: &EnvRef,
    args: &[Object],
    pos: Position,
) -> Object {
    match call_closure_impl(&c, caller_env, args, pos) {
        Ok(v) => v,
        Err(e) => e,
    }
}

fn call_closure_impl(
    c: &crate::bytecode::closure::ClosureData,
    caller_env: &EnvRef,
    args: &[Object],
    pos: Position,
) -> Result<Object, Object> {
    let proto = &c.proto;
    if proto.is_async {
        return Err(new_error(
            pos,
            "VMError: async function calls are not supported until stage 9",
        ));
    }

    let scope = crate::object::Environment::child(&c.home_env);
    if !proto.lexical_this {
        scope.borrow_mut().this = None;
    }

    if let Err(e) = crate::evaluator::expressions::bind_params(
        &scope,
        caller_env,
        &proto.params,
        args,
        pos.clone(),
    ) {
        return Err(e);
    }
    let _frame = crate::bytecode::frame::CallFrame::from_bound_env(proto.clone(), &scope, 0);

    let chunk = match proto.chunk.borrow().clone() {
        Some(c) => c,
        None => return Err(new_error(pos, "VMError: function body not compiled")),
    };
    let result = super::interp::interpret(&chunk, &scope);
    if result.is_runtime_error() {
        Err(result)
    } else {
        Ok(result)
    }
}
