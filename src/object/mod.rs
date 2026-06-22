//! Runtime object system: values, environments, the virtual machine, and
//! promises.

mod awaitable;
mod environment;
mod event_loop;
mod io_awaitable;
#[cfg(not(feature = "tokio"))]
mod io_selector;
mod promise;
mod timer_wheel;
mod value;
mod vm;

pub use awaitable::{Awaitable, PollResult, Waker, WakerRegistry};
pub use environment::{Binding, Environment};
pub use event_loop::EventLoop;
pub use io_awaitable::{TcpConnectAwaitable, TcpReadAwaitable, TcpWriteAwaitable};
#[cfg(not(feature = "tokio"))]
pub use io_selector::{Event, Interest, Token};
pub use promise::{Promise, PromiseState};
pub use timer_wheel::{TimerAwaitable, TimerWheel};
pub use value::{
    bool_obj, format_number, new_error, new_named_error, num_obj, str_obj, strict_equal, ArrayData,
    Builtin, BuiltinFn, CallContext, Class, ErrorData, Function, HashData, Instance, MapData,
    NativeCtor, Object, RegexpData, SetData,
};
pub use vm::{
    vm_error, EnvRef, EvaluatorFn, ImporterFn, NodeRef, VirtualMachine, EXEC_MODE_BYTECODE,
    EXEC_MODE_TREEWALK,
};
