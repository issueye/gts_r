//! @std/gtp/server - GTP server module
//!
//! Provides functions to create GTP servers.

use crate::object::{new_error, CallContext, HashData, Object};
use std::cell::RefCell;
use std::rc::Rc;

/// Create the @std/gtp/server module
pub fn gtp_server_module() -> Object {
    let mut exports = HashData::default();

    exports.set(
        "createServer",
        native_fn("gtp.createServer", gtp_create_server),
    );
    exports.set("listen", native_fn("gtp.listen", gtp_listen));

    Object::Hash(Rc::new(RefCell::new(exports)))
}

/// Helper to create a native function
fn native_fn(name: &str, f: fn(&mut CallContext, &[Object]) -> Object) -> Object {
    use crate::object::Builtin;
    Object::Builtin(Rc::new(Builtin {
        name: name.to_string(),
        func: Rc::new(f),
        extra: None,
    }))
}

/// Create a GTP server
fn gtp_create_server(ctx: &mut CallContext, _args: &[Object]) -> Object {
    // TODO: Implement in Phase 4
    new_error(ctx.pos.clone(), "gtp.createServer: not yet implemented")
}

/// Listen for GTP connections
fn gtp_listen(ctx: &mut CallContext, _args: &[Object]) -> Object {
    // TODO: Implement in Phase 4
    new_error(ctx.pos.clone(), "gtp.listen: not yet implemented")
}
