/// Async runtime abstraction for event loop execution
///
/// This module documents the async runtime architecture and provides utilities
/// for future tokio integration. The current implementation uses a single-threaded
/// poll-based EventLoop with Rc/RefCell semantics.
///
/// ## Current Architecture (Native)
///
/// The native runtime is built on:
/// - `EventLoop`: Poll-based task scheduler with Rc/RefCell
/// - `TimerWheel`: Binary heap-based timer scheduling
/// - `Awaitable`: Poll-based future trait with Waker callbacks
/// - `Promise`: Core async primitive for deferred values
///
/// ## Future Architecture (Tokio Integration)
///
/// When the `tokio` feature is enabled, the runtime will:
/// - Use tokio's multi-threaded scheduler for true parallelism
/// - Bridge GTS Awaitables to tokio Tasks using channels
/// - Convert single-threaded Rc/RefCell to Arc/Mutex for thread-safety
/// - Maintain backward compatibility through runtime detection
///
/// ## Integration Points
///
/// The key integration points for tokio are:
/// 1. `VirtualMachine` - Add optional `tokio::runtime::Runtime` field
/// 2. `EventLoop` - Implement tokio task spawning adapter
/// 3. `Promise` - Bridge Promise resolution to tokio channels
/// 4. `stdlib` - Feature-gate async I/O to use tokio primitives
///
/// ## Design Principles
///
/// - **Zero-cost abstraction**: Native runtime has no tokio overhead
/// - **Feature-gated**: Tokio dependency only with `tokio` feature
/// - **Backward compatible**: Existing code works unchanged
/// - **Opt-in parallelism**: Users choose runtime via feature flag
pub mod native;

#[cfg(feature = "tokio")]
pub mod tokio_rt;

#[cfg(feature = "tokio")]
pub mod awaitable_bridge;

// Re-export the native runtime as the default
pub use native::NativeRuntime;

#[cfg(feature = "tokio")]
pub use tokio_rt::TokioRuntime;

#[cfg(feature = "tokio")]
pub use awaitable_bridge::{spawn_blocking_gts, AsyncCoordinator, SerializedResult};
