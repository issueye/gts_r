/// Native single-threaded poll-based runtime
///
/// This module provides utilities and documentation for the native EventLoop
/// runtime. The actual runtime is implemented in `src/object/event_loop.rs`.
///
/// ## Architecture
///
/// The native runtime uses:
/// - `EventLoop`: Task queue with poll-based scheduling
/// - `TimerWheel`: Binary heap for efficient timer management
/// - `Awaitable`: Poll-based future trait with waker callbacks
/// - `Rc/RefCell`: Single-threaded shared ownership
///
/// ## Usage
///
/// ```rust,ignore
/// use gts::object::EventLoop;
/// use gts::object::Promise;
///
/// let event_loop = EventLoop::new();
/// let promise = Promise::new();
///
/// // Spawn an awaitable
/// event_loop.spawn(promise.clone(), |result| {
///     println!("Promise resolved: {:?}", result);
/// });
///
/// // Run until all tasks complete
/// event_loop.run_until_quiescent();
/// ```
///
/// ## Timer Management
///
/// Timers are managed through the `TimerWheel`:
///
/// ```rust,ignore
/// let event_loop = EventLoop::new();
/// let timer_wheel = event_loop.timer_wheel();
///
/// // Schedule a timer
/// let waker = Rc::new(|| println!("Timer fired!"));
/// let timer_id = timer_wheel.borrow_mut().schedule(
///     Duration::from_secs(1),
///     waker
/// );
///
/// // Cancel a timer
/// timer_wheel.borrow_mut().cancel(timer_id);
/// ```
///
/// ## Future Enhancements
///
/// Planned improvements for the native runtime:
/// 1. Add `run_once()` method for single-iteration polling
/// 2. Add I/O readiness tracking (epoll/kqueue/IOCP abstraction)
/// 3. Add metrics and performance monitoring
/// 4. Optimize timer wheel for large numbers of timers
use crate::object::EventLoop;
use std::rc::Rc;

/// Type alias for the native runtime
///
/// This is just an alias to EventLoop for semantic clarity.
/// Use this when you want to emphasize runtime semantics.
pub type NativeRuntime = EventLoop;

/// Create a new native runtime instance
///
/// This is a convenience function that creates a new EventLoop.
pub fn create_runtime() -> Rc<NativeRuntime> {
    EventLoop::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_runtime() {
        let runtime = create_runtime();
        // Just verify it can be created
        assert!(runtime.timer_wheel().borrow().next_deadline().is_none());
    }
}
