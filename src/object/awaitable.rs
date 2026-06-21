//! Awaitable abstraction — GTS's analog of Rust's Future trait.
//!
//! This is adapted from Go's implementation to work in a single-threaded Rust
//! context using Rc/RefCell. The core idea is poll-based async: an Awaitable
//! can be polled for readiness, and if not ready, it registers a waker callback
//! that will be invoked when it becomes ready.
//!
//! Key differences from Rust's std::future::Future:
//! - No Pin: we use Rc/RefCell for interior mutability
//! - Waker is a simple callback closure, not a vtable-backed Arc
//! - Single-threaded: no Send/Sync requirements
//!
//! This abstraction enables:
//! - Uniform handling of Promises, timers, and I/O
//! - Efficient select/race without spawning threads
//! - Event loop integration

use std::cell::RefCell;
use std::rc::Rc;

use super::value::Object;

/// Waker is a callback that signals an Awaitable has become ready.
///
/// When an Awaitable returns Pending, it must arrange for the waker to be
/// called once it becomes ready. Waker is idempotent: calling it multiple
/// times is safe (may cause spurious polls).
pub type Waker = Rc<dyn Fn()>;

/// PollResult represents the outcome of polling an Awaitable.
#[derive(Debug, Clone)]
pub enum PollResult {
    /// The awaitable is not yet ready. The waker has been registered.
    Pending,
    /// The awaitable completed successfully with a value.
    Ready(Object),
    /// The awaitable was rejected with an error.
    Rejected(Object),
}

impl PollResult {
    pub fn is_ready(&self) -> bool {
        matches!(self, PollResult::Ready(_) | PollResult::Rejected(_))
    }

    pub fn is_pending(&self) -> bool {
        matches!(self, PollResult::Pending)
    }
}

/// Awaitable is the unified async abstraction.
///
/// The poll method is NON-BLOCKING: it checks readiness and either returns
/// a result or registers a waker for future notification.
///
/// Contract:
/// - If poll returns Pending, the awaitable MUST call the waker when ready
/// - Spurious wakes are safe (poll will return Pending again)
/// - Once poll returns Ready/Rejected, it must not be polled again
pub trait Awaitable {
    /// Poll for readiness. If ready, returns Ready or Rejected.
    /// If pending, registers the waker and returns Pending.
    fn poll(&self, waker: Waker) -> PollResult;
}

/// WakerRegistry manages multiple wakers for an awaitable.
///
/// Used by Promises and other awaitables that can be awaited by multiple
/// tasks concurrently. When the awaitable settles, all registered wakers
/// are invoked.
#[derive(Default)]
pub struct WakerRegistry {
    inner: RefCell<WakerRegistryInner>,
}

#[derive(Default)]
struct WakerRegistryInner {
    wakers: Vec<Waker>,
    closed: bool,
}

impl WakerRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a waker. If already settled, the waker is invoked immediately.
    pub fn register(&self, waker: Waker) {
        let mut inner = self.inner.borrow_mut();
        if inner.closed {
            // Already settled: wake immediately
            drop(inner); // Release borrow before calling waker
            waker();
            return;
        }
        inner.wakers.push(waker);
    }

    /// Mark as settled and invoke all registered wakers.
    pub fn wake_all(&self) {
        let mut inner = self.inner.borrow_mut();
        inner.closed = true;
        let wakers = std::mem::take(&mut inner.wakers);
        drop(inner); // Release borrow before calling wakers

        for waker in wakers {
            waker();
        }
    }
}
